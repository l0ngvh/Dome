import json
import analyze as legacy
import protocol_v6 as audit

require = legacy.require
checked_window = legacy.checked_window
canonical_order = legacy.canonical_order
above = legacy.above
CASES = audit.CASES
scheduled_cases = audit.scheduled_cases
schedule = audit.schedule
owned_by = audit.owned_by


def checked_snapshot(snapshot, expected=None, ownership=None):
    fixtures = snapshot["fixtures"]
    require(snapshot.get("invalid_reason") is None, "native snapshot invalid " + str(snapshot.get("invalid_reason")))
    require(all(type(snapshot[key]) is int and snapshot[key] >= 0 for key in ("started_us", "finished_us")), "invalid snapshot time")
    require(snapshot["started_us"] <= snapshot["finished_us"], "reversed snapshot time")
    require(len(fixtures) in (4, 5), "missing fixtures")
    if snapshot.get("scope") == "local":
        require("raw_enumeration" not in snapshot and "global_order" not in snapshot, "local snapshot claims global evidence")
        legacy.local_graph(snapshot)
        order = None
    else:
        require(snapshot.get("scope") == "global" and snapshot["enumeration_ok"], "invalid global scope")
        order = canonical_order(snapshot)
        by_handle = {item["handle"]: item for item in order}
        require(all(item == by_handle.get(item["handle"]) for item in fixtures), "fixture changed during global read")
    require(len({item["handle"] for item in fixtures}) == len(fixtures), "duplicate fixture")
    baseline = fixtures[:-1] if ownership else fixtures
    for item in fixtures:
        checked_window(item)
        require(item["visible"] and item["handle"] != 0, "fixture hidden or null")
    for item in baseline:
        require(item["owner"] == 0, "baseline fixture owned")
    for key in ("process", "thread"):
        require(len({item[key] for item in baseline}) == len(baseline), "baseline fixtures not independent")
    if ownership:
        require(len(fixtures) == 5, "missing scheduled O")
        owner = fixtures[0 if ownership == "A" else 3]
        owned = fixtures[4]
        require(owned["owner"] == owner["handle"] and all(owned[key] == owner[key] for key in ("process", "thread")), "scheduled ownership relationship differs")
    if expected is not None:
        require([(item["handle"], item["process"], item["thread"]) for item in fixtures] == expected, "fixture identity changed")
    return fixtures, [item["handle"] for item in order] if order else None


def initial_bands(case):
    bands = [not (case.startswith("held_c_") or "a_promotion" in case), "sole_" not in case,
             case == CASES[20], case in (CASES[3], CASES[8], CASES[9], CASES[22], CASES[24], CASES[28]) or case in CASES[29:34]]
    if case.startswith("held_c_"):
        bands.append(False)
    if owned_by(case):
        bands.append(bands[0 if owned_by(case) == "A" else 3])
    return bands


def clean_prefix(snapshot, last):
    order = canonical_order(snapshot)
    index = [item["handle"] for item in order].index(last)
    return all(item["topmost"] for item in order[:index + 1]) and all(not item["topmost"] for item in order[index + 1:])


def premise(snapshot, case, bands, ownership=None):
    fixtures, order = checked_snapshot(snapshot, ownership=ownership)
    require([item["topmost"] for item in fixtures] == bands, "wrong actual fixture bands")
    a, b, predecessor, overlay = fixtures[:4]
    if bands[0] and bands[1] and case != CASES[7]:
        require(above(snapshot, a["handle"], b["handle"]) is True, "B below A not proven")
    if case == CASES[5]:
        global_order = canonical_order(snapshot)
        require(global_order[0]["handle"] == a["handle"] and [item["handle"] for item in global_order if item["topmost"]] == [a["handle"]], "A not sole global flagged chain head")
    if case == CASES[7]:
        require(clean_prefix(snapshot, a["handle"]) and order.index(b["handle"]) < order.index(a["handle"]), "A not clean prefix boundary with B above")
    if case in (CASES[19], CASES[20]):
        require(overlay["above"] == predecessor["handle"] and predecessor["below"] == overlay["handle"], "P not strict immediate predecessor of X")
    if case == CASES[20]:
        require(clean_prefix(snapshot, predecessor["handle"]), "P not clean prefix boundary")
    if case == CASES[7]:
        require(overlay["above"] == predecessor["handle"] and overlay["above"] != a["handle"], "clean boundary X not displaced after P")
    if case == CASES[27]:
        require(above(snapshot, overlay["handle"], predecessor["handle"]) is True, "normal Bottom X not above stationary P")
    if case.startswith("held_c_"):
        chain = [a, overlay, predecessor, fixtures[4]]
        require(all(left["below"] == right["handle"] and right["above"] == left["handle"] for left, right in zip(chain, chain[1:])), "race requires actual A-X-P-C chain")


