import copy
import unittest

import analyze
import analyze_v5 as protocol
from test_analyze import SyntheticTrial, encoded
from verify_output_reduction import scoped_snapshot


class Trial(SyntheticTrial):
    def __init__(self, case, environment="private"):
        self.case, self.environment = case, environment
        self.clock, self.records = 1000000, []
        self.bands = dict(zip(range(101, 106), protocol.initial_bands(case)))
        self.handles = list(self.bands)
        self.order = [handle for handle in self.handles if self.bands[handle]] + [handle for handle in self.handles if not self.bands[handle]]
        if case == protocol.CASES[7]:
            self.order = [102, 101, 103, 104]
        if case.startswith("held_c_"):
            self.order = [102, 101, 104, 103, 105]
        if protocol.owned_by(case):
            self.order.remove(105)
            self.order.insert(self.order.index(101 if protocol.owned_by(case) == "A" else 104), 105)
        self.before = self.snapshot("before")
        if "async_a_" in case:
            self.transition()
        elif case.startswith("held_c_"):
            self.race()
        else:
            if case in (protocol.CASES[1], protocol.CASES[2], protocol.CASES[9], protocol.CASES[25]):
                promotion = case in (protocol.CASES[2], protocol.CASES[25])
                self.call("local_overlay_call", 104, -1 if promotion else -2)
                self.level(104, promotion)
                self.snapshot("after_explicit_topmost" if promotion else "after_explicit_notopmost")
            self.insert_overlay_or_source()
        self.settle("settling")
        if case in protocol.CASES[27:]:
            self.snapshot("bottom_global_endpoint")
        self.records.append({"kind": "trial_end", "environment": environment, "case": case,
                             "repetition": 0, "status": "collected_pending_independent_analysis"})
        for record in self.records:
            if "snapshot" in record:
                record["snapshot"] = scoped_snapshot(record["snapshot"],
                    case in (protocol.CASES[5], protocol.CASES[7], protocol.CASES[20]) and record["kind"] in ("before", "before_insert")
                    or record["kind"] == "bottom_global_endpoint")

    def window(self, handle):
        item = super().window(handle)
        if handle == 105 and protocol.owned_by(self.case):
            owner = 101 if protocol.owned_by(self.case) == "A" else 104
            item.update(owner=owner, process=owner + 1000, thread=owner + 2000)
        return item

    def level(self, handle, topmost):
        super().level(handle, topmost)
        if protocol.owned_by(self.case):
            owner = 101 if protocol.owned_by(self.case) == "A" else 104
            if handle == owner:
                super().level(105, topmost)
            elif handle == 105 and not topmost:
                super().level(owner, False)
                self.order.remove(105)
                self.order.insert(self.order.index(owner), 105)

    def call(self, kind, window, after, asynchronous=False):
        record = super().call(kind, window, after, asynchronous)
        record["call"].update(flags_names="NOMOVE|NOSIZE|NOACTIVATE" + ("|ASYNCWINDOWPOS" if asynchronous else ""),
                              last_error_on_failure=None, error=None)
        if asynchronous:
            record["read_started_us"] = record["call"]["returned_us"]
        if window == 101 and not asynchronous:
            self.records.pop()
            self.records.append(fixture_reply(record["call"], self.window(window)))
            if kind == "treatment_call":
                self.records.append(record)
        if window == 105 and protocol.owned_by(self.case):
            owner = 101 if protocol.owned_by(self.case) == "A" else 104
            record["call"].update(calling_process=owner + 1000, calling_thread=owner + 2000)
        return record

    def insert_overlay_or_source(self):
        special = self.case in protocol.CASES[21:25] or self.case in protocol.CASES[27:]
        alternative = self.case in (protocol.CASES[18], protocol.CASES[19], protocol.CASES[20], protocol.CASES[26])
        target = 105 if self.case in (protocol.CASES[23], protocol.CASES[24]) else 101 if alternative else 104
        reference = 104 if self.case in (protocol.CASES[18], protocol.CASES[26]) else 103 if alternative or self.case in (protocol.CASES[3], protocol.CASES[10]) else 101
        if special:
            reference = 1 if self.case in protocol.CASES[27:] else -1 if self.case in (protocol.CASES[21], protocol.CASES[23]) else -2
        self.snapshot("before_insert")
        self.call("treatment_call", target, reference)
        if reference in (-1, -2):
            self.level(target, reference == -1)
        elif reference == 1:
            self.level(target, False)
            self.order.remove(target)
            self.order.append(target)
        else:
            self.insert(target, reference, self.bands[reference])
            if protocol.owned_by(self.case):
                owner = 101 if protocol.owned_by(self.case) == "A" else 104
                self.bands[105] = self.bands[owner]
                self.order.remove(105)
                self.order.insert(self.order.index(owner), 105)
        self.snapshot("immediate_after_local_write")
        if self.case == protocol.CASES[11]:
            self.settle("first_settling")
            self.snapshot("before_repeated_insert")
            self.call("treatment_call", target, reference)
            self.snapshot("immediate_after_repeated_write")

    def begin(self):
        return {"kind": "trial_begin", "environment": self.environment, "case": self.case, "repetition": 0,
                "owned_by": protocol.owned_by(self.case), "fixture_slots": ["A", "B", "P", "X"] +
                (["C"] if self.case.startswith("held_c_") else ["O"] if protocol.owned_by(self.case) else [])}

    def identities(self):
        labels = {101: "A", 102: "B", 103: "normalA", 104: "X", 105: "O" if protocol.owned_by(self.case) else "C-synthetic"}
        return {(item["handle"], item["process"], item["thread"]): labels[item["handle"]]
                for item in self.record("before")["snapshot"]["fixtures"]}

    def result(self):
        return protocol.analyze_trial(dict(self.begin(), records=self.records), (1104, 2104), self.identities())


