import collections
import json
import io
import pathlib
import sys

CASES = (
    "normal_x_after_topmost_a",
    "notopmost_x_then_after_topmost_a",
    "topmost_x_then_after_topmost_a",
    "topmost_x_after_normal_a",
    "held_async_a_demotion_then_local_x_after_a",
    "normal_x_after_sole_global_topmost_a",
    "normal_x_after_sole_controlled_topmost_a",
    "normal_x_after_bottommost_topmost_a_with_b_above",
    "already_topmost_x_after_topmost_a",
    "demoted_topmost_x_then_after_topmost_a",
    "normal_x_after_normal_reference",
    "repeat_x_after_topmost_a",
    "wait_for_async_a_demotion_then_x_after_a",
    "held_async_a_promotion_then_local_x_after_a",
    "wait_for_async_a_promotion_then_x_after_a",
    "held_c_after_observed_normal_a_then_promote_a",
    "held_c_after_observed_normal_a_keep_a_normal",
    "held_c_after_observed_normal_a_promote_then_demote_a",
    "topmost_a_after_normal_x_below_overlay_control",
    "topmost_a_after_normal_predecessor_of_x",
    "topmost_a_after_topmost_predecessor_of_x",
)


def require(condition, reason):
    if not condition:
        raise ValueError(reason)


def checked_window(item):
    require(item["valid"] and item["style_read_error"] is None, "invalid style read")
    require(item["topmost"] == bool(item["extended_style"] & 8), "style decoding disagrees")
    require(item["process"] > 0 and item["thread"] > 0, "missing window identity")


def derived_order(raw_enumeration):
    require(0 < len(raw_enumeration) <= 8192, "empty or oversized enumeration")
    by_handle = {}
    for item in raw_enumeration:
        checked_window(item)
        require(item["handle"] != 0 and item["handle"] not in by_handle, "zero or duplicate global handle")
        by_handle[item["handle"]] = item
    heads = [item for item in raw_enumeration if item["above"] == 0]
    tails = [item for item in raw_enumeration if item["below"] == 0]
    require(len(heads) == len(tails) == 1, "chain needs a single head and tail")
    reverse_order = []
    seen = set()
    current, following = tails[0]["handle"], 0
    for _ in range(len(raw_enumeration)):
        if current == 0:
            break
        require(current not in seen, "cycle in neighbor links")
        require(current in by_handle, "neighbor missing from enumeration")
        item = by_handle[current]
        require(item["below"] == following, "nonreciprocal neighbor links")
        reverse_order.append(item)
        seen.add(current)
        following, current = current, item["above"]
    require(current == 0 and len(seen) == len(raw_enumeration), "cycle or incomplete neighbor coverage")
    return list(reversed(reverse_order))


def canonical_order(snapshot):
    require(snapshot.get("scope") != "local", "global evidence required")
    if "raw_enumeration" in snapshot:
        return derived_order(snapshot["raw_enumeration"])
    return snapshot["global_order"]


def local_graph(snapshot):
    require(snapshot.get("fixture_rereads") == snapshot["fixtures"], "fixture changed after local traversal")
    neighbors = snapshot["neighbors"]
    require(0 < len(neighbors) <= 175, "empty or oversized local graph")
    graph = {}
    for item in neighbors:
        require(item["handle"] != 0 and item["handle"] not in graph, "zero or duplicate local handle")
        require(item["valid"] and item["process"] > 0 and item["thread"] > 0, "invalid neighbor identity")
        graph[item["handle"]] = item
    for item in neighbors:
        for direction, reciprocal in (("above", "below"), ("below", "above")):
            neighbor = graph.get(item[direction])
            require(neighbor is None or neighbor[reciprocal] == item["handle"], "nonreciprocal local links")
        seen = set()
        current = item["handle"]
        for _ in range(len(neighbors) + 1):
            if current not in graph:
                break
            require(current not in seen, "cycle in local links")
            seen.add(current)
            current = graph[current]["above"]
    for fixture in snapshot["fixtures"]:
        node = graph.get(fixture["handle"])
        require(node is not None and all(node[key] == fixture[key] for key in
            ("handle", "valid", "process", "thread", "above", "below")), "fixture changed during local read")
        require(all(fixture[key] == 0 or fixture[key] in graph for key in ("above", "below")), "missing immediate neighbor")
    return graph


