# Windows topmost insertion experiment

**SCHEMA 6 PREPARED, NOT RUN ON WINDOWS.** The last verified schema-5 run answered the sole-global and predecessor-anchor questions, but ownership and two unchanged-position controls still need bounded follow-ups. The question matrix is below. Operator commands are in `README.md`.

## Verified native evidence

Run `runs/20261027T003013400Z-24124` measured Windows 25H2 build 26200.9711. Its 165 scheduled trials produced 155 collected trials and 150 eligible observations. Five eligible owner-promotion controls contradicted their reference expectation. Ten both-topmost setups failed without treatment, and five later insertion premises failed. There were 145 eligible observations without a failed control, not an all-controls-pass result.

The run's `REPORT.generated.md`, `VERIFICATION.generated.md`, analysis, raw data and captured source remain immutable. The native raw SHA-256 is `07190550967db31935c341061c0caff5ffc97509f190226770ee8dd304eba431`. The independent native audit already checked actual target handles, signed HWND constants, owner relationships, fixture process/thread identity, exact window inventories and absence of later demoting writes. This revision does not repeat that audit or replace its findings.

| Question | Last verified result | Remaining experiment |
| --- | --- | --- |
| Interior relative insertion | Normal X became topmost with B below A on both desktops | Retain comparison controls. No new fixture needed. |
| True sole-global topmost | X moved after sole A and stayed normal, five times | Cleared for this fixture and build. |
| Clean topmost boundary | X stayed normal after A, but already occupied that position | Row 8 now requires X after normal P before treatment, then crossing P after A. Its explicit v6 variant does not relabel old evidence. |
| Normal predecessor anchor | P-A-X formed with A normal, five times | Cleared for synchronous owner-thread placement with an observed predecessor. |
| Topmost predecessor anchor | P-A-X formed with A still topmost, five times | Cleared for the measured private boundary. |
| Queued placement | C crossed stationary P only after release, with bands depending on A's intervening state | Retain fixed sampling and movement controls. |
| Owner promotion with O already present | X remained normal and O became topmost, five times | Repeat original rows 22 and 26 on ordinary desktop. Add paired flag diagnostics. |
| Explicit owner and owned demotion | Required X/O topmost setup failed ten times | Rows 30 and 31 promote X before O creation, verify both flags, then demote X or O. Rows 33 and 34 compare the two explicit promotion orders. |
| Relative insertion of topmost owner X | Row 26 executed After(A) after a failed promotion premise | Keep that sequence as measured diagnostic. Row 32 requires both topmost before After(A). |
| Normal Bottom movement | X remained at the tail it already occupied | Row 28 now starts X normal above stationary P, then requires complete-chain tail and crossing P. |
| Topmost Bottom and A-owned relative demotion | X moved to tail and cleared topmost. A/O demoted into X-O-A | Cleared for these measured fixtures. Strict adjacency remains separate from group order. |

Run `runs/20261026T090428503Z-11092` remains historical schema-4 evidence. Its lowest-flagged A was not a clean contiguous boundary. Its raw hash remains `62de6552ce92933a0ca37dcef2fd7eefe578457c995dc575cb98205a3abecb4f`. Current legacy analysis preserves old interpretation rather than importing schema-6 premise rules.

## Ownership hypothesis and bounded discrimination

The cause of X remaining normal while O became topmost is unknown. Creation-order and SWP_NOOWNERZORDER comparisons can distinguish fixture-sensitive behavior, but neither source inspection nor the public API documentation explains the verified result.

