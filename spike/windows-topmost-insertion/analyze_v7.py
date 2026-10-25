import collections
import json
import analyze_v6 as previous
import protocol_v6 as native_audit
from protocol_v7 import CASES, ROLES, MANIFEST, schedule

require = previous.require


def identity(window):
    return tuple(window[key] for key in ("handle", "process", "thread"))


def interval(record, start, end, first="started_us", last="finished_us"):
    require(all(type(record[key]) is int for key in (first, last)), "invalid native interval type")
    require(start <= record[first] <= record[last] <= end, "native interval outside required bounds")


def thread_ready(record, desktop_name, start, end):
    controller = (record["process"], record["thread"])
    desktop, method = record["desktop"], record["input_method"]
    require(desktop["verified"] and desktop["requested"] == desktop["actual"]["name"] == desktop_name, "wrong assigned desktop")
    require((desktop["process"], desktop["thread"]) == controller, "desktop assigned on wrong thread")
    require(not desktop["switch_requested"] and desktop["requested_open_access"] & 256 == 0, "desktop switch access requested")
    require(method["ok"] and method["argument"] == 0 and method["before_first_window"], "IME preflight failed")
    require((method["process"], method["thread"]) == controller and start <= method["unix_us"] <= end, "IME called on wrong thread or outside creation lifetime")
    return controller


def attributes(actual, expected, start, end):
    interval(actual, start, end)
    previous.checked_window(actual["window"])
    require(identity(actual["window"]) == expected, "attribute identity mismatch")
    require(actual["class"] == MANIFEST["class"] and actual["style_read_error"] is None, "class or style read failed")
    require(type(actual["style"]) is int and len(actual["rectangle"]) == 4 and all(type(value) is int for value in actual["rectangle"]), "invalid native geometry or style")
    window = actual["window"]
    return {"class": actual["class"], "style": actual["style"], "rectangle": actual["rectangle"],
            "extended_style": window["extended_style"], "visible": window["visible"], "topmost": window["topmost"]}


def fixture_state(snapshot, expected):
    fixtures, _ = previous.checked_snapshot(snapshot, expected, "X" if len(expected) == 5 else None)
    roles = dict(zip((value[0] for value in expected), ROLES))
    def reference(handle):
        return roles.get(handle, "outside")
    return [{"role": role, "topmost": window["topmost"], "extended_style": window["extended_style"],
             "above": reference(window["above"]), "below": reference(window["below"]), "owner": roles.get(window["owner"], 0)}
            for role, window in zip(ROLES, fixtures)]


def inventory(snapshot, expected):
    entries = snapshot["fixture_process_classes"]
    require(len(entries) == len(expected) and {identity(row["window"]) for row in entries} == set(expected), "extra or missing fixture-process window")
    by_identity = {identity(window): window for window in snapshot["fixtures"]}
    for entry in entries:
        previous.checked_window(entry["window"])
        require(entry["window"] == by_identity[identity(entry["window"])], "inventory reread differs from fixture snapshot")
        require(entry["class_name"] == MANIFEST["class"] and entry["class_read_error"] is None, "fixture inventory class read failed")


def context(snapshot, expected):
    by_handle = {row["handle"]: row for row in previous.canonical_order(snapshot)}
    fixtures = {item[0] for item in expected}
    boundary = []
    for role, window in zip(ROLES, snapshot["fixtures"]):
        for direction in ("above", "below", "owner"):
            handle = window[direction]
            if handle and handle not in fixtures:
                boundary.append([role, direction, {key: by_handle[handle][key] for key in
                    ("handle", "process", "thread", "owner", "extended_style", "topmost", "visible")}])
    return boundary


