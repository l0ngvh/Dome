import copy
import json
import pathlib
import re
import unittest

import analyze
import analyze_v6 as protocol
import protocol_v6 as audit
from test_analyze import SyntheticTrial
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
        if case == protocol.CASES[27]:
            self.order = [101, 102, 104, 103]
        self.setup_records()
        self.before = self.snapshot("before")
        if "async_a_" in case:
            self.transition()
        elif case.startswith("held_c_"):
            self.race()
        else:
            if case in (protocol.CASES[1], protocol.CASES[2], protocol.CASES[9], protocol.CASES[25], protocol.CASES[35]):
                promotion = case in (protocol.CASES[2], protocol.CASES[25], protocol.CASES[35])
                self.call("local_overlay_call", 104, -1 if promotion else -2)
                self.level(104, promotion)
                self.snapshot("after_explicit_topmost" if promotion else "after_explicit_notopmost")
            self.insert_overlay_or_source()
        self.settle("settling")
        if case in protocol.CASES[27:29]:
            self.snapshot("bottom_global_endpoint")
        self.records.append({"kind": "trial_end", "recorded_us": self.clock + 10000, "environment": environment, "case": case,
                             "repetition": 0, "status": "collected_pending_independent_analysis"})
        for record in self.records:
            if "snapshot" in record:
                record["snapshot"] = scoped_snapshot(record["snapshot"],
                    case in (protocol.CASES[5], protocol.CASES[7], protocol.CASES[20]) and record["kind"] in ("before", "before_insert")
                    or record["kind"] == "bottom_global_endpoint")

    def setup_records(self):
        steps = audit.setup_steps(self.case)
        creation_before = 2 if self.case in protocol.CASES[29:32] else 1 if self.case in protocol.CASES[32:34] else 0
        for index, (slot, after, kind) in enumerate(steps):
            if protocol.owned_by(self.case) and index == creation_before:
                self.creation_record()
            reference = {"A": 101, "B": 102, "P": 103, "X": 104}.get(after, after)
            target = self.handles[slot]
            record = self.call("local_overlay_call", target, reference)
            if slot in (1, 2) or slot == 4 and not protocol.owned_by(self.case):
                self.records.pop()
                self.records.append(fixture_reply(record["call"], self.window(target)))
            if kind in ("owner_reset_before_creation", "owner_promoted_before_creation"):
                old_handles, old_order, old_band = self.handles[:], self.order[:], self.bands[104]
                self.handles.remove(105)
                self.order.remove(105)
                self.bands[104] = kind == "owner_promoted_before_creation"
                self.snapshot(kind)
                self.handles, self.order, self.bands[104] = old_handles, old_order, old_band
            else:
                self.snapshot(kind)
        self.records.append({"kind": "setup_eligibility", "eligible": True, "reason": None})

    def creation_record(self):
        owner = 101 if protocol.owned_by(self.case) == "A" else 104
        self.records.append({"kind": "owned_identity", "owner_slot": protocol.owned_by(self.case), "window": self.window(105),
            "creation": {"api": "CreateWindowExW", "owner": owner, "style": 0x90000000, "extended_style": 0x08000080,
                         "calling_process": owner + 1000, "calling_thread": owner + 2000,
                         "started_us": self.advance(), "returned_us": self.clock + 1}})
        snapshot = self.snapshot("after_owned_creation")
        if self.case in protocol.CASES[32:34]:
            for item in snapshot["global_order"] + snapshot["fixtures"]:
                if item["handle"] in (104, 105):
                    item["topmost"] = False
                    item["extended_style"] &= ~8
            self.records[-2]["window"]["topmost"] = False
            self.records[-2]["window"]["extended_style"] &= ~8

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
        if self.case in protocol.CASES[34:36] and kind in ("treatment_call", "local_overlay_call") and self.records and any(row["kind"] == "before" for row in self.records):
            record["call"].update(flags=531, flags_names="NOMOVE|NOSIZE|NOACTIVATE|NOOWNERZORDER")
        return record

    def insert_overlay_or_source(self):
        special = self.case in protocol.CASES[21:25] or self.case in protocol.CASES[27:31] or self.case in protocol.CASES[32:35]
        alternative = self.case in (protocol.CASES[18], protocol.CASES[19], protocol.CASES[20], protocol.CASES[26])
        target = 105 if self.case in (protocol.CASES[23], protocol.CASES[24], protocol.CASES[30]) else 101 if alternative else 104
        reference = 104 if self.case in (protocol.CASES[18], protocol.CASES[26]) else 103 if alternative or self.case in (protocol.CASES[3], protocol.CASES[10]) else 101
        if special:
            reference = 1 if self.case in protocol.CASES[27:29] else -1 if self.case in (protocol.CASES[21], protocol.CASES[23], protocol.CASES[34]) else -2
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
        return {"kind": "trial_begin", "recorded_us": 999000, "setup_variant": self.case, "treatment_flags": 531 if self.case in protocol.CASES[34:36] else 19, "environment": self.environment, "case": self.case, "repetition": 0,
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
    return {"kind": "session", "schema_version": 6, "protocol": "bounded-topmost-v6", "scenario_count": 36, "environment_case_count": 49, "repetitions": 1, "case_names": protocol.CASES, "desktop": {"name": "ordinary"},
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
            records.extend(trial.records)
        records += [{"kind": "environment_cleanup", "environment": environment,
                     "desktop_closed": environment == "private", "borrowed_original_retained": environment == "ordinary"},
                    {"kind": "environment_end", "environment": environment, "ok": True}]
    return records + [{"kind": "collection_complete"}, {"kind": "cleanup_complete"}]


class ProtocolTests(unittest.TestCase):
    def test_fixed_matrix_and_native_schedule(self):
        source = pathlib.Path(__file__).with_name("src").joinpath("protocol.rs").read_text()
        block = source.split("const CASES:", 1)[1].split("];", 1)[0]
        self.assertEqual(re.findall(r'"([a-z_0-9]+)"', block), protocol.CASES)
        self.assertEqual(len(protocol.schedule(1)), 49)
        self.assertEqual(len(protocol.schedule(5)), 245)

    def test_parser_fixtures_for_all_keys_not_windows_predictions(self):
        for environment, case, _ in protocol.schedule(1):
            with self.subTest(environment=environment, case=case):
                result = Trial(case, environment).result()
                self.assertEqual(result["status"], "eligible_observation", result)
                self.assertTrue(result["instrumentation_valid"], result)
                self.assertTrue(result["question_observed"], result)
                self.assertTrue(all(row["valid"] for row in result["snapshot_validity"]), result)

    def assert_protocol_invalid(self, result, stage=None):
        self.assertEqual(result["status"], "protocol_invalid", result)
        self.assertFalse(result["instrumentation_valid"], result)
        self.assertFalse(result.get("question_observed", False), result)
        if stage:
            self.assertIn(stage, result["reason"])

    def test_precreation_identity_substitution_rejects_required_premise(self):
        for kind in ("owner_reset_before_creation", "owner_promoted_before_creation"):
            with self.subTest(kind=kind):
                trial = Trial(protocol.CASES[29])
                replace_handle(trial.record(kind)["snapshot"], 104, 204)
                result = trial.result()
                self.assert_protocol_invalid(result, kind)
                self.assertFalse(next(row for row in result["snapshot_validity"] if row["kind"] == kind)["valid"])

    def test_precreation_cannot_include_owned_fixture(self):
        trial = Trial(protocol.CASES[29])
        snapshot = copy.deepcopy(trial.record("after_owned_creation")["snapshot"])
        snapshot.update({key: trial.record("owner_promoted_before_creation")["snapshot"][key]
                         for key in ("started_us", "finished_us")})
        trial.record("owner_promoted_before_creation")["snapshot"] = snapshot
        self.assert_protocol_invalid(trial.result(), "owner_promoted_before_creation")

    def test_after_creation_identity_substitution_rejects_required_premise(self):
        trial = Trial(protocol.CASES[29])
        replace_handle(trial.record("after_owned_creation")["snapshot"], 104, 204)
        self.assert_protocol_invalid(trial.result(), "after_owned_creation")

    def test_after_creation_requires_all_five_identities(self):
        trial = Trial(protocol.CASES[29])
        snapshot = trial.record("after_owned_creation")["snapshot"]
        for key in ("fixtures", "fixture_rereads"):
            snapshot[key].pop()
        self.assert_protocol_invalid(trial.result(), "after_owned_creation")

    def test_invalid_reached_prerequisite_reports_actual_stop_stage(self):
        for index, kind in ((29, "owner_reset_before_creation"), (29, "owner_promoted_before_creation"),
                            (29, "after_owned_creation"), (32, "owner_reset_before_creation"),
                            (33, "after_owned_creation")):
            with self.subTest(index=index, kind=kind):
                trial = Trial(protocol.CASES[index])
                snapshot = trial.record(kind)["snapshot"]
                snapshot["fixture_rereads"][3]["topmost"] = not snapshot["fixture_rereads"][3]["topmost"]
                result = stopped_result(trial, kind)
                self.assert_protocol_invalid(result, kind)
                self.assertFalse(result["treatment_executed"])
                self.assertNotIn("missing", result["reason"])

    def test_invalid_native_prerequisite_bit_is_not_valid_band_failure(self):
        trial = Trial(protocol.CASES[29])
        snapshot = trial.record("owner_promoted_before_creation")["snapshot"]
        change_band(snapshot, 104, False)
        for key in ("fixtures", "fixture_rereads", "neighbors"):
            for item in snapshot[key]:
                if item["handle"] == 104:
                    item["valid"] = False
        self.assert_protocol_invalid(stopped_result(trial, "owner_promoted_before_creation"), "owner_promoted_before_creation")

    def test_native_invalid_reason_cannot_become_setup_not_run(self):
        trial = Trial(protocol.CASES[29])
        trial.record("owner_promoted_before_creation")["snapshot"]["invalid_reason"] = "invalid fixture state"
        self.assert_protocol_invalid(stopped_result(trial, "owner_promoted_before_creation"), "owner_promoted_before_creation")

    def test_missing_unreached_prerequisite_is_not_a_valid_failed_observation(self):
        trial = Trial(protocol.CASES[29])
        self.assert_protocol_invalid(stopped_result(trial, "owner_reset_before_creation"), "owner_promoted_before_creation")

    def test_creation_cannot_overlap_any_precreation_read(self):
        for index in range(29, 34):
            kinds = ("owner_reset_before_creation", "owner_promoted_before_creation") if index < 32 else ("owner_reset_before_creation",)
            for kind in kinds:
                with self.subTest(index=index, kind=kind):
                    trial = Trial(protocol.CASES[index])
                    reset = trial.record(kind)["snapshot"]
                    trial.record("owned_identity")["creation"].update(started_us=reset["started_us"], returned_us=reset["finished_us"])
                    self.assert_protocol_invalid(trial.result(), "observation after creation")

    def test_fixture_reply_target_read_fields_are_required(self):
        for field in ("post_return_target_read", "read_started_us", "read_finished_us"):
            with self.subTest(field=field):
                trial = Trial(protocol.CASES[29])
                del placed_reply(trial)[field]
                self.assert_protocol_invalid(trial.result(), field)

    def test_fixture_reply_target_read_cannot_finish_after_trial(self):
        trial = Trial(protocol.CASES[29])
        placed_reply(trial)["read_finished_us"] = trial.records[-1]["recorded_us"] + 100000000
        self.assert_protocol_invalid(trial.result(), "immediate target read")

    def test_fixture_reply_target_read_cannot_overlap_collector_snapshot(self):
        trial = Trial(protocol.CASES[29])
        placed_reply(trial)["read_finished_us"] = trial.record("setup_normal_reference")["snapshot"]["started_us"] + 1
        self.assert_protocol_invalid(trial.result(), "immediate target read")

    def test_fixture_reply_target_read_cannot_precede_native_return(self):
        trial = Trial(protocol.CASES[29])
        reply = placed_reply(trial)
        reply["read_started_us"] = reply["call"]["returned_us"] - 1
        self.assert_protocol_invalid(trial.result(), "immediate target read")

    def test_source_treatment_read_cannot_overlap_its_collector_snapshot(self):
        trial = Trial(protocol.CASES[18])
        treatment = trial.record("treatment_call")["call"]
        reply = next(row["reply"]["record"] for row in trial.records
                     if row["kind"] == "fixture_reply" and row["reply"]["record"].get("call") == treatment)
        reply["read_finished_us"] = trial.record("immediate_after_local_write")["snapshot"]["started_us"] + 1
        self.assert_protocol_invalid(trial.result(), "immediate target read")

    def test_source_treatment_read_cannot_be_omitted(self):
        trial = Trial(protocol.CASES[18])
        treatment = trial.record("treatment_call")["call"]
        reply = next(row["reply"]["record"] for row in trial.records
                     if row["kind"] == "fixture_reply" and row["reply"]["record"].get("call") == treatment)
        del reply["post_return_target_read"]
        self.assert_protocol_invalid(trial.result(), "post_return_target_read")

    def test_fixture_read_validation_runs_before_valid_failed_setup(self):
        trial = Trial(protocol.CASES[29])
        del placed_reply(trial)["post_return_target_read"]
        change_band(trial.before, 104, False)
        self.assert_protocol_invalid(stopped_result(trial, "before"), "post_return_target_read")

    def test_async_post_read_fields_and_collector_bounds_are_required(self):
        for mutation in ("post_return_target_read", "read_started_us", "read_finished_us", "overlap", "after_trial"):
            with self.subTest(mutation=mutation):
                trial = Trial(protocol.CASES[15])
                reply = trial.record("external_async_call")
                if mutation == "overlap":
                    reply["read_finished_us"] = trial.record("after_async_post_while_held")["snapshot"]["started_us"] + 1
                elif mutation == "after_trial":
                    reply["read_finished_us"] = trial.records[-1]["recorded_us"] + 1
                else:
                    del reply[mutation]
                self.assert_protocol_invalid(trial.result())

    def test_fixture_read_association_uses_native_time_not_emission_order(self):
        trial = Trial(protocol.CASES[29])
        reply = next(row for row in trial.records if row["kind"] == "fixture_reply")
        trial.records.remove(reply)
        snapshot = trial.record("setup_normal_reference")
        reply["recorded_us"] = snapshot["snapshot"]["finished_us"] + 1
        trial.records.insert(trial.records.index(snapshot) + 1, reply)
        self.assertEqual(trial.result()["status"], "eligible_observation")

    def test_every_setup_write_is_permitted_and_bounded(self):
        for case in protocol.CASES:
            for mutation in ("extra", "wrong_target", "late", "wrong_flags"):
                with self.subTest(case=case, mutation=mutation):
                    trial = Trial(case)
                    call = audit.native_calls(trial.records)[0]
                    if mutation == "extra":
                        extra_write(trial, 104, call["started_us"] + 20)
                    elif mutation == "wrong_target":
                        call["window"] = 101 if call["window"] != 101 else 104
                    elif mutation == "late":
                        call["returned_us"] = trial.before["finished_us"] + 1
                    else:
                        call["flags"] = 531
                    self.assertEqual(trial.result()["status"], "protocol_invalid")

    def test_creation_styles_owner_and_inheritance_are_independent(self):
        for field, value in (("owner", 101), ("extended_style", 0x08000088), ("calling_thread", 99)):
            trial = Trial(protocol.CASES[29])
            trial.record("owned_identity")["creation"][field] = value
            self.assertEqual(trial.result()["status"], "protocol_invalid")
        trial = Trial(protocol.CASES[29])
        snapshot = trial.record("after_owned_creation")["snapshot"]
        change_band(snapshot, 104, False)
        result = trial.result()
        self.assertEqual(result["status"], "setup_not_run" if not result["treatment_executed"] else "protocol_invalid")

    def test_rejected_creation_and_precreation_stages_do_not_claim_treatment(self):
        for kind, handle, topmost in (("owner_reset_before_creation", 104, True), ("owner_promoted_before_creation", 104, False), ("after_owned_creation", 104, False)):
            trial = Trial(protocol.CASES[29])
            identities = trial.identities()
            record = trial.record(kind)
            change_band(record["snapshot"], handle, topmost)
            end = trial.records[-1]
            end.update(status="ineligible_or_failed", error="native prerequisite not established")
            trial.records = trial.records[:trial.records.index(record) + 1] + [end]
            result = protocol.analyze_trial(dict(trial.begin(), records=trial.records), (1104, 2104), identities)
            self.assertEqual(result["status"], "setup_not_run", result)
            self.assertTrue(result["instrumentation_valid"], result)
            self.assertFalse(result["treatment_executed"])
            self.assertIn(kind, result["reason"])
        trial = Trial(protocol.CASES[33])
        record = trial.record("after_owned_creation")
        change_band(record["snapshot"], 105, True)
        identities = trial.identities()
        end = trial.records[-1]
        end.update(status="ineligible_or_failed", error="normal pair prerequisite not established")
        trial.records = trial.records[:trial.records.index(record) + 1] + [end]
        result = protocol.analyze_trial(dict(trial.begin(), records=trial.records), (1104, 2104), identities)
        self.assertEqual(result["status"], "setup_not_run", result)
        self.assertTrue(result["instrumentation_valid"], result)
        self.assertIn("after_owned_creation", result["reason"])

    def test_bad_both_topmost_setup_has_no_treatment(self):
        for case in protocol.CASES[29:34]:
            trial = Trial(case)
            before = trial.record("before")
            change_band(before["snapshot"], 104, False)
            cutoff = trial.records.index(before) + 1
            end = trial.records[-1]
            end.update(status="ineligible_or_failed", error="actual initial bands differ")
            trial.records = trial.records[:cutoff] + [end]
            result = trial.result()
            self.assertEqual(result["status"], "setup_not_run", result)
            self.assertTrue(result["instrumentation_valid"], result)
            self.assertFalse(result["treatment_executed"])

    def test_displacement_and_movement_witnesses(self):
        for index in (7, 27):
            trial = Trial(protocol.CASES[index])
            for kind in ("before", "before_insert"):
                snapshot = trial.record(kind)["snapshot"]
                endpoint = trial.record("settling")["snapshot"]
                for key in ("fixtures", "fixture_rereads", "neighbors", "raw_enumeration"):
                    if key in snapshot and key in endpoint:
                        snapshot[key] = copy.deepcopy(endpoint[key])
                if snapshot["scope"] == "global":
                    snapshot["raw_enumeration"] = copy.deepcopy(endpoint["fixtures"])
            self.assertNotEqual(trial.result()["status"], "eligible_observation")

    def test_fixed_plateau_does_not_discard_earlier_invalid_samples(self):
        trial = Trial(protocol.CASES[15])
        trial.record("immediate_after_release")["snapshot"]["fixture_rereads"][0]["topmost"] = False
        trial.record("settling")["snapshot"]["fixture_rereads"][0]["topmost"] = False
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertTrue(any(not row["valid"] for row in result["snapshot_validity"]))
        trial.records = [row for row in trial.records if not (row["kind"] == "settling" and row["sample"] == 20)]
        self.assertEqual(trial.result()["status"], "protocol_invalid")

    def test_failed_prefix_keeps_measured_bands_without_claiming_intended_case(self):
        for index in (25, 35):
            trial = Trial(protocol.CASES[index])
            active = False
            for row in trial.records:
                if row["kind"] == "after_explicit_topmost":
                    active = True
                if active and "snapshot" in row:
                    change_band(row["snapshot"], 104, False)
            result = trial.result()
            self.assertEqual(result["status"], "measured_operation_failed_prefix", result)
            self.assertTrue(result["endpoint_resolved"])
            self.assertFalse(result["question_observed"])
            self.assertFalse(result["settled"]["X_topmost"])

    def test_streaming_schedule_environments_repetitions_and_counters(self):
        records = run_records()
        report = analyze.analyze_text(encoded(records))
        self.assertEqual(report["malformed_or_protocol_errors"], [])
        self.assertTrue(report["all_trials_eligible_and_controls_passed"], [(row["case"], row.get("reason")) for row in report["trials"] if row["status"] != "eligible_observation"])
        self.assertEqual(report["counts"]["questions_observed"], 49)
        duplicated = copy.deepcopy(records)
        begin = next(row for row in duplicated if row["kind"] == "trial_begin")
        begin["repetition"] = 1
        bad = analyze.analyze_text(encoded(duplicated))
        self.assertFalse(bad["all_trials_eligible_and_controls_passed"])
        self.assertTrue(bad["malformed_or_protocol_errors"])
        expanded = copy.deepcopy(records)
        expanded[0]["repetitions"] = 2
        expanded[0]["schedule"] = [dict(environment=environment, case=case, repetition=repetition) for environment, case, repetition in protocol.schedule(2)]
        for environment in ("private", "ordinary"):
            begin = next(index for index, row in enumerate(expanded) if row["kind"] == "trial_begin" and row["environment"] == environment)
            end = next(index for index in range(begin, len(expanded)) if expanded[index]["kind"] == "environment_cleanup")
            second = copy.deepcopy(expanded[begin:end])
            for row in second:
                if row["kind"] in ("trial_begin", "trial_end"):
                    row["repetition"] = 1
            expanded[end:end] = second
        report = analyze.analyze_text(encoded(expanded))
        self.assertEqual(report["expected_trials"], 98)
        self.assertTrue(report["all_trials_eligible_and_controls_passed"], report["malformed_or_protocol_errors"])

    def test_reference_contradiction_is_not_instrumentation_failure(self):
        trial = Trial(protocol.CASES[21])
        active = False
        for row in trial.records:
            if row["kind"] == "immediate_after_local_write":
                active = True
            if active and "snapshot" in row:
                change_band(row["snapshot"], 104, False)
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertTrue(result["instrumentation_valid"])
        self.assertTrue(result["reference_expectation_contradicted"])
        self.assertFalse(result["control_passed"])

    def test_extra_measurement_write_never_gets_attributed_to_after_a(self):
        for case in protocol.CASES:
            trial = Trial(case)
            extra_write(trial, 104, trial.record("settling")["snapshot"]["finished_us"] + 1)
            self.assertEqual(trial.result()["status"], "protocol_invalid")


def placed_reply(trial):
    return next(row["reply"]["record"] for row in trial.records
                if row["kind"] == "fixture_reply" and row["reply"]["record"].get("kind") == "placed")


def stopped_result(trial, kind):
    identities = trial.identities()
    end = trial.records[-1]
    end.update(status="ineligible_or_failed", error="owner promotion before creation unverified")
    trial.records = trial.records[:trial.records.index(trial.record(kind)) + 1] + [end]
    return protocol.analyze_trial(dict(trial.begin(), records=trial.records), (1104, 2104), identities)


def replace_handle(snapshot, old, new):
    for key in ("fixtures", "fixture_rereads", "neighbors"):
        for item in snapshot.get(key, []):
            for field in ("handle", "above", "below", "owner"):
                if item.get(field) == old:
                    item[field] = new


def encoded(records):
    return "\n".join(json.dumps({"unix_us": row.get("recorded_us", 1), "record": row}) for row in records)


def change_band(snapshot, handle, topmost):
    for key in ("fixtures", "fixture_rereads", "raw_enumeration"):
        for item in snapshot.get(key, []):
            if item["handle"] == handle:
                item["topmost"] = topmost
                item["extended_style"] = item["extended_style"] & ~8 | (8 if topmost else 0)


if __name__ == "__main__":
    unittest.main()
