import copy
import json
import pathlib
import unittest

import analyze
from verify_output_reduction import scoped_snapshot


class SyntheticTrial:
    def __init__(self, case):
        self.case = case
        self.clock = 1000000
        self.records = []
        self.bands = dict(zip(range(101, 106), analyze.initial_bands(case)))
        self.handles = list(self.bands)
        self.order = [handle for handle in self.handles if self.bands[handle]] + [handle for handle in self.handles if not self.bands[handle]]
        if case == analyze.CASES[7]:
            self.order = [102, 101, 103, 104]
        if case in (analyze.CASES[19], analyze.CASES[20]):
            self.order = [101, 102, 103, 104]
        self.before = self.snapshot("before")
        if "async_a_" in case:
            self.transition()
        elif case.startswith("held_c_"):
            self.race()
        else:
            if case in (analyze.CASES[1], analyze.CASES[2], analyze.CASES[9]):
                self.call("local_overlay_call", 104, -1 if case == analyze.CASES[2] else -2)
                self.level(104, case == analyze.CASES[2])
                self.snapshot("after_explicit_topmost" if case == analyze.CASES[2] else "after_explicit_notopmost")
            self.insert_overlay_or_source()
        self.settle("settling")
        self.records.append({"kind": "trial_end", "case": case, "repetition": 0,
            "status": "collected_pending_independent_analysis"})

    def reset_to_before(self):
        self.bands = {item["handle"]: item["topmost"] for item in self.before["fixtures"]}
        self.order = [item["handle"] for item in self.before["global_order"]]

    def advance(self, amount=10000):
        self.clock += amount
        return self.clock

    def window(self, handle):
        position = self.order.index(handle)
        return {"handle": handle, "valid": True, "process": handle + 1000,
            "thread": handle + 2000, "extended_style": 8 if self.bands[handle] else 0,
            "style_read_error": None, "topmost": self.bands[handle], "visible": True,
            "above": self.order[position - 1] if position else 0,
            "below": self.order[position + 1] if position + 1 < len(self.order) else 0,
            "owner": 0}

    def snapshot(self, kind):
        observation = {"started_us": self.advance(), "finished_us": self.clock + 1,
            "enumeration_ok": True, "chain_complete": True,
            "global_order": [self.window(handle) for handle in self.order],
            "fixtures": [self.window(handle) for handle in self.handles],
            "fixture_order": [handle for handle in self.order if handle in self.handles]}
        self.records.append({"kind": kind, "snapshot": observation})
        return observation

    def level(self, handle, topmost):
        self.bands[handle] = topmost
        self.order.remove(handle)
        position = 0 if topmost else next((index for index, other in enumerate(self.order) if not self.bands[other]), len(self.order))
        self.order.insert(position, handle)

    def insert(self, handle, reference, topmost):
        self.bands[handle] = topmost
        self.order.remove(handle)
        self.order.insert(self.order.index(reference) + 1, handle)

    def call(self, kind, window, after, asynchronous=False):
        caller = 104 if asynchronous else window
        call = {"api": "SetWindowPos", "window": window, "after": after,
            "x": 0, "y": 0, "width": 0, "height": 0, "ok": True,
            "flags": 16403 if asynchronous else 19,
            "calling_process": caller + 1000, "calling_thread": caller + 2000,
            "started_us": self.advance(), "returned_us": self.clock + 1}
        record = {"kind": kind, "call": call}
        if asynchronous:
            record.update(post_return_target_read=self.window(window), read_finished_us=self.clock + 2)
        self.records.append(record)
        return record

    def reply(self, handle, kind):
        self.records.append({"kind": "fixture_reply", "process": handle + 1000,
            "reply": {"unix_us": self.advance(), "record": {"kind": kind, "handle": handle}}})

    def settle(self, kind):
        for sample in range(21):
            self.advance(25000)
            self.snapshot(kind)
            self.records[-1]["sample"] = sample

    def insert_overlay_or_source(self):
        alternative = self.case in analyze.CASES[18:]
        target = 101 if alternative else 104
        reference = 104 if self.case == analyze.CASES[18] else 103 if alternative or self.case in (analyze.CASES[3], analyze.CASES[10]) else 101
        self.snapshot("before_insert")
        self.call("treatment_call", target, reference)
        self.insert(target, reference, self.bands[reference])
        self.snapshot("immediate_after_local_write")
        if self.case == analyze.CASES[11]:
            self.settle("first_settling")
            self.snapshot("before_repeated_insert")
            self.call("treatment_call", target, reference)
            self.snapshot("immediate_after_repeated_write")

    def transition(self):
        promotion = "promotion" in self.case
        waiting = self.case.startswith("wait_")
        self.reply(101, "held")
        self.snapshot("held_before_post")
        self.call("external_async_call", 101, -1 if promotion else -2, True)
        self.snapshot("after_async_post_while_held")
        if waiting:
            self.reply(101, "released_before_pump")
            self.level(101, promotion)
            self.snapshot("after_release")
            self.snapshot("reference_transition_observed")
        self.insert_overlay_or_source()
        if not waiting:
            self.advance(100000)
            self.snapshot("held_after_100ms")
            self.reply(101, "released_before_pump")
            self.level(101, promotion)
            self.snapshot("after_release")

    def race(self):
        self.reply(105, "held")
        self.snapshot("queried_normal_a_before_c_post")
        self.call("external_async_call", 105, 101, True)
        self.snapshot("after_async_post_while_held")
        if self.case != analyze.CASES[16]:
            self.call("source_call", 101, -1)
            self.level(101, True)
            self.snapshot("after_a_promotion_while_c_held")
        if self.case == analyze.CASES[17]:
            self.call("source_call", 101, -2)
            self.level(101, False)
            self.snapshot("after_a_demotion_while_c_held")
        self.snapshot("before_c_release")
        self.reply(105, "released_before_pump")
        self.insert(105, 101, self.bands[101])
        self.snapshot("immediate_after_release")

    def result(self):
        labels = {101: "A", 102: "B", 103: "normalA", 104: "X", 105: "C-synthetic"}
        identities = {(handle, handle + 1000, handle + 2000): labels[handle] for handle in self.handles}
        return analyze.analyze_trial({"case": self.case, "repetition": 0, "records": self.records}, (1104, 2104), identities)

    def record(self, kind):
        return next(record for record in self.records if record["kind"] == kind)


