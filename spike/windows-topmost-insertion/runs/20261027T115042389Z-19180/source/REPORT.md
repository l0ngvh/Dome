# Windows topmost insertion experiment

**SCHEMA 7 PREPARED, NOT RUN ON WINDOWS.** Native runs answered the original insertion and delayed-placement questions within their measured bounds. The remaining ownership comparisons need fresh fixtures and matched construction histories. This revision prepares those comparisons only. Operator commands are in `README.md`.

## Verified native evidence

The three preserved runs measured Windows 25H2 build 26200.9711. Their raw files, captured source, native hashes and generated reports remain unchanged.

| Run | Scope | Native raw SHA-256 |
| --- | --- | --- |
| `20261026T090428503Z-11092` | Schema 4, original insertion and delayed-placement evidence | `62de6552ce92933a0ca37dcef2fd7eefe578457c995dc575cb98205a3abecb4f` |
| `20261027T003013400Z-24124` | Schema 5, clean boundary, predecessor anchors, queued movement and initial ownership gap | `07190550967db31935c341061c0caff5ffc97509f190226770ee8dd304eba431` |
| `20261027T091915888Z-22720` | Schema 6, ownership follow-up and displaced boundary/Bottom controls | `afb0d2692693951d2b5ba1e3f99305080cf80d126299f1f1d98d03ee7bfa97a3` |

Schema 4 showed normal X becoming topmost after insertion inside the topmost group. An earlier NotTopmost request did not prevent that outcome. Querying normal A also did not prevent a queued insertion from promoting C after A changed. Its lowest-flagged-A setup was not a clean contiguous boundary.

Schema 5 established the sole-global-topmost result, both predecessor-anchor alternatives and C crossing stationary P after release. It recorded 165 scheduled attempts, 155 collected trials and 150 eligible observations. Five eligible owner-promotion controls contradicted their reference expectation. Ten both-topmost setups failed without treatment, and five later insertion premises failed.

Schema 6 accounted for all 245 scheduled trials. It produced 235 collected endpoints and 215 intended questions observed. Twenty operations had failed intended prefixes, ten setups were rejected and no trial was protocol-invalid. Forty-five reference expectations were contradicted. This was not an all-controls-pass run.

Promoting X before O creation established both topmost flags in all thirty creation sequences across the two desktops. Demoting X then demoted both X and O. Demoting O demoted both on the ordinary desktop, but left X topmost on the private desktop, five times each. Relative insertion of the verified topmost owner after A preserved both flags and produced A-O-X-B-P. Displaced clean-boundary and normal Bottom trials visibly crossed stationary P. These results close the earlier unchanged-position limits without making universal Win32 claims.

## The old flag comparison is confounded

Unequal starting orders and reused-window construction histories prevent flag-only attribution. Schema-6 rows 22 and 26 began with A-B-P-O-X, while rows 35 and 36 began with A-B-O-X-P. Equal setup flags did not make those conditions equal. X was reused, and NotTopmost did not reset its normal-band position.

O in row 26 was created while X was normal. O in row 36 was created while X was topmost after row 35. Later normal flags did not erase that history. Row 33 also promoted both windows with default flags from A-B-O-X-P under a different setup history.

The ordinary/private owned-demotion difference had the same verified both-topmost starting order, but reused windows and surrounding trial history still limit the comparison. Fresh repetition can test whether the desktop-condition difference persists. It cannot isolate inactivity from other desktop differences.

The original cases remain reference material in legacy handlers, protocols and immutable captured collectors. Schema-7 identifiers do not relabel their results. The schema-4, schema-5 and schema-6 analyzer paths retain their existing interpretations.

## Fresh construction and fixed schedule

The focused protocol has five variants on each desktop, ten trials per repetition and fifty at the default five repetitions. Each repetition produces two flag pairs per desktop and one cross-desktop owned-demotion pair. Flag order alternates with predetermined zero-based repetition parity. The raw session declares every member and its construction manifest before collection starts.