def fixture_reply(call, read):
    return {"kind": "fixture_reply", "process": call["calling_process"],
            "reply": {"unix_us": call["returned_us"] + 3, "record": {
                "kind": "placed", "call": copy.deepcopy(call), "post_return_target_read": copy.deepcopy(read),
                "read_started_us": call["returned_us"] + 1, "read_finished_us": call["returned_us"] + 2}}}


def extra_write(trial, window, started, returned=None, after=-1, as_reply=False):
    call = copy.deepcopy(next(row["call"] for row in trial.records if "call" in row))
    owner = next(item for item in trial.before["fixtures"] if item["handle"] == window)
    call.update(window=window, after=after, flags=19, flags_names="NOMOVE|NOSIZE|NOACTIVATE",
                calling_process=owner["process"], calling_thread=owner["thread"],
                started_us=started, returned_us=started + 1 if returned is None else returned)
    record = fixture_reply(call, owner) if as_reply else {"kind": "local_overlay_call", "call": call}
    trial.records.insert(-1, record)
    return call


def session():
    return {"kind": "session", "schema_version": 5, "repetitions": 1, "case_names": protocol.CASES, "desktop": {"name": "ordinary"},
            "schedule": [{"environment": environment, "case": case, "repetition": repetition}
                         for environment, case, repetition in protocol.schedule(1)]}


def input_method(process, thread):
    return {"ok": True, "argument": 0, "before_first_window": True, "process": process, "thread": thread}


def desktop(name, process=1104, thread=2104):
    return {"verified": True, "requested": name, "actual": {"name": name}, "switch_requested": False,
            "requested_open_access": 199, "process": process, "thread": thread}