def observed_facts(snapshot, target_index, reference_index, ownership):
    fixtures, order = checked_snapshot(snapshot, ownership=ownership)
    target, reference = fixtures[target_index], fixtures[reference_index]
    overlay = fixtures[3]
    return {"target_topmost": target["topmost"], "target_directly_after_reference": target["above"] == reference["handle"],
            "target_above_X": above(snapshot, target["handle"], overlay["handle"]),
            "A_topmost": fixtures[0]["topmost"], "X_topmost": overlay["topmost"],
            "target_above": target["above"], "target_below": target["below"],
            "fixture_order": [handle for handle in order if handle in {item["handle"] for item in fixtures}] if order else None}


def analyze_trial(trial, controller, identities):
    records = trial["records"]
    case = trial["case"]
    result = {"case": case, "repetition": trial["repetition"], "environment": trial["environment"],
              "setup_valid": False, "endpoint_resolved": False, "treatment_executed": any(row["kind"] in ("treatment_call", "external_async_call") for row in records),
              "snapshot_validity": []}
    ownership = owned_by(case)
    result["collector_error"] = next((row.get("error") for row in records if row["kind"] == "trial_end"), None)
    phase = "protocol"

    def checked(snapshot, expected=None):
        return checked_snapshot(snapshot, expected, ownership if len(snapshot["fixtures"]) == 5 else None)

    def facts(snapshot, target_index, reference_index):
        return observed_facts(snapshot, target_index, reference_index, ownership)

    def placement_premise(snapshot, case, bands):
        return premise(snapshot, case, bands, ownership)
    try:
        require(case in CASES, "unknown case")
        expected = audit.expected_identities(trial, identities)
        for record in records:
            if "snapshot" not in record:
                continue
            try:
                if record["kind"] in ("owner_reset_before_creation", "owner_promoted_before_creation"):
                    checked_snapshot(record["snapshot"], expected[:4])
                else:
                    checked(record["snapshot"], expected)
                reason = None
            except (ValueError, KeyError, TypeError, IndexError) as error:
                reason = str(error)
            result["snapshot_validity"].append({"kind": record["kind"], "sample": record.get("sample"), "valid": reason is None, "reason": reason})
        audit.validate_setup(trial, controller, expected, checked_snapshot)
        result["setup_protocol_valid"] = True
        if ownership:
            created = next((row["snapshot"] for row in records if row["kind"] == "after_owned_creation"), None)
            if created:
                result["observed_creation_bands"] = {slot: item["topmost"] for slot, item in zip(trial["fixture_slots"], created["fixtures"])}
        synchronous_flags = 531 if case in CASES[34:36] else 19
        race = case.startswith("held_c_")
        transition = "async_a_" in case
        waiting = case.startswith("wait_")
        promotion = "a_promotion" in case
        repeat = case == CASES[11]
        alternative = case in (CASES[18], CASES[19], CASES[20], CASES[26])
        special = case in CASES[21:25] or case in CASES[27:31] or case in CASES[32:35]
        bands = initial_bands(case)

        def one(kind):
            selected = [item for item in records if item["kind"] == kind]
            require(len(selected) == 1, f"expected exactly one {kind}, got {len(selected)}")
            return selected[0]

        def setup_premise(condition, reason):
            nonlocal phase
            if not condition:
                phase = "setup"
                require(False, reason)

        if case in CASES[29:34]:
            reset = one("owner_reset_before_creation")["snapshot"]
            setup_premise(not reset["fixtures"][3]["topmost"], "owner_reset_before_creation: owner reset before creation unverified")
        if case in CASES[29:32]:
            promoted = one("owner_promoted_before_creation")["snapshot"]
            setup_premise(promoted["fixtures"][3]["topmost"], "owner_promoted_before_creation: owner promotion before creation unverified")
        if case in CASES[29:34]:
            created = one("after_owned_creation")["snapshot"]
            if case in CASES[32:34]:
                setup_premise(not any(created["fixtures"][index]["topmost"] for index in (3, 4)), "after_owned_creation: ordered promotion requires normal owner and owned after creation")
            else:
                setup_premise(all(created["fixtures"][index]["topmost"] for index in (3, 4)), "after_owned_creation: creation did not preserve both-topmost premise")
        before = one("before")["snapshot"]
        fixtures, _ = checked(before, expected)
        require(all(identity in identities for identity in expected), "identity has no creation record")
        roles = [identities[identity] for identity in expected]
        require(trial["fixture_slots"] == (["A", "B", "P", "X", "C"] if race else ["A", "B", "P", "X", "O"] if ownership else ["A", "B", "P", "X"]), "wrong scheduled fixture slots")
        require(trial["owned_by"] == ownership, "wrong scheduled ownership")
        require(roles[:4] == ["A", "B", "normalA", "X"], "fixture roles disagree with creation records")
        if race:
            require(len(roles) == 5 and roles[4].startswith("C-"), "missing separate C creation record")
        require((fixtures[3]["process"], fixtures[3]["thread"]) == controller, "X is not coordinator-local")
        result["observed_setup_bands"] = {slot: item["topmost"] for slot, item in zip(trial["fixture_slots"], fixtures)}
        phase = "setup"
        placement_premise(before, case, bands)
        result["setup_valid"] = True
        phase = "protocol"
        result["other_global_topmost_before"] = (None if before.get("scope") == "local" else
            [item["handle"] for item in canonical_order(before)
             if item["topmost"] and item["handle"] not in {entry[0] for entry in expected}])
        target_index = 4 if race or case in (CASES[23], CASES[24], CASES[30]) else 0 if alternative else 3
        reference_index = 0 if race else 3 if case in (CASES[18], CASES[26]) else 2 if alternative or case in (CASES[3], CASES[10]) else 0
        result["initial"] = facts(before, target_index, reference_index)

        def valid(snapshot):
            try:
                checked(snapshot, expected)
                return None
            except (ValueError, KeyError, TypeError, IndexError) as error:
                return str(error)

        def snap(kind, required=True):
            observation = one(kind)["snapshot"]
            if required:
                checked(observation, expected)
            return observation

        calls = []
        representations = []
        for record in records:
            if "call" in record:
                calls.append(record["call"])
                representations.append((record["call"], record["kind"]))
            if record["kind"] == "fixture_reply":
                reply = record["reply"]["record"]
                require(reply.get("kind") != "hold_timeout", "pump hold timed out")
                if "call" in reply:
                    calls.append(reply["call"])
                    require(record["process"] == reply["call"]["calling_process"], "fixture reply has wrong native caller")
                    representations.append((reply["call"], "fixture_reply"))

        require(calls and all(call["ok"] for call in calls), "native call failure")
        by_handle = {item["handle"]: item for item in fixtures}
        for call in calls:
            require(call["api"] == "SetWindowPos" and call["flags"] in (19, 531, 16403), "wrong native API or flags")
            require(all(type(call[key]) is int and call[key] >= 0 for key in ("started_us", "returned_us")), "invalid native call time")
            require(call["started_us"] <= call["returned_us"], "reversed native call time")
            require(all(call[key] == 0 for key in ("x", "y", "width", "height")), "geometry changed")
            require(call["window"] in by_handle, "write to nonfixture window")
            caller = (call["calling_process"], call["calling_thread"])
            owner = by_handle[call["window"]]
            require(caller == (controller if call["flags"] == 16403 else (owner["process"], owner["thread"])), "wrong native caller")
        unique_calls = {}
        for call, source in representations:
            identity = (call["calling_process"], call["calling_thread"], call["started_us"])
            if identity in unique_calls:
                original, sources = unique_calls[identity]
                require(call == original, "conflicting duplicate native call")
                require(call["window"] == fixtures[0]["handle"] and call["flags"] == 19
                        and len(sources) == 1 and {sources[0], source} == {"fixture_reply", "treatment_call"},
                        "unexpected duplicate native call")
                sources.append(source)
            else:
                unique_calls[identity] = (call, [source])
        calls = sorted((call for call, _ in unique_calls.values()), key=lambda call: (call["started_us"], call["returned_us"]))
        ending = one("trial_end")
        require(ending["status"] == "collected_pending_independent_analysis", ending.get("error") or "collector did not finish treatment")
        require(not ending.get("release_a_error") and not ending.get("release_c_error") and not ending.get("owned_cleanup_error") and not ending.get("other_cleanup_error"), "fixture release or cleanup failed")
        treatment_calls = sorted((item["call"] for item in records if item["kind"] == "treatment_call"),
                                 key=lambda call: call["started_us"])
        external = [item for item in records if item["kind"] == "external_async_call"]
        require(len(external) == int(race or transition), "wrong number of async calls")
        require(len(treatment_calls) == (0 if race else 2 if repeat else 1), "wrong number of treatments")
        target = fixtures[target_index]["handle"]
        reference = fixtures[reference_index]["handle"]
        if special:
            reference = 1 if case in CASES[27:29] else -1 if case in (CASES[21], CASES[23], CASES[34]) else -2
        for call in treatment_calls:
            require(call["window"] == target and call["after"] == reference and call["flags"] == synchronous_flags, "wrong synchronous treatment")
            if target == fixtures[0]["handle"]:
                require(any(source == "fixture_reply" and reply == call for reply, source in representations),
                        "fixture treatment differs from native reply")
        immediate = snap("immediate_after_release" if race else "immediate_after_local_write", required=False)
        if not race:
            before_insert = snap("before_insert")
            insertion_bands = bands.copy()
            if case in (CASES[1], CASES[9]):
                insertion_bands[3] = False
            if case in (CASES[2], CASES[25], CASES[35]):
                insertion_bands[3] = True
                if ownership: insertion_bands[4] = True
            if waiting:
                insertion_bands[0] = promotion
            if case in (CASES[25], CASES[35]):
                result["intended_topmost_owner_premise"] = all(before_insert["fixtures"][index]["topmost"] for index in (3, 4))
                insertion_bands[3:5] = [item["topmost"] for item in before_insert["fixtures"][3:5]]
            placement_premise(before_insert, case, insertion_bands)
            require(before_insert["finished_us"] <= treatment_calls[0]["started_us"] <= treatment_calls[0]["returned_us"] <= immediate["started_us"], "treatment not bracketed by observations")
        if case in (CASES[1], CASES[2], CASES[9], CASES[25], CASES[35]):
            stage = snap("after_explicit_topmost" if case in (CASES[2], CASES[25], CASES[35]) else "after_explicit_notopmost")
            if case not in (CASES[25], CASES[35]):
                require(stage["fixtures"][3]["topmost"] == (case == CASES[2]), "explicit band change was not observed")
            level = -1 if case in (CASES[2], CASES[25], CASES[35]) else -2
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
                    require(delayed["fixtures"][0]["topmost"] == bands[0], "A transitioned before release")
                    if valid(immediate) is None:
                        require(immediate["fixtures"][0]["topmost"] == bands[0], "A transitioned at valid immediate read")
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
                require(above(queried, fixtures[2]["handle"], fixtures[4]["handle"]) is True, "C did not start below P")
                require(above(pre_release, fixtures[2]["handle"], fixtures[4]["handle"]) is True, "C crossed P before release")
                require(pre_release["fixtures"][4]["above"] != fixtures[0]["handle"], "C already After(A) before release")
                require(not any(call["window"] == fixtures[2]["handle"] and queried["finished_us"] <= call["started_us"] for call in calls), "stationary P was repositioned")
                require(not any(call["window"] in {item["handle"] for item in fixtures} and call["started_us"] >= release_time for call in calls), "placement after release contaminates C movement")
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
            for observation in observations[-5:]:
                checked(observation, expected)
            require(observations[-1]["started_us"] - observations[0]["started_us"] >= 500000, "settling interval too short")
            for left, right in zip(observations, observations[1:]):
                require(right["started_us"] - left["finished_us"] >= 25000, "settling interval too short or reversed")
            states = [facts(item, target_index, reference_index) for item in observations[-5:]]
            keys = ("target_topmost", "target_directly_after_reference", "target_above_X", "A_topmost", "X_topmost", "target_above", "target_below")
            require(len({tuple(state[key] for key in keys) for state in states}) == 1, "final target state or neighbors did not settle")
            fixture_states = [tuple((item["topmost"], item["above"], item["below"], item["owner"])
                                    for item in observation["fixtures"]) for observation in observations[-5:]]
            require(len(set(fixture_states)) == 1, "final fixture states did not settle")
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
            repeated_immediate = snap("immediate_after_repeated_write", required=False)
            require(immediate["finished_us"] <= first_observations[0]["started_us"], "first settling precedes first treatment")
            require(first_observations[-1]["finished_us"] <= repeated_before["started_us"] <= repeated_before["finished_us"] <= treatment_calls[1]["started_us"] <= treatment_calls[1]["returned_us"] <= repeated_immediate["started_us"], "repeat not after first convergence")
            require(facts(repeated_before, target_index, reference_index) == first_settled, "state changed before repeated call")
            result["first_settled"] = first_settled
            result["repeated_immediate"] = facts(repeated_immediate, target_index, reference_index) if valid(repeated_immediate) is None else None
            result["idempotent_band_and_neighbors"] = all(first_settled[key] == settled[key] for key in ("target_topmost", "target_above", "target_below"))
            last_immediate = repeated_immediate
        require(last_immediate["finished_us"] <= observations[0]["started_us"], "settling precedes treatment")

        allowed = []

        def allow(window, after, flags, preceding, following):
            for observation in (preceding, following):
                require(all(type(observation[key]) is int and observation[key] >= 0 for key in ("started_us", "finished_us"))
                        and observation["started_us"] <= observation["finished_us"], "invalid native write observation bounds")
            allowed.append((window, after, flags, preceding["finished_us"], following["started_us"]))

        if race or transition:
            posted_snapshot = snap("after_async_post_while_held")
            preceding = snap("queried_normal_a_before_c_post" if race else "held_before_post")
            allow(fixtures[4 if race else 0]["handle"], fixtures[0]["handle"] if race else -1 if promotion else -2,
                  16403, preceding, posted_snapshot)
            if race and case != CASES[16]:
                promoted = snap("after_a_promotion_while_c_held")
                allow(fixtures[0]["handle"], -1, 19, posted_snapshot, promoted)
                if case == CASES[17]:
                    allow(fixtures[0]["handle"], -2, 19, promoted, snap("after_a_demotion_while_c_held"))
        if not race:
            if case in (CASES[1], CASES[2], CASES[9], CASES[25], CASES[35]):
                topmost = case in (CASES[2], CASES[25], CASES[35])
                allow(fixtures[3]["handle"], -1 if topmost else -2, synchronous_flags, before,
                      snap("after_explicit_topmost" if topmost else "after_explicit_notopmost"))
            allow(target, reference, synchronous_flags, before_insert, immediate)
            if repeat:
                allow(target, reference, 19, repeated_before, repeated_immediate)
        endpoint = observations[-1]
        if case in CASES[27:29]:
            endpoint = snap("bottom_global_endpoint")
            require(observations[-1]["finished_us"] <= endpoint["started_us"], "bottom endpoint precedes settling")
        interval_start, interval_end = before["finished_us"], endpoint["finished_us"]
        require(interval_start <= interval_end, "reversed measurement interval")
        measured_calls = [call for call in calls
                          if call["returned_us"] >= interval_start and call["started_us"] <= interval_end]
        require(len(measured_calls) == len(allowed), "unscheduled native write in measurement interval")
        previous_return = interval_start
        for call, (window, after, flags, earliest, latest) in zip(measured_calls, allowed):
            require((call["window"], call["after"], call["flags"]) == (window, after, flags),
                    "wrong native write sequence")
            require(max(interval_start, previous_return, earliest) <= call["started_us"] <= call["returned_us"] <= min(interval_end, latest),
                    "native write outside scheduled observation bounds")
            previous_return = call["returned_us"]
        for observation in [item for item in [immediate] + observations if valid(item) is None]:
            sample = observation["fixtures"]
            require(sample[1]["topmost"] == bands[1] and sample[2]["topmost"] == bands[2], "B or reference band changed")
            if not transition and not alternative:
                require(sample[0]["topmost"] == (case == CASES[15] if race else bands[0]), "A band changed outside treatment")
            if race or alternative:
                require(not sample[3]["topmost"], "normal X changed band")
        if transition:
            require(all(item["fixtures"][0]["topmost"] == promotion for item in observations[-5:]), "async reference transition did not settle")
        result["instrumentation_valid"] = True
        result.update(status="eligible_observation", immediate=facts(immediate, target_index, reference_index) if valid(immediate) is None else None, settled=settled)
        if race:
            crossed = above(observations[-1], fixtures[4]["handle"], fixtures[2]["handle"]) is True
            result["C_crossed_stationary_P"] = crossed
            require(crossed and settled["target_directly_after_reference"], "C queued movement across stationary P not established")
        if case in CASES[27:29]:
            global_end = snap("bottom_global_endpoint")
            target_end = global_end["fixtures"][3]
            result["complete_chain_tail"] = canonical_order(global_end)[-1]["handle"] == target_end["handle"]
            result["control_passed"] = not settled["target_topmost"] and not target_end["topmost"] and result["complete_chain_tail"]
        if ownership:
            endpoint = observations[-1]["fixtures"]
            owner_index = 0 if ownership == "A" else 3
            result["owner_group"] = {"owner_topmost": endpoint[owner_index]["topmost"],
                "owned_topmost": endpoint[4]["topmost"], "owned_above_owner": above(observations[-1], endpoint[4]["handle"], endpoint[owner_index]["handle"]),
                "strict_target_after_reference": settled["target_directly_after_reference"]}
            if case in CASES[21:25] or case in CASES[29:31] or case in CASES[32:35]:
                expected_pair = (True, True) if case in (CASES[21], CASES[34]) else (False, True) if case == CASES[23] else (False, False)
                result["control_passed"] = (endpoint[owner_index]["topmost"], endpoint[4]["topmost"]) == expected_pair and result["owner_group"]["owned_above_owner"] is True
            else:
                expected_band = case in (CASES[25], CASES[31], CASES[35])
                result["control_passed"] = endpoint[owner_index]["topmost"] == endpoint[4]["topmost"] == expected_band and result["owner_group"]["owned_above_owner"] is True
        if case in (CASES[7], CASES[27]):
            moved = (settled["target_directly_after_reference"] and above(observations[-1], fixtures[3]["handle"], fixtures[2]["handle"]) is True
                     if case == CASES[7] else above(endpoint, fixtures[2]["handle"], fixtures[3]["handle"]) is True)
            result["movement_across_stationary_P"] = moved
            result["control_passed"] = moved and result.get("control_passed", True)
        result["endpoint_resolved"] = True
        result["question_observed"] = result.get("intended_topmost_owner_premise", True)
        if not result["question_observed"]:
            result["status"] = "measured_operation_failed_prefix"
        result["question_kind"] = "known_contradicted_reference_expectation" if case == CASES[21] else "ownership_diagnostic" if case in (CASES[25], CASES[34], CASES[35]) else "bounded_question_or_control"
        controls = {CASES[2]: True, CASES[3]: False, CASES[10]: False, CASES[16]: False, CASES[17]: False, CASES[18]: False, CASES[19]: False}
        if case in controls:
            result["control_passed"] = settled["target_topmost"] == controls[case] and settled["target_directly_after_reference"]
            if case == CASES[19]:
                result["control_passed"] = result["control_passed"] and settled["target_above_X"] is True
        result["reference_expectation_contradicted"] = result.get("control_passed") is False
    except (ValueError, KeyError, IndexError, TypeError, AttributeError) as error:
        result.update(status="setup_not_run" if phase == "setup" and not result["treatment_executed"] else "protocol_invalid",
                      reason=str(error) or type(error).__name__)
        result["instrumentation_valid"] = phase == "setup" and result.get("setup_protocol_valid", False)
    return result



