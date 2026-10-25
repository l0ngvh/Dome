# Schema 5 native Windows results

Run `20261027T003013400Z-24124` closed the sole-global, clean-boundary, predecessor-anchor and queued-movement gaps, but did not pass the full suite. The captured streaming analyzer returned 2 with 150 eligible observations, five failed ownership controls, ten setup rejections and five additional post-setup ineligible trials.

## Provenance and execution

The native raw SHA-256 matches the pulled file exactly:

`07190550967db31935c341061c0caff5ffc97509f190226770ee8dd304eba431`

The executable matches `hashes.json`, SHA-256 `39064479978860dd778ee422c3e59f8fdca3465910a878eddb1ed48531aa40d4`. All 17 captured source files match their manifest hashes after reconstructing CRLF in memory, and those bytes occur in the executable. No normalization was written. Analysis used captured sources, not assumed-equivalent checkout files. `provenance.generated.json` records each comparison.

Native metadata reports 25H2, build 26200.9711, Console session 1 and initial desktop Default. The registry product string is `Windows 10 Pro`. This is one start-time capture, not repeated OS/session sampling. Runtime rustc/cargo identification is not compiler provenance.

Supervisor and collection exited 0 within 2400 seconds, from 2026-10-27 00:30:13.401 to 00:31:48.161 UTC. All 165 scheduled keys remain, five repetitions of 33 combinations across 29 scenarios. Ordinary contributes 18 combinations, private 15. Collector setup acceptance and collection are both 155. Independent analysis accepts 150. There are no missing keys, not-run trials or protocol errors.

## Matrix

T means WS_EX_TOPMOST, N means normal. `A-X` denotes immediate adjacency. Each outcome repeats five times. Counts are eligible observations out of five. Raw anchors give repetition-zero before/treatment/final sample. Complete per-repetition anchors are in `verification.generated.json` and `facts.generated.log`.

| Case | Environment | Native outcome and eligibility | Raw lines |
| --- | --- | --- | --- |
| 1 | ordinary | X N to T, A-X-B (5/5) | 23/27/48 |
| 1 | private | X N to T, A-X-B (5/5) | 4244/4248/4269 |
| 2 | ordinary | Explicit X NotTopmost, then X T, A-X-B (5/5) | 62/68/89 |
| 3 | ordinary | Explicit X Topmost, then X T, A-X-B, control passes (5/5) | 103/109/130 |
| 3 | private | Same band/adjacency control passes (5/5) | 4283/4289/4310 |
| 4 | ordinary | X T to N, P-X, control passes (5/5) | 144/148/169 |
| 4 | private | X T to N, P-X, control passes (5/5) | 4324/4328/4349 |
| 5 | ordinary | Pending A demotion, X remains T after A becomes N, not adjacent (5/5) | 183/192/217 |
| 6 | private | Sole global A T, X stays N and reaches A-X-B (5/5) | 4363/4367/4388 |
| 7 | ordinary | Sole controlled A T, X becomes T directly after A (5/5) | 231/235/256 |
| 8 | private | Clean B T-A T prefix, X stays N directly after A (5/5) | 4405/4409/4430 |
| 9 | ordinary | Already T X remains T, A-X-B (5/5) | 270/274/295 |
| 10 | ordinary | Freshly demoted X returns T, A-X-B (5/5) | 309/315/336 |
| 11 | ordinary | Normal X stays N, P-X, control passes (5/5) | 350/354/375 |
| 11 | private | Normal X stays N, P-X, control passes (5/5) | 4444/4448/4469 |
| 12 | ordinary | Repeat preserves X T and both neighbors (5/5) | 389/393,417/438 |
| 13 | ordinary | A demotion observed first, X stays N, A-X-P (5/5) | 452/465/486 |
| 14 | ordinary | Pending A promotion, X stays N after A becomes T, not adjacent (5/5) | 500/509/534 |
| 15 | ordinary | A promotion observed first, X becomes T, A-X-B (5/5) | 548/561/582 |
| 16 | ordinary | Queried-normal A becomes T, queued C becomes T, A-C-B (5/5) | 609/614/644 |
| 17 | ordinary | A stays N, C moves N across P into A-C-X, control passes (5/5) | 671/676/702 |
| 18 | ordinary | A returns N, C moves N across P into A-C-X, control passes (5/5) | 729/734/767 |
| 19 | ordinary | A becomes N below X, X-A-P, control passes (5/5) | 781/787/808 |
| 20 | ordinary | Normal P-X becomes P-A-X, A becomes N, control passes (5/5) | 824/830/851 |
| 21 | private | Topmost P-X boundary becomes P-A-X, A stays T (5/5) | 4493/4499/4520 |
| 22 | private | X Topmost promotes O only, X remains N. Five failed controls (5/5) | 4535/4539/4560 |
| 23 | private | Required X/O T missing, actual X N/O T, no treatment (0/5) | 4575/4576/4578 |
| 24 | private | O Topmost promotes O, X stays N, control passes (5/5) | 4592/4596/4617 |
| 25 | private | Required X/O T missing, actual X N/O T, no treatment (0/5) | 4632/4633/4635 |
| 26 | private | X Topmost leaves X N/O T, X After(A) moves O after A. Required insertion bands fail (0/5) | 4649/4655/4676 |
| 27 | private | A After(X) demotes A/O, final X-O-A. Group control passes, strict adjacency fails (5/5) | 4693/4699/4720 |
| 28 | private | Normal X stays N at complete desktop tail, control passes (5/5) | 4736/4740/4762 |
| 29 | private | Topmost X becomes N at complete desktop tail, control passes (5/5) | 4776/4780/4802 |