def run_records():
    records = [session()]
    for environment in ("ordinary", "private"):
        records.append({"kind": "environment_ready", "environment": environment, "process": 1104, "thread": 2104,
                        "desktop": desktop(environment), "input_method": input_method(1104, 2104)})
        for handle, label in ((101, "A"), (102, "B"), (103, "normalA"), (105, "C-synthetic")):
            records += [{"kind": "spawn", "process": handle + 1000, "label": label},
                        {"kind": "fixture_reply", "process": handle + 1000, "reply": {"record": {
                            "kind": "ready", "handle": handle, "process": handle + 1000, "thread": handle + 2000,
                            "desktop": desktop(environment, handle + 1000, handle + 2000), "input_method": input_method(handle + 1000, handle + 2000)}}}]
        records.append({"kind": "overlay_identity", "handle": 104, "process": 1104, "thread": 2104})
        for case in protocol.scheduled_cases(environment):
            trial = Trial(case, environment)
            records.append(trial.begin())
            if protocol.owned_by(case):
                records.append({"kind": "owned_identity", "owner_slot": protocol.owned_by(case),
                                "window": trial.record("before")["snapshot"]["fixtures"][4]})
            records.extend(trial.records)
        records += [{"kind": "environment_cleanup", "environment": environment,
                     "desktop_closed": environment == "private", "borrowed_original_retained": environment == "ordinary"},
                    {"kind": "environment_end", "environment": environment, "ok": True}]
    return records + [{"kind": "collection_complete"}, {"kind": "cleanup_complete"}]