def serialized_run():
    records = [{"kind": "session", "schema_version": 2, "process": 1104, "thread": 2104,
        "repetitions": 1, "case_names": list(analyze.CASES)}]
    for handle in (101, 102, 103, 105):
        labels = {101: "A", 102: "B", 103: "normalA", 105: "C-synthetic"}
        records.append({"kind": "spawn", "process": handle + 1000, "label": labels[handle]})
        records.append({"kind": "fixture_reply", "process": handle + 1000,
            "reply": {"record": {"kind": "ready", "handle": handle, "process": handle + 1000, "thread": handle + 2000}}})
    records.append({"kind": "overlay_identity", "handle": 104, "process": 1104, "thread": 2104})
    for case in analyze.CASES:
        records.append({"kind": "trial_begin", "case": case, "repetition": 0})
        records.extend(SyntheticTrial(case).records)
    records.extend([{"kind": "collection_complete"}, {"kind": "cleanup_complete"}])
    return records


def encoded(records):
    return "\n".join(json.dumps({"record": record}) for record in records)


class ScopedObservationTests(unittest.TestCase):
    def test_analyze_schema_four_roundtrip_for_all_cases_without_native_claim(self):
        records = serialized_run()
        records[0]["schema_version"] = 4
        case = None
        for record in records:
            if record["kind"] == "trial_begin":
                case = record["case"]
            if "snapshot" in record:
                record["snapshot"] = scoped_snapshot(record["snapshot"],
                    case in (analyze.CASES[5], analyze.CASES[7], analyze.CASES[20])
                    and record["kind"] in ("before", "before_insert"))
        report = analyze.analyze_lines(iter(encoded(records).splitlines()))
        self.assertTrue(report["all_trials_eligible_and_controls_passed"], report)
        for row in report["trials"]:
            self.assertEqual(len(row["settling_timing"]["read_spans_us"]), 21)
            self.assertEqual(len(row["settling_timing"]["gaps_us"]), 20)

    def test_analyze_timestamps_not_emission_order_bracket_treatment(self):
        trial = SyntheticTrial(analyze.CASES[0])
        call = trial.record("treatment_call")
        trial.records.remove(call)
        index = trial.records.index(trial.record("immediate_after_local_write"))
        trial.records.insert(index + 1, call)
        self.assertEqual(trial.result()["status"], "eligible_observation")

    def test_validate_local_paths_prove_order_without_global_claim(self):
        original = json.loads(pathlib.Path(__file__).with_name("tests").joinpath("scrambled-snapshot.json").read_text())
        snapshot = scoped_snapshot(original)
        fixtures, order = analyze.checked_snapshot(snapshot)
        self.assertIsNone(order)
        self.assertTrue(analyze.above(snapshot, fixtures[0]["handle"], fixtures[1]["handle"]))
        self.assertFalse(analyze.above(snapshot, fixtures[1]["handle"], fixtures[0]["handle"]))
        self.assertIsNone(analyze.above(snapshot, fixtures[0]["handle"], -99))
        with self.assertRaisesRegex(ValueError, "global evidence"):
            analyze.canonical_order(snapshot)
        snapshot["scope"] = "global"
        snapshot["raw_enumeration"] = [item for item in original["global_order"]
            if item["handle"] in {node["handle"] for node in snapshot["neighbors"]}]
        snapshot["enumeration_ok"] = True
        with self.assertRaises(ValueError):
            analyze.checked_snapshot(snapshot)

    def test_validate_local_with_invalid_identity_style_and_links_rejects(self):
        original = SyntheticTrial(analyze.CASES[0]).before
        for key, value in (("valid", False), ("thread", 0), ("process", 0),
                           ("above", 999), ("below", 999)):
            snapshot = scoped_snapshot(original)
            snapshot["neighbors"][0][key] = value
            with self.subTest(key=key), self.assertRaises(ValueError):
                analyze.checked_snapshot(snapshot)
        for key, value in (("style_read_error", 5), ("topmost", False), ("owner", 1),
                           ("visible", False), ("handle", 999)):
            snapshot = scoped_snapshot(original)
            snapshot["fixtures"][0][key] = value
            snapshot["fixture_rereads"] = copy.deepcopy(snapshot["fixtures"])
            with self.subTest(key=key), self.assertRaises(ValueError):
                analyze.checked_snapshot(snapshot)
        snapshot = scoped_snapshot(original)
        snapshot["neighbors"].pop()
        with self.assertRaises(ValueError):
            analyze.checked_snapshot(snapshot)
        snapshot = scoped_snapshot(original)
        node = snapshot["neighbors"][0]
        snapshot["neighbors"].extend([dict(node, handle=-10, above=-11, below=-11),
                                      dict(node, handle=-11, above=-10, below=-10)])
        with self.assertRaisesRegex(ValueError, "cycle"):
            analyze.checked_snapshot(snapshot)

    def test_validate_local_with_full_fixture_field_changed_after_traversal_rejects(self):
        for case in (analyze.CASES[0], analyze.CASES[16]):
            original = scoped_snapshot(SyntheticTrial(case).before)
            analyze.checked_snapshot(original)
            for index, fixture in enumerate(original["fixtures"]):
                changes = {"handle": fixture["handle"] + 1, "valid": False,
                    "process": fixture["process"] + 1, "thread": fixture["thread"] + 1,
                    "extended_style": fixture["extended_style"] ^ 16, "style_read_error": 5,
                    "topmost": not fixture["topmost"], "visible": False,
                    "above": fixture["above"] + 1, "below": fixture["below"] + 1, "owner": 1}
                for key, value in changes.items():
                    snapshot = copy.deepcopy(original)
                    snapshot["fixture_rereads"][index][key] = value
                    with self.subTest(case=case, index=index, key=key), self.assertRaisesRegex(
                            ValueError, "fixture changed after local traversal"):
                        analyze.checked_snapshot(snapshot)
                snapshot = copy.deepcopy(original)
                snapshot["fixture_rereads"][index].update(
                    extended_style=fixture["extended_style"] ^ 8, topmost=not fixture["topmost"])
                with self.subTest(case=case, index=index, key="decoded band"), self.assertRaisesRegex(
                        ValueError, "fixture changed after local traversal"):
                    analyze.checked_snapshot(snapshot)
            for alteration in ("missing", "short", "duplicate"):
                snapshot = copy.deepcopy(original)
                if alteration == "missing":
                    del snapshot["fixture_rereads"]
                elif alteration == "short":
                    snapshot["fixture_rereads"].pop()
                else:
                    snapshot["fixture_rereads"][1] = copy.deepcopy(snapshot["fixture_rereads"][0])
                with self.subTest(case=case, alteration=alteration), self.assertRaisesRegex(
                        ValueError, "fixture changed after local traversal"):
                    analyze.checked_snapshot(snapshot)

    def test_analyze_normal_predecessor_control_requires_target_above_x(self):
        trial = SyntheticTrial(analyze.CASES[19])
        trial.order = [102, 104, 103, 101]
        for record in trial.records:
            if record["kind"] in ("immediate_after_local_write", "settling"):
                snapshot = record["snapshot"]
                snapshot["global_order"] = [trial.window(handle) for handle in trial.order]
                snapshot["fixtures"] = [trial.window(handle) for handle in trial.handles]
                snapshot["fixture_order"] = trial.order.copy()
                record["snapshot"] = scoped_snapshot(snapshot)
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertTrue(result["settled"]["target_directly_after_reference"])
        self.assertFalse(result["settled"]["target_above_X"])
        self.assertFalse(result["control_passed"])

    def test_analyze_c_normal_control_rejects_destination_before_release(self):
        trial = SyntheticTrial(analyze.CASES[16])
        destination = trial.record("immediate_after_release")["snapshot"]
        before_release = trial.record("before_c_release")["snapshot"]
        for key in ("fixtures", "global_order", "fixture_order"):
            before_release[key] = copy.deepcopy(destination[key])
        for record in trial.records:
            if "snapshot" in record:
                record["snapshot"] = scoped_snapshot(record["snapshot"])
        result = trial.result()
        self.assertEqual(result["status"], "ineligible_or_incomplete")
        self.assertIn("C moved before release", result["reason"])

    def test_analyze_promotion_cases_do_not_require_unchanged_c_neighbors(self):
        for case in (analyze.CASES[15], analyze.CASES[17]):
            result = SyntheticTrial(case).result()
            self.assertEqual(result["status"], "eligible_observation", result)
            self.assertIn("A moved", result["held_C_adjacency_interpretation"])

    def test_analyze_scoped_global_proof_required_at_preinsertion(self):
        for case in (analyze.CASES[5], analyze.CASES[7], analyze.CASES[20]):
            trial = SyntheticTrial(case)
            trial.record("before_insert")["snapshot"] = scoped_snapshot(trial.record("before_insert")["snapshot"])
            result = trial.result()
            self.assertEqual(result["status"], "ineligible_or_incomplete")
            self.assertIn("global evidence", result["reason"])


