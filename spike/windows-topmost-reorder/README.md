# Windows topmost reorder spike

Tests whether an observed `WS_EX_TOPMOST` change can lack a nearby `EVENT_OBJECT_REORDER`. Global events are ambiguous temporal evidence, not proof that the target caused them.

## Controlled run

Stop Dome first. Use a current Rust toolchain on an interactive Windows desktop. Avoid secure desktops and run at the target app's integrity level. Record your Windows build, app versions, and whether PowerToys is running alongside the results.

From the repository root, in PowerShell:

```powershell
cd spike/windows-topmost-reorder
cargo build --release
.\target\release\windows-topmost-reorder.exe controlled --repeats 5 --seconds 120 --out controlled-local-boundary.jsonl
```

Leave the three fixture windows and unrelated windows alone. Each repetition promotes and demotes the target below an ordinary peer, as the highest ordinary window, and alongside another topmost fixture. It also runs a demotion-only `lowest_topmost_unchanged_adjacency` trial. Only fixtures are repositioned. Each toggle waits for a sampled flag transition and then a delivery-settle interval.

The demotion-only case puts the normal target at the top of the normal band and captures its direct topmost predecessor and ordinary successor. It promotes the target, places it directly after that predecessor, and checks that both captured neighbors remain. If this setup cannot be observed, `setup_ineligible` records the failure and no demotion is attempted for that repetition. Fixture replies include the normal reference and prepared snapshots, with the setup phase and any failure reason.

A demotion trial counts as exercised only when the target changes from topmost to normal, the same two neighboring handles retain their respective bands, observations are complete, and no extra edges or API or sampling failures occur. A changed neighbor marks the trial contaminated, even if the flag changed and a reorder event arrived. Hidden topmost flags elsewhere in the stack do not define this local boundary. This case never moves unrelated windows.

The highest-ordinary case requires no ordinary window above the target and no other enumerated topmost window, including hidden windows and the shell/taskbar. Missing preconditions, failed enumeration, or extra sampled edges mark a trial `unexercised_or_contaminated`. Do not change unrelated windows to manufacture a clean desktop. `preconditions_observed` is not a coverage verdict.

## Manual run

Choose a target handle. This lists main windows, not every window in a multi-window app:

```powershell
Get-Process | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object Id, ProcessName, MainWindowHandle
```

Replace `123456` with the chosen decimal handle. Hexadecimal with a `0x` prefix also works:

```powershell
.\target\release\windows-topmost-reorder.exe manual --hwnd 123456 --seconds 60 --out manual.jsonl
```

Wait for the baseline-finished message, then toggle the app's always-on-top control or PowerToys Always On Top (normally `Win+Ctrl+T` on the focused window). Leave at least a second between toggles at default timings. Test app-controlled and PowerToys-controlled changes in separate runs.

Manual mode never changes or closes the target. Closure ends monitoring and preserves the log. A handle reused within the same process and thread between samples cannot be distinguished from a surviving window.

## Timing

| Flag | Default | Accepted range |
| --- | --- | --- |
| `--seconds` | 120 | 1 to 3600 |
| `--sample-ms` | 10 | 1 to 1000 |
| `--settle-ms` | 500 | 1 to 60000 |
| `--baseline-ms` | 1000 | 1 to 60000 |
| `--transition-ms` | 2000 | 1 to 60000 |
| `--out` | `reorder.jsonl` | A new file path |
| `--repeats` | 5 | 1 to 1000, controlled mode only |

The duration includes setup and baselines. Controlled runs may finish sooner. The transition timeout bounds flag observation and child replies. Settling starts at the sampled edge. Interrupted or truncated waits are not evaluable.

Sampling continues without events, but its cadence is best effort. Observations report actual gaps. Scheduling, disk writes, and desktop enumeration can delay reads. Windows tick resolution can be coarser than the requested interval.

## Interpret results

Logs are JSON lines with `kind`, `logged_tick`, and `fields`. Existing files are refused. Completed lines are flushed but are not guaranteed durable across power loss.

Read `controlled_trial` or `manual_trial` for classifications, then inspect their raw evidence:

- `global_candidates_ambiguous` lists event sequences, never a confident pass. An unrelated global reorder can produce this result. The callback HWND is not assumed to identify the changed target.
- `no_event_counterexample_candidate` means an observed transition had no candidate within the recorded limits. Check preconditions, sampling gaps, late events, and extra or overlapping edges before drawing a conclusion.
- `not_evaluable` distinguishes failed API calls, absent transitions, closure, sampling or fixture failures, and truncated observation windows. Hook setup failure is logged separately.

`reorder` records native and delivery timestamps plus callback HWND, object/child IDs, thread, and sequence. No object-ID or target filtering is applied. `observation` and `observed_transition` are independent flag reads. `fixture_reply` records API results, not transition completion. Baselines report background event sequences. Desktop snapshots include bands, identities, classes, and neighbors. `adjacency_changed` compares both neighbors. Snapshots are non-atomic and may race with desktop changes.

Ticks are uptime milliseconds, with a Unix-time anchor in `session`. The expanded native tick accounts for wraparound, assuming delivery delay below 2^31 ms. Controlled correlation starts at command dispatch. Manual correlation starts at the previous sample, allowing events before the sampled edge. Both native and delivery times must fall within the recorded correlation window. Late and background events remain in the raw log.

Rapid double toggles between samples can be missed. Manual edges may share candidates and report `overlapping_edges`. Notifications may arrive after settling. Results cover only the accessible desktop/session and cannot establish a universal Windows guarantee. Exit code zero means normal collector termination, not coverage or completion of every case.

## Stop

Ctrl+C or Ctrl+Break stops collection. Completion and error cleanup close only fixture windows, allowing about a second for graceful shutdown. Manual mode leaves the target alone. Forced termination, system shutdown, and disk failure are outside the cleanup guarantee. Running the executable on a non-Windows host fails explicitly.
