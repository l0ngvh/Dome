import collections
import copy
import hashlib
import json
import pathlib
import subprocess
import sys

import analyze


def verify(raw_path):
    original = raw_path.read_bytes()
    text = original.decode("utf-8-sig")
    records = [json.loads(line)["record"] for line in text.splitlines()]
    session = records[0]
    if session["schema_version"] != 2:
        raise ValueError("This diagnostic replays the preserved schema 2 collector failure")
    legacy = subprocess.run([sys.executable, str(raw_path.parent / "source/analyze.py"), str(raw_path)],
                            capture_output=True, text=True, check=False)
    if legacy.returncode != 2 or legacy.stderr:
        raise ValueError("Unexpected preserved-analyzer execution result")
    legacy_report = json.loads(legacy.stdout)
    if analyze.analyze_text(text) != legacy_report:
        raise ValueError("Schema 2 analysis changed from the preserved analyzer")
    counts = collections.Counter(record["kind"] for record in records)
    snapshots = 0
    changed_order = 0
    setup = collections.defaultdict(lambda: {"recorded_before": 0, "diagnostic_premise_pass": 0, "reasons": collections.Counter()})
    case = None
    for record in records:
        if record["kind"] == "trial_begin":
            case = record["case"]
        if "snapshot" not in record:
            continue
        snapshot = copy.deepcopy(record["snapshot"])
        snapshot["raw_enumeration"] = snapshot["global_order"]
        snapshot["global_order"] = analyze.derived_order(snapshot["raw_enumeration"])
        changed_order += snapshot["global_order"] != snapshot["raw_enumeration"]
        analyze.checked_snapshot(snapshot)
        snapshots += 1
        if record["kind"] == "before":
            row = setup[case]
            row["recorded_before"] += 1
            try:
                analyze.placement_premise(snapshot, case, analyze.initial_bands(case))
                row["diagnostic_premise_pass"] += 1
            except ValueError as error:
                row["reasons"][str(error)] += 1
    if raw_path.read_bytes() != original:
        raise ValueError("Raw evidence changed during diagnostic replay")
    return {
        "scope": "Read-only ordering diagnostic, not a native rerun or recovered treatment evidence",
        "raw_sha256": hashlib.sha256(original).hexdigest(),
        "schema_two_analysis_matches_preserved_analyzer": True,
        "snapshots_validated_from_recorded_links": snapshots,
        "snapshots_with_scrambled_enumeration": changed_order,
        "attempts": counts["trial_begin"],
        "recorded_treatment_calls": counts["treatment_call"],
        "recorded_asynchronous_posts": counts["external_async_call"],
        "recorded_settling_samples": counts["settling"],
        "legacy_eligible_observations": sum(row["eligible"] for row in legacy_report["matrix"].values()),
        "diagnostic_setup_only": dict(setup),
        "limitation": "Old finished_us excludes fixture reads. No timing bound or native outcome is repaired. Cases without before never reached their completed setup.",
    }


if __name__ == "__main__":
    print(json.dumps(verify(pathlib.Path(sys.argv[1])), indent=2))