def above(snapshot, upper, lower):
    if upper == lower:
        return False
    if snapshot.get("scope") != "local":
        order = [item["handle"] for item in canonical_order(snapshot)]
        return order.index(upper) < order.index(lower)
    graph = local_graph(snapshot)
    for start, end, answer in ((lower, upper, True), (upper, lower, False)):
        current = start
        for _ in range(len(graph)):
            node = graph.get(current)
            previous = graph.get(node["above"]) if node else None
            if previous is None:
                break
            require(previous["below"] == current, "nonreciprocal relation path")
            if previous["handle"] == end:
                return answer
            current = previous["handle"]
    return None


def checked_snapshot(snapshot, expected=None):
    fixtures = snapshot["fixtures"]
    require(snapshot["started_us"] <= snapshot["finished_us"], "reversed snapshot time")
    require(len(fixtures) in (4, 5), "missing windows")
    local = snapshot.get("scope") == "local"
    if local:
        require("raw_enumeration" not in snapshot and "global_order" not in snapshot, "local snapshot claims global evidence")
        local_graph(snapshot)
        handles = None
    else:
        require(snapshot.get("scope") in (None, "global"), "unknown snapshot scope")
        order = canonical_order(snapshot)
        require(snapshot["enumeration_ok"], "enumeration failed")
        require(order, "missing windows")
        require(len({item["handle"] for item in order}) == len(order), "duplicate global window")
        require(order[0]["above"] == 0 and order[-1]["below"] == 0, "incomplete chain")
        for upper, lower in zip(order, order[1:]):
            require(upper["below"] == lower["handle"] and lower["above"] == upper["handle"], "chain changed during read")
        global_by_handle = {item["handle"]: item for item in order}
        for item in order:
            checked_window(item)
        for item in fixtures:
            require(item == global_by_handle.get(item["handle"]), "fixture changed during read")
        if "raw_enumeration" in snapshot and "global_order" in snapshot:
            require(snapshot["global_order"] == order, "canonical order decoding disagrees")
        handles = [item["handle"] for item in order]
        if "fixture_order" in snapshot:
            fixture_handles = {item["handle"] for item in fixtures}
            require(snapshot["fixture_order"] == [item for item in handles if item in fixture_handles], "fixture order decoding disagrees")
    for item in fixtures:
        checked_window(item)
        require(item["handle"] != 0 and item["visible"] and item["owner"] == 0, "fixture hidden or owned")
    count = len(fixtures)
    for key in ("handle", "process", "thread"):
        require(len({item[key] for item in fixtures}) == count, "fixtures not independent")
    if expected is not None:
        require(len(expected) == count, "fixture count changed")
        for item, identity in zip(fixtures, expected):
            require((item["handle"], item["process"], item["thread"]) == identity, "fixture identity changed")
    return fixtures, handles


def initial_bands(case):
    return [not (case.startswith("held_c_") or "a_promotion" in case),
            "sole_" not in case,
            case == CASES[20],
            case in (CASES[3], CASES[8], CASES[9])] + ([False] if case.startswith("held_c_") else [])


def placement_premise(snapshot, case, bands):
    fixtures, order = checked_snapshot(snapshot)
    require([item["topmost"] for item in fixtures] == bands, "wrong actual fixture bands")
    a, b, predecessor, overlay = fixtures[:4]
    if bands[0] and bands[1] and case != CASES[7]:
        require(above(snapshot, a["handle"], b["handle"]) is True, "B below A is not proven")
    global_order = canonical_order(snapshot) if case in (CASES[5], CASES[7], CASES[20]) else None
    if case == CASES[5]:
        require([item["handle"] for item in global_order if item["topmost"]] == [a["handle"]],
                "another global topmost window exists, including hidden windows")
    if case == CASES[7]:
        position = order.index(a["handle"])
        require(order.index(b["handle"]) < position, "B is not above A")
        require(not any(item["topmost"] for item in global_order[position + 1:]),
                "A is not the actual bottommost flagged target")
    if case in (CASES[19], CASES[20]):
        require(overlay["above"] == predecessor["handle"] and predecessor["below"] == overlay["handle"], "P is not the immediate predecessor of X")
    if case == CASES[20]:
        require(not any(item["topmost"] for item in global_order[order.index(predecessor["handle"]) + 1:]),
                "P is not the actual bottommost flagged predecessor")