class AnalyzerTests(unittest.TestCase):
    def test_each_synthetic_protocol_is_accepted_without_native_claim(self):
        for case in analyze.CASES:
            with self.subTest(case=case):
                result = SyntheticTrial(case).result()
                self.assertEqual(result["status"], "eligible_observation", result)
                self.assertTrue(result.get("control_passed", True), result)

    def test_legacy_matrix_retains_original_lowest_flagged_variant(self):
        self.assertEqual(len(analyze.CASES), 21)
        self.assertEqual(analyze.CASES[7], "normal_x_after_bottommost_topmost_a_with_b_above")

    def test_hidden_topmost_invalidates_global_sole_but_not_controlled_sole(self):
        trial = SyntheticTrial(analyze.CASES[5])
        trial.reset_to_before()
        trial.bands[999] = True
        trial.order.insert(0, 999)
        observation = trial.snapshot("extra")
        observation["global_order"][0]["visible"] = False
        with self.assertRaisesRegex(ValueError, "another global topmost"):
            analyze.placement_premise(observation, analyze.CASES[5], analyze.initial_bands(analyze.CASES[5]))
        analyze.placement_premise(observation, analyze.CASES[6], analyze.initial_bands(analyze.CASES[6]))

    def test_hidden_topmost_below_a_invalidates_bottom_boundary(self):
        trial = SyntheticTrial(analyze.CASES[7])
        trial.reset_to_before()
        trial.bands[999] = True
        trial.order.append(999)
        observation = trial.snapshot("extra")
        observation["global_order"][-1]["visible"] = False
        with self.assertRaisesRegex(ValueError, "bottommost"):
            analyze.placement_premise(observation, trial.case, analyze.initial_bands(trial.case))

    def test_below_b_case_does_not_require_an_empty_desktop(self):
        trial = SyntheticTrial(analyze.CASES[0])
        trial.reset_to_before()
        trial.bands[999] = True
        trial.order.insert(0, 999)
        analyze.placement_premise(trial.snapshot("extra"), trial.case, analyze.initial_bands(trial.case))

    def test_wrong_predecessor_does_not_prove_above_overlay_arrangement(self):
        trial = SyntheticTrial(analyze.CASES[19])
        trial.reset_to_before()
        trial.order = [101, 103, 102, 104]
        with self.assertRaisesRegex(ValueError, "immediate predecessor"):
            analyze.placement_premise(trial.snapshot("extra"), trial.case, analyze.initial_bands(trial.case))

    def test_collector_eligible_flag_cannot_override_inconsistent_chain(self):
        trial = SyntheticTrial(analyze.CASES[0])
        trial.before["chain_complete"] = True
        trial.before["global_order"][0]["below"] = 999
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_api_success_without_reference_transition_is_rejected(self):
        for case in (analyze.CASES[12], analyze.CASES[14]):
            trial = SyntheticTrial(case)
            trial.records = [record for record in trial.records if record["kind"] != "reference_transition_observed"]
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_race_requires_observed_promotion_not_only_api_success(self):
        trial = SyntheticTrial(analyze.CASES[15])
        trial.records = [record for record in trial.records if record["kind"] != "after_a_promotion_while_c_held"]
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_queued_c_cannot_release_before_promotion(self):
        trial = SyntheticTrial(analyze.CASES[15])
        release = next(record for record in trial.records if record["kind"] == "fixture_reply" and record["reply"]["record"]["kind"] == "released_before_pump")
        release["reply"]["unix_us"] = trial.before["finished_us"]
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_wrong_async_target_and_unheld_call_are_rejected(self):
        for change in ("target", "hold"):
            trial = SyntheticTrial(analyze.CASES[4])
            if change == "target":
                trial.record("external_async_call")["call"]["window"] = 102
            else:
                trial.records = [record for record in trial.records if record["kind"] != "fixture_reply"]
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_repeat_requires_first_convergence(self):
        trial = SyntheticTrial(analyze.CASES[11])
        trial.records = [record for record in trial.records if record["kind"] != "first_settling"]
        self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_incomplete_settling_and_native_failures_are_rejected(self):
        for change in ("settling", "failure"):
            trial = SyntheticTrial(analyze.CASES[0])
            if change == "settling":
                trial.records.pop(-2)
            else:
                trial.record("treatment_call")["call"]["ok"] = False
            self.assertEqual(trial.result()["status"], "ineligible_or_incomplete")

    def test_unknown_insertion_result_is_an_observation_not_a_failed_control(self):
        trial = SyntheticTrial(analyze.CASES[0])
        for record in trial.records:
            if record["kind"] in ("immediate_after_local_write", "settling"):
                for entry in record["snapshot"]["fixtures"] + record["snapshot"]["global_order"]:
                    if entry["handle"] == 104:
                        entry.update(topmost=False, extended_style=0)
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertFalse(result["settled"]["target_topmost"])
        self.assertNotIn("control_passed", result)

    def test_boundary_is_checked_again_immediately_before_insertion(self):
        trial = SyntheticTrial(analyze.CASES[5])
        trial.reset_to_before()
        trial.bands[999] = True
        trial.order.insert(0, 999)
        changed = trial.snapshot("unrelated_boundary_change")
        before_insert = trial.record("before_insert")["snapshot"]
        changed["started_us"] = before_insert["started_us"]
        changed["finished_us"] = before_insert["finished_us"]
        trial.record("before_insert")["snapshot"] = changed
        result = trial.result()
        self.assertEqual(result["status"], "ineligible_or_incomplete")
        self.assertIn("another global topmost", result["reason"])

    def test_failed_control_is_not_reported_as_a_passing_observation(self):
        trial = SyntheticTrial(analyze.CASES[3])
        for record in trial.records:
            if record["kind"] in ("immediate_after_local_write", "settling"):
                for entry in record["snapshot"]["fixtures"] + record["snapshot"]["global_order"]:
                    if entry["handle"] == 104:
                        entry.update(topmost=True, extended_style=8)
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation", result)
        self.assertFalse(result["control_passed"])

    def test_nonobject_json_and_invalid_trial_identity_fail_closed(self):
        for record in (None, [], {"kind": "trial_begin", "case": [], "repetition": 0},
                       {"kind": "fixture_reply", "process": 1, "reply": {"record": None}}):
            result = analyze.analyze_text(encoded([record]))
            self.assertFalse(result["all_trials_eligible_and_controls_passed"])
            self.assertTrue(result["malformed_or_protocol_errors"])

    def test_empty_missing_duplicate_and_malformed_runs_fail_closed(self):
        records = serialized_run()
        complete = analyze.analyze_text(encoded(records))
        self.assertTrue(complete["all_trials_eligible_and_controls_passed"], complete["trials"])
        bad_runs = ["", encoded(records[:-1]), encoded(records[:-2]), encoded(records) + "\n{",
            encoded([record for record in records if record["kind"] != "overlay_identity"])]
        duplicate = copy.deepcopy(records)
        for record in duplicate:
            if record["kind"] in ("trial_begin", "trial_end") and record["case"] == analyze.CASES[1]:
                record["case"] = analyze.CASES[0]
        bad_runs.append(encoded(duplicate))
        for text in bad_runs:
            self.assertFalse(analyze.analyze_text(text)["all_trials_eligible_and_controls_passed"])