def retirement(trial):
    records = trial["records"]
    proof = {"retirement_verified": False}
    try:
        endings = [row for row in records if row["kind"] == "trial_end"]
        joins = [row for row in records if row["kind"] == "trial_thread_retired"]
        require(len(endings) == len(joins) == 1, "missing or duplicated retirement envelope")
        ending, joined = endings[0], joins[0]
        start, end = trial["recorded_us"], ending["recorded_us"]
        local, spawned = {}, {}
        last_activity = start
        retiring = False
        retirements = []
        first_creation = None
        for row in records:
            kind = row["kind"]
            if kind in ("window_retired", "process_retired"):
                retiring = True
                retirements.append(row)
                continue
            if kind in ("trial_end", "trial_thread_retired"):
                continue
            require(not retiring, "native activity after retirement started")
            last_activity = max(last_activity, row["recorded_us"])
            if kind == "spawn":
                require(row["label"] in ROLES[:3] and row["label"] not in spawned, "ambiguous spawned process")
                spawned[row["label"]] = row["process"]
            if kind in ("creation", "window_acquired"):
                creation = row["creation"]
                wanted = (creation["handle"], creation["calling_process"], creation["calling_thread"])
                require(row["role"] in ("X", "O") and wanted[0] != 0, "invalid acquired local window")
                require(row["role"] not in local or local[row["role"]] == wanted, "conflicting acquired local window")
                local[row["role"]] = wanted
                first_creation = min(first_creation or creation["started_us"], creation["started_us"])
            if "snapshot" in row:
                reached = ("X",) if kind in ("fresh_inventory", "owner_promoted_before_creation") else ("X", "O")
                require(all(role in local for role in reached), "reached observation has untracked local resource")
            if kind == "initialization_failed" and row["resource_owned"]:
                require(row["role"] in (spawned if row["stage"] == "readiness" else local), "failed initialization resource untracked")
        require(start <= last_activity <= end, "retirement envelope excludes native activity")
        require(records.index(ending) < records.index(joined), "thread join preceded trial end")
        if retirements:
            require(records.index(retirements[-1]) < records.index(ending), "retirement recorded after trial end")
        roles = [role for role in ("O", "X") if role in local] + [role for role in ("P", "B", "A") if role in spawned]
        require([row["role"] for row in retirements] == roles, "missing or reordered retirement")
        previous_finished = last_activity
        for row in retirements:
            role = row["role"]
            if role in local:
                require(row["kind"] == "window_retired" and identity(row["identity"]) == local[role]
                        and row["api"] == "DestroyWindow" and row["ok"] is True and row["absent"] is True,
                        "local destruction unproven")
                interval(row, previous_finished, end, "started_us", "returned_us")
                require(row["returned_us"] <= row["checked_us"] <= end, "destruction reread not bracketed")
                previous_finished = row["checked_us"]
            else:
                require(row["kind"] == "process_retired" and row["process"] == spawned[role]
                        and row["exited"] is True and not row["error"], "source process retirement unproven")
                require(previous_finished <= row["finished_us"] <= end, "source retirement out of order")
                previous_finished = row["finished_us"]
        require(not ending["cleanup_errors"], "collector cleanup failed")
        require(joined["lifetime_id"] == trial["lifetime_id"] and joined["joined"] is True
                and not joined.get("error") and end <= joined["recorded_us"], "thread retirement unproven")
        proof.update(retirement_verified=True, retired_us=joined["recorded_us"])
        if first_creation is not None:
            proof["first_creation_us"] = first_creation
    except (ValueError, KeyError, TypeError, IndexError, AttributeError) as error:
        proof["retirement_error"] = str(error)
    return proof


