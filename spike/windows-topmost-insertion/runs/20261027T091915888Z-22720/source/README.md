# Windows topmost insertion spike

**SCHEMA 6 PREPARED, NOT RUN ON WINDOWS.** Collect one repetition first, preserve the whole evidence directory, and review independent analysis before running five repetitions. Read `REPORT.md` to distinguish pending questions from verified outcomes.

## Run on Windows

Stop Rust Dome yourself first. Record whether another window manager or PowerToys Always On Top is active. Ordinary-desktop fixtures appear briefly. Private-desktop fixtures must not appear on the interactive desktop. Stop and preserve the evidence if they do.

Copy this directory, including `src` and `tests`, excluding `target`, `runs` and `verification`. From its root:

```sh
cargo run --release -- --repetitions 1
```

After reviewing that capture, use `cargo run --release` for the default five repetitions. `--out NEW_DIRECTORY` and `--repetitions 1..20` are the only public options. Existing output directories are refused. No remote host or transfer operation is configured.

The binary prints the evidence directory at startup and completion. Preserve that entire directory even on failure. Do not overwrite a prior run or normalize captured source files.

## Check the capture

Accept collection as complete only when the supervisor and collector exit codes are zero, cleanup reports zero remaining job processes without errors, the private desktop is closed, and the borrowed ordinary desktop is retained. Inspect every stderr file for individual destruction failures. Zero processes alone does not prove every DestroyWindow succeeded.

Compare the transferred raw file with `raw-native-sha256.json`. An unavailable hash is a provenance gap, not a match. Analyze using the captured analyzer and all its captured Python siblings:

```sh
python3 runs/RUN/source/analyze.py runs/RUN/raw.jsonl > runs/RUN/analysis.json
```

Exit 2 can mean invalid instrumentation, a missing key, an unresolved setup or insertion premise, or a contradicted explicit reference expectation. Read `counts`, per-trial status and the matrix rather than treating every exit 2 as the same failure. A measured band observation can remain useful when its reference expectation is contradicted.

Exit 0 means this bounded schedule has eligible endpoints and passing controls, not a universal Win32 guarantee. Collector exit 0 only means collection finished. Its controls remain pending independent analysis.

Use each old run's captured analyzer for historical evidence. Schema-6 variants do not change what an older capture measured. If a named premise still fails, report that conditional gap rather than expanding the experiment until it passes.
