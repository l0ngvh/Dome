# Windows topmost insertion spike

**SCHEMA 7 PREPARED, NOT RUN ON WINDOWS.** The next capture is focused on fresh-fixture ownership comparisons, not another insertion and latency matrix. Read `REPORT.md` for the known confound and the limits of attribution.

## Run on Windows

Stop Rust Dome yourself first. Record whether another window manager or PowerToys Always On Top is active. Ordinary-desktop fixtures appear briefly. Private-desktop fixtures must not appear on the interactive desktop. Stop and preserve the evidence if they do.

Copy this directory, including every current Python file, `Cargo.toml`, `Cargo.lock`, `src` and `tests`. Exclude `target`, `runs`, `verification` and `__pycache__`. The new `paired_native.rs`, `paired_protocol.rs`, `analyze_v7.py`, `protocol_v7.py` and `test_analyze_v7.py` must accompany the existing files. From its root:

```sh
cargo run --release -- --repetitions 1
```

Preserve the whole evidence directory and review independent analysis before running `cargo run --release` for the default five repetitions. This is a predetermined schedule, not an instruction to retry failed pairs until they match. `--out NEW_DIRECTORY` and `--repetitions 1..20` remain the only public options. Existing output directories are refused. No remote host or transfer operation is configured.

The binary prints the evidence directory at startup and completion. Preserve that entire directory even on failure. Do not overwrite prior runs or normalize captured source files.

## Focused schedule

Both desktops run these five variants per repetition:

| Family | Variants | Required start | Treatment |
| --- | --- | --- | --- |
| Owner promotion | `fresh_owner_promotion_default`, `fresh_owner_promotion_noownerzorder` | Fresh normal X, then O created while X is normal | X Topmost with flags 19 or 531 |
| Relative insertion | `fresh_owner_after_a_default`, `fresh_owner_after_a_noownerzorder` | Fresh X promoted with flags 19 before O creation, both verified topmost | X After(A) with flags 19 or 531 |
| Owned demotion | `fresh_owned_demotion` | Same precreation promotion, both verified topmost | O NotTopmost with flags 19, compared across desktops |

Every trial requires the actual controlled order A-B-O-X-P before treatment. Relative insertion therefore starts displaced from After(A). The session declares every case, environment, repetition, pair identifier, window lifetime identifier and execution position before collection starts.

## Check the capture

Accept collection as complete only when supervisor and collector exit codes are zero, cleanup reports zero remaining job processes without errors, the private desktop is closed, and the borrowed ordinary desktop is retained. Check per-trial retirement records and every stderr file. Zero processes alone does not prove every explicit DestroyWindow succeeded.

Compare the transferred raw file with `raw-native-sha256.json`. An unavailable hash is a provenance gap, not a match. Analyze using the captured analyzer and all its captured Python siblings:

```sh
python3 -B runs/RUN/source/analyze.py runs/RUN/raw.jsonl > runs/RUN/analysis.json
```

For schema 7, read `pairs`, `trials`, `counts` and `malformed_or_protocol_errors`. A pair is unresolved if either member lacks valid instrumentation, its intended premise, treatment or a stable valid final five, or their actual setup signatures differ. A flag pair also requires unchanged recorded external boundary context. Individual observations remain available when pairing fails.

Retirement is checked independently of the measured endpoint, including partial initialization. Missing, failed or out-of-order retirement is a fatal admission error across both desktops. Later measurements remain available with `contaminated_admission`, but cannot support matched pairs. An invalid endpoint with verified retirement does not contaminate later admissions. `setup_not_run` requires a valid observed prefix with a failed band premise. `setup_failed` requires recorded initialization failure and verified cleanup. Corrupt or missing reached-stage evidence is `protocol_invalid`, regardless of the collector's error label.

Exit 0 means all scheduled pairs meet these bounded checks. It does not require a particular Windows outcome and does not prove a Windows-internal cause. Exit 2 means some protocol, coverage, cleanup or pair-comparability question remains unresolved. Collector exit 0 only means collection finished, not that the pairs matched.

Schemas 4, 5 and 6 retain their existing analyzer interpretation, including contradicted controls. Use each old run's captured analyzer for historical evidence. Do not infer new pair controls from old results.