class ProtocolTests(unittest.TestCase):
    def test_frozen_schema5_matrix(self):
        self.assertEqual(protocol.CASES[:7], list(analyze.CASES[:7]))
        self.assertEqual(len(protocol.CASES), 29)
        self.assertEqual(len(protocol.schedule(1)), 33)
        self.assertEqual(len(protocol.schedule(5)), 165)

    def test_all_synthetic_scenarios_without_native_claim(self):
        for case in protocol.CASES:
            with self.subTest(case=case):
                result = Trial(case).result()
                self.assertEqual(result["status"], "eligible_observation", result)
                self.assertTrue(result.get("control_passed", True), result)

    def test_streaming_full_schedule(self):
        report = analyze.analyze_text(encoded(run_records()))
        self.assertTrue(report["all_trials_eligible_and_controls_passed"], report)

    def test_intervening_native_writes_cannot_preserve_false_green_stream(self):
        for as_reply, window, after in ((False, 104, -1), (True, 101, 102)):
            with self.subTest(as_reply=as_reply):
                records = run_records()
                begin = next(index for index, row in enumerate(records) if row["kind"] == "trial_begin")
                end = next(index for index in range(begin, len(records)) if records[index]["kind"] == "trial_end")
                trial = Trial(protocol.CASES[0], "ordinary")
                extra_write(trial, window, trial.record("before_insert")["snapshot"]["finished_us"] + 1,
                            after=after, as_reply=as_reply)
                records[begin + 1:end] = trial.records[:-1]
                report = analyze.analyze_text(encoded(records))
                self.assertFalse(report["all_trials_eligible_and_controls_passed"])
                self.assertEqual(report["trials"][0]["reason"], "unscheduled native write in measurement interval")

    def test_every_fixture_is_protected_through_final_plateau(self):
        for environment, case, _ in protocol.schedule(1):
            baseline = Trial(case, environment)
            stages = ["before", "immediate_after_release" if case.startswith("held_c_") else "immediate_after_local_write", "settling"]
            if "async_a_" in case or case.startswith("held_c_"):
                stages.append("after_async_post_while_held")
            for window in baseline.handles:
                for stage in stages:
                    with self.subTest(environment=environment, case=case, window=window, stage=stage):
                        trial = copy.deepcopy(baseline)
                        rows = [row for row in trial.records if row["kind"] == stage]
                        started = rows[-1]["snapshot"]["finished_us"] if stage == "settling" else rows[0]["snapshot"]["finished_us"] + 1
                        extra_write(trial, window, started, returned=started, as_reply=window != 104)
                        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_only_identical_fixture_and_treatment_representations_are_deduplicated(self):
        for case in (protocol.CASES[18], protocol.CASES[19], protocol.CASES[20], protocol.CASES[26]):
            trial = Trial(case)
            reply = next(row for row in trial.records if row["kind"] == "fixture_reply")
            treatment = trial.record("treatment_call")
            self.assertEqual(reply["reply"]["record"]["call"], treatment["call"])
            self.assertEqual(trial.result()["status"], "eligible_observation")
            for field, value in (("after", 102), ("returned_us", treatment["call"]["returned_us"] + 1),
                                 ("flags_names", "conflicting"), ("error", "conflicting")):
                with self.subTest(case=case, field=field):
                    changed = copy.deepcopy(trial)
                    duplicate = next(row for row in changed.records if row["kind"] == "fixture_reply")
                    duplicate["reply"]["record"]["call"][field] = value
                    self.assertEqual(changed.result()["reason"], "conflicting duplicate native call")
            for change in ("missing", "moved_before_interval"):
                changed = copy.deepcopy(trial)
                duplicate = next(row for row in changed.records if row["kind"] == "fixture_reply")
                if change == "missing":
                    changed.records.remove(duplicate)
                else:
                    call = duplicate["reply"]["record"]["call"]
                    call.update(started_us=changed.before["started_us"] - 2, returned_us=changed.before["started_us"] - 1)
                self.assertEqual(changed.result()["reason"], "fixture treatment differs from native reply")
            trial.records.insert(-1, copy.deepcopy(reply))
            self.assertEqual(trial.result()["reason"], "unexpected duplicate native call")

        trial = Trial(protocol.CASES[0])
        trial.records.insert(-1, fixture_reply(trial.record("treatment_call")["call"], trial.before["fixtures"][3]))
        self.assertEqual(trial.result()["reason"], "unexpected duplicate native call")

    def test_native_execution_order_not_emission_order_defines_sequence(self):
        for environment, case, _ in protocol.schedule(1):
            with self.subTest(case=case, environment=environment):
                trial = Trial(case, environment)
                calls = [row for row in trial.records if "call" in row or
                         row["kind"] == "fixture_reply" and "call" in row["reply"]["record"]]
                trial.records = [row for row in trial.records if row not in calls]
                trial.records[-1:-1] = reversed(calls)
                self.assertEqual(trial.result()["status"], "eligible_observation")

    def test_prefix_and_race_calls_must_match_the_exact_schedule(self):
        for case in (protocol.CASES[1], protocol.CASES[2], protocol.CASES[9], protocol.CASES[15], protocol.CASES[17], protocol.CASES[25]):
            for change in ("missing", "extra", "late"):
                with self.subTest(case=case, change=change):
                    trial = Trial(case)
                    record = next(row for row in trial.records if row["kind"] == "local_overlay_call" or
                                  row["kind"] == "fixture_reply" and "call" in row["reply"]["record"])
                    call = record.get("call") or record["reply"]["record"]["call"]
                    if change == "missing":
                        trial.records.remove(record)
                    elif change == "extra":
                        extra_write(trial, call["window"], call["returned_us"] + 10, after=call["after"],
                                    as_reply=record["kind"] == "fixture_reply")
                    else:
                        call["started_us"] = call["returned_us"] = trial.record("settling")["snapshot"]["started_us"]
                    self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_interval_boundaries_use_exact_native_timestamps(self):
        baseline = Trial(protocol.CASES[0])
        start = baseline.before["finished_us"]
        end = [row for row in baseline.records if row["kind"] == "settling"][-1]["snapshot"]["finished_us"]
        for started, returned, accepted in ((start - 2, start - 1, True), (start - 1, start, False),
                                            (start, start, False), (end, end, False),
                                            (end - 1, end + 1, False), (end + 1, end + 2, True)):
            with self.subTest(started=started, returned=returned):
                trial = copy.deepcopy(baseline)
                extra_write(trial, 104, started, returned)
                self.assertEqual(trial.result()["status"] == "eligible_observation", accepted)
        for value in (-1, True, 1.5, float("nan"), "100"):
            for field in ("started_us", "returned_us"):
                trial = copy.deepcopy(baseline)
                trial.record("treatment_call")["call"][field] = value
                self.assertEqual(trial.result()["reason"], "invalid native call time")
        for value in (True, 1.5, float("nan")):
            trial = copy.deepcopy(baseline)
            trial.record("immediate_after_local_write")["snapshot"]["started_us"] = value
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_treatment_bounds_and_bottom_global_endpoint_are_protected(self):
        for field, stage, bound in (("started_us", "before", "finished_us"),
                                    ("returned_us", "immediate_after_local_write", "finished_us")):
            trial = Trial(protocol.CASES[0])
            trial.record("treatment_call")["call"][field] = trial.record(stage)["snapshot"][bound]
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")
        for case in protocol.CASES[27:]:
            trial = Trial(case)
            endpoint = trial.record("bottom_global_endpoint")["snapshot"]
            extra_write(trial, 102, endpoint["finished_us"], returned=endpoint["finished_us"], as_reply=True)
            self.assertEqual(trial.result()["reason"], "unscheduled native write in measurement interval")

    def test_failed_native_write_remains_a_failure(self):
        trial = Trial(protocol.CASES[0])
        call = extra_write(trial, 104, trial.record("before_insert")["snapshot"]["finished_us"] + 1)
        call.update(ok=False, last_error_on_failure=5, error="access denied")
        self.assertEqual(trial.result()["reason"], "native call failure")

    def test_missing_cleanup_or_private_default_reuse_prevents_complete_verdict(self):
        records = run_records()
        records = [row for row in records if row["kind"] != "environment_cleanup"]
        self.assertFalse(analyze.analyze_text(encoded(records))["all_trials_eligible_and_controls_passed"])
        records = run_records()
        private = next(row for row in records if row["kind"] == "environment_ready" and row["environment"] == "private")
        private["desktop"] = desktop("ordinary")
        report = analyze.analyze_text(encoded(records))
        self.assertTrue(report["malformed_or_protocol_errors"])
        self.assertFalse(report["all_trials_eligible_and_controls_passed"])

    def test_private_preflight_failure_reports_every_affected_key(self):
        records = [session()]
        for environment, case, repetition in protocol.schedule(1):
            records.append({"kind": "trial_not_run", "environment": environment, "case": case,
                            "repetition": repetition, "reason": "desktop preflight failed"})
        records += [{"kind": "collection_complete"}, {"kind": "cleanup_complete"}]
        report = analyze.analyze_text(encoded(records))
        self.assertEqual(len(report["trials"]), 33)
        self.assertTrue(report["matrix_complete"])
        self.assertFalse(report["all_trials_eligible_and_controls_passed"])
        self.assertEqual(sum(row["environment"] == "private" for row in report["trials"]), 15)

    def test_missing_and_duplicate_schedule_fail_closed(self):
        records = run_records()
        report = analyze.analyze_text(encoded(records[:10]))
        self.assertTrue(report["missing_schedule"])
        self.assertFalse(report["matrix_complete"])
        records.insert(-2, copy.deepcopy(next(row for row in records if row["kind"] == "trial_begin")))
        report = analyze.analyze_text(encoded(records))
        self.assertTrue(report["malformed_or_protocol_errors"])
        self.assertFalse(report["all_trials_eligible_and_controls_passed"])

    def test_invalid_immediate_and_early_sample_keep_stable_endpoint(self):
        trial = Trial(protocol.CASES[0])
        trial.record("immediate_after_local_write")["snapshot"]["fixture_rereads"][3]["topmost"] = False
        trial.record("settling")["snapshot"]["fixture_rereads"][3]["topmost"] = False
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertIsNone(result["immediate"])
        self.assertEqual(sum(not row["valid"] for row in result["snapshot_validity"]), 2)

    def test_final_five_cannot_skip_invalid_slot_or_extend_window(self):
        for change in ("invalid", "extra", "missing"):
            trial = Trial(protocol.CASES[0])
            rows = [row for row in trial.records if row["kind"] == "settling"]
            if change == "invalid":
                rows[-3]["snapshot"]["fixture_rereads"][3]["topmost"] = False
            elif change == "extra":
                trial.records.insert(-1, copy.deepcopy(rows[-1]))
                trial.records[-2]["sample"] = 21
            else:
                trial.records.remove(rows[10])
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_owned_fixture_does_not_relax_baseline(self):
        owned = Trial(protocol.CASES[21]).record("before")["snapshot"]
        protocol.checked_snapshot(owned, ownership="X")
        with self.assertRaises(ValueError):
            protocol.checked_snapshot(owned)
        with self.assertRaises(ValueError):
            protocol.checked_snapshot(owned, ownership="A")

    def test_clean_boundary_rejects_normal_above_flagged_prefix(self):
        trial = Trial(protocol.CASES[7])
        snapshot = trial.record("before")["snapshot"]
        raw = snapshot["raw_enumeration"]
        normal = copy.deepcopy(raw[0])
        normal.update(handle=999, topmost=False, extended_style=0, above=0, below=raw[0]["handle"])
        raw[0]["above"] = 999
        snapshot["fixtures"][1]["above"] = 999
        raw.insert(0, normal)
        self.assertFalse(protocol.clean_prefix(snapshot, 101))
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_child_wrong_desktop_or_failed_input_method_prevents_complete_verdict(self):
        for field in ("desktop", "input_method"):
            records = run_records()
            ready = next(row["reply"]["record"] for row in records if row["kind"] == "fixture_reply" and row["reply"]["record"].get("kind") == "ready")
            if field == "desktop":
                ready["desktop"]["actual"]["name"] = "Default"
            else:
                ready["input_method"]["ok"] = False
            report = analyze.analyze_text(encoded(records))
            self.assertTrue(report["malformed_or_protocol_errors"])
            self.assertFalse(report["all_trials_eligible_and_controls_passed"])

    def test_race_with_nonatomic_immediate_retains_later_crossing(self):
        trial = Trial(protocol.CASES[15])
        trial.record("immediate_after_release")["snapshot"]["fixture_rereads"][4]["topmost"] = False
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertIsNone(result["immediate"])
        self.assertTrue(result["C_crossed_stationary_P"])

    def test_owned_final_plateau_cannot_ignore_owned_neighbors(self):
        trial = Trial(protocol.CASES[21])
        row = [row for row in trial.records if row["kind"] == "settling"][-2]
        row["snapshot"]["fixture_rereads"][4]["owner"] = 101
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_c_crossing_before_release_is_rejected(self):
        trial = Trial(protocol.CASES[16])
        before = trial.record("before_c_release")["snapshot"]
        destination = copy.deepcopy(trial.record("immediate_after_release")["snapshot"])
        destination.update(started_us=before["started_us"], finished_us=before["finished_us"])
        trial.record("before_c_release")["snapshot"] = destination
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_bottom_local_tail_does_not_replace_complete_chain_proof(self):
        trial = Trial(protocol.CASES[28])
        endpoint = trial.record("bottom_global_endpoint")["snapshot"]
        raw = endpoint["raw_enumeration"]
        tail = next(row for row in raw if row["below"] == 0)
        hidden = copy.deepcopy(tail)
        hidden.update(handle=999, visible=False, above=tail["handle"], below=0)
        tail["below"] = 999
        endpoint["fixtures"][3]["below"] = 999
        raw.append(hidden)
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertFalse(result["control_passed"])
        self.assertFalse(result["complete_chain_tail"])

    def test_c_cannot_pass_with_destination_before_release_or_stationary_endpoint(self):
        for case in protocol.CASES[15:18]:
            trial = Trial(case)
            pre_release = trial.record("before_c_release")["snapshot"]
            for row in trial.records:
                if row["kind"] in ("immediate_after_release", "settling"):
                    start, end = row["snapshot"]["started_us"], row["snapshot"]["finished_us"]
                    row["snapshot"] = copy.deepcopy(pre_release)
                    row["snapshot"].update(started_us=start, finished_us=end)
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")


if __name__ == "__main__":
    unittest.main()
