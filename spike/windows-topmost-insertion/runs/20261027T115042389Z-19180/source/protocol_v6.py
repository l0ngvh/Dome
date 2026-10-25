import analyze_v5 as previous

require = previous.require
CASES = previous.CASES.copy()
CASES[7] += "_displaced_v6"
CASES[27] = "normal_x_displaced_to_bottom"
CASES += ["owner_x_demotion_owner_topmost_before_o_creation",
          "owned_o_demotion_owner_topmost_before_o_creation",
          "topmost_x_after_a_owner_topmost_before_o_creation",
          "owner_x_demotion_owner_then_owned_topmost",
          "owner_x_demotion_owned_then_owner_topmost",
          "owner_x_promotion_noownerzorder",
          "topmost_x_then_after_a_with_x_owned_o_noownerzorder"]


def scheduled_cases(environment):
    return [case for number, case in enumerate(CASES, 1) if
            (number not in (6, 8, 21, 23, 24, 25, 27, 28, 29) if environment == "ordinary" else
             number in (1, 3, 4, 6, 8, 11) or number >= 21 if environment == "private" else False)]


def schedule(repetitions):
    return [(environment, case, repetition) for environment in ("ordinary", "private")
            for repetition in range(repetitions) for case in scheduled_cases(environment)]


def owned_by(case):
    return "X" if case in CASES[29:] else previous.owned_by(case)


def setup_steps(case):
    steps = []
    if case in CASES[29:34]:
        steps.append((3, -2, "owner_reset_before_creation"))
    if case in CASES[29:32]:
        steps.append((3, -1, "owner_promoted_before_creation"))
    steps += [(2, -2, "setup_normal_reference"),
              (1, -2 if "sole_" in case else -1, "setup_b"),
              (0, -2 if case.startswith("held_c_") or "a_promotion" in case else -1, "setup_a")]
    if case.startswith("held_c_"):
        steps.append((4, -2, "setup_c"))
    if case == CASES[33]:
        steps.append((4, -1, "setup_o_first"))
    if case not in CASES[29:32]:
        topmost = case in (CASES[3], CASES[8], CASES[9], CASES[22], CASES[24], CASES[28], CASES[32], CASES[33])
        steps.append((3, -1 if topmost else -2, "setup_x"))
    if case == CASES[32]:
        steps.append((4, -1, "setup_o_last"))
    if case == CASES[7]:
        steps += [(0, "B", "setup_clean_prefix_a"), (3, "P", "setup_displaced_x")]
    if case == CASES[27]:
        steps += [(3, -1, "setup_displacement_promote"), (3, -2, "setup_displaced_x")]
    if case == CASES[19]:
        steps.append((3, "P", "setup_normal_predecessor"))
    if case == CASES[20]:
        steps += [(2, -1, "setup_topmost_predecessor"), (2, "B", "setup_clean_prefix_p"),
                  (3, -1, "setup_boundary_overlay_promote"), (3, -2, "setup_boundary_overlay")]
    if case.startswith("held_c_"):
        steps += [(3, "A", "setup_race_x_after_a"), (2, "X", "setup_race_p_after_x"),
                  (4, "P", "setup_race_c_after_p")]
    return steps


def native_calls(records):
    unique = {}
    for record in records:
        call = record.get("call")
        source = record["kind"]
        if source == "fixture_reply":
            call = record["reply"]["record"].get("call")
        if call is None:
            continue
        key = (call["calling_process"], call["calling_thread"], call["started_us"])
        if key in unique:
            original, sources = unique[key]
            require(original == call and len(sources) == 1 and {sources[0], source} == {"fixture_reply", "treatment_call"}, "unexpected or conflicting duplicate call")
            sources.append(source)
        else:
            require(source != "treatment_call" or call["flags"] != 16403, "asynchronous synchronous treatment")
            unique[key] = (call, [source])
    return sorted((call for call, _ in unique.values()), key=lambda call: call["started_us"])


