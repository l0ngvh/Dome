# Windows topmost insertion spike

**SCHEMA 5 PREPARED, NOT RUN ON WINDOWS.** This bounded suite has 29 scenarios and 33 environment/scenario combinations. It is not complete native coverage until the new fixtures, private desktop, controls and cleanup have been observed. See `REPORT.md` for preserved measurements and verification limits.

## Scenario matrix

A and B are source windows. X is the overlay. normalA is the normal reference, reused as predecessor P in the anchor cases. C is an additional source window for race trials. Baseline fixtures are unowned and independent. Ownership scenarios add O on its owner's process and thread. Unexpected hidden companions remain in the evidence. `After(A)` means the real `SetWindowPos` insert-after argument A.

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
| 8. Clean topmost prefix ending at A | B above A, all windows through A flagged, every window below A normal | Normal X `After(A)` | Does crossing the lower boundary differ from inserting above another topmost? |
| 9. Already-topmost X | X starts topmost | X `After(A)` | Does an existing topmost window retain its band and reach A? |
| 10. Just-demoted X | X starts topmost | X `NotTopmost`, observe normal, then `After(A)` | Does immediately preceding demotion change the insertion result? |
| 11. Normal-to-normal placement | X and normalA normal | X `After(normalA)` | Ordinary normal-band placement control. |
| 12. Repeated identical insertion | Default | X `After(A)`, observe convergence, repeat | Are band and immediate neighbors unchanged by the second call? |
| 13. Demotion observed before insertion | A topmost, pump held | Post A `NotTopmost`, release, observe A normal, X `After(A)` | Compare with row 5 after an observed native transition. |
| 14. Promotion posted, insert before release | A normal, pump held, B topmost | Post A `Topmost`, read flag, X `After(A)`, release A | The inverse ordering problem on entry into topmost. |
| 15. Promotion observed before insertion | A normal, pump held, B topmost | Post A `Topmost`, release, observe A topmost, X `After(A)` | Compare with row 14 after actual promotion. |
| 16. C queued against observed-normal A | Actual normal A-X-P-C chain, C pump held | Read A normal, post C `After(A)`, promote A, observe topmost, release C | Can a normal-state check become stale before another process applies its queued placement? |
| 17. C normal-reference control | Actual normal A-X-P-C chain, C pump held | Read A normal, post C `After(A)`, leave A normal, release C | Control that queued external placement settles normal directly after A. |
| 18. A returns normal before C release | Actual normal A-X-P-C chain, C pump held | Post C `After(A)`, promote A, observe, demote A, observe, release C | Does returning A to normal before C runs change the outcome from row 16? |
| 19. A below normal overlay | A topmost, X normal | A `After(X)` | Demotion control that deliberately puts A **below** X, not the desired border arrangement. |
| 20. A above X through normal P | A topmost, normal P immediately above normal X | A `After(P)` | Does A become normal and land above X? The API anchor is P, not a fictional insert-before-X operation. |
| 21. A above X through topmost P | Contiguous topmost prefix ends at P, X first normal, strict P-X adjacency | A `After(P)` | Does the same above-X arrangement retain topmost when its real predecessor is topmost? |

| 22. Owner promotion | X and its O normal | X `Topmost` | Both become topmost, O remains above X. |
| 23. Owner demotion | X and O topmost | X `NotTopmost` | Both become normal, O remains above X. |
| 24. Owned promotion | X and O normal | O `Topmost` | O becomes topmost, X stays normal. |
| 25. Owned demotion | X and O topmost | O `NotTopmost` | Both become normal under documented owner demotion rules. |
| 26. Relative insertion with X owning O | X and O normal, A/B topmost | X `Topmost`, then `After(A)` | Record strict adjacency separately from O-above-X group order. |
| 27. Relative insertion with A owning O | A and O topmost, X normal | A `After(X)` | Record strict adjacency separately from O-above-A group order. |
| 28. Normal Bottom | X normal | X `Bottom` | X remains normal at the complete desktop chain tail. |
| 29. Topmost Bottom | X topmost | X `Bottom` | X becomes normal at the complete desktop chain tail. |

Rows 1 to 5, 7 and 9 to 20 run on the ordinary desktop (18 combinations). Rows 6, 8, 21 to 29 and comparison controls 1, 3, 4 and 11 run on a fresh private desktop (15 combinations). Each environment has its own fixture processes. The default five repetitions schedule 165 trials. One repetition schedules 33. The raw session declares every environment/case/repetition key before collection.