def facts(snapshot, target_index, reference_index):
    fixtures, order = checked_snapshot(snapshot)
    target, reference = fixtures[target_index], fixtures[reference_index]
    overlay = fixtures[3]
    return {"target_topmost": target["topmost"],
            "target_directly_after_reference": target["above"] == reference["handle"],
            "target_above_X": above(snapshot, target["handle"], overlay["handle"]),
            "A_topmost": fixtures[0]["topmost"], "X_topmost": overlay["topmost"],
            "target_above": target["above"], "target_below": target["below"],
            "fixture_order": ([handle for handle in order if handle in {item["handle"] for item in fixtures}] if order else None)}


def analyze_trial(trial, controller, identities):
    records = trial["records"]
    case = trial["case"]
    result = {"case": case, "repetition": trial["repetition"]}
    try:
        require(case in CASES, "unknown case")
        race = case.startswith("held_c_")
        transition = "async_a_" in case
        waiting = case.startswith("wait_")
        promotion = "a_promotion" in case
        repeat = case == CASES[11]
        alternative = case in CASES[18:]
        bands = initial_bands(case)

        def one(kind):
            selected = [item for item in records if item["kind"] == kind]
            require(len(selected) == 1, f"expected exactly one {kind}, got {len(selected)}")
            return selected[0]

        before = one("before")["snapshot"]
        fixtures, _ = checked_snapshot(before)
        expected = [(item["handle"], item["process"], item["thread"]) for item in fixtures]
        require(all(identity in identities for identity in expected), "identity has no creation record")
        roles = [identities[identity] for identity in expected]
        require(roles[:4] == ["A", "B", "normalA", "X"], "fixture roles disagree with creation records")
        if race:
            require(len(roles) == 5 and roles[4].startswith("C-"), "missing separate C creation record")
        require((fixtures[3]["process"], fixtures[3]["thread"]) == controller, "X is not coordinator-local")
        placement_premise(before, case, bands)
        result["other_global_topmost_before"] = (None if before.get("scope") == "local" else
            [item["handle"] for item in canonical_order(before)
             if item["topmost"] and item["handle"] not in {entry[0] for entry in expected}])
        target_index = 4 if race else 0 if alternative else 3
        reference_index = 0 if race else 3 if case == CASES[18] else 2 if alternative or case in (CASES[3], CASES[10]) else 0
        result["initial"] = facts(before, target_index, reference_index)

        def snap(kind):
            observation = one(kind)["snapshot"]
            checked_snapshot(observation, expected)
            return observation

        calls = []
        for record in records:
            if "call" in record:
                calls.append(record["call"])
            if record["kind"] == "fixture_reply":
                reply = record["reply"]["record"]
                require(reply.get("kind") != "hold_timeout", "pump hold timed out")
                if "call" in reply:
                    calls.append(reply["call"])
                    if "post_return_target_read" in reply:
                        read = reply["post_return_target_read"]
                        checked_window(read)
                        require((read["handle"], read["process"], read["thread"]) in expected,
                                "owner-thread immediate read has wrong identity")
                        require(read["handle"] == reply["call"]["window"], "owner-thread immediate read has wrong target")
                        require(reply["call"]["returned_us"] <= reply["read_started_us"] <= reply["read_finished_us"],
                                "owner-thread immediate read time reversed")
        require(calls and all(call["ok"] for call in calls), "native call failure")
        by_handle = {item["handle"]: item for item in fixtures}
        for call in calls:
            require(call["api"] == "SetWindowPos" and call["flags"] in (19, 16403), "wrong native API or flags")
            require(call["started_us"] <= call["returned_us"], "reversed native call time")
            require(all(call[key] == 0 for key in ("x", "y", "width", "height")), "geometry changed")
            require(call["window"] in by_handle, "write to nonfixture window")
            caller = (call["calling_process"], call["calling_thread"])
            owner = by_handle[call["window"]]
            require(caller == (controller if call["flags"] == 16403 else (owner["process"], owner["thread"])), "wrong native caller")
        ending = one("trial_end")
        require(ending["status"] == "collected_pending_independent_analysis", ending.get("error") or "collector did not finish treatment")
        require(not ending.get("release_a_error") and not ending.get("release_c_error"), "pump release failed")
        treatment_calls = [item["call"] for item in records if item["kind"] == "treatment_call"]
        external = [item for item in records if item["kind"] == "external_async_call"]
        require(len(external) == int(race or transition), "wrong number of async calls")
        require(len(treatment_calls) == (0 if race else 2 if repeat else 1), "wrong number of treatments")
        target = fixtures[target_index]["handle"]
        reference = fixtures[reference_index]["handle"]
        for call in treatment_calls:
            require(call["window"] == target and call["after"] == reference and call["flags"] == 19, "wrong synchronous treatment")
        immediate = snap("immediate_after_release" if race else "immediate_after_local_write")
        if not race:
            before_insert = snap("before_insert")
            insertion_bands = bands.copy()
            if case in (CASES[1], CASES[9]):
                insertion_bands[3] = False
            if case == CASES[2]:
                insertion_bands[3] = True
            if waiting:
                insertion_bands[0] = promotion
            placement_premise(before_insert, case, insertion_bands)
            require(before_insert["finished_us"] <= treatment_calls[0]["started_us"] <= treatment_calls[0]["returned_us"] <= immediate["started_us"], "treatment not bracketed by observations")
        if case in (CASES[1], CASES[2], CASES[9]):
            stage = snap("after_explicit_topmost" if case == CASES[2] else "after_explicit_notopmost")
            require(stage["fixtures"][3]["topmost"] == (case == CASES[2]), "explicit band change was not observed")
            level = -1 if case == CASES[2] else -2
            require(any(call["window"] == fixtures[3]["handle"] and call["after"] == level
                and before["finished_us"] <= call["started_us"] <= call["returned_us"] <= stage["started_us"] for call in calls), "explicit band call missing")
            require(stage["finished_us"] <= before_insert["started_us"], "explicit band observation out of order")

        if race or transition:
            posted = external[0]["call"]
            held_index = 4 if race else 0
            held_fixture = fixtures[held_index]
            require(posted["window"] == held_fixture["handle"] and posted["flags"] == 16403, "wrong held async target")
            require(posted["after"] == (fixtures[0]["handle"] if race else -1 if promotion else -2), "wrong async insert-after")
            read = external[0]["post_return_target_read"]
            checked_window(read)
            require((read["handle"], read["process"], read["thread"]) == expected[held_index], "immediate flag read wrong identity")
            require(read["topmost"] == bands[held_index], "held target changed before immediate read")
            require(posted["returned_us"] <= external[0]["read_finished_us"], "immediate flag read time reversed")
            if "read_started_us" in external[0]:
                require(posted["returned_us"] <= external[0]["read_started_us"] <= external[0]["read_finished_us"],
                        "immediate flag read bounds reversed")
            replies = [item for item in records if item["kind"] == "fixture_reply" and item["process"] == held_fixture["process"]]
            held = [item["reply"] for item in replies if item["reply"]["record"].get("kind") == "held"]
            released = [item["reply"] for item in replies if item["reply"]["record"].get("kind") == "released_before_pump"]
            require(len(held) == len(released) == 1, "hold/release protocol missing or duplicated")
            hold_time, release_time = held[0]["unix_us"], released[0]["unix_us"]
            require(held[0]["record"]["handle"] == released[0]["record"]["handle"] == held_fixture["handle"], "wrong held handle")
            require(hold_time <= posted["started_us"] <= posted["returned_us"] <= release_time, "post not inside held interval")
            require(release_time - hold_time < 3000000, "hold exceeded three-second guard")
            posted_snapshot = snap("after_async_post_while_held")
            require(posted["returned_us"] <= posted_snapshot["started_us"] <= posted_snapshot["finished_us"] <= release_time, "post observation outside hold")
            require(posted_snapshot["fixtures"][held_index]["topmost"] == bands[held_index], "target changed while held")
            if transition:
                held_before = snap("held_before_post")
                require(hold_time <= held_before["started_us"] <= held_before["finished_us"] <= posted["started_us"], "hold observation out of order")
                require(held_before["fixtures"][0]["topmost"] == bands[0], "wrong A state at hold")
                if waiting:
                    changed = snap("reference_transition_observed")
                    require(changed["fixtures"][0]["topmost"] == promotion, "reference transition not observed")
                    require(release_time <= changed["started_us"] <= changed["finished_us"] <= before_insert["started_us"], "wait observation out of order")
                else:
                    delayed = snap("held_after_100ms")
                    require(posted_snapshot["finished_us"] <= before_insert["started_us"], "local insert preceded post read")
                    require(immediate["fixtures"][0]["topmost"] == delayed["fixtures"][0]["topmost"] == bands[0], "A transitioned before release")
                    require(delayed["started_us"] - immediate["finished_us"] >= 100000, "held observation interval too short")
                    result["held_after_write_gap_us"] = delayed["started_us"] - immediate["finished_us"]
                    require(immediate["finished_us"] <= delayed["started_us"] <= delayed["finished_us"] <= release_time, "held observations out of order")
            else:
                queried = snap("queried_normal_a_before_c_post")
                require(not queried["fixtures"][0]["topmost"], "A was not normal when queried")
                require(queried["fixtures"][4]["above"] != fixtures[0]["handle"], "C already after A before queued treatment")
                require(all(queried["fixtures"][4][key] == posted_snapshot["fixtures"][4][key] == read[key] for key in ("topmost", "above", "below")), "C moved before held post observation")
                require(hold_time <= queried["started_us"] <= queried["finished_us"] <= posted["started_us"], "query not before queued C write")
                previous_time = posted_snapshot["finished_us"]
                if case != CASES[16]:
                    promoted = snap("after_a_promotion_while_c_held")
                    require(promoted["fixtures"][0]["topmost"], "A promotion not observed")
                    require(any(call["window"] == fixtures[0]["handle"] and call["after"] == -1 and previous_time <= call["started_us"] <= call["returned_us"] <= promoted["started_us"] for call in calls), "A promotion call missing or out of order")
                    previous_time = promoted["finished_us"]
                if case == CASES[17]:
                    demoted = snap("after_a_demotion_while_c_held")
                    require(not demoted["fixtures"][0]["topmost"], "A return to normal not observed")
                    require(any(call["window"] == fixtures[0]["handle"] and call["after"] == -2 and previous_time <= call["started_us"] <= call["returned_us"] <= demoted["started_us"] for call in calls), "A demotion call missing or out of order")
                    previous_time = demoted["finished_us"]
                pre_release = snap("before_c_release")
                require(pre_release["fixtures"][0]["topmost"] == (case == CASES[15]), "wrong A state before C release")
                require(not pre_release["fixtures"][4]["topmost"], "C changed band while held")
                require(previous_time <= pre_release["started_us"] <= pre_release["finished_us"] <= release_time <= immediate["started_us"], "C race ordering not established")
                if case == CASES[16]:
                    require(all(pre_release["fixtures"][4][key] == queried["fixtures"][4][key]
                        for key in ("topmost", "above", "below")), "C moved before release in unchanged-A control")
                    require(pre_release["fixtures"][4]["above"] != fixtures[0]["handle"], "C reached queued destination before release")
                else:
                    result["held_C_adjacency_interpretation"] = "A moved, changed C neighbors alone do not establish queued execution"
                result["queried_A_normal_before_queued_C_write"] = True
            result["hold_span_us"] = release_time - hold_time

        def settling(kind):
            rows = [item for item in records if item["kind"] == kind]
            require([item["sample"] for item in rows] == list(range(21)), f"incomplete {kind}")
            observations = [item["snapshot"] for item in rows]
            for observation in observations:
                checked_snapshot(observation, expected)
            require(observations[-1]["started_us"] - observations[0]["started_us"] >= 500000, "settling interval too short")
            for left, right in zip(observations, observations[1:]):
                require(right["started_us"] - left["finished_us"] >= 25000, "settling interval too short or reversed")
            states = [facts(item, target_index, reference_index) for item in observations[-5:]]
            keys = ("target_topmost", "target_directly_after_reference", "target_above_X", "A_topmost", "X_topmost", "target_above", "target_below")
            require(len({tuple(state[key] for key in keys) for state in states}) == 1, "final target state or neighbors did not settle")
            result[kind + "_timing"] = {
                "span_us": observations[-1]["finished_us"] - observations[0]["started_us"],
                "read_spans_us": [item["finished_us"] - item["started_us"] for item in observations],
                "gaps_us": [right["started_us"] - left["finished_us"] for left, right in zip(observations, observations[1:])]}
            return observations, states[-1]

        observations, settled = settling("settling")
        last_immediate = immediate
        if repeat:
            first_observations, first_settled = settling("first_settling")
            repeated_before = snap("before_repeated_insert")
            repeated_immediate = snap("immediate_after_repeated_write")
            require(immediate["finished_us"] <= first_observations[0]["started_us"], "first settling precedes first treatment")
            require(first_observations[-1]["finished_us"] <= repeated_before["started_us"] <= repeated_before["finished_us"] <= treatment_calls[1]["started_us"] <= treatment_calls[1]["returned_us"] <= repeated_immediate["started_us"], "repeat not after first convergence")
            require(facts(repeated_before, target_index, reference_index) == first_settled, "state changed before repeated call")
            result["first_settled"] = first_settled
            result["repeated_immediate"] = facts(repeated_immediate, target_index, reference_index)
            result["idempotent_band_and_neighbors"] = all(first_settled[key] == settled[key] for key in ("target_topmost", "target_above", "target_below"))
            last_immediate = repeated_immediate
        require(last_immediate["finished_us"] <= observations[0]["started_us"], "settling precedes treatment")
        for observation in [immediate] + observations:
            sample = observation["fixtures"]
            require(sample[1]["topmost"] == bands[1] and sample[2]["topmost"] == bands[2], "B or reference band changed")
            if not transition and not alternative:
                require(sample[0]["topmost"] == (case == CASES[15] if race else bands[0]), "A band changed outside treatment")
            if race or alternative:
                require(not sample[3]["topmost"], "normal X changed band")
        if transition:
            require(all(item["fixtures"][0]["topmost"] == promotion for item in observations[-5:]), "async reference transition did not settle")
        result.update(status="eligible_observation", immediate=facts(immediate, target_index, reference_index), settled=settled)
        controls = {CASES[2]: True, CASES[3]: False, CASES[10]: False, CASES[16]: False, CASES[18]: False, CASES[19]: False}
        if case in controls:
            result["control_passed"] = settled["target_topmost"] == controls[case] and settled["target_directly_after_reference"]
            if case == CASES[19]:
                result["control_passed"] = result["control_passed"] and settled["target_above_X"] is True
    except (ValueError, KeyError, IndexError, TypeError, AttributeError) as error:
        result.update(status="ineligible_or_incomplete", reason=str(error) or type(error).__name__)
    return result