def analyze_trial(trial, desktop_name):
    result = {key: trial[key] for key in ("case", "environment", "repetition", "pair_id", "lifetime_id", "family", "planned_order", "treatment_flags")}
    records = trial["records"]
    result.update(instrumentation_valid=False, intended_premise=False, endpoint_resolved=False,
                  treatment_executed=any(row["kind"] == "treatment_call" for row in records), snapshot_validity=[])
    phase = "protocol"
    def one(kind):
        rows = [row for row in records if row["kind"] == kind]
        require(len(rows) == 1, "missing or duplicated " + kind)
        return rows[0]
    try:
        allowed_kinds = {"trial_begin", "trial_thread_ready", "spawn", "fixture_reply", "creation", "fresh_inventory",
                         "local_overlay_call", "owner_promoted_before_creation", "after_owned_creation", "fixture_command",
                         "setup_p_bottom", "setup_b_topmost", "setup_a_topmost", "before", "setup_eligibility", "before_insert",
                         "immediate_after_local_write", "treatment_call", "settling", "window_retired", "process_retired",
                         "trial_end", "trial_thread_retired", "window_acquired", "initialization_failed"}
        require(all(row["kind"] in allowed_kinds for row in records), "unscheduled trial record")
        end_record = one("trial_end")
        result["collector_error"] = end_record.get("error")
        start, end = trial["recorded_us"], end_record["recorded_us"]
        require(start <= end, "reversed trial envelope")
        require(trial["fixture_slots"] == ROLES and trial["owned_by"] == "X", "wrong fixture roles")
        require(all(end_record[key] == trial[key] for key in ("case", "environment", "repetition", "lifetime_id")), "trial end identity mismatch")
        ready = one("trial_thread_ready")
        controller = thread_ready(ready, desktop_name, trial["admitted_us"], end)
        creations, source_ready, spawns = [], {}, {}
        for row in records:
            if row["kind"] == "fixture_reply":
                require(row["reply"]["record"].get("kind") in ("ready", "placed"), "unscheduled fixture operation")
            if row["kind"] == "spawn":
                require(row["label"] in ROLES[:3] and row["label"] not in spawns, "duplicate or unscheduled source spawn")
                spawns[row["label"]] = row["process"]
            if row["kind"] == "fixture_reply" and row["reply"]["record"].get("kind") == "ready":
                source = row["reply"]["record"]
                roles = [role for role, process in spawns.items() if process == row["process"]]
                require(len(roles) == 1 and source["process"] == row["process"], "source identity missing spawn")
                role = roles[0]
                require(role not in source_ready, "duplicate source readiness")
                source_ready[role] = source
                creations.append((role, source["creation"]))
            if row["kind"] == "creation":
                creations.append((row["role"], row["creation"]))
        require([role for role, _ in creations] == ROLES[:len(creations)] and len(creations) <= 5, "missing or reordered fresh creations")
        expected = [(creation["handle"], creation["calling_process"], creation["calling_thread"]) for _, creation in creations]
        require(len(set(expected)) == len({item[0] for item in expected}) == len(creations), "duplicate window lifetime")
        require(all(item[1:] == controller for item in expected[3:]), "X/O not fresh-thread local")
        require(len({item[1] for item in expected[:4]}) == len({item[2] for item in expected[:4]}) == min(4, len(expected)), "baseline processes or threads not independent")
        topmost = trial["family"] != "owner_promotion"
        signature = []
        previous_finished = start
        for index, (role, creation) in enumerate(creations):
            wanted, overlay = expected[index], index >= 3
            require(creation["api"] == "CreateWindowExW" and creation["class"] == MANIFEST["class"], "wrong constructor")
            require(creation["desktop"]["name"] == desktop_name, "constructor on wrong desktop")
            require(creation["geometry"] == MANIFEST["overlay_geometry" if overlay else "source_geometry"], "wrong requested geometry")
            require(creation["style"] == (0x90000000 if overlay else 0x10cf0000) and creation["extended_style"] == (0x08000080 if overlay else 0x08000000), "wrong requested style")
            interval(creation, previous_finished, end, "started_us", "returned_us")
            interval(creation, previous_finished, creation["started_us"], "owner_read_started_us", "owner_read_finished_us")
            actual = attributes(creation["actual"], wanted, creation["returned_us"], end)
            require(actual["style"] & creation["style"] == creation["style"]
                    and actual["extended_style"] & creation["extended_style"] == creation["extended_style"], "actual style missing requested flags")
            window = creation["actual"]["window"]
            require(window["visible"] and window["owner"] == creation["owner"] == (expected[3][0] if index == 4 else 0), "wrong constructor ownership")
            require(index == 4 or not window["topmost"], "unexpected baseline creation band")
            if index < 3:
                source = source_ready[role]
                require((source["handle"], source["process"], source["thread"]) == wanted, "ready identity differs from creation")
                thread_ready(source, desktop_name, start, creation["started_us"])
                previous.checked_window(source["initial"])
                require(identity(source["initial"]) == wanted, "initial source read identity mismatch")
            if index == 3:
                require(ready["input_method"]["unix_us"] <= creation["started_us"], "X preceded IME disable")
            if index == 4:
                owner = creation["owner_before"]
                previous.checked_window(owner)
                require(identity(owner) == expected[3] and owner["owner"] == 0 and owner["topmost"] == topmost, "wrong actual precreation owner state")
            else:
                require(creation["owner_before"] is None, "unowned creation records owner")
            signature.append({"role": role, "requested_style": creation["style"], "requested_extended_style": creation["extended_style"],
                              "requested_geometry": creation["geometry"], "actual": actual, "owner": "X" if index == 4 else 0,
                              "owner_topmost_at_creation": topmost if index == 4 else None})
            previous_finished = creation["actual"]["finished_us"]
        failures = [row for row in records if row["kind"] == "initialization_failed"]
        acquired = [row for row in records if row["kind"] == "window_acquired"]
        require(len(failures) <= 1, "duplicated initialization failure")
        require([row["role"] for row in acquired] in ([], ["X"], ["X", "O"]), "reordered local acquisition")
        for row in acquired:
            creation = row["creation"]
            interval(creation, start, end, "started_us", "returned_us")
            require(creation["handle"] != 0 and (creation["calling_process"], creation["calling_thread"]) == controller,
                    "acquisition not local to fresh thread")
            completed_creation = next((item for role, item in creations if role == row["role"]), None)
            if completed_creation is not None:
                require(all(creation[key] == completed_creation[key] for key in
                            ("handle", "calling_process", "calling_thread", "started_us", "returned_us")), "acquisition differs from constructor")
            else:
                require(len(failures) == 1 and failures[0]["role"] == row["role"] and failures[0]["stage"] == "attributes",
                        "acquired window missing metadata or failure")
        if failures:
            failure = failures[0]
            require(len(creations) < 5 and failure["role"] == ROLES[len(creations)] and bool(failure["error"]), "initialization failure at wrong stage")
            role, stage = failure["role"], failure["stage"]
            require(stage in (("launch", "readiness") if role in ROLES[:3] else ("create", "attributes")), "wrong initialization operation")
            owned = role in spawns if role in ROLES[:3] else any(row["role"] == role for row in acquired)
            require(failure["resource_owned"] is owned and owned == (stage in ("readiness", "attributes")), "initialization ownership proof missing")
            interval(failure, previous_finished, end)
            if stage == "attributes":
                acquisition = next(row["creation"] for row in acquired if row["role"] == role)
                require(failure["started_us"] <= acquisition["started_us"] <= acquisition["returned_us"] <= failure["finished_us"], "acquisition outside failed initialization")
            failure_index = records.index(failure)
            require(all(row["kind"] in ("window_retired", "process_retired", "trial_end", "trial_thread_retired") for row in records[failure_index + 1:]), "activity after initialization failure")
        require(set(spawns) == set(source_ready) | ({failures[0]["role"]} if failures and failures[0]["stage"] == "readiness" else set()), "spawn without readiness or initialization failure")
        for row in (row for row in records if "snapshot" in row):
            snapshot = row["snapshot"]
            count = 4 if row["kind"] in ("fresh_inventory", "owner_promoted_before_creation") else 5
            identities = expected[:count]
            try:
                require(len(identities) == count, "snapshot missing reached constructor identities")
                interval(snapshot, start, end)
                fixture_state(snapshot, identities)
                reason = None
            except (ValueError, KeyError, TypeError, IndexError) as error:
                reason = str(error)
            result["snapshot_validity"].append(dict(kind=row["kind"], sample=row.get("sample"), valid=reason is None, reason=reason))
            require(snapshot["scope"] == ("global" if row["kind"] == "before" else "local"), "global observation outside proof stage")
            if row["kind"] not in ("settling", "immediate_after_local_write"):
                require(reason is None, row["kind"] + ": " + str(reason))
        calls = native_audit.native_calls(records)
        steps = [("X", -1, "owner_promoted_before_creation")] if topmost else []
        steps += [("P", 1, "setup_p_bottom"), ("B", -1, "setup_b_topmost"), ("A", -1, "setup_a_topmost")]
        treatments = [row["call"] for row in records if row["kind"] == "treatment_call"]
        require(len(treatments) <= 1 and len(calls) <= len(steps) + len(treatments), "unscheduled native call")
        setup_calls = calls[:-1] if treatments and calls and calls[-1] == treatments[0] else calls
        reached_steps = steps[:len(setup_calls)]
        by_role = dict(zip(ROLES, expected))
        commands = [(row["process"], row["command"]) for row in records if row["kind"] == "fixture_command"]
        require(commands == [(by_role[role][1], {"operation": "place", "after": after})
                             for role, after, _ in reached_steps if role in ROLES[:3]], "unscheduled or missing source command")
        for call in calls:
            interval(call, start, end, "started_us", "returned_us")
            require(call["api"] == "SetWindowPos" and call["ok"] and all(call[key] == 0 for key in ("x", "y", "width", "height")), "failed or geometry-changing native write")
            owners = [item for item in expected if item[0] == call["window"]]
            require(len(owners) == 1 and owners[0][1:] == (call["calling_process"], call["calling_thread"]), "write not fixture-owner local")
            require(call["flags_names"] == "NOMOVE|NOSIZE|NOACTIVATE" + ("|NOOWNERZORDER" if call["flags"] == 531 else ""), "flag names disagree")
        native_audit.validate_target_reads(records, expected, start, end)
        fresh = one("fresh_inventory")["snapshot"] if len(expected) >= 4 else None
        if fresh is not None:
            inventory(fresh, expected[:4])
            require(creations[3][1]["actual"]["finished_us"] <= fresh["started_us"], "inventory precedes X creation")
        created = one("after_owned_creation")["snapshot"] if len(expected) == 5 else None
        stop_reason = stop_stage = None
        if created is not None:
            inventory(created, expected)
            require(creations[4][1]["actual"]["finished_us"] <= created["started_us"], "owned observation precedes creation read")
            if [row["topmost"] for row in created["fixtures"]] != [False, False, False, topmost, topmost]:
                stop_reason = "unexpected bands after owned creation"
                stop_stage = "after_owned_creation"
        if fresh is not None and topmost:
            previous_finished = fresh["finished_us"]
        elif created is not None:
            previous_finished = created["finished_us"]
        for call, (role, after, kind) in zip(setup_calls, reached_steps):
            following = one(kind)["snapshot"]
            require((call["window"], call["after"], call["flags"]) == (by_role[role][0], after, 19), "wrong setup call sequence")
            interval(call, previous_finished, following["started_us"], "started_us", "returned_us")
            previous_finished = following["finished_us"]
            if kind == "owner_promoted_before_creation":
                require(fresh is not None, "promotion without fresh inventory")
                if not following["fixtures"][3]["topmost"]:
                    stop_reason = "precreation promotion premise failed"
                    stop_stage = kind
                if created is not None:
                    require(previous_finished <= creations[4][1]["owner_read_started_us"], "O read or creation preceded promotion observation")
                    require(creations[4][1]["owner_before"] == following["fixtures"][3], "owner changed between prefix and creation")
                    previous_finished = created["finished_us"]
            else:
                require(created is not None, "source setup preceded O creation")
                require(created["finished_us"] <= call["started_us"], "source setup preceded O observation")
        for _, _, kind in steps[len(setup_calls):]:
            require(not any(row["kind"] == kind for row in records), "setup observation without native call")
        if not topmost and created is not None:
            require(fresh["finished_us"] <= creations[4][1]["owner_read_started_us"], "O preceded fresh inventory")
            require(creations[4][1]["owner_before"] == fresh["fixtures"][3], "normal owner changed before O creation")
        if stop_reason:
            stopped_at = records.index(one(stop_stage))
            require(all(row["kind"] in ("window_retired", "process_retired", "trial_end", "trial_thread_retired")
                        for row in records[stopped_at + 1:]), "continued after failed prefix premise")
            phase = "setup_not_run"
            raise ValueError(stop_reason)
        if failures:
            require(not treatments, "treatment after initialization failure")
            role = failures[0]["role"]
            required_calls = int(topmost and role == "O")
            require(len(setup_calls) == required_calls, "missing prefix before initialization failure")
            interval(failures[0], previous_finished, end)
            phase = "setup_failed"
            raise ValueError(failures[0]["stage"] + ": " + failures[0]["error"])
        require(len(expected) == 5, "missing fresh creations without native initialization failure")
        require(len(setup_calls) == len(steps), "missing native setup call")
        before_row = one("before")
        before = before_row["snapshot"]
        require(previous_finished <= before["started_us"], "before precedes setup")
        inventory(before, expected)
        before_state = fixture_state(before, expected)
        order = previous.canonical_order(before)
        role_handles = {value[0]: role for role, value in by_role.items()}
        controlled_order = [role_handles[row["handle"]] for row in order if row["handle"] in role_handles]
        require(not any(row["owner"] in role_handles and row["handle"] != expected[4][0] for row in order), "unscheduled owned window in global premise")
        result["initial"] = {"state": before_state, "fixture_order": controlled_order}
        result["external_boundary"] = context(before, expected)
        require(trial["environment"] != "private" or len(order) == 5, "private desktop contains nonfixture windows")
        before_attributes = before_row["attributes"]
        require(len(before_attributes) == 5, "missing before attributes")
        compared_attributes = []
        for index, actual in enumerate(before_attributes):
            compared_attributes.append(attributes(actual, expected[index], before["finished_us"], end))
            require(actual["window"] == before["fixtures"][index], "attribute read differs from before")
            require(actual["rectangle"] == creations[index][1]["actual"]["rectangle"], "geometry changed after creation")
            require(actual["style"] == creations[index][1]["actual"]["style"], "window style changed after creation")
            entry = next(entry for entry in before["fixture_process_classes"] if identity(entry["window"]) == expected[index])
            require(entry["window_style"] == actual["style"], "before style differs from inventory reread")
        def controlled_state(snapshot):
            return [{key: value for key, value in state.items() if key not in ("above", "below")}
                    for state in fixture_state(snapshot, expected)]
        result["setup_signature"] = {"construction": signature, "setup_calls": [[role, after, 19] for role, after, _ in steps],
                                     "initial": {"state": controlled_state(before), "fixture_order": controlled_order},
                                     "attributes": compared_attributes, "created": controlled_state(created),
                                     "owner_group_adjacency": [snapshot["fixtures"][4]["below"] == expected[3][0]
                                         and snapshot["fixtures"][3]["above"] == expected[4][0] for snapshot in (created, before)]}
        premise_ok = controlled_order == MANIFEST["initial_fixture_order"] and [row["topmost"] for row in before["fixtures"]] == [True, True, False, topmost, topmost]
        require(one("setup_eligibility")["eligible"] == premise_ok, "collector premise disagrees with native evidence")
        if not premise_ok:
            require(not treatments, "treatment despite rejected setup")
            phase = "premise"
            raise ValueError("actual starting order or bands differ from fixed premise")
        insertion = one("before_insert")["snapshot"]
        require(max(actual["finished_us"] for actual in before_attributes) <= insertion["started_us"], "insertion preceded attribute reads")
        require(insertion["fixtures"] == before["fixtures"], "starting order or bands changed before treatment")
        if trial["family"] == "owner_after_a":
            require(insertion["fixtures"][3]["above"] != by_role["A"][0], "relative target already after A")
        result["intended_premise"] = True
        require(len(treatments) == 1 and calls[-1] == treatments[0], "treatment missing or not last native write")
        role, after = MANIFEST[trial["family"]]["treatment"]
        after = by_role[after][0] if isinstance(after, str) else after
        call = treatments[0]
        require((call["window"], call["after"], call["flags"]) == (by_role[role][0], after, trial["treatment_flags"]), "wrong treatment mutation")
        immediate = one("immediate_after_local_write")["snapshot"]
        interval(call, insertion["finished_us"], immediate["started_us"], "started_us", "returned_us")
        samples = [row for row in records if row["kind"] == "settling"]
        require([row["sample"] for row in samples] == list(range(21)), "missing or adaptive settlement")
        observations = [row["snapshot"] for row in samples]
        require(immediate["finished_us"] <= observations[0]["started_us"], "settlement preceded immediate read")
        for left, right in zip(observations, observations[1:]):
            require(right["started_us"] - left["finished_us"] >= 25000, "settlement gap too short or reversed")
        require(observations[-1]["started_us"] - observations[0]["started_us"] >= 500000, "settlement shorter than 500ms")
        for snapshot in observations[-5:]:
            interval(snapshot, start, end)
        finals = [fixture_state(snapshot, expected) for snapshot in observations[-5:]]
        require(all(snapshot["fixtures"] == observations[-1]["fixtures"] for snapshot in observations[-5:]), "final five native states differ")
        require(all(state == finals[-1] for state in finals), "final five normalized states differ")
        result.update(settled=finals[-1], endpoint_resolved=True,
                      outcome={"owner_topmost": finals[-1][3]["topmost"], "owned_topmost": finals[-1][4]["topmost"],
                               "owned_above_owner": previous.above(observations[-1], expected[4][0], expected[3][0]),
                               "owner_directly_after_A": observations[-1]["fixtures"][3]["above"] == expected[0][0],
                               "owned_directly_after_A": observations[-1]["fixtures"][4]["above"] == expected[0][0]})
        require(not end_record.get("error") and end_record["status"] == "collected_pending_independent_analysis", "collector reported failure after measured endpoint")
        result.update(instrumentation_valid=True, status="eligible_observation", first_creation_us=creations[0][1]["started_us"])
    except (ValueError, KeyError, TypeError, IndexError, AttributeError) as error:
        status = "premise_unmatched" if phase == "premise" else phase if phase in ("setup_not_run", "setup_failed") else "protocol_invalid"
        result.update(status=status, reason=str(error))
    result.update(retirement(trial))
    if not result["retirement_verified"]:
        result.update(status="protocol_invalid", instrumentation_valid=False)
        result.setdefault("reason", result["retirement_error"])
    return result