def expected_identities(trial, identities):
    expected = []
    for role in ("A", "B", "normalA", "X"):
        matches = [identity for identity, label in identities.items() if label == role]
        require(len(matches) == 1, "missing or ambiguous baseline identity " + role)
        expected.append(matches[0])
    if owned_by(trial["case"]):
        creations = [row for row in trial["records"] if row["kind"] == "owned_identity"]
        require(len(creations) <= 1, "duplicate O creation")
        if creations:
            window = creations[0]["window"]
            identity = tuple(window[key] for key in ("handle", "process", "thread"))
            require(identities.get(identity) == "O", "O identity has no creation record")
            expected.append(identity)
    elif trial["case"].startswith("held_c_"):
        candidates = [row["snapshot"] for row in trial["records"] if "snapshot" in row]
        require(bool(candidates), "no C identity observation")
        window = candidates[0]["fixtures"][4]
        identity = tuple(window[key] for key in ("handle", "process", "thread"))
        require(identities.get(identity, "").startswith("C-"), "missing separate C creation record")
        expected.append(identity)
    return expected


def validate_target_reads(records, expected, start, end):
    snapshots = [row["snapshot"] for row in records if "snapshot" in row]
    for record in records:
        if record["kind"] == "fixture_reply":
            reply = record["reply"]["record"]
            if "call" not in reply and reply.get("kind") != "placed":
                continue
            require(reply["kind"] == "placed", "native fixture call without placed reply")
            require(record["process"] == reply["call"]["calling_process"], "fixture reply has wrong native caller")
        elif record["kind"] == "external_async_call":
            reply = record
        else:
            continue
        call = reply["call"]
        read = reply["post_return_target_read"]
        previous.checked_window(read)
        require(tuple(read[key] for key in ("handle", "process", "thread")) in expected, "immediate read has wrong identity")
        require(read["handle"] == call["window"], "immediate read has wrong target")
        require(all(type(reply[key]) is int for key in ("read_started_us", "read_finished_us")), "invalid immediate read time")
        # Record emission order does not establish native read order.
        following = [snapshot["started_us"] for snapshot in snapshots if snapshot["started_us"] >= call["returned_us"]]
        require(bool(following), "immediate target read has no collector observation")
        require(start <= call["returned_us"] <= reply["read_started_us"] <= reply["read_finished_us"] <= min(following) <= end,
                "immediate target read outside collector observation bounds")


