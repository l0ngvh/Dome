import hashlib
import json
from pathlib import Path
import subprocess
import sys

root = Path(__file__).resolve().parents[1]
results = []
for name, known in (("20261026T090428503Z-11092", "62de6552ce92933a0ca37dcef2fd7eefe578457c995dc575cb98205a3abecb4f"),
                    ("20261027T003013400Z-24124", "07190550967db31935c341061c0caff5ffc97509f190226770ee8dd304eba431")):
    run = root / "runs" / name
    raw = run / "raw.jsonl"
    before = hashlib.sha256(raw.read_bytes()).hexdigest()
    assert before == known
    captured = subprocess.run([sys.executable, "-B", str(run / "source/analyze.py"), str(raw)], capture_output=True, check=False)
    current = subprocess.run([sys.executable, "-B", str(root / "analyze.py"), str(raw)], capture_output=True, check=False)
    assert captured.returncode == current.returncode == 2
    assert captured.stdout == current.stdout
    assert captured.stderr == current.stderr == b""
    assert hashlib.sha256(raw.read_bytes()).hexdigest() == before
    report = json.loads(current.stdout)
    results.append({"run": name, "raw_sha256": before, "captured_and_current_stdout_byte_identical": True,
                    "exit_code": current.returncode, "schema_version": report["schema_version"],
                    "eligible": sum(row["eligible"] for row in report["matrix"].values()),
                    "control_failed": sum(row.get("control_passed") is False for row in report["trials"])})
print(json.dumps(results, indent=2))
