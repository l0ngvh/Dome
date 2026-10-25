import copy
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest

import analyze
import analyze_v7 as audit
from protocol_v7 import CASES, ROLES, MANIFEST, schedule
from test_analyze import SyntheticTrial
from test_analyze_v6 import fixture_reply
from verify_output_reduction import scoped_snapshot


class FreshTrial(SyntheticTrial):
    def __init__(self, member, start=1000000):
        self.member = member
        self.case = member["case"]
        self.clock, self.records = start, []
        self.handles, self.order, self.bands = [], [], {}
        self.desktop = "Default" if member["environment"] == "ordinary" else "Private"
        self.add(dict(member, kind="trial_begin", fixture_slots=ROLES, owned_by="X"))
        self.add(dict(self.ready(1104, 2104), kind="trial_thread_ready"))
        for role, handle in zip(ROLES[:4], range(101, 105)):
            if role != "X":
                self.add(dict(kind="spawn", label=role, process=handle + 1000))
                readiness = self.ready(handle + 1000, handle + 2000)
            creation = self.create(handle)
            if role == "X":
                self.acquisition(role, creation)
                self.add(dict(kind="creation", role=role, creation=creation))
            else:
                readiness.update(kind="ready", handle=handle, creation=creation, initial=self.window(handle))
                self.add(dict(kind="fixture_reply", process=handle + 1000, reply=dict(unix_us=self.advance(), record=readiness)))
        self.snapshot("fresh_inventory")
        topmost = member["family"] != "owner_promotion"
        if topmost:
            self.placement(104, -1, "owner_promoted_before_creation")
        creation = self.create(105, topmost)
        self.acquisition("O", creation)
        self.add(dict(kind="creation", role="O", creation=creation))
        self.snapshot("after_owned_creation")
        for handle, after, kind in [(103, 1, "setup_p_bottom"), (102, -1, "setup_b_topmost"), (101, -1, "setup_a_topmost")]:
            self.placement(handle, after, kind)
        self.snapshot("before")
        before = self.records[-1]
        before["attributes"] = [self.actual(handle) for handle in self.handles]
        before["recorded_us"] = self.advance()
        self.add(dict(kind="setup_eligibility", eligible=True, reason=None))
        self.snapshot("before_insert")
        target, after = MANIFEST[member["family"]]["treatment"]
        target = dict(zip(ROLES, range(101, 106)))[target]
        after = dict(zip(ROLES, range(101, 106))).get(after, after)
        self.placement(target, after, "immediate_after_local_write", treatment=True)
        for sample in range(21):
            self.advance(25000)
            self.snapshot("settling")
            self.records[-1]["sample"] = sample
        for role, handle in [("O", 105), ("X", 104)]:
            self.add(dict(kind="window_retired", role=role, identity=self.window(handle), api="DestroyWindow",
                          started_us=self.advance(), returned_us=self.advance(), checked_us=self.advance(), ok=True, absent=True))
        for role, handle in [("P", 103), ("B", 102), ("A", 101)]:
            self.add(dict(kind="process_retired", role=role, handle=handle, process=handle + 1000, thread=handle + 2000,
                          finished_us=self.advance(), exited=True, error=None))
        self.add(dict(kind="trial_end", **{key: member[key] for key in ("case", "environment", "repetition", "lifetime_id")},
                      status="collected_pending_independent_analysis", cleanup_errors=[], error=None))
        self.add(dict(kind="trial_thread_retired", lifetime_id=member["lifetime_id"], joined=True))

    def acquisition(self, role, creation):
        self.records.append(dict(kind="window_acquired", role=role, recorded_us=creation["returned_us"] + 1,
                                 creation={key: creation[key] for key in ("handle", "calling_process", "calling_thread", "started_us", "returned_us")}))

    def add(self, record):
        record["recorded_us"] = self.advance()
        self.records.append(record)
        return record

    def ready(self, process, thread):
        return dict(process=process, thread=thread,
                    desktop=dict(verified=True, requested=self.desktop, actual=dict(name=self.desktop), process=process, thread=thread,
                                 switch_requested=False, requested_open_access=199),
                    input_method=dict(ok=True, argument=0, before_first_window=True, process=process, thread=thread, unix_us=self.advance()))

    def window(self, handle):
        window = super().window(handle)
        window["extended_style"] |= 0x08000080 if handle >= 104 else 0x08000000
        if handle == 105:
            window.update(owner=104, process=1104, thread=2104)
        return window

    def actual(self, handle):
        return dict(started_us=self.advance(), finished_us=self.advance(), window=self.window(handle),
                    style=0x90000000 if handle >= 104 else 0x10cf0000, style_read_error=None, **{"class": MANIFEST["class"]},
                    rectangle=[120, 160, 420, 340] if handle >= 104 else [160, 160, 460, 340])

    def create(self, handle, topmost=False):
        owner_started = self.advance()
        owner_before = self.window(104) if handle == 105 else None
        owner_finished = self.advance()
        started = self.advance()
        self.handles.append(handle)
        self.order.insert(0, handle)
        self.bands[handle] = topmost
        returned = self.advance()
        actual = self.actual(handle)
        return dict(api="CreateWindowExW", handle=handle, owner=104 if handle == 105 else 0,
                    owner_before=owner_before, owner_read_started_us=owner_started, owner_read_finished_us=owner_finished,
                    style=0x90000000 if handle >= 104 else 0x10cf0000, extended_style=0x08000080 if handle >= 104 else 0x08000000,
                    **{"class": MANIFEST["class"]}, geometry=[120, 160, 300, 180] if handle >= 104 else [160, 160, 300, 180],
                    calling_process=1104 if handle == 105 else handle + 1000, calling_thread=2104 if handle == 105 else handle + 2000,
                    started_us=started, returned_us=returned, actual=actual, desktop=dict(name=self.desktop))

    def snapshot(self, kind):
        raw = super().snapshot(kind)
        record = self.records[-1]
        record["recorded_us"] = self.advance()
        snapshot = scoped_snapshot(raw, kind == "before")
        record["snapshot"] = snapshot
        if kind in ("fresh_inventory", "after_owned_creation", "before"):
            snapshot["fixture_process_classes"] = [dict(window=self.window(handle), class_name=MANIFEST["class"], class_read_error=None,
                                                       window_style=0x90000000 if handle >= 104 else 0x10cf0000) for handle in self.handles]
        return snapshot

    def placement(self, target, after, kind, treatment=False):
        if target in (101, 102, 103):
            self.add(dict(kind="fixture_command", process=target + 1000, command=dict(operation="place", after=after)))
        call = dict(api="SetWindowPos", window=target, after=after, x=0, y=0, width=0, height=0, ok=True,
                    flags=self.member["treatment_flags"] if treatment else 19,
                    flags_names="NOMOVE|NOSIZE|NOACTIVATE" + ("|NOOWNERZORDER" if treatment and self.member["treatment_flags"] == 531 else ""),
                    calling_process=1104 if target == 105 else target + 1000, calling_thread=2104 if target == 105 else target + 2000,
                    started_us=self.advance(), returned_us=self.advance(), error=None, last_error_on_failure=None)
        if after == 1:
            self.order.remove(target)
            self.order.append(target)
            self.bands[target] = False
        elif after in (-1, -2):
            self.level(target, after == -1)
            if treatment:
                self.level(104, after == -1)
                self.level(105, after == -1)
        else:
            self.order = [101, 105, 104, 102, 103]
        if target in (101, 102, 103):
            self.add(fixture_reply(call, self.window(target)))
        else:
            self.add(dict(kind="treatment_call" if treatment else "local_overlay_call", call=call))
        self.snapshot(kind)

    def stop_after(self, kind, retired_roles):
        cutoff = next(index for index, row in enumerate(self.records) if row["kind"] == kind)
        self.records = self.records[:cutoff + 1] + [row for row in self.records[cutoff + 1:]
            if row["kind"] in ("trial_end", "trial_thread_retired")
            or row["kind"] in ("window_retired", "process_retired") and row["role"] in retired_roles]
        self.record("trial_end").update(status="ineligible_or_failed", error="native setup stopped")

    def failed_promotion(self):
        self.stop_after("owner_promoted_before_creation", ("X", "P", "B", "A"))
        def normal_owner(value):
            if isinstance(value, dict):
                if value.get("handle") == 104 and "topmost" in value:
                    value["topmost"] = False
                    value["extended_style"] &= ~8
                for item in value.values():
                    normal_owner(item)
            elif isinstance(value, list):
                for item in value:
                    normal_owner(item)
        normal_owner(self.record("owner_promoted_before_creation")["snapshot"])

    def initialization_failure(self, role, stage):
        owned = stage in ("readiness", "attributes")
        if role in ROLES[:3]:
            spawn_index = next(index for index, row in enumerate(self.records) if row["kind"] == "spawn" and row["label"] == role)
            cutoff = spawn_index + int(owned)
            started = self.records[spawn_index]["recorded_us"] - 1
            finished = self.records[spawn_index + 1]["recorded_us"]
            acquisition = None
        else:
            cutoff = next(index for index, row in enumerate(self.records) if row["kind"] == "creation" and row["role"] == role)
            creation = self.records[cutoff]["creation"]
            started, finished = creation["owner_read_started_us"], creation["actual"]["finished_us"]
            acquisition = dict(kind="window_acquired", role=role, recorded_us=creation["returned_us"] + 1,
                               creation={key: creation[key] for key in ("handle", "calling_process", "calling_thread", "started_us", "returned_us")}) if owned else None
            cutoff = next(index for index, row in enumerate(self.records) if row["kind"] == "window_acquired" and row["role"] == role)
        retired_roles = ROLES[:ROLES.index(role) + int(owned)]
        tail = [row for row in self.records[cutoff:] if row["kind"] in ("trial_end", "trial_thread_retired")
                or row["kind"] in ("window_retired", "process_retired") and row["role"] in retired_roles]
        self.records = self.records[:cutoff]
        if acquisition:
            self.records.append(acquisition)
        self.records.append(dict(kind="initialization_failed", role=role, stage=stage, resource_owned=owned,
                                 started_us=started, finished_us=finished, recorded_us=finished + 1, error="captured native initialization error"))
        self.records.extend(tail)
        self.record("trial_end").update(status="ineligible_or_failed", error="captured native initialization error")
        if stage == "readiness":
            retired = next(row for row in tail if row["kind"] == "process_retired" and row["role"] == role)
            retired.update(handle=0, thread=0)

    def result(self):
        begin = self.records[0]
        return audit.analyze_trial(dict(begin, records=self.records, admitted_us=0), self.desktop)


