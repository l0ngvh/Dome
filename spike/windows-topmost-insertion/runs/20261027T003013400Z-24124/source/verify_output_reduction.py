import copy
import hashlib
import json
import pathlib

import analyze


def scoped_snapshot(original, global_scope=False):
    raw = original.get("raw_enumeration", original.get("global_order"))
    snapshot = {key: copy.deepcopy(original[key]) for key in ("started_us", "finished_us", "fixtures")}
    snapshot.update(scope="global" if global_scope else "local", invalid_reason=None)
    if global_scope:
        snapshot.update(enumeration_ok=original["enumeration_ok"], raw_enumeration=raw)
        return snapshot
    snapshot["fixture_rereads"] = copy.deepcopy(snapshot["fixtures"])
    by_handle = {item["handle"]: item for item in raw}
    selected = set()
    fixture_handles = {item["handle"] for item in snapshot["fixtures"]}
    for fixture in snapshot["fixtures"]:
        selected.update(handle for handle in (fixture["handle"], fixture["above"], fixture["below"]) if handle)
        current = fixture["below"]
        for _ in range(32):
            if not current or current in fixture_handles:
                break
            selected.add(current)
            current = by_handle[current]["below"]
    snapshot["neighbors"] = [{key: by_handle[handle][key] for key in
        ("handle", "valid", "process", "thread", "above", "below")} for handle in sorted(selected)]
    return snapshot


def record_size(snapshot):
    return len(json.dumps({"unix_us": snapshot["finished_us"], "record": {
        "kind": "settling", "sample": 0, "snapshot": snapshot}}, separators=(",", ":")).encode()) + 1


def example():
    path = pathlib.Path(__file__).with_name("tests") / "scrambled-snapshot.json"
    original_bytes = path.read_bytes()
    original = json.loads(original_bytes)
    raw = original["global_order"]
    legacy = dict(original, raw_enumeration=raw, global_order=analyze.derived_order(raw),
                  chain_complete=True, invalid_reason=None, fixture_process_classes=[])
    local = scoped_snapshot(original)
    global_snapshot = scoped_snapshot(original, True)
    analyze.checked_snapshot(local)
    analyze.checked_snapshot(global_snapshot)
    return {
        "scope": "Serialized shape example from preserved snapshot fields, not new native evidence or a full-run measurement",
        "source_snapshot_sha256": hashlib.sha256(original_bytes).hexdigest(),
        "source_windows": len(raw), "local_neighbor_nodes": len(local["neighbors"]),
        "schema_three_duplicate_array_record_bytes": record_size(legacy),
        "schema_four_local_record_bytes": record_size(local),
        "schema_four_global_record_bytes": record_size(global_snapshot),
        "limitations": "No classes were recorded in the source snapshot. New class diagnostics and native timing costs are not measured. The unavailable 261 MB run was not inspected.",
    }


if __name__ == "__main__":
    print(json.dumps(example(), indent=2))