def analyze_lines(lines):
    session = None
    active = None
    environment = None
    controller = None
    identities = {}
    processes = {}
    results = []
    keys = []
    errors = []
    environment_results = []
    complete = False
    collected = False
    expected = []
    for line_number, line in enumerate(lines, 1):
        try:
            envelope = json.loads(line)
            record = envelope["record"]
            record["recorded_us"] = envelope["unix_us"]
            kind = record["kind"]
            require(not complete, "record after cleanup")
            if kind == "session":
                require(session is None and not keys, "duplicate or late session")
                session = record
                require(record["schema_version"] == 6 and record["case_names"] == CASES, "wrong v6 matrix")
                require(record["protocol"] == "bounded-topmost-v6" and record["scenario_count"] == len(CASES)
                        and record["environment_case_count"] == len(schedule(1)), "wrong v6 metadata")
                repetitions = record["repetitions"]
                require(type(repetitions) is int and 1 <= repetitions <= 20, "invalid repetitions")
                expected = schedule(repetitions)
                declared = [(item["environment"], item["case"], item["repetition"]) for item in record["schedule"]]
                require(declared == expected, "declared schedule differs from fixed suite")
            require(session is not None, "missing session")
            if "call" in record or kind == "fixture_reply" and "call" in record["reply"]["record"]:
                require(active is not None, "native write outside scheduled trial")
            if kind == "environment_ready":
                require(active is None, "environment change inside trial")
                environment = record["environment"]
                controller = (record["process"], record["thread"])
                require(environment in ("ordinary", "private"), "unknown environment")
                desktop = record["desktop"]
                require(desktop["verified"] and desktop["requested"] == desktop["actual"]["name"], "desktop not verified")
                require((desktop["process"], desktop["thread"]) == controller, "desktop assigned on wrong collector thread")
                if environment == "private":
                    require(desktop["requested"] != session["desktop"]["name"], "private environment reused original desktop")
                require(not desktop["switch_requested"] and desktop["requested_open_access"] & 256 == 0, "desktop switching requested")
                require(record["input_method"]["ok"] and record["input_method"]["argument"] == 0 and record["input_method"]["before_first_window"], "collector IME preflight failed")
                require((record["input_method"]["process"], record["input_method"]["thread"]) == controller, "IME called on wrong collector thread")
                identities = {}
                processes = {}
                current_desktop = desktop["requested"]
            if kind == "spawn":
                processes[record["process"]] = record["label"]
            if kind == "fixture_reply" and record["reply"]["record"].get("kind") == "ready":
                ready = record["reply"]["record"]
                require(ready["process"] == record["process"] and ready["process"] in processes, "ready without spawn")
                require(ready["desktop"]["verified"] and ready["desktop"]["requested"] == ready["desktop"]["actual"]["name"] == current_desktop, "child connected to wrong desktop")
                require((ready["desktop"]["process"], ready["desktop"]["thread"]) == (ready["process"], ready["thread"]), "desktop assigned on wrong child thread")
                require(ready["input_method"]["ok"] and ready["input_method"]["argument"] == 0 and ready["input_method"]["before_first_window"], "child IME preflight failed")
                require((ready["input_method"]["process"], ready["input_method"]["thread"]) == (ready["process"], ready["thread"]), "IME called on wrong fixture thread")
                identities[(ready["handle"], ready["process"], ready["thread"])] = processes[ready["process"]]
            if kind == "overlay_identity":
                require((record["process"], record["thread"]) == controller, "X not collector local")
                identities[(record["handle"], record["process"], record["thread"])] = "X"
            if kind == "owned_identity":
                require(active is not None and owned_by(active["case"]) == record["owner_slot"], "unscheduled O")
                item = record["window"]
                identities[(item["handle"], item["process"], item["thread"])] = "O"
            if "snapshot" in record:
                snapshot = record["snapshot"]
                require(snapshot.get("scope") in ("local", "global"), "missing observation scope")
                if snapshot["scope"] == "global":
                    require("raw_enumeration" in snapshot and "global_order" not in snapshot, "invalid global encoding")
                    require(active is not None and (
                        active["case"] in (CASES[5], CASES[7], CASES[20]) and kind in ("before", "before_insert")
                        or active["case"] in CASES[27:29] and kind == "bottom_global_endpoint"), "global observation outside proof stage")
            if kind in ("trial_begin", "trial_not_run"):
                key = (record["environment"], record["case"], record["repetition"])
                require(active is None and not collected, "nested or late trial")
                require(key in expected and key not in keys, "unexpected or duplicate scheduled key")
                keys.append(key)
                if kind == "trial_not_run":
                    require(bool(record["reason"]), "not-run reason missing")
                    results.append({"environment": key[0], "case": key[1], "repetition": key[2],
                        "status": "not_run", "reason": record["reason"], "setup_valid": False, "treatment_executed": False})
                else:
                    require(record["environment"] == environment and controller is not None, "trial environment not ready")
                    active = dict(record, records=[])
            if active is not None:
                active["records"].append(record)
            if kind == "trial_end":
                require(active is not None, "trial end without begin")
                require(all(record[key] == active[key] for key in ("case", "environment", "repetition")), "trial end key mismatch")
                results.append(analyze_trial(active, controller, identities))
                active = None
            if kind in ("environment_end", "environment_cleanup"):
                environment_results.append(record)
            if kind == "collection_complete":
                require(active is None and not collected, "duplicate completion or active trial")
                collected = True
            if kind == "cleanup_complete":
                require(collected and active is None, "cleanup before collection complete")
                complete = True
        except (json.JSONDecodeError, KeyError, ValueError, TypeError, IndexError, AttributeError, UnboundLocalError) as error:
            errors.append({"line": line_number, "error": str(error)})
    if active is not None:
        results.append(analyze_trial(active, controller, identities))
    missing = [key for key in expected if key not in keys]
    matrix = {}
    for environment_name in ("ordinary", "private"):
        for case in scheduled_cases(environment_name):
            rows = [row for row in results if row["case"] == case and row["environment"] == environment_name]
            matrix[environment_name + "/" + case] = {
                "scheduled": session["repetitions"] if session else None,
                "reported": len(rows), "setup_valid": sum(row.get("setup_valid", False) for row in rows),
                "treatment_executed": sum(row.get("treatment_executed", False) for row in rows),
                "eligible": sum(row["status"] == "eligible_observation" for row in rows),
                "not_run": sum(row["status"] == "not_run" for row in rows),
                "questions_observed": sum(row.get("question_observed", False) for row in rows),
                "setup_not_run": sum(row["status"] == "setup_not_run" for row in rows),
                "protocol_invalid": sum(row["status"] == "protocol_invalid" for row in rows),
                "measured_operation_failed_prefix": sum(row["status"] == "measured_operation_failed_prefix" for row in rows),
                "reference_expectation_contradicted": sum(row.get("reference_expectation_contradicted", False) for row in rows),
                "control_failed": sum(row.get("control_passed") is False for row in rows)}
    environments_ok = True
    for name in ("ordinary", "private"):
        endings = [row for row in environment_results if row["kind"] == "environment_end" and row.get("environment") == name]
        cleanups = [row for row in environment_results if row["kind"] == "environment_cleanup" and row.get("environment") == name]
        environments_ok = environments_ok and len(endings) == len(cleanups) == 1 and endings[0].get("ok") is True and not cleanups[0].get("error")
        if len(cleanups) == 1:
            environments_ok = environments_ok and (cleanups[0].get("desktop_closed") is True if name == "private"
                else cleanups[0].get("borrowed_original_retained") is True)
    valid = bool(session and expected and complete and not errors and not missing and active is None and environments_ok
                 and len(results) == len(expected) and all(row["status"] == "eligible_observation" and row.get("control_passed", True) for row in results))
    counts = {"scheduled": len(expected), "questions_observed": sum(row.get("question_observed", False) for row in results),
              "setup_not_run": sum(row["status"] == "setup_not_run" for row in results),
              "protocol_invalid": sum(row["status"] == "protocol_invalid" for row in results),
              "reference_expectation_contradicted": sum(row.get("reference_expectation_contradicted", False) for row in results),
              "measured_operation_failed_prefix": sum(row["status"] == "measured_operation_failed_prefix" for row in results),
              "not_run": sum(row["status"] == "not_run" for row in results)}
    return {"counts": counts, "schema_version": 6, "scenario_count": 36, "environment_case_count": 49,
            "expected_trials": len(expected), "actual_trials": len(keys), "matrix_complete": bool(expected) and not missing and len(keys) == len(expected),
            "missing_schedule": [{"environment": key[0], "case": key[1], "repetition": key[2]} for key in missing],
            "cleanup_complete": complete, "collection_complete": collected, "environment_results": environment_results,
            "malformed_or_protocol_errors": errors, "matrix": matrix, "trials": results,
            "all_trials_eligible_and_controls_passed": valid}
