# Windows topmost insertion results

Normal coordinator-owned X became topmost after synchronous `After(A)` with A topmost and B below it, in all five repetitions. Explicit `NotTopmost` immediately beforehand did not prevent promotion. A normal-state query before posting asynchronous `After(A)` also failed to guarantee normal C when A was promoted before C resumed.

The risk premise behind checking topmost state is supported, but a prior read is not a barrier for a queued write. Explicit `Topmost` was not necessary to promote normal X in these fixtures. These are measurements on one desktop, not universal Windows guarantees. Band results and immediate-adjacency controls must be separated.

## Provenance and collection

Run `20261026T090428503Z-11092` lasted from 2026-10-26 09:04:28.504 to 09:05:26.233 UTC. `environment.json` records 25H2, build 26200.9711, registry product name `Windows 10 Pro`, Console session and Default desktop. Runtime PATH tools report Rust/Cargo 1.96.0 and the MSVC host, not compiler provenance. Build logs were not captured.

The executable SHA-256 matches `hashes.json`:

`3e4f502c199504c641c5677b480fe36acfffd733e148e4374c257e3392f36714`

All 13 pulled source files equal current source. Their LF bytes differ from the Windows manifest. Converting LF to CRLF in memory matches every manifest hash, and every reconstructed complete byte sequence occurs in the executable. `provenance.generated.json` records each pulled-source hash, manifest hash and comparison. Nothing was normalized on disk.

Preserved `raw.jsonl` has 4,370 lines and SHA-256:

`62de6552ce92933a0ca37dcef2fd7eefe578457c995dc575cb98205a3abecb4f`

The captured analyzer streamed raw input into `analysis.json`. Exit 2 and empty stderr are saved in `analysis.generated.exit.json` and `analysis.generated.stderr.log`. Native collection and supervisor exited 0 within 900 seconds. Cleanup records zero remaining owned processes and no errors. Captured stderr files are empty.

Of 105 attempts, 90 passed setup and collected treatment observations, while 15 were rejected during setup. There were 35 asynchronous posts, 80 synchronous treatment calls, 1,890 final-settling samples and 105 first-settling samples for repeated insertion. Analysis accepts 89 complete observations, including 25 failed adjacency controls. The other 64 have no failed explicit control. One additional collected trial has an inconsistent immediate snapshot, not a rejected setup.

## Matrix and raw anchors

N means normal and T means `WS_EX_TOPMOST` set. Bands show target state before insertion, immediately afterward, then final settlement. For C, the first state precedes posting and the immediate state follows release acknowledgement. Every row was attempted five times.

All anchors refer to `raw.jsonl`, repetition 0. Add **872 × repetition**, for repetitions 0 through 4, to get each corresponding record. This offset was verified throughout the matrix. Triplets identify before-insertion/query, immediate and final records. P is normalA reused as predecessor.

| Case | Operation or condition | Target outcome | Complete observations | Raw anchors |
| --- | --- | --- | --- | --- |
| 1 | Normal X `After(A)`, B below A | N → T → T, 5/5 | 5 | 23, 24, 46 |
| 2 | X `NotTopmost`, then `After(A)` | N → T → T, 5/5 | 5 | 64, 65, 87 |
| 3 | X `Topmost`, then `After(A)` | T → T → T, 5/5 | 5, adjacency fails | 105, 106, 128 |
| 4 | Topmost X `After(normalA)` | T → N → N, 5/5 | 5, adjacency fails | 144, 145, 167 |
| 5 | A demotion queued, insert while A held | N → T → T, 5/5 | 5 | 188, 189, 215 |
| 6 | Sole global topmost A | No treatment | 0, setup rejected | 229 to 231 |
| 7 | Sole controlled topmost A | N → T → T, 5/5 | 5 | 246, 247, 269 |
| 8 | Actual bottommost topmost A, B above | N → T → T, 5/5 | 5 | 289, 290, 312 |
| 9 | Already-topmost X `After(A)` | T → T → T, 5/5 | 5 | 328, 329, 351 |
| 10 | Just-demoted X `After(A)` | N → T → T, 5/5 | 5 | 369, 370, 392 |
| 11 | Normal X `After(normalA)` | N → N → N, 5/5 | 5, adjacency fails | 408, 409, 431 |
| 12 | Repeat X `After(A)` | First call promotes, repeat unchanged, 5/5 | 5 | 447, 448, 471 to 494 |
| 13 | Wait until A demotion observed | N → N → N, 5/5 | 5 | 519, 520, 542 |
| 14 | A promotion queued, insert while A held | N → N → N, 5/5 | 5 | 563, 564, 590 |
| 15 | Wait until A promotion observed | N → T → T, 5/5 | 5 | 615, 616, 638 |
| 16 | Queue C after normal A, promote A while C held | N → N → T in 4 clean sequences. Fifth immediate spans transition, final T | 4, plus separate fifth endpoint | 661, 670, 691 |
| 17 | Queue C, keep A normal | N → N → N, 5/5 | 5, adjacency fails | 714, 720, 741 |
| 18 | Queue C, promote then demote A before release | N → N → N, 5/5 | 5 | 764, 776, 797 |
| 19 | Topmost A `After(normal X)` | T → N → N, 5/5 | 5, adjacency fails | 813, 816, 838 |
| 20 | A above X via normal P | No treatment | 0, setup rejected | 854 to 856 |
| 21 | A above X via topmost P | No treatment | 0, setup rejected | 878 to 880 |