class RecordedOrderingTests(unittest.TestCase):
    def recorded(self):
        snapshot = json.loads(pathlib.Path(__file__).with_name("tests").joinpath("scrambled-snapshot.json").read_text())
        snapshot["raw_enumeration"] = snapshot["global_order"]
        snapshot["global_order"] = analyze.derived_order(snapshot["raw_enumeration"])
        return snapshot

    def test_validate_recorded_scrambled_enumeration_derives_global_neighbors(self):
        snapshot = self.recorded()
        fixtures, order = analyze.checked_snapshot(snapshot)
        self.assertNotEqual(snapshot["raw_enumeration"], snapshot["global_order"])
        self.assertEqual(order.index(1770198) + 1, order.index(327832))
        self.assertEqual(order.index(327832) + 1, order.index(328988))
        analyze.placement_premise(snapshot, analyze.CASES[0], analyze.initial_bands(analyze.CASES[0]))
        self.assertLess(order.index(fixtures[0]["handle"]), order.index(fixtures[1]["handle"]))

    def test_validate_raw_graph_with_corruption_rejects_even_with_valid_collector_order(self):
        for corruption in ("cycle", "missing", "duplicate", "reciprocal", "invalid", "style", "identity", "decoding"):
            with self.subTest(corruption=corruption):
                snapshot = self.recorded()
                raw = snapshot["raw_enumeration"] = copy.deepcopy(snapshot["global_order"])
                if corruption == "cycle":
                    raw[-1]["below"], raw[0]["above"] = raw[0]["handle"], raw[-1]["handle"]
                elif corruption == "missing":
                    raw.pop(1)
                elif corruption == "duplicate":
                    raw.append(copy.deepcopy(raw[0]))
                elif corruption == "reciprocal":
                    raw[1]["above"] = raw[2]["handle"]
                elif corruption == "invalid":
                    raw[1]["valid"] = False
                elif corruption == "style":
                    raw[1]["style_read_error"] = 5
                elif corruption == "identity":
                    raw[1]["thread"] = 0
                else:
                    raw[1]["topmost"] = not raw[1]["topmost"]
                snapshot["chain_complete"] = True
                with self.assertRaises(ValueError):
                    analyze.checked_snapshot(snapshot)

    def test_validate_raw_graph_with_detached_cycle_rejects_partial_coverage(self):
        snapshot = self.recorded()
        raw = snapshot["raw_enumeration"] = copy.deepcopy(snapshot["global_order"])
        raw[0]["below"], raw[3]["above"] = raw[3]["handle"], raw[0]["handle"]
        raw[1]["above"] = raw[1]["below"] = raw[2]["handle"]
        raw[2]["above"] = raw[2]["below"] = raw[1]["handle"]
        with self.assertRaisesRegex(ValueError, "coverage"):
            analyze.checked_snapshot(snapshot)

    def test_validate_snapshot_with_partial_or_changed_fixture_read_rejects(self):
        for corruption in ("enumeration", "fixture", "timestamp", "derived_order"):
            with self.subTest(corruption=corruption):
                snapshot = self.recorded()
                if corruption == "enumeration":
                    snapshot["enumeration_ok"] = False
                elif corruption == "fixture":
                    snapshot["fixtures"][0]["below"] = 0
                elif corruption == "timestamp":
                    snapshot["finished_us"] = snapshot["started_us"] - 1
                else:
                    snapshot["global_order"].reverse()
                with self.assertRaises(ValueError):
                    analyze.checked_snapshot(snapshot)

    def test_analyze_schema_three_recomputes_scrambled_raw_and_requires_it(self):
        records = serialized_run()
        records[0]["schema_version"] = 3
        for record in records:
            if "snapshot" in record:
                snapshot = record["snapshot"]
                snapshot["raw_enumeration"] = list(reversed(copy.deepcopy(snapshot["global_order"])))
                snapshot["chain_complete"] = False
        self.assertTrue(analyze.analyze_text(encoded(records))["all_trials_eligible_and_controls_passed"])
        del next(record for record in records if "snapshot" in record)["snapshot"]["raw_enumeration"]
        result = analyze.analyze_text(encoded(records))
        self.assertFalse(result["all_trials_eligible_and_controls_passed"])
        self.assertIn("missing raw enumeration", result["malformed_or_protocol_errors"][0]["error"])

    def test_analyze_with_snapshot_overlapping_treatment_rejects_invalid_time_window(self):
        trial = SyntheticTrial(analyze.CASES[0])
        trial.record("before_insert")["snapshot"]["finished_us"] = trial.record("treatment_call")["call"]["started_us"] + 1
        result = trial.result()
        self.assertEqual(result["status"], "ineligible_or_incomplete")
        self.assertIn("not bracketed", result["reason"])

    def test_analyze_failed_control_preserves_measured_band_and_adjacency(self):
        trial = SyntheticTrial(analyze.CASES[3])
        for record in trial.records:
            if record["kind"] in ("immediate_after_local_write", "settling"):
                for entry in record["snapshot"]["fixtures"] + record["snapshot"]["global_order"]:
                    if entry["handle"] == 104:
                        entry.update(topmost=True, extended_style=8)
        result = trial.result()
        self.assertEqual(result["status"], "eligible_observation")
        self.assertFalse(result["control_passed"])
        self.assertTrue(result["immediate"]["target_topmost"])
        self.assertTrue(result["settled"]["target_topmost"])
        self.assertTrue(result["settled"]["target_directly_after_reference"])


if __name__ == "__main__":
    unittest.main()