Each member creates A, B, P, X and O in that order. A, B and P each have a fresh source process and window. X and O share a fresh owning thread and have fresh HWNDs. The collector process, registered class and environment desktop persist. Reused numeric handles or process/thread IDs belong to a new recorded lifetime, not an earlier fixture.

Every owning thread attaches to its intended desktop and disables IME before its first window. Creation records contain native caller and timestamps, desktop, the actual owner state immediately before creation, requested styles and geometry, actual styles, class and GetWindowRect. O is a visible owned popup without explicit WS_EX_TOPMOST. Inventories must contain exactly the scheduled fixture-process windows.

Owner promotion creates X and O normal, without a promotion prefix. Relative insertion and owned demotion issue one X Topmost request with flags 19 before O creation, then require both actual topmost flags. The prefix never varies with the treatment flag.

Setup then requests P Bottom, B Topmost and A Topmost, all with flags 19. This fixed native construction must yield actual controlled order A-B-O-X-P. It is not a synthetic sort or a NotTopmost reset. Both flag members require equal measured histories, starting bands and order. Failed construction leaves a pair unresolved. O is never omitted to obtain a match.

Owner promotion tests X Topmost, and relative insertion tests X After(A) from below B. Only treatment flags differ, 19 versus 531. NOACTIVATE, NOMOVE and NOSIZE remain set. Owned demotion tests O NotTopmost with flags 19 on both desktops.

O and X receive explicit DestroyWindow calls and immediate absence checks. Every spawned source process must exit, and the owning thread must join before the next member can create anything. Child ownership is retained before readiness validation. Local window ownership is retained and recorded before fallible attribute or desktop reads. Partial initialization records its failed stage and error, then follows the same explicit retirement path. A child that never supplies a valid HWND still requires recorded process exit, not a DestroyWindow call on an unknown handle. Retirement failure stops further fixture admission across both desktops. Job termination, deadlines and private-desktop closure remain bounded. No desktop activation, SwitchDesktop access, input attachment or unrelated-window write is added.

## Independent analysis and attribution

Pair eligibility comes from actual evidence, not the manifest alone. `analyze_v7.py` checks lifetime-scoped HWND roles, independent source process/thread identities, X/O sharing, creation intervals and attributes, ownership, the permitted native call sequence and the full controlled starting order. A mismatched read or extra write invalidates instrumentation even when final flags look plausible.

Each member captures one global premise. The private desktop must contain exactly the five fixtures. Ordinary-desktop foreign windows are allowed. Pair signatures compare controlled order, attributes, bands and owner-group adjacency. Flag pairs also compare recorded foreign boundaries adjacent to fixtures. They do not require unrelated whole-desktop title or class histories to match. Foreign titles are not collected. Changes outside the observed boundary remain a limit.

The global premise is followed by an identity-checked local before-insertion read. Settlement remains twenty-one fixed local samples. Only the final five must be valid and agree on every fixture field, including native neighbor handles. Earlier invalid samples remain recorded and invalid. There are no replacements, extended waits or adaptive repairs.

A matched pair needs both members' valid instrumentation, intended premise, treatment and resolved final five, plus equal setup signatures. Individual observations remain available when pairing fails. Outcomes are separate from reference expectations. Flag attribution is conditional on matched recorded context. The desktop comparison does not isolate activation or a Windows-internal cause. Such a causal test would be a separate, currently unauthorized experiment.

## Verification limits

Local Rust checks and synthetic Python tests establish implementation coverage, not Windows outcomes. Legacy replay compares all three old raw hashes and their analysis output. Source capture embeds the new modules and tests with existing files. Native raw hashing still waits until collection exits and the owned job reports zero processes.

The first schema-7 Windows capture remains pending. It must establish actual starting comparability, ownership outcomes, retirement and native provenance. A failed premise remains an unresolved pair, not permission to activate the private desktop or rerun the original full matrix.