def compare_pair(members):
    result = {"pair_id": members[0]["pair_id"], "family": members[0]["family"], "members": [row["lifetime_id"] for row in members], "matched": False}
    try:
        require(len(members) == 2, "pair member missing or duplicated")
        require(not any(row.get("contaminated_admission") for row in members), "contaminated admission prevents pair attribution")
        require(all(row.get("instrumentation_valid") and row.get("intended_premise") and row.get("endpoint_resolved") and row.get("treatment_executed") for row in members), "member instrumentation, premise, treatment or final five unresolved")
        left, right = members
        require(left["pair_id"] == right["pair_id"] and left["family"] == right["family"]
                and left["repetition"] == right["repetition"] and left["lifetime_id"] != right["lifetime_id"], "pair identity mismatch")
        require(left["setup_signature"] == right["setup_signature"], "actual construction history, styles, geometry, setup calls or starting order differ")
        environment_pair = left["family"] == "owned_demotion"
        if environment_pair:
            require({left["environment"], right["environment"]} == {"ordinary", "private"} and left["treatment_flags"] == right["treatment_flags"] == 19, "wrong desktop comparison")
        else:
            require(left["environment"] == right["environment"] and {left["treatment_flags"], right["treatment_flags"]} == {19, 531}, "wrong flag comparison")
            require(left["external_boundary"] == right["external_boundary"], "relevant external boundary context changed")
        result.update(matched=True, outcome_differs=left["outcome"] != right["outcome"],
                      outcomes=[{"lifetime_id": row["lifetime_id"], "flags": row["treatment_flags"], "environment": row["environment"], "state": row["settled"], "outcome": row["outcome"]} for row in members],
                      interpretation="desktop-condition comparison, not isolation of activation or Windows internal cause" if environment_pair else "flag comparison conditional on matched recorded fixture and external-boundary context")
    except (ValueError, KeyError, TypeError) as error:
        result["reason"] = str(error)
    return result