def full_capture(repetitions=1):
    members = schedule(repetitions)
    records = [dict(kind="session", schema_version=7, protocol="fresh-ownership-v7", case_names=CASES,
                    scenario_count=5, environment_case_count=10, pair_manifest=MANIFEST, schedule=members,
                    repetitions=repetitions, desktop=dict(name="Default"), recorded_us=1)]
    clock = 1000000
    for environment in ("ordinary", "private"):
        records.append(dict(kind="environment_ready", environment=environment, desktop_name="Default" if environment == "ordinary" else "Private", recorded_us=clock))
        for member in members:
            if member["environment"] == environment:
                trial = FreshTrial(member, clock)
                records.extend(trial.records)
                clock = trial.clock + 1000000
        records.extend([dict(kind="environment_end", environment=environment, ok=True, recorded_us=clock),
                        dict(kind="environment_cleanup", environment=environment, desktop_closed=environment == "private", borrowed_original_retained=environment == "ordinary", error=None, recorded_us=clock)])
    records.extend([dict(kind="collection_complete", recorded_us=clock), dict(kind="cleanup_complete", recorded_us=clock)])
    return records


def report(records):
    return analyze.analyze_lines(json.dumps(dict(unix_us=row["recorded_us"], record={key: value for key, value in row.items() if key != "recorded_us"})) for row in records)


