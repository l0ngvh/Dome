# Windows topmost insertion spike

**SCOPED CAPTURE PREPARED, NOT RERUN.** The locally preserved first Windows run stopped all 105 trials in setup and produced no treatment evidence. The collector still needs a new native run. See `REPORT.md` for the capture protocol, evidence limits and local checks.

## Scenario matrix

A and B are source windows. X is the overlay. normalA is the normal reference, reused as predecessor P in the anchor cases. C is an additional source window for race trials. Each named fixture is unowned, but hidden owned companions can exist. `After(A)` means the real `SetWindowPos` insert-after argument A.

Unless a row says otherwise, A and B start topmost with B below A, normalA and X start normal, and unrelated windows remain untouched. The row numbers match `case_names` in the raw session record.

| # / Scenario | Required starting condition | Operation | Uncertainty or control |
| --- | --- | --- | --- |
| 1. Normal X below A, B below A | Default fixture arrangement | X `After(A)` | Does normal X enter topmost when another controlled topmost window is below A? |
| 2. Explicitly normal X below A | Default, X already normal | X `NotTopmost`, then `After(A)` | Does an explicit normal request prevent subsequent promotion? |
| 3. Explicit topmost entry | Default | X `Topmost`, then `After(A)` | Positive band-and-placement control for the two-call sequence. |
| 4. Topmost X after normalA | X starts topmost | X `After(normalA)` | Normal-reference demotion control. |
| 5. Demotion posted, insert before release | A topmost, A's actual pump held | Post asynchronous A `NotTopmost`, read flag, X `After(A)`, release A | What happens when demotion is queued but A is still observed topmost? |
| 6. True sole global topmost | A is the only flagged topmost window on the enumerated desktop, B normal | Normal X `After(A)` | Does insertion differ at a genuinely otherwise empty topmost band? Hidden windows count. |
| 7. Sole controlled topmost only | A is the only topmost fixture, B normal, unrelated topmost windows allowed | Normal X `After(A)` | How does this desktop control compare with the strict global-sole case? It is not evidence of an empty global band. |
| 8. Actual bottommost topmost A | B above A, no flagged topmost window anywhere below A | Normal X `After(A)` | Does crossing the lower boundary differ from inserting above another topmost? |
| 9. Already-topmost X | X starts topmost | X `After(A)` | Does an existing topmost window retain its band and reach A? |
| 10. Just-demoted X | X starts topmost | X `NotTopmost`, observe normal, then `After(A)` | Does immediately preceding demotion change the insertion result? |
| 11. Normal-to-normal placement | X and normalA normal | X `After(normalA)` | Ordinary normal-band placement control. |
| 12. Repeated identical insertion | Default | X `After(A)`, observe convergence, repeat | Are band and immediate neighbors unchanged by the second call? |
| 13. Demotion observed before insertion | A topmost, pump held | Post A `NotTopmost`, release, observe A normal, X `After(A)` | Compare with row 5 after an observed native transition. |
| 14. Promotion posted, insert before release | A normal, pump held, B topmost | Post A `Topmost`, read flag, X `After(A)`, release A | The inverse ordering problem on entry into topmost. |
| 15. Promotion observed before insertion | A normal, pump held, B topmost | Post A `Topmost`, release, observe A topmost, X `After(A)` | Compare with row 14 after actual promotion. |
| 16. C queued against observed-normal A | A and C normal, C pump held | Read A normal, post C `After(A)`, promote A, observe topmost, release C | Can a normal-state check become stale before another process applies its queued placement? |
| 17. C normal-reference control | A and C normal, C pump held | Read A normal, post C `After(A)`, leave A normal, release C | Control that queued external placement settles normal directly after A. |
| 18. A returns normal before C release | A and C normal, C pump held | Post C `After(A)`, promote A, observe, demote A, observe, release C | Does returning A to normal before C runs change the outcome from row 16? |
| 19. A below normal overlay | A topmost, X normal | A `After(X)` | Demotion control that deliberately puts A **below** X, not the desired border arrangement. |
| 20. A above X through normal P | A topmost, normal P immediately above normal X | A `After(P)` | Does A become normal and land above X? The API anchor is P, not a fictional insert-before-X operation. |
| 21. A above X through topmost P | P topmost, immediately above normal X, no flagged topmost below P | A `After(P)` | Does the same above-X arrangement retain topmost when its real predecessor is topmost? |

The anchor cases measure mechanisms. They do not approve a stacking design. Rows 3, 4, 11, 17, 19 and 20 have explicit expected band-and-adjacency controls. Other rows report the observed outcome without asserting the disputed result as a setup premise. Above-X placement and band membership are reported separately.

## Eligibility and evidence

Global boundary checks include hidden and shell windows. The spike never demotes unrelated windows to make a boundary eligible. A hidden flagged window outside a contiguous band can make boundary trials ineligible. The positive A/B cases do not require an otherwise empty desktop.

`analyze.py` independently recomputes setup eligibility, native call ordering and endpoints. An API return or release acknowledgement is not proof a queued write landed. Incomplete or inconsistent observations are rejected rather than counted as successful trials.