def analyze_lines(lines):
    session = active = environment = None
    expected, results, errors, keys, environment_records = [], [], [], [], []
    complete = collected = False
    previous_retired = 0
    admission_failure = None
    for number, line in enumerate(lines, 1):
        try:
            envelope = json.loads(line)
            row = dict(envelope["record"], recorded_us=envelope["unix_us"])
            kind = row["kind"]
            require(not complete, "record after cleanup")
            if kind == "session":
                require(session is None, "duplicate session")
                session = row
                require(row["schema_version"] == 7 and row["protocol"] == "fresh-ownership-v7", "wrong protocol")
                require(row["case_names"] == CASES and row["scenario_count"] == 5 and row["environment_case_count"] == 10, "wrong focused matrix")
                require(row["pair_manifest"] == MANIFEST, "wrong construction manifest")
                require(type(row["repetitions"]) is int and 1 <= row["repetitions"] <= 20, "invalid repetition count")
                expected = schedule(row["repetitions"])
                require(row["schedule"] == expected, "declared schedule differs from fixed suite")
                previous_retired = row["recorded_us"]
            require(session is not None, "missing session")
            if kind == "environment_ready":
                require(active is None, "environment changed during trial")
                environment = row
                require(row["environment"] in ("ordinary", "private"), "unknown environment")
                require((row["desktop_name"] == session["desktop"]["name"]) == (row["environment"] == "ordinary"), "environment desktop identity mismatch")
            if kind in ("trial_begin", "trial_not_run"):
                require(active is None and not collected, "nested or late trial")
                candidate = row if kind == "trial_begin" else row["member"]
                require(len(keys) < len(expected) and all(candidate.get(key) == value for key, value in expected[len(keys)].items()), "executed schedule differs from predetermined order")
                keys.append(candidate["lifetime_id"])
                if kind == "trial_not_run":
                    require(bool(row["reason"]), "missing not-run reason")
                    results.append(dict(candidate, status="not_run", reason=row["reason"]))
                else:
                    require(environment is not None and row["environment"] == environment["environment"], "trial environment not ready")
                    active = dict(row, records=[], admitted_us=previous_retired)
                    require(row["recorded_us"] >= previous_retired, "new lifetime before previous retirement")
            if kind in ("environment_end", "environment_cleanup"):
                environment_records.append(row)
            if active is not None:
                active["records"].append(row)
            elif "call" in row or kind in ("creation", "window_retired", "process_retired", "fixture_reply", "spawn"):
                require(False, "native activity outside scheduled lifetime")
            if kind == "trial_thread_retired":
                require(active is not None, "thread retirement without trial")
                result = analyze_trial(active, environment["desktop_name"])
                require(result.get("first_creation_us", previous_retired) >= previous_retired, "fixture creation overlaps previous lifetime")
                if admission_failure is not None:
                    result.update(contaminated_admission=True, admission_error=admission_failure)
                results.append(result)
                if result["retirement_verified"]:
                    previous_retired = result["retired_us"]
                else:
                    admission_failure = admission_failure or result["retirement_error"]
                    errors.append({"line": number, "error": "fatal admission retirement failure: " + result["retirement_error"]})
                active = None
            if kind == "collection_complete":
                require(active is None and not collected, "completion during trial or duplicate completion")
                collected = True
            if kind == "cleanup_complete":
                require(collected and active is None, "cleanup before collection complete")
                complete = True
        except (ValueError, KeyError, TypeError, IndexError, AttributeError) as error:
            errors.append({"line": number, "error": str(error)})
    if active is not None:
        results.append(analyze_trial(active, environment["desktop_name"]))
    grouped = collections.defaultdict(list)
    for row in results:
        grouped[row["pair_id"]].append(row)
    planned_pairs = {member["pair_id"]: member["family"] for member in expected}
    pairs = [compare_pair(grouped[pair_id]) if grouped[pair_id] else
             dict(pair_id=pair_id, family=family, members=[], matched=False, reason="no pair members recorded")
             for pair_id, family in planned_pairs.items()]
    if errors:
        for pair in pairs:
            pair.update(matched=False, reason="session protocol errors prevent pair attribution")
    environments_ok = True
    for name in ("ordinary", "private"):
        endings = [row for row in environment_records if row["kind"] == "environment_end" and row["environment"] == name]
        cleanups = [row for row in environment_records if row["kind"] == "environment_cleanup" and row["environment"] == name]
        environments_ok = environments_ok and len(endings) == len(cleanups) == 1 and endings[0]["ok"] and not cleanups[0].get("error")
        if len(cleanups) == 1:
            environments_ok = environments_ok and cleanups[0].get("desktop_closed" if name == "private" else "borrowed_original_retained") is True
    resolved = bool(expected and complete and collected and not errors and environments_ok and len(keys) == len(expected) and len(results) == len(expected) and all(pair["matched"] for pair in pairs))
    return {"schema_version": 7, "scenario_count": 5, "environment_case_count": 10,
            "expected_trials": len(expected), "actual_trials": len(keys), "matrix_complete": bool(expected) and len(keys) == len(expected),
            "missing_schedule": [member for member in expected if member["lifetime_id"] not in keys],
            "collection_complete": collected, "cleanup_complete": complete, "environment_results": environment_records,
            "malformed_or_protocol_errors": errors, "trials": results, "pairs": pairs,
            "counts": {"scheduled": len(expected), "eligible_observations": sum(row.get("instrumentation_valid", False) for row in results),
                       "treatment_executed": sum(row.get("treatment_executed", False) for row in results),
                       "setup_failed": sum(row["status"] == "setup_failed" for row in results),
                       "contaminated_admissions": sum(row.get("contaminated_admission", False) for row in results),
                       "setup_not_run": sum(row["status"] == "setup_not_run" for row in results),
                       "premise_unmatched": sum(row["status"] == "premise_unmatched" for row in results),
                       "protocol_invalid": sum(row["status"] == "protocol_invalid" for row in results),
                       "not_run": sum(row["status"] == "not_run" for row in results),
                       "planned_pairs": 5 * session["repetitions"] if session else 0,
                       "matched_pairs": sum(pair["matched"] for pair in pairs), "unresolved_pairs": sum(not pair["matched"] for pair in pairs)},
            "all_pairs_resolved": resolved, "all_trials_eligible_and_controls_passed": resolved,
            "limits": [MANIFEST["freshness"], MANIFEST["comparison_limit"],
                       "Desktop comparison does not isolate active versus inactive desktop. No desktop activation requested.",
                       "Different outcomes are observations, not failed reference expectations or proof of Windows internal cause."]}
