# Schema 6 ownership follow-up verification

The run establishes both-topmost ownership premises and measures demotion and relative insertion from them. Demoting O differs between desktops, and promotion depends on the tested setup and flags. These are bounded observations on Windows 25H2 build 26200.9711, not general ownership laws or production asynchronous safety.

## Classification

All 245 scheduled attempts are present across 36 scenarios and 49 environment combinations, each repeated five times. Ordinary has 27 combinations and private has 22. Of 235 collected endpoints, 215 observe their intended question and 20 follow a failed both-topmost prefix. Ten setups reject actual initial bands. No schedule entry is missing and no trial is protocol-invalid.

The captured analyzer exits 2 with empty stderr because the run is not all-controls-pass. Its 45 contradicted reference expectations comprise case 22 (10), case 26 (10), private case 31 (5), case 35 (10), and case 36 (10). Contradiction is separate from instrumentation validity and question coverage. Case 22 retains its original expectation.

Native supervisor and collector exit 0. Cleanup reports zero job processes before and after cleanup, no errors, ordinary desktop retained, and private desktop closed. All 24 captured stderr files are empty. These facts do not provide individual DestroyWindow success acknowledgments.

## Complete matrix

Every populated cell represents five repetitions. T means flagged topmost and N means normal. Q means question observed. R means Q with reference_expectation_contradicted. F means measured_operation_failed_prefix, with a contradicted expectation and an unobserved intended question. S means setup_not_run. A dash means not scheduled, not missing. All cells have zero protocol_invalid. Ownership pairs are X/O unless explicitly A/O. `generated-matrix.json` contains full names, counts, bands, fixture order and raw trial anchors.

| Case | Operation | Ordinary | Private |
|---|---|---|---|
| 1 | Normal X After(A) | Q X T | Q X T |
| 2 | NotTopmost X then After(A) | Q X T | - |
| 3 | Topmost X then After(A) | Q X T | Q X T |
| 4 | Topmost X After(normal reference) | Q X N | Q X N |
| 5 | Held A demotion, insert X | Q A N, X T | - |
| 6 | Sole global topmost A | - | Q X N |
| 7 | Sole controlled topmost A | Q X T | - |
| 8 | Displaced X, clean B-A boundary | - | Q X N, moved |
| 9 | Already-topmost X After(A) | Q X T | - |
| 10 | Demote X then After(A) | Q X T | - |
| 11 | Normal X After(normal reference) | Q X N | Q X N |
| 12 | Repeat After(A) | Q X T, unchanged | - |
| 13 | Wait for A demotion | Q X N | - |
| 14 | Held A promotion | Q X N | - |
| 15 | Wait for A promotion | Q X T | - |
| 16 | Queued C, promote A | Q C T, moved | - |
| 17 | Queued C, keep A normal | Q C N, moved | - |
| 18 | Queued C, promote/demote A | Q C N, moved | - |
| 19 | A After(normal X) | Q A N | - |
| 20 | A After(normal P before X) | Q P-A-X, A N | - |
| 21 | A After(topmost P before X) | - | Q P-A-X, A T |
| 22 | Promote X with existing O | R N/T | R N/T |
| 23 | Original owner demotion setup | - | S |
| 24 | Promote O | - | Q N/T |
| 25 | Original owned demotion setup | - | S |
| 26 | Original promote X, After(A) | F N/T | F N/T |
| 27 | A After(X), O owned by A | - | Q A/O N/N, X-O-A |
| 28 | Displaced normal X Bottom | - | Q N, moved to tail |
| 29 | Topmost X Bottom | - | Q N, moved to tail |
| 30 | Promote X before O, demote X | Q N/N | Q N/N |
| 31 | Promote X before O, demote O | Q N/N | R T/N |
| 32 | Promote X before O, X After(A) | Q T/T, A-O-X | Q T/T, A-O-X |
| 33 | Promote X then O, demote X | Q N/N | Q N/N |
| 34 | Promote O then X, demote X | Q N/N | Q N/N |
| 35 | Promote X, NOOWNERZORDER | R T/N | R T/N |
| 36 | Promote X, After(A), NOOWNERZORDER | F T/N | F T/N |

## Ownership evidence

Cases 30 through 32 prove X's actual topmost bit in the four-fixture observation before the O constructor starts. The full reread finishes before creation. O is created owner-local with WS_POPUP | WS_VISIBLE and extended style 0x08000080, without WS_EX_TOPMOST. The five-fixture creation read finds both X and O topmost in all 30 trials. GW_OWNER and process/thread identities agree throughout each lifetime. The parser correctly attributes precreation observations to X, not O.