[SetWindowPos](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowpos) says that promoting a non-topmost window promotes its owned windows, and that a topmost owner cannot own a normal window. [Window Features](https://learn.microsoft.com/en-us/windows/win32/winmsg/window-features#owned-windows) describes popup ownership through the constructor's parent argument and the owned-above-owner constraint. Neither source supplies an exception explaining the explicit Topmost call targeting X. This is not an assertion of a Windows bug.

The current constructor supplies the real owner HWND to CreateWindowExW for a visible WS_POPUP with WS_EX_NOACTIVATE and WS_EX_TOOLWINDOW, without WS_EX_TOPMOST. O shares its owner's process and thread. The window procedure delegates to DefWindowProcW and does not rewrite WINDOWPOS or issue another placement. Schema 6 records the constructor arguments, native caller and interval alongside independent owner/owned observations.

Creation order distinguishes inheritance from promotion of an existing ownership group. Rows 30 to 32 reset unowned X normal, promote X, record its actual flag, create O without explicit topmost style, and observe both independently. The collector rejects failed both-topmost creation. No subsequent X setup promotion repairs inheritance. P, B and A are then arranged, and both bands are checked again before treatment.

Rows 33 and 34 reset X before creating O, then use predetermined X-Topmost/O-Topmost and O-Topmost/X-Topmost orders. They do not adapt until green. Each call has its own observation. Both rows attempt owner demotion only from an independently verified both-topmost state. If inheritance and both explicit orders fail, the demotion question remains conditional rather than silently changing the fixture.

SWP_NOOWNERZORDER (0x0200) is documented to leave the owner window's Z position unchanged. It is not documented as a topmost-propagation switch. Rows 35 and 36 add only that flag to row 22's treatment and row 26's explicit promotion plus relative treatment, respectively. Setup flags remain unchanged. Each has ordinary/private and flags-off comparisons. NOACTIVATE remains set everywhere. A measured difference can identify flag sensitivity, not prove a Windows-internal mechanism.

## Protocol metadata

The raw session declares the complete schedule and maps each numbered row through `case_names`. Every trial declares environment, repetition, fixture slots, setup variant and treatment flags. Both desktops include the original owner-promotion and relative-insertion sequences and every new ownership variant.

| Native operation | Bits | Scope |
| --- | --- | --- |
| NOMOVE, NOSIZE, NOACTIVATE | 19 | All setup and ordinary synchronous placements |
| Add ASYNCWINDOWPOS | 16403 | Existing external queued placement cases only |
| Add NOOWNERZORDER | 531 | Row 35 treatment, row 36 explicit promotion and treatment only |
| O constructor style | 0x90000000 | WS_POPUP and WS_VISIBLE |
| O constructor extended style | 0x08000080 | WS_EX_NOACTIVATE and WS_EX_TOOLWINDOW, no explicit WS_EX_TOPMOST |

Rows 23 and 25 keep their original setup histories. The displacement variants of rows 8 and 28 have distinct schema-6 identifiers. Schema-5 analysis is frozen in `analyze_v5.py`. Only schema 6 dispatches to the new analyzer and setup audit.

The source capture embeds current files at compilation, including both analyzers and their tests. The native raw hash is written only after collection exits and the job has zero processes. The collector never switches desktops or starts Explorer. Ownership remains same-process and same-thread. Ordinary observations remain local except proof stages. Cleanup and time limits retain the existing bounded supervisor and desktop handling.

## Eligibility and attribution

Schema 6 distinguishes reference-expectation contradiction from invalid instrumentation. An eligible owner-promotion endpoint can fail its explicit reference control without losing its measured bands. Original row 26 and its flag variant retain endpoints under `measured_operation_failed_prefix` when X/O are not both topmost before insertion. Those endpoints do not answer the intended topmost-owner question.

The independent analyzer checks the full permitted setup write sequence, O constructor ownership/styles, creation ordering, observation bounds, measurement writes and writes after the endpoint. Extra writes invalidate causal attribution even when final bands match expectations. Separate counters report scheduled keys, questions observed, setup not run, protocol invalid, measured operations with failed prefixes, not-run keys and contradicted reference expectations. A scientific conclusion does not require every native outcome to match documentation.

Rows 8 and 28 use P as a movement witness, without another framework or unrelated-window writes. Other rows retain their operation sequences. Row 12 explicitly tests idempotence. Rows 2 and 11 intentionally begin with a normal-band request or normal target, but their subsequent relative placement has a distinct controlled destination. Band-retention questions do not claim execution from an unchanged position alone. A targeted review of captured initial/settled neighbors found unchanged predecessors in rows 15 and 27 as well as rows 8 and 28. Rows 15 and 27 still changed the target's measured band, so neither was used as an unchanged-position movement proof.

All 21 post samples remain fixed, with only the final five required to form a valid consistent plateau. Earlier invalid non-atomic snapshots remain invalid. There is no early exit, replacement sample or extended wait. Full desktop snapshots remain restricted to global premises and Bottom endpoints.

## Verification and first-run limits

Local checks establish parser and compilation coverage, not Windows outcomes. Host tests check ownership/flag admission, fixture relationships, global/local observations and the fixed matrix. Python fixtures check parser acceptance and rejection, every scheduled environment/case key, repetition keys, setup rejection, constructor contamination, unscheduled writes, movement premises and final-five resolution. Synthetic outcomes are parser fixtures, not predictions or simulations that establish Windows behavior.

The verification logs under `verification/v6-*` record host tests, formatting, host Clippy and Windows GNU check/Clippy with warnings denied. Read-only legacy replay compares current schema-5 analysis with the captured analyzer, without writing into the preserved run.

The next native run must establish which ownership setup variants actually work, whether ordinary/private outcomes differ, whether the flag pair changes behavior, whether rows 8 and 28 move across P, and whether cleanup and native provenance remain correct with the expanded schedule. If a named premise still fails, report that conditional gap. Do not add activation, input-queue attachment, external application ownership, minimized windows, virtual desktops or notification tests to obtain a passing suite.