Each executed row has the same target band in all its 105 final-settling samples. Repeated insertion preserves X's band and both neighbors. Case 19 places A below X, not above it.

## Companion and boundary findings

The ten predecessor rejections are exactly cases 20 and 21, all five repetitions. Each records hidden `IME` 328300 immediately above X 984656, owned by X. Hidden `MSCTFIME UI` 262774 is owned by that IME window. Case 20 has P → MSCTFIME UI → IME → X, not P → X. Case 21 also has X's companions above X while P is elsewhere. These are the anticipated conditional companion obstructions, now identified by class and owner.

Raw final neighbors independently confirm the failed controls. Cases 3, 4 and 11 end reference → X companions → X. Case 19 ends X → A's MSCTFIME UI → A's IME → A. All five control scenarios meet their expected band result. Their adjacency failures are not band-measurement failures.

Case 17 needs a narrower interpretation. C's neighbors stay unchanged from before posting through final settlement in every repetition. Its predecessor belongs to C's process/thread, but C's companion classes and owners were not captured. The initial chain already puts C below A with intervening same-process windows. The control observes normal C but does not prove queued execution through an observable move. Case 18 likewise does not establish a release-time move merely from its normal endpoint.

Case 6 has 51 flagged topmost windows including A, with 48 hidden, in every before scan. It supplies no negative insertion evidence. Case 7 is sole controlled topmost only. Case 8 meets its lowest-globally-flagged predicate in all repetitions. Both global before and before-insert chains prove B above A and zero flagged windows below A. However, 129 normal windows, including X and normalA, sit above A. This is not a clean contiguous topmost/normal boundary. Local evidence was not substituted for the global predicate.

## Timing and protocol audit

All 605 distinct captured native calls succeed, maximum recorded duration 1.965 milliseconds. `SetWindowPos` itself has no per-call timeout. Process supervision, finite fixture lifetimes and the three-second hold guard bound the experiment. Lifetime expiry is not proof that a hold ended.

Captured `source/src/native.rs` uses synchronous owner-thread flags 19 and external flags 16403, adding `SWP_ASYNCWINDOWPOS` (216 to 255, 836 to 904). The real window loop stops pumping before acknowledging hold and acknowledges release before pumping resumes (605 to 660). The caller checks acknowledgements (548 to 574). All 35 holds have matching releases, lasting 2.026 to 105.118 milliseconds, without timeout. Cases 5/14 retain the old A state through observations over 100 milliseconds after insertion, then settle A into its requested band.

Raw lines 1 to 8 establish X on coordinator process/thread 11744/18660, A on 19164/11740, B on 5252/11544 and normalA on 15236/12344. Named fixtures are visible and unowned. Every C has a fresh separate process/thread.

Case 1 reads normal X through 09:04:28.813478. The call runs at .813892 to .814550, and the immediate read at .814550 to .814588 finds topmost X. Final observation at 09:04:29.338135 to .338205 agrees. The call record is emitted after the immediate record, so envelope order alone would misorder them.

Case 16 repetition 0 reads normal A at 09:04:37.778535 to .778549, posts C at .778799 to .778818, observes promoted A at .781504 to .781615, and sees normal C before release. Release acknowledgement is .783052. C is still normal at .783074 to .783091, then topmost at .783412 to .783426 and throughout settlement. Lines 660 to 671 establish this order.

Repetition 1's line 1542 spans C changing normal to topmost within a 50-microsecond snapshot, with A/X neighbors also changing. The analyzer correctly rejects a complete immediate observation. Lines 1533 to 1541 establish query/post/promotion/hold order. All 21 valid samples at 1543 to 1563 show topmost C. Line 194 is a separate inconsistent optional after-release snapshot in case 5. Its required held and settling observations remain valid. Neither inconsistent snapshot is atomic evidence.

Snapshot spans range from 7 to 701 microseconds. Settling spans are 514.112 to 527.906 milliseconds, with gaps of 25.181 to 27.541 milliseconds. Owned application targets, self-restacking, focus behavior and notification ordering were not tested. This analysis changed only generated files inside this run. No new native run, build, source edit or memory update was performed.