The private desktop is never switched to or activated. No Explorer is started there. The suite changes no access control, elevation, desktop setting or unrelated window. If desktop creation or connection fails, affected scheduled trials are reported as not run, with a reason. There is no fallback to Default.

## Run on Windows

Stop Rust Dome yourself first. Record whether another window manager or PowerToys Always On Top is active. Ordinary-desktop fixtures appear briefly. Private-desktop fixtures should never become visible on your interactive desktop. This first native validation must check that behavior and cleanup, not assume it from compilation.

Copy this directory, including `src` and `tests`, excluding `target`, `runs` and `verification`. From its root, collect one repetition of both declared environments:

```sh
cargo run --release -- --repetitions 1
```

Use `cargo run --release` for the fixed five-repetition suite. `--out NEW_DIRECTORY` and `--repetitions 1..20` remain the only public options. Existing output directories are refused. No remote host or transfer operation is configured.

The binary prints the new evidence directory at startup and completion. Preserve that entire directory even on failure. It includes raw records, embedded source, source/executable SHA-256 hashes, the executable, native environment metadata, fixture stderr under each environment directory, collection/cleanup exits and summary counters. `raw-native-sha256.json` hashes the closed raw file on Windows after the owned collection processes have exited. Compare that hash after transfer. An unavailable hash is a provenance gap, not a match.

Analyze with the captured analyzer and its sibling `analyze_v5.py`:

```sh
python3 runs/RUN/source/analyze.py runs/RUN/raw.jsonl > runs/RUN/analysis.json
```

Exit 2 means incomplete or invalid evidence, a missing/not-run scheduled key, or a failed explicit control. Exit 0 means this bounded schedule has eligible endpoints and passing controls, not a universal Win32 guarantee. Collector exit 0 only means collection finished. Its summary prints scheduled, attempted, not-run, setup and treatment counts with controls explicitly pending independent analysis.

## Evidence and cleanup

Every window-owning thread calls `ImmDisableIME(0)` before its first window creation. A failed call rejects that environment or fixture. This is not a guarantee that Text Services Framework companions are absent. Class, owner, style and visibility inventory retains fixture-process windows after creation, at setup and on failure. Strict P-X checks never filter away companions or O. Main source windows retain their overlapped style, and X retains its nonactivating tool-popup style. Only scheduled O is an explicitly owned popup.

Ordinary observations retain full fixture states before and after bounded local neighbor traversal. Full unfiltered desktop proof is reserved for global premises and Bottom endpoints. Hidden windows count. No arbitrary shell handle is used to construct the private topmost prefix. Rows 6, 8 and 21 reject a normal window above that prefix.

Race rows 16 to 18 require C below stationary P immediately before release, not already After(A). After release, C must cross above P and become directly After(A). No placement to a fixture is allowed between release and endpoint. Rows 17 and 18 are actual normal-movement controls, rather than unchanged-neighbor observations.

Each completed trial has 21 fixed post samples. Row 12 also has 21 before its second insertion. The final five scheduled samples must be valid and consistent across fixture states. Earlier non-atomic invalid samples remain individually invalid in the raw evidence without discarding a later valid plateau. No sample is replaced and the window is never extended to obtain a pass. A missing plateau remains unresolved.

The supervisor bounds the entire collection to 480 seconds per repetition, with 240 seconds per environment and admission stopping 20 seconds before that environment limit. Main fixtures expire 30 seconds beyond the environment limit. Each C process expires after 30 seconds and each pump hold after three seconds. The inherited kill-on-close job contains all collector and fixture processes. The suite explicitly releases holds and bounds child cleanup. Desktop handles are closed only after successful environment-thread and child teardown. On an environment failure, closing is deferred to process exit and reported, rather than claiming verified native destruction.

Inspect `cleanup.json`, `collection-exit.json`, `exit.json`, every stderr file, environment results and independent analysis. Zero remaining processes does not prove every `DestroyWindow` call succeeded. Interrupted runs remain partial evidence and must not be overwritten.

## Scope

The historical schema-4 lowest-flagged case 8 is preserved as a different predicate, not relabeled as a clean boundary. Old schemas retain their original analyzer behavior. The suite does not test arbitrary applications, self-restacking, activation/focus behavior, minimized windows, virtual desktops, rendering or notifications. `HWND_TOP` is not used by Dome's current ordering enum and is not added here. Results can establish counterexamples and controls for these measured fixtures, not every possible Windows behavior.