def validate_setup(trial, controller, expected, checked_snapshot):
    case, records = trial["case"], trial["records"]
    require(trial.get("setup_variant") == case, "wrong explicit setup variant")
    require(trial.get("treatment_flags") == (531 if case in CASES[34:36] else 19), "wrong declared treatment flags")
    slots = ["A", "B", "P", "X"] + (["O"] if owned_by(case) else ["C"] if case.startswith("held_c_") else [])
    require(trial["fixture_slots"] == slots and trial["owned_by"] == owned_by(case), "wrong fixture contract")
    start = trial["recorded_us"]
    endings = [row for row in records if row["kind"] == "trial_end"]
    require(len(endings) == 1, "missing trial end")
    end = endings[0]["recorded_us"]
    require(type(start) is int and type(end) is int and 0 <= start <= end, "invalid trial envelope time")
    snapshots = {kind: [row["snapshot"] for row in records if row["kind"] == kind and "snapshot" in row] for kind in {row["kind"] for row in records}}

    def one(kind):
        require(len(snapshots.get(kind, [])) == 1, "missing or duplicated setup observation " + kind)
        return snapshots[kind][0]

    fixtures = [dict(zip(("handle", "process", "thread"), identity)) for identity in expected]

    def observation(kind):
        snapshot = one(kind)
        precreation = kind in ("owner_reset_before_creation", "owner_promoted_before_creation")
        try:
            checked_snapshot(snapshot, expected[:4] if precreation else expected,
                             ownership=None if precreation else owned_by(case))
            require(start <= snapshot["started_us"] <= snapshot["finished_us"] <= end, "setup observation outside trial")
        except (ValueError, KeyError, TypeError, IndexError) as error:
            raise ValueError(kind + ": " + str(error)) from error
        return snapshot

    before = snapshots.get("before", [])
    if before:
        observation("before")
    by_handle = {item["handle"]: item for item in fixtures}
    calls = native_calls(records)
    for call in calls:
        require(call["api"] == "SetWindowPos" and call["flags"] in (19, 531, 16403), "wrong native API or flags")
        require(call["ok"], "native call failure")
        require(all(type(call[key]) is int for key in ("started_us", "returned_us")), "invalid native call time")
        require(start <= call["started_us"] <= call["returned_us"] <= end, "native write outside trial bounds")
        require(all(call[key] == 0 for key in ("x", "y", "width", "height")), "geometry changed")
        require(call["window"] in by_handle, "write to nonfixture window")
        owner = by_handle[call["window"]]
        require((call["calling_process"], call["calling_thread"]) == (controller if call["flags"] == 16403 else (owner["process"], owner["thread"])), "wrong native caller")
        names = "NOMOVE|NOSIZE|NOACTIVATE" + ("|NOOWNERZORDER" if call["flags"] == 531 else "|ASYNCWINDOWPOS" if call["flags"] == 16403 else "")
        require(call["flags_names"] == names, "flag names disagree with bits")
    for left, right in zip(calls, calls[1:]):
        require(left["returned_us"] <= right["started_us"], "overlapping fixture writes")
    if before:
        require(len(before) == 1, "duplicate before")
        cutoff = before[0]["started_us"]
        setup_calls = [call for call in calls if call["started_us"] < cutoff]
    else:
        require(not any(row["kind"] in ("treatment_call", "external_async_call") for row in records), "treatment without before")
        setup_calls = calls
        cutoff = end
    steps = setup_steps(case)
    require(len(setup_calls) <= len(steps), "unscheduled native setup write")
    if before:
        require(len(setup_calls) == len(steps), "missing scheduled setup write")
    for index, (_, _, kind) in enumerate(steps):
        if snapshots.get(kind):
            require(index < len(setup_calls), "setup observation without scheduled write " + kind)
    previous = start
    for call, (index, after, kind) in zip(setup_calls, steps):
        following = observation(kind)
        reference = fixtures[slots.index(after)]["handle"] if isinstance(after, str) else after
        require((call["window"], call["after"], call["flags"]) == (fixtures[index]["handle"], reference, 19), "wrong setup call sequence")
        require(previous <= call["started_us"] <= call["returned_us"] <= following["started_us"] <= following["finished_us"] <= cutoff, "setup call outside observation bounds")
        previous = following["finished_us"]
    creations = [row for row in records if row["kind"] == "owned_identity"]
    require(len(creations) <= int(owned_by(case) is not None), "wrong number of O creations")
    if before:
        require(len(creations) == int(owned_by(case) is not None), "missing O creation")
    if creations:
        row = creations[0]
        creation, window = row["creation"], row["window"]
        owner = fixtures[0 if owned_by(case) == "A" else 3]
        require(row["owner_slot"] == owned_by(case), "wrong constructor owner slot")
        require(creation["api"] == "CreateWindowExW" and creation["owner"] == window["owner"] == owner["handle"], "wrong constructor owner")
        require((window["process"], window["thread"]) == (creation["calling_process"], creation["calling_thread"]) == (owner["process"], owner["thread"]), "O constructor not owner-local")
        require(creation["style"] == 0x90000000 and creation["extended_style"] == 0x08000080, "unexpected O construction styles or implicit explicit topmost")
        require(start <= creation["started_us"] <= creation["returned_us"] <= end, "creation outside trial")
        created = observation("after_owned_creation")
        require(creation["returned_us"] <= created["started_us"] <= created["finished_us"] <= cutoff, "creation read not bracketed")
        require(window["handle"] == created["fixtures"][4]["handle"], "O identity differs from created fixture")
        before_create_calls = 2 if case in CASES[29:32] else 1 if case in CASES[32:34] else 0
        require(len(setup_calls) >= before_create_calls, "missing precreation writes")
        for _, _, kind in steps[:before_create_calls]:
            require(observation(kind)["finished_us"] <= creation["started_us"], kind + " observation after creation")
        require(all(call["started_us"] >= created["finished_us"] for call in setup_calls[before_create_calls:]), "setup write before O creation read")
    validate_target_reads(records, expected, start, end)
    endpoints = snapshots.get("bottom_global_endpoint", []) or snapshots.get("settling", [])
    if endpoints:
        require(not any(call["returned_us"] > endpoints[-1]["finished_us"] for call in calls), "unscheduled native write after endpoint")