The four ordinary/private comparisons, cases 1, 3, 4 and 11, agree on bands, fixture order and target adjacency. Their unrelated neighbors and desktop contents differ. They are not pooled.

## Previously missing premises

Case 6's complete pre-insertion chain is exactly A T, B N, P N, X N. A is the only globally flagged window and the head. Case 8's is B T, A T, X N, P N, with no normal window above the prefix or hidden flagged window below it. X was already directly after A, so case 8 proves an unchanged normal endpoint, not movement across a boundary. Both premises hold before and immediately before treatment in all five repetitions.

Case 20 has strict normal P-X adjacency before insertion and normal P-A-X afterward. Case 21 starts with complete A T-B T-P T-X N and ends B T-P T-A T-X N. Production placement of an externally owned A is still asynchronous, and P can change between query and execution. Neither result supplies an ordering barrier.

Cases 16 to 18 start with normal A-X-P-C. C stays below stationary P until release, then crosses above P directly after A in all 15 trials. Case 16's immediate snapshots are valid but still show old C position in repetitions 1, 3 and 4. All five final plateaus show C topmost. Case 18 has three invalid immediate snapshots, retained as invalid, followed by valid normal plateaus. The release acknowledgment precedes resuming the pump, not completion of queued placement. Raw movement establishes the later result.

## Ownership failures and remaining intent

The ten setup failures are exactly private cases 23 and 25, repetitions 0 to 4. Setup calls P NotTopmost, B Topmost, A Topmost, then X Topmost with O already created. Every call succeeds. Immediately after X Topmost, X remains N and O becomes T, giving A/B/P/X/O bands TTNNT instead of TTNTT. No later setup call explains this mismatch. Case 23's rejection lines are 4576, 5148, 5720, 6292, 6864. Case 25's are 4633, 5205, 5777, 6349, 6921.

This matches case 22's failed promotion control, not a failed demotion. Case 26 passes initial setup, encounters the same anomaly, then executes its relative call. Its final order is A T-O T-B T-P N-X N. Collector completion does not satisfy the analyzer's required X/O-topmost insertion premise. Case 27 separately establishes relative owner-group demotion with O intervening. Explicit owner and owned NotTopmost demotion from both-topmost X/O remain untested.

All 30 O identities have an explicit actual owner and share its process/thread. Cases 22 to 26 use X handle 984836, process/thread 24360/10652. Case 27 uses A handle 328082, process/thread 10484/19076. Per-trial O handles remain in the diagnostic JSON.

The capture identifies the operation and actual states, but not Windows' internal reason for leaving X normal. Calling this merely a bad fixture would discard a failed control. A minimal candidate repair for cases 23/25 is to seed X topmost before creating O, then verify both flags before treatment. That ordering is untested for X, not a proven fix. Preserve cases 22/26 rather than changing their tested operations.

## Integrity, timing and cleanup

Independent raw checks found 970 distinct successful native calls, including 35 async posts and 145 synchronous treatment calls. All 220 measurement-interval calls match allowed targets, anchors, flags and observation bounds. No explicit X Topmost intervenes in the normal-X insertion cases.

All 155 completed trials have 21 final samples. Case 12 adds 105 first-settling samples. All 160 groups have valid, identical final-five fixture bands, neighbors and owners. Gaps range from 25.072 to 27.776 ms, spans from 506.148 to 526.000 ms. Ten of 4777 snapshots are non-atomic and invalid, none in settling. No samples were replaced.

Both environment threads and all 21 children report successful pre-window ImmDisableIME(0). Private desktop `DomeTopmost-24360-1793061070141550` is verified for its coordinator and three children, access 199, no switch requested. Creation, setup and failure inventories retain fixture-process windows, with no unexpected companions. The capture cannot establish what the user visually saw.

Private CloseDesktop succeeds after teardown. Ordinary retains its borrowed desktop. Cleanup has zero remaining job processes and no errors. Every stderr is empty. This is no logged destruction failure, not separate success acknowledgments for every DestroyWindow or child desktop close.

The native payload has 53 files, 25,318,047 bytes, including 7,095 raw lines in 23,772,442 bytes. No older payload was read for size comparison.

## Gap disposition and follow-up

- Closed premises need no further fixture-repair run for cases 6, 8, 20, 21 or the C movement controls. Retain the unchanged-position limitations for case 8 and normal Bottom, which already started at the tail.
- Open native coverage requires a verified both-topmost X/O setup for cases 23/25, investigation of case 22, and a decision on case 26's unmet insertion premise. Parent verification should precede any fixture edit or evidence archival.
- Open production questions require separate evidence for asynchronous ordering, arbitrary applications, self-restacking, focus and notifications. This run establishes native facts and counterexamples, not universal Windows guarantees.