class PairedAnalysisTests(unittest.TestCase):
    def test_run_analyzer_entrypoint_reads_focused_capture_and_returns_resolved_pairs(self):
        with tempfile.TemporaryDirectory() as directory:
            raw = pathlib.Path(directory) / "raw.jsonl"
            raw.write_text("".join(json.dumps(dict(unix_us=row["recorded_us"], record={key: value for key, value in row.items() if key != "recorded_us"})) + "\n" for row in full_capture()))
            completed = subprocess.run([sys.executable, "-B", str(pathlib.Path(__file__).with_name("analyze.py")), str(raw)], capture_output=True, text=True, check=False)
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertEqual(json.loads(completed.stdout)["counts"]["matched_pairs"], 5)

    def test_analyze_focused_schedule_resolves_all_pairs_with_reused_handle_values(self):
        result = report(full_capture(5))
        self.assertEqual(result["malformed_or_protocol_errors"], [])
        self.assertEqual(result["counts"]["scheduled"], 50)
        self.assertEqual(result["counts"]["matched_pairs"], 25, result["trials"])
        self.assertTrue(result["all_pairs_resolved"])

    def test_analyze_reversed_flag_execution_order_preserves_matched_pair_identity(self):
        members = schedule(2)[5:7]
        results = [FreshTrial(member).result() for member in members]
        self.assertEqual([row["treatment_flags"] for row in results], [531, 19])
        self.assertTrue(audit.compare_pair(results)["matched"], results)

    def test_compare_individually_valid_style_difference_rejects_pair(self):
        left, right = [FreshTrial(member) for member in schedule(1)[:2]]
        creation = next(row["creation"] for row in right.records if row["kind"] == "creation" and row["role"] == "X")
        creation["actual"]["style"] ^= 0x40000
        right.record("before")["attributes"][3]["style"] ^= 0x40000
        next(entry for entry in right.record("before")["snapshot"]["fixture_process_classes"] if entry["window"]["handle"] == 104)["window_style"] ^= 0x40000
        results = [left.result(), right.result()]
        self.assertTrue(all(row["instrumentation_valid"] for row in results), results)
        self.assertFalse(audit.compare_pair(results)["matched"])

    def test_compare_history_order_and_prefix_differences_rejects_individually_valid_members(self):
        original = [FreshTrial(member).result() for member in schedule(1)[:2]]
        for field in ("construction", "setup_calls", "initial", "created"):
            with self.subTest(field=field):
                results = copy.deepcopy(original)
                results[1]["setup_signature"][field] = ["different actual evidence"]
                self.assertFalse(audit.compare_pair(results)["matched"])

    def test_analyze_extra_owner_promotion_rejects_contaminated_measurement(self):
        trial = FreshTrial(schedule(1)[2])
        call = copy.deepcopy(trial.record("treatment_call"))
        call["call"].update(after=-1, started_us=call["call"]["returned_us"] + 1, returned_us=call["call"]["returned_us"] + 2)
        call["kind"] = "local_overlay_call"
        trial.records.insert(-3, call)
        self.assertIn("unscheduled", trial.result()["reason"])

    def test_analyze_changed_promotion_prefix_flag_rejects_insertion_pair(self):
        trial = FreshTrial(schedule(1)[3])
        call = trial.record("local_overlay_call")["call"]
        call.update(flags=531, flags_names="NOMOVE|NOSIZE|NOACTIVATE|NOOWNERZORDER")
        self.assertIn("setup call", trial.result()["reason"])

    def test_analyze_required_snapshot_identity_or_timestamp_mismatch_rejects_protocol(self):
        for mutation in ("identity", "time", "reread"):
            trial = FreshTrial(schedule(1)[0])
            snapshot = trial.record("before_insert")["snapshot"]
            if mutation == "identity":
                snapshot["fixtures"][3]["thread"] += 1
            elif mutation == "time":
                snapshot["started_us"] = 0
            else:
                snapshot["fixture_rereads"][4]["owner"] = 0
            with self.subTest(mutation=mutation):
                self.assertEqual(trial.result()["status"], "protocol_invalid")

    def test_analyze_native_target_read_after_snapshot_rejects_protocol(self):
        trial = FreshTrial(schedule(1)[0])
        reply = next(row["reply"]["record"] for row in trial.records if row["kind"] == "fixture_reply" and row["reply"]["record"]["kind"] == "placed")
        reply["read_finished_us"] = trial.clock
        self.assertIn("read", trial.result()["reason"])

    def test_analyze_retirement_failure_prevents_pair_attribution(self):
        trial = FreshTrial(schedule(1)[0])
        trial.record("window_retired")["absent"] = False
        result = trial.result()
        self.assertTrue(result["endpoint_resolved"])
        self.assertFalse(result["instrumentation_valid"])
        self.assertFalse(audit.compare_pair([result, FreshTrial(schedule(1)[1]).result()])["matched"])

    def test_analyze_wrong_lifetime_pair_metadata_rejects_reused_handles(self):
        records = full_capture()
        begins = [row for row in records if row["kind"] == "trial_begin"]
        begins[1]["lifetime_id"] = begins[0]["lifetime_id"]
        self.assertTrue(report(records)["malformed_or_protocol_errors"])

    def test_analyze_transient_invalid_sample_preserves_fixed_final_five(self):
        trial = FreshTrial(schedule(1)[0])
        trial.record("settling")["snapshot"]["invalid_reason"] = "transient"
        result = trial.result()
        self.assertTrue(result["instrumentation_valid"], result)
        self.assertTrue(any(not row["valid"] for row in result["snapshot_validity"]))
        next(row for row in trial.records if row["kind"] == "settling" and row["sample"] == 20)["snapshot"]["invalid_reason"] = "final invalid"
        self.assertFalse(trial.result()["instrumentation_valid"])

    def test_analyze_changed_actual_starting_order_reports_unmatched_premise(self):
        trial = FreshTrial(schedule(1)[0])
        before = trial.record("before")
        order = [101, 102, 103, 105, 104]
        by_handle = {window["handle"]: window for window in before["snapshot"]["fixtures"]}
        for index, handle in enumerate(order):
            by_handle[handle]["above"] = order[index - 1] if index else 0
            by_handle[handle]["below"] = order[index + 1] if index + 1 < len(order) else 0
        before["snapshot"]["raw_enumeration"] = [copy.deepcopy(by_handle[handle]) for handle in order]
        for item in before["snapshot"]["fixture_process_classes"]:
            item["window"] = copy.deepcopy(by_handle[item["window"]["handle"]])
        for item in before["attributes"]:
            item["window"] = copy.deepcopy(by_handle[item["window"]["handle"]])
        trial.record("setup_eligibility")["eligible"] = False
        trial.records = [row for row in trial.records if row["kind"] not in ("before_insert", "treatment_call", "immediate_after_local_write", "settling")]
        result = trial.result()
        self.assertEqual(result["status"], "premise_unmatched", result)
        self.assertFalse(audit.compare_pair([result, FreshTrial(schedule(1)[1]).result()])["matched"])

    def test_analyze_actual_owner_precreation_flag_mismatch_rejects_pair(self):
        trial = FreshTrial(schedule(1)[2])
        creation = next(row["creation"] for row in trial.records if row["kind"] == "creation" and row["role"] == "O")
        creation["owner_before"]["topmost"] = False
        creation["owner_before"]["extended_style"] &= ~8
        self.assertIn("precreation owner", trial.result()["reason"])

    def test_analyze_creation_reordered_timestamps_rejects_hidden_history_change(self):
        trial = FreshTrial(schedule(1)[0])
        creation = next(row["creation"] for row in trial.records if row["kind"] == "creation" and row["role"] == "O")
        creation["owner_read_started_us"] = creation["started_us"] = trial.records[0]["recorded_us"]
        self.assertFalse(trial.result()["instrumentation_valid"])

    def test_analyze_failed_precreation_premise_reports_unexecuted_setup(self):
        trial = FreshTrial(schedule(1)[2])
        trial.failed_promotion()
        ending = trial.record("trial_end")
        ending.update(status="ineligible_or_failed", error="fresh unowned owner promotion unverified")
        result = trial.result()
        self.assertEqual(result["status"], "setup_not_run")
        self.assertFalse(result["treatment_executed"])
        self.assertFalse(audit.compare_pair([result, FreshTrial(schedule(1)[3]).result()])["matched"])

    def test_analyze_failed_retirement_latches_all_environments_but_preserves_measurements(self):
        for mutation in ("missing", "absent", "destroy", "process", "ordering", "join"):
            rows = full_capture()
            retirements = [row for row in rows if row["kind"] == "window_retired"]
            if mutation == "missing":
                rows.remove(retirements[0])
            elif mutation in ("absent", "destroy"):
                retirements[0]["absent" if mutation == "absent" else "ok"] = False
            elif mutation == "process":
                next(row for row in rows if row["kind"] == "process_retired")["exited"] = False
            elif mutation == "ordering":
                retirements[1]["started_us"] = retirements[0]["started_us"]
            else:
                next(row for row in rows if row["kind"] == "trial_thread_retired")["joined"] = False
            with self.subTest(mutation=mutation):
                result = report(rows)
                self.assertEqual(result["counts"]["matched_pairs"], 0)
                self.assertTrue(result["malformed_or_protocol_errors"])
                self.assertEqual(result["counts"]["contaminated_admissions"], 9)
                self.assertTrue(all(row["endpoint_resolved"] for row in result["trials"]))
                self.assertTrue(all(row["instrumentation_valid"] and row["contaminated_admission"] for row in result["trials"][1:]))

    def test_analyze_invalid_endpoint_with_verified_retirement_keeps_later_pairs(self):
        for mutation in ("invalid", "time"):
            rows = full_capture()
            snapshot = next(row["snapshot"] for row in rows if row["kind"] == "settling" and row["sample"] == 20)
            if mutation == "invalid":
                snapshot["invalid_reason"] = "final read corrupt"
            else:
                snapshot["finished_us"] = 999999999
            result = report(rows)
            with self.subTest(mutation=mutation):
                self.assertEqual(result["counts"]["matched_pairs"], 4)
                self.assertEqual(result["counts"]["contaminated_admissions"], 0)
                self.assertTrue(result["trials"][0]["retirement_verified"])
                self.assertEqual(result["malformed_or_protocol_errors"], [])

    def test_analyze_failed_promotion_validates_reached_prefix_before_classifying_exit(self):
        for mutation in ("caller", "call_time", "flags", "snapshot_time", "invalid", "constructor", "missing_snapshot", "missing_call", "retirement"):
            trial = FreshTrial(schedule(1)[2])
            trial.failed_promotion()
            call = trial.record("local_overlay_call")["call"]
            snapshot = trial.record("owner_promoted_before_creation")["snapshot"]
            if mutation == "caller":
                call["calling_process"] = 999999
            elif mutation == "call_time":
                call["started_us"] = 0
            elif mutation == "flags":
                call.update(flags=531, flags_names="NOMOVE|NOSIZE|NOACTIVATE|NOOWNERZORDER")
            elif mutation == "snapshot_time":
                snapshot.update(started_us=0, finished_us=1)
            elif mutation == "invalid":
                snapshot["invalid_reason"] = "native read failed"
            elif mutation == "constructor":
                next(row["reply"]["record"]["creation"] for row in trial.records if row["kind"] == "fixture_reply")["calling_process"] = 999999
            elif mutation.startswith("missing"):
                kind = "owner_promoted_before_creation" if mutation == "missing_snapshot" else "local_overlay_call"
                trial.records = [row for row in trial.records if row["kind"] != kind]
            else:
                trial.record("window_retired")["absent"] = False
            with self.subTest(mutation=mutation):
                self.assertEqual(trial.result()["status"], "protocol_invalid", trial.result())

    def test_analyze_valid_owned_creation_band_miss_reports_unexecuted_setup(self):
        trial = FreshTrial(schedule(1)[2])
        trial.stop_after("after_owned_creation", ROLES)
        def normal_owned(value):
            if isinstance(value, dict):
                if value.get("handle") == 105 and "topmost" in value:
                    value["topmost"] = False
                    value["extended_style"] &= ~8
                for item in value.values():
                    normal_owned(item)
            elif isinstance(value, list):
                for item in value:
                    normal_owned(item)
        normal_owned(trial.records)
        result = trial.result()
        self.assertEqual(result["status"], "setup_not_run", result)
        self.assertTrue(result["retirement_verified"])
        self.assertTrue(all(row["valid"] for row in result["snapshot_validity"]))

    def test_analyze_invalid_initial_global_without_treatment_is_protocol_invalid(self):
        trial = FreshTrial(schedule(1)[0])
        trial.stop_after("setup_eligibility", ROLES)
        trial.record("before")["snapshot"]["invalid_reason"] = "broken chain"
        result = trial.result()
        self.assertEqual(result["status"], "protocol_invalid")
        self.assertTrue(result["retirement_verified"])
        self.assertTrue(any(row["kind"] == "before" and not row["valid"] for row in result["snapshot_validity"]))

    def test_analyze_native_initialization_failure_retires_every_acquired_resource(self):
        for role in ROLES:
            for stage in (("launch", "readiness") if role in ROLES[:3] else ("create", "attributes")):
                trial = FreshTrial(schedule(1)[2])
                trial.initialization_failure(role, stage)
                result = trial.result()
                with self.subTest(role=role, stage=stage):
                    self.assertEqual(result["status"], "setup_failed", result)
                    self.assertTrue(result["retirement_verified"])
                    self.assertFalse(result["treatment_executed"])
                    self.assertIn("captured native initialization error", result["reason"])

    def test_analyze_initialization_failure_missing_retirement_blocks_all_later_admissions(self):
        for role, stage in (("A", "readiness"), ("X", "attributes")):
            rows = full_capture()
            trial = FreshTrial(schedule(1)[0])
            trial.initialization_failure(role, stage)
            trial.records = [row for row in trial.records if not (row["kind"] in ("window_retired", "process_retired") and row["role"] == role)]
            first_end = next(index for index, row in enumerate(rows) if row["kind"] == "trial_thread_retired")
            rows[2:first_end + 1] = trial.records
            result = report(rows)
            with self.subTest(role=role):
                self.assertEqual(result["counts"]["matched_pairs"], 0)
                self.assertEqual(result["counts"]["contaminated_admissions"], 9)
                self.assertFalse(result["trials"][0]["retirement_verified"])

    def test_analyze_initialization_failure_does_not_mask_reached_corruption(self):
        for mutation in ("constructor", "prefix_caller", "prefix_time", "failure_time", "missing_acquisition", "missing_failure"):
            trial = FreshTrial(schedule(1)[2])
            trial.initialization_failure("O", "attributes")
            if mutation == "constructor":
                trial.record("creation")["creation"]["desktop"]["name"] = "Wrong"
            elif mutation == "prefix_caller":
                trial.record("local_overlay_call")["call"]["calling_process"] = 99999
            elif mutation == "prefix_time":
                trial.record("owner_promoted_before_creation")["snapshot"]["started_us"] = 0
            elif mutation == "failure_time":
                trial.record("initialization_failed")["started_us"] = 0
            else:
                kind = "window_acquired" if mutation == "missing_acquisition" else "initialization_failed"
                trial.records = [row for row in trial.records if row["kind"] != kind]
            with self.subTest(mutation=mutation):
                self.assertEqual(trial.result()["status"], "protocol_invalid", trial.result())

    def test_analyze_failed_source_setup_validates_reply_actor_without_later_stages(self):
        trial = FreshTrial(schedule(1)[0])
        trial.stop_after("setup_p_bottom", ROLES)
        reply = next(row for row in trial.records if row["kind"] == "fixture_reply" and row["reply"]["record"]["kind"] == "placed")
        reply["process"] = 99999
        result = trial.result()
        self.assertEqual(result["status"], "protocol_invalid")
        self.assertIn("native caller", result["reason"])

    def test_analyze_invalid_immediate_observation_keeps_fixed_valid_final_five(self):
        trial = FreshTrial(schedule(1)[0])
        trial.record("immediate_after_local_write")["snapshot"]["invalid_reason"] = "transient"
        result = trial.result()
        self.assertTrue(result["instrumentation_valid"], result)
        self.assertTrue(result["endpoint_resolved"])

    def test_analyze_partial_session_counts_missing_pairs_as_unresolved(self):
        result = report(full_capture()[:1])
        self.assertEqual(result["counts"]["unresolved_pairs"], 5)
        self.assertEqual(len(result["missing_schedule"]), 10)
        self.assertFalse(result["all_pairs_resolved"])

    def test_analyze_wrong_desktop_assignment_rejects_member(self):
        trial = FreshTrial(schedule(1)[0])
        trial.record("trial_thread_ready")["desktop"]["requested"] = "Private"
        self.assertIn("desktop", trial.result()["reason"])

    def test_analyze_hidden_owned_creation_rejects_extra_fixture_operation(self):
        trial = FreshTrial(schedule(1)[0])
        trial.records.insert(2, dict(kind="fixture_reply", process=1101, reply=dict(record=dict(kind="owned_ready"))))
        self.assertIn("unscheduled fixture", trial.result()["reason"])

    def test_analyze_extra_constructor_record_rejects_unrecorded_history(self):
        trial = FreshTrial(schedule(1)[0])
        trial.records.insert(2, dict(kind="owned_identity", creation=dict(api="CreateWindowExW")))
        self.assertIn("unscheduled trial record", trial.result()["reason"])

    def test_compare_changed_external_boundary_keeps_observation_but_rejects_flag_pair(self):
        members = [FreshTrial(member).result() for member in schedule(1)[:2]]
        members[1]["external_boundary"] = ["different boundary"]
        self.assertFalse(audit.compare_pair(members)["matched"])
        self.assertTrue(all(row["endpoint_resolved"] for row in members))


if __name__ == "__main__":
    unittest.main()