Case 31 demotes both on ordinary, but only O on private. Private X remains topmost and ends above O in A-B-X-O-P. This is an observed contradiction, not failed setup or a bad call. Native calls are at raw lines 6825, 7714, 8603, 9492 and 10381. Final endpoints are at 6846, 7735, 8624, 9513 and 10402. Ordinary repetition zero has the same controlled starting order A-B-O-X-P but ends N/N at line 1022. The Windows-internal cause is unknown.

Case 32 produces A-O-X-B-P with both flags preserved. O intervenes, so owner-group insertion is not strict target adjacency. Repetition-zero endpoints are 1065 and 6889. Cases 33 and 34 create a normal pair, use opposite promotion orders, and verify both windows topmost before demoting X in all 20 trials. These checks read actual bands rather than infer them from API success.

Original cases 23 and 25 reject all ten setups before treatment. Case 26 moves O below A while X stays normal. Case 36 instead ends directly A-X while O remains normal. These diagnostic endpoints do not recover their intended both-topmost question.

Cases 22/26 begin A-B-P-O-X, while 35/36 begin A-B-O-X-P. The named pairs differ in initial order as well as flags. Case 33's setup X promotion uses flags 19 from A-B-O-X-P and already makes both windows topmost before the later O promotion, unlike case 22. The evidence establishes setup-sensitive native outcomes and a 531-condition result, not clean single-variable 22-versus-35 causal isolation. NOACTIVATE remains present throughout. Case 35 does not promote both windows.

## Movement

Case 8 moves X across stationary P from B-A-P-X to B-A-X-P and stays normal, five times. Case 28 moves normal X across P to the complete global tail, five times. Repetition-zero before/end anchors are 6352/6377 and 6694/6720. These close the earlier no-movement-control limitation without reopening the original promotion, delay, sole-topmost or normal-anchor questions.

## Native-call integrity

No unexpected call, cross-lifetime call, missing required target read, overlapping call or post-endpoint write was found. The independent diagnostic matches all 1,470 deduplicated SetWindowPos calls to source-derived setup and treatment intervals in native timestamp order. Counts are 1,405 with flags 19, 30 with 531, and 35 with 16403. All report success. Twenty treatment records duplicate fixture replies rather than add calls. Captured source has one SetWindowPos execution site and no hidden extra promotion path. Call durations are 5 to 1,797 microseconds.

## Snapshot integrity

All 4,935 settling samples are valid, and every collected endpoint retains its scheduled final five identical full fixture states. The repeat control adds 105 first-settling samples. Seven non-atomic post-release snapshots remain explicitly invalid. Ten four-fixture failure inventories lack scheduled O and remain diagnostic-only, distinct from their valid setup observations. Settling spans are 506.210 to 531.077 ms and gaps are 25.086 to 29.281 ms.

## Provenance

Raw is 36,552,997 bytes and 10,625 lines. Its SHA-256 matches the native post-cleanup hash, and the executable hash matches. All 20 captured source files require CRLF reconstruction in memory to match their manifest hashes. All reconstructed bytes occur in the executable. Nothing was normalized on disk and no captured-source __pycache__ was created.

## Desktop scope

Environment records identify Default and DomeTopmost-12508-1793092839782834. Collector threads and all 21 fixture subprocesses verify desktop assignment and disable IME before window creation. Inventories contain only scheduled fixture-class windows, including all private global enumerations. Source uses CreateDesktopW and SetThreadDesktop, not SwitchDesktop. This is not visual proof. Application self-restacking, focus, notification ordering and production asynchronous safety remain outside scope.

## Files and source anchors

`analysis.json` is captured-analyzer output, with `generated-analyzer-exit.json` and `generated-analyzer-stderr.log`. `generated-independent-audit.py` produces the zero-error `generated-independent-audit.json` and 36-by-2 `generated-matrix.json`. Its exit and stderr are saved separately. The diagnostic retains each trial's raw anchors, calls, creation/setup stages, styles, owner links and endpoint order.

Captured `source/src/native.rs` anchors are lines 130 (constructor), 206 (flags and native call), 267 (native reads), 367 (snapshot timing), 1092 (ordered promotions), 1306 (prefix promotion), 1333 (failed-prefix diagnostic), 1481 (precreation sequence), and 1561 (creation premise). `source/protocol_v6.py` validates timestamp-ordered calls, immediate reads and four-to-five identity transitions. No native execution, build, source edit, manifest edit or preservation-note write occurred during analysis.