The raw stream retains setup operations, every attempted trial, failed native calls and ineligible reasons. Ordinary observations cover the fixtures and their local ordering evidence rather than the whole desktop. Local evidence is rejected if any fixture identity or state differs between the full reads before and after neighbor traversal. Read an unknown relative order as missing proof, not as an observed placement failure. Local observations cannot establish a global boundary.

Full desktop observations remain available for the strict global-premise cases. Companion diagnostics are limited to fixture-process windows at predecessor checks. No window titles are collected. Smaller capture files do not make an ineligible setup valid.

Use the observation and API timestamps when interpreting order, not the order records appear in the file. Requested sleeps do not guarantee sampling cadence. The analysis reports actual read spans, gaps and hold timing. See `REPORT.md` for the complete protocol and validation details.

Rows 20 and 21 require P to be X's actual immediate predecessor. Hidden companions between them make these rows ineligible, even when setup calls return success. Failure of these rows does not stop the remaining matrix.

## Run on Windows

Use your interactive Windows desktop and a current Rust toolchain. Stop Dome yourself before running, and record whether another window manager or PowerToys Always On Top is active. Leave other applications alone. The spike does not change settings or stop unrelated processes.

Copy this entire directory, including `tests`, excluding `target`, `runs` and `verification`, to Windows. From the spike directory in any terminal, collect the unchanged 21-case matrix with five repetitions:

```sh
cargo run --release
```

For an optional one-repetition preflight instead:

```sh
cargo run --release -- --repetitions 1
```

Cargo builds only this isolated crate. No PowerShell script is needed. The binary creates a new `runs/<UTC-timestamp>-<process-id>/` directory relative to the working directory and prints its absolute path at startup and on completion or a handled failure. Optional `--out NEW_DIRECTORY` chooses a different new directory. Existing directories are refused, even if empty. Preserve the whole directory even if collection fails or a scenario is ineligible. No remote host or transfer procedure is configured.

The directory contains the source bytes embedded at compilation, SHA-256 source and running-executable hashes, a copy of the executable, and Windows registry build/revision, desktop, session and runtime toolchain metadata. Missing metadata is marked unavailable. Toolchain queries describe tools on the run-time `PATH`, not compiler provenance. Cargo build output stays in the terminal. Build failures happen before a run directory exists, and the binary cannot impose a build timeout.

The collector returns 0 after collection and process cleanup succeed, or 1 on a handled collection, capture or cleanup failure. A successful collector exit is not a verdict on trial eligibility or insertion behavior. The launcher prints setup-accepted, collected and rejected counts, rejection reasons, treatment calls, asynchronous posts and settling samples. These are also saved in `eligibility-summary.json`. Zero treatment calls and zero posts produce an explicit `NO TREATMENT EXECUTED` warning. A summary-read failure is reported separately and does not turn collection completion into a verdict.

Analyze the returned run with its captured analyzer to keep protocol provenance explicit:

```sh
python3 runs/RUN/source/analyze.py runs/RUN/raw.jsonl > runs/RUN/analysis.json
```

Exit code 2 means an incomplete or malformed collection, at least one ineligible trial, or a failed explicit control. Expect that exit if hidden shell windows prevent the strict sole-global case. Read the per-case results rather than discarding eligible observations from other rows. Failed explicit controls retain their measured immediate and settled band and adjacency facts. Exit code 0 means complete eligible observations and passing explicit controls, not that every disputed insertion promoted its target. The current analyzer accepts schemas 2, 3 and 4, not the old five-case format. Use each run's embedded analyzer when reproducing its original verdict. Always preserve prior run directories.

## Stop and cleanup

The binary supervises a collector process with a limit of 180 seconds per requested repetition. That is a safety ceiling, not an expected duration. Admission stops 20 seconds earlier. Main fixtures expire 30 seconds after the collection limit, each race C expires after 30 seconds, and every pump hold expires after three seconds. Cleanup attempts release held pumps even after a trial error.

Collection refuses to start if process containment cannot be established. On exit or timeout the binary terminates its remaining collection processes and records the observed process count. Windows also requests their termination if the supervisor is forcibly terminated. Cleanup targets only the spike's own processes. Forced termination, power loss or disk errors can prevent final records from being written. Preserve partial evidence rather than rerunning into the same directory.

After a run, inspect all fixture stderr files, `stderr.log`, `cleanup.json`, `collection-exit.json` and `exit.json`. Keep them with `raw.jsonl`, `environment.json`, `hashes.json`, the executable and `source`. A cleanup marker or zero remaining processes is not proof that every native destruction succeeded.

## Evidence limits

Snapshots are non-atomic. Results would apply only to the measured desktop, Windows build, unowned fixtures and exact operation order. Owned-window rules, real application self-restacking, activation/focus rules, `HWND_TOP`/`HWND_BOTTOM`, minimized windows, virtual desktops, rendering and reorder notifications are outside this bounded matrix. No result here could by itself establish a window-manager ordering barrier or a universal Win32 guarantee.