def analyze_lines(lines):
    results = []
    actual_keys = []
    session = None
    controller = None
    complete = False
    collection_complete = False
    errors = []
    identities = {}
    creation_labels = {}
    active = None
    for line_number, line in enumerate(lines, 1):
        try:
            envelope = json.loads(line)
            record = envelope["record"]
            kind = record["kind"]
            require(not complete, "record follows cleanup_complete")
            if kind == "session":
                require(session is None and not actual_keys, "duplicate or late session")
                session = record
                controller = (record["process"], record["thread"])
                require(record["schema_version"] in (2, 3, 4), "unsupported schema, expected 2, 3 or 4")
                require(record["case_names"] == list(CASES), "session case matrix differs from analyzer")
                require(type(record["repetitions"]) is int and 1 <= record["repetitions"] <= 20, "invalid repetitions")
            if "snapshot" in record and session and session["schema_version"] == 3:
                require("raw_enumeration" in record["snapshot"], "schema 3 snapshot missing raw enumeration")
            if "snapshot" in record and session and session["schema_version"] == 4:
                observation = record["snapshot"]
                require(observation.get("scope") in ("local", "global"), "schema 4 snapshot missing scope")
                if observation["scope"] == "global":
                    require("raw_enumeration" in observation, "global snapshot missing raw enumeration")
                    require("global_order" not in observation, "schema 4 duplicates global order")
                    require(active is not None and active["case"] in (CASES[5], CASES[7], CASES[20])
                        and kind in ("before", "before_insert", "boundary_anchor_scan"), "global read outside global premise stage")
                else:
                    require("neighbors" in observation, "local snapshot missing neighbor evidence")
            if kind == "spawn":
                creation_labels[record["process"]] = record["label"]
            if kind == "fixture_reply" and record["reply"]["record"].get("kind") == "ready":
                ready = record["reply"]["record"]
                require(ready["process"] == record["process"] and ready["process"] in creation_labels, "fixture ready has no matching spawn")
                identities[(ready["handle"], ready["process"], ready["thread"])] = creation_labels[ready["process"]]
            if kind == "overlay_identity":
                require((record["process"], record["thread"]) == controller, "overlay creation not coordinator-local")
                identities[(record["handle"], record["process"], record["thread"])] = "X"
            if kind == "trial_begin":
                require(type(record["case"]) is str and type(record["repetition"]) is int, "invalid trial identity")
                require(active is None and not collection_complete, "nested or late trial")
                active = {"case": record["case"], "repetition": record["repetition"], "records": []}
                actual_keys.append((record["case"], record["repetition"]))
            if active is not None:
                active["records"].append(record)
            if kind == "trial_end":
                require(active is not None, "trial end without begin")
                require((record["case"], record["repetition"]) == (active["case"], active["repetition"]), "trial end identity mismatch")
                results.append(analyze_trial(active, controller, identities))
                active = None
            if kind == "collection_complete":
                require(active is None and not collection_complete, "incomplete trial or duplicate collection_complete")
                collection_complete = True
            if kind == "cleanup_complete":
                require(collection_complete and active is None, "cleanup before collection completed")
                complete = True
        except (json.JSONDecodeError, KeyError, ValueError, TypeError, AttributeError) as error:
            errors.append({"line": line_number, "error": str(error)})
    repetitions = session.get("repetitions", 5) if session else 5
    if type(repetitions) is not int or not 1 <= repetitions <= 20:
        repetitions = 5
    expected_keys = {(case, repetition) for case in CASES for repetition in range(repetitions)}
    matrix_complete = len(actual_keys) == len(expected_keys) and set(actual_keys) == expected_keys
    if active is not None:
        results.append(analyze_trial(active, controller, identities))
    matrix = {}
    for case in CASES:
        rows = [row for row in results if row["case"] == case]
        outcomes = collections.Counter((row["immediate"]["target_topmost"], row["settled"]["target_topmost"],
            row["settled"]["target_directly_after_reference"], row["settled"]["target_above_X"])
            for row in rows if row["status"] == "eligible_observation")
        matrix[case] = {"attempted": len(rows), "eligible": sum(row["status"] == "eligible_observation" for row in rows),
            "outcomes": [{"immediate_target_topmost": key[0], "settled_target_topmost": key[1],
                "settled_target_directly_after_reference": key[2], "settled_target_above_X": key[3], "count": count}
                for key, count in outcomes.items()]}
    valid = (session is not None and complete and collection_complete and active is None and matrix_complete
        and not errors and all(row["status"] == "eligible_observation" and row.get("control_passed", True) for row in results))
    return {"schema_version": session.get("schema_version", 2) if session else 2, "cleanup_complete": complete, "collection_complete": collection_complete,
        "expected_trials": len(expected_keys), "actual_trials": len(actual_keys), "matrix_complete": matrix_complete,
        "malformed_or_protocol_errors": errors, "matrix": matrix, "trials": results, "all_trials_eligible_and_controls_passed": valid}


def analyze_text(text):
    return analyze_lines(io.StringIO(text))


def main():
    try:
        with pathlib.Path(sys.argv[1]).open(encoding="utf-8-sig") as stream:
            report = analyze_lines(stream)
    except (OSError, IndexError) as error:
        print(json.dumps({"error": str(error), "all_trials_eligible_and_controls_passed": False}))
        return 2
    print(json.dumps(report, indent=2))
    return 0 if report["all_trials_eligible_and_controls_passed"] else 2


if __name__ == "__main__":
    sys.exit(main())
