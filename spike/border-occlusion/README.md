# border-occlusion-spike

Throwaway spike. One click-through overlay window draws a border around every normal window on
the primary display. It removes the border parts that windows in front cover. The first round
polled `CGWindowListCopyWindowInfo` at a fixed 60 Hz. This round polls when Accessibility (AX) and
NSWorkspace events say the window list may have changed. A driver measures both with no manual
step. The plan is `plans/border-occlusion-event-driven-spike.md` in the Dome repository.

## Layout

- `src/lib.rs` holds `geometry`, the rectangle math, and `windows`, the CGWindowList reader.
- `src/main.rs` is the overlay binary, `border-occlusion-spike`. `src/triggers.rs` holds its AX and
  NSWorkspace observers.
- `src/bin/fixture.rs` owns the test windows. It takes one command per line on stdin.
- `src/bin/driver.rs` and the files under `src/bin/driver/` run every mode through every scenario.
  They write `out/<run>/results.md` with the raw logs and stills beside it.

## Modes

| Mode | Poll clock | Redraw |
|---|---|---|
| `poll` | fixed 60 Hz | every tick |
| `poll-diff` | fixed 60 Hz | only on change |
| `events` | event bursts plus a safety poll | only on change |
| `events-focused` | as `events`, but a drag reads one window | only on change |

A burst polls every 16 ms. It ends 300 ms after its last trigger. Outside a burst, a safety poll
runs every 1000 ms. When the safety poll finds a change, the overlay logs it as unreported and
starts a burst.

## Run

```
cd spike/border-occlusion
cargo run --release --bin driver -- --out out/<run>
```

The driver covers the screen with a gray backdrop for about 20 minutes. It waits for 30 s with
no user input before it starts. `--modes` picks the modes. `--parts cpu,fidelity,lag` picks the
parts. `--no-baseline` skips the run with no overlay. `driver dump` prints the window list.

The overlay also runs alone:

```
cargo run --release -- --mode events
```

Its flags are `--mode`, `--level` (default 1000), `--tail-ms` (default 300), and `--safety-ms`
(default 1000). It writes one `key=value` record per line to stderr.

## Results

The tables below are copied from `out/final/results.md`. Other build jobs ran on the machine
during the run. The load average was 3.5 at the start and 5.5 at the end. Treat a CPU difference
below about 1 percentage point as noise.

Overlay and WindowServer CPU come from the `ps` CPU time delta over the wall time. Idle wakeups
come from `top`. Interrupt wakeups come from `proc_pid_rusage`. Polls, redraws, and triggers come
from the overlay log.

### CPU per mode and scenario

| Mode | Scenario | Overlay CPU % | Overlay idle wakeups/s | Overlay interrupt wakeups/s | Polls/s | Redraws/s | Triggers/s | Mean poll µs | WindowServer CPU % | WindowServer idle wakeups/s | WindowServer power |
|---|---|---|---|---|---|---|---|---|---|---|---|
| poll | idle | 4.78 | 0.70 | 60.0 | 59.9 | 59.9 | 0.0 | 1657 | 7.6 | 0.50 | 7.6 |
| poll | drag | 3.52 | 0.25 | 60.1 | 60.0 | 60.0 | 0.0 | 1073 | 26.1 | 2.05 | 25.2 |
| poll | resize | 2.63 | 0.15 | 60.1 | 60.0 | 60.0 | 0.0 | 1255 | 36.4 | 2.90 | 35.1 |
| poll | app switch | 4.29 | 0.45 | 60.0 | 59.9 | 59.9 | 0.0 | 1527 | 9.8 | 0.30 | 9.7 |
| poll-diff | idle | 2.88 | 0.47 | 60.0 | 60.0 | 0.0 | 0.0 | 1561 | 6.9 | 0.27 | 6.8 |
| poll-diff | drag | 2.33 | 0.05 | 60.1 | 60.0 | 44.1 | 0.0 | 1582 | 38.3 | 3.80 | 37.0 |
| poll-diff | resize | 4.45 | 0.20 | 60.1 | 60.0 | 45.9 | 0.0 | 2288 | 41.7 | 1.50 | 40.0 |
| poll-diff | app switch | 2.74 | 0.50 | 60.1 | 60.0 | 1.0 | 0.0 | 1457 | 9.3 | 0.25 | 9.1 |
| events | idle | 0.03 | 0.00 | 1.1 | 1.0 | 0.0 | 0.0 | 1127 | 1.8 | 0.28 | 1.8 |
| events | idle (safety 250 ms) | 0.28 | 0.05 | 4.2 | 4.0 | 0.0 | 0.0 | 2251 | 2.0 | 0.12 | 1.9 |
| events | idle (safety 2000 ms) | 0.03 | 0.00 | 0.7 | 0.5 | 0.0 | 0.0 | 1245 | 1.6 | 0.28 | 1.5 |
| events | drag | 4.90 | 0.45 | 60.3 | 60.0 | 57.9 | 8.7 | 3491 | 42.3 | 2.70 | 40.4 |
| events | resize | 3.22 | 0.05 | 60.3 | 60.0 | 55.5 | 56.9 | 1080 | 37.1 | 3.45 | 35.7 |
| events | app switch | 0.90 | 0.15 | 19.1 | 19.9 | 1.0 | 1.0 | 1622 | 5.8 | 0.40 | 5.6 |
| events-focused | idle | 0.05 | 0.00 | 1.7 | 1.7 | 0.0 | 0.0 | 1019 | 1.7 | 0.27 | 1.7 |
| events-focused | drag | 2.72 | 0.20 | 60.4 | 60.0 | 53.4 | 8.9 | 2738 | 40.5 | 1.80 | 38.8 |
| events-focused | resize | 2.43 | 0.10 | 60.4 | 60.0 | 55.3 | 57.2 | 1221 | 36.5 | 3.50 | 35.0 |
| events-focused | app switch | 0.95 | 0.20 | 19.3 | 20.0 | 1.0 | 1.0 | 1855 | 5.7 | 0.15 | 5.5 |
| no overlay | idle | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 1.7 | 0.32 | 1.6 |
| no overlay | drag | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 28.3 | 4.20 | 27.4 |
| no overlay | resize | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 28.7 | 2.15 | 27.6 |
| no overlay | app switch | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 4.3 | 0.20 | 4.1 |


### Fidelity per case and mode

Each count reads missing / over content, out of the ring midline points checked. "Missing" is an
expected border point that is not magenta. "Over content" is a magenta point that a window in front
covers. The checker skips a covered point that lies on the ring of a bordered window in front,
because that ring is magenta by design. A "-" staleness means no redraw after the action changed
the path. For the launcher, QuickLook, and banner cases, the action time is the moment the new
window first shows in CGWindowList.

| Case | Mode | Early | Late | Staleness ms | Unreported changes | Note |
|---|---|---|---|---|---|---|
| raise | poll | 0 / 0 of 1238 | 0 / 0 of 1238 | 62 | none |  |
| drag | poll | 0 / 0 of 1237 | 0 / 0 of 1237 | - | none |  |
| resize | poll | 0 / 0 of 1112 | 0 / 0 of 1112 | 4 | none |  |
| overlap | poll | 0 / 0 of 1256 | 0 / 0 of 1256 | 54 | none |  |
| context menu open | poll | 0 / 0 of 1221 | 0 / 0 of 1221 | 62 | none |  |
| context menu Escape | poll | 0 / 0 of 1221 | 0 / 0 of 1238 | 230 | none |  |
| pop-up menu open | poll | 0 / 0 of 1235 | 0 / 0 of 1235 | 32 | none |  |
| pop-up menu Escape | poll | 0 / 0 of 1235 | 0 / 0 of 1238 | 209 | none |  |
| launcher open | poll | 0 / 0 of 1229 | 0 / 0 of 1229 | - | none |  |
| launcher Escape | poll | 0 / 0 of 1238 | 0 / 0 of 1238 | 29 | none |  |
| QuickLook open | poll | 0 / 0 of 1230 | 0 / 0 of 1230 | 0 | none |  |
| QuickLook killed | poll | 0 / 0 of 1238 | 0 / 0 of 1238 | 10 | none |  |
| floating window, raise below it | poll | 0 / 0 of 798 | 0 / 0 of 798 | 71 | none |  |
| order front | poll | 0 / 0 of 1238 | 0 / 0 of 1238 | 25 | none |  |
| minimize | poll | 256 / 0 of 1056 | 0 / 0 of 800 | 42 | none |  |
| restore | poll | 363 / 0 of 1076 | 0 / 0 of 1238 | 28 | none |  |
| open | poll | 0 / 0 of 1559 | 0 / 0 of 1559 | 52 | none |  |
| close | poll | 0 / 0 of 1238 | 0 / 0 of 1238 | 18 | none |  |
| app switch | poll | 0 / 0 of 1264 | 0 / 0 of 1264 | 38 | none |  |
| notification banner | poll | - | - | - | none | not exercised: no banner window appeared within 2 s |
| Mission Control | poll | 108336 magenta px | 70 magenta px | 123 | none |  |
| Mission Control Escape | poll | 0 / 0 of 1234 | 0 / 0 of 1238 | 19 | none |  |
| raise | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 57 | none |  |
| drag | events | 0 / 0 of 1237 | 0 / 0 of 1237 | 3 | none |  |
| resize | events | 0 / 0 of 1112 | 0 / 0 of 1112 | 11 | none |  |
| overlap | events | 0 / 0 of 1256 | 0 / 0 of 1256 | 50 | none |  |
| context menu open | events | 0 / 0 of 1221 | 0 / 0 of 1221 | 65 | none |  |
| context menu Escape | events | 0 / 0 of 1221 | 0 / 0 of 1238 | 238 | none |  |
| pop-up menu open | events | 0 / 0 of 1235 | 0 / 0 of 1235 | 42 | none |  |
| pop-up menu Escape | events | 0 / 0 of 1235 | 0 / 0 of 1238 | 238 | none |  |
| launcher open | events | 0 / 0 of 1229 | 0 / 0 of 1229 | - | none |  |
| launcher Escape | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 32 | none |  |
| QuickLook open | events | 0 / 0 of 1230 | 0 / 0 of 1230 | - | none |  |
| QuickLook killed | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 11 | none |  |
| floating window, raise below it | events | 0 / 0 of 798 | 0 / 0 of 798 | 75 | none |  |
| order front | events | 124 / 124 of 1238 | 0 / 0 of 1238 | 450 | fixture reordered |  |
| minimize | events | 348 / 0 of 1024 | 0 / 0 of 800 | 280 | none |  |
| restore | events | 264 / 0 of 1064 | 0 / 0 of 1238 | 385 | none |  |
| open | events | 0 / 0 of 1559 | 0 / 0 of 1559 | 47 | none |  |
| close | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 25 | none |  |
| app switch | events | 0 / 0 of 1264 | 0 / 0 of 1264 | 31 | none |  |
| notification banner | events | - | - | - | none | not exercised: no banner window appeared within 2 s |
| Mission Control | events | 108336 magenta px | 70 magenta px | 1057 | Dock appeared, Dock appeared, Dock appeared, fixture moved, fixture moved, fixture moved, fixture moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Google_Chrome moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Brave_Browser moved |  |
| Mission Control Escape | events | 0 / 0 of 1234 | 0 / 0 of 1238 | 267 | none |  |


### Drag lag per mode

The drag moves right at 400 pt/s, so one frame at 60 Hz is about 6.7 pt. A positive lag means the
ring trails the window.

| Mode | Moving frames | Median pt | 95th percentile pt | Note |
|---|---|---|---|---|
| poll | 115 | 7.0 | 8.0 | 425 frames decoded |
| events | 106 | 0.0 | 14.0 | 363 frames decoded |
| events-focused | 108 | 7.0 | 9.0 | 525 frames decoded |

## Verdict

- Adopt `events`. Idle overlay CPU drops from 2.9 to 4.8% to 0.03%. Interrupt wakeups drop from
  60 to 1.1 per second. WindowServer drops from about 7% to 1.8%, against 1.7% with no overlay.
- `poll-diff` shows where the idle saving comes from. It skips every idle redraw, yet WindowServer
  still spends 6.9%. The only difference from `events` is about 59 more
  `CGWindowListCopyWindowInfo` calls per second, and each call is a request to WindowServer.
- During a drag or a resize, every mode polls 60 times per second. The modes differ there by less
  than the run's noise.
- Drop `events-focused`. In the step 6 drag, the one-window read ran for 1056 of 1077 polls. Its
  mean was 856 µs, against 1075 µs for the full read in the step 4 `events` drag. The final run
  shows the overlay at 2.72% against 4.90% in a drag, but step 6 showed 3.47% against 2.62%. The
  saving is not consistent.

Parameter defaults:

- Keep `SAFETY` at 1000 ms. It bounds the staleness of a change that no event reports. In this run
  that was order front at 450 ms and Mission Control at 1057 ms. At 250 ms the idle wakeups rise to
  4.2 per second. At 2000 ms they fall by only 0.4 per second, and the bound doubles.
- Keep `TAIL` at 300 ms. The run gives no reason to change it. Every late still passed. The menu,
  minimize, and restore events arrived at the end of their animations, not at the start. For
  example, `AXMenuClosed` arrived 238 ms after Escape, and `AXWindowMiniaturized` arrived 280 ms
  after the minimize command.
- Keep `FRAME` at 16 ms. The `events` drag lag was 0 pt at the median and 14 pt at the 95th
  percentile, against 7 pt and 8 pt in `poll`. Both stay within about two frames.

## Findings for Dome

- Notification Center keeps one window at layer 21 over the whole display. Persistent alerts and
  new banners draw inside it, so no banner window appears in CGWindowList. As an occluder, this
  window removes every border on the screen. The spike drops it from the window list, so a border
  draws over a banner.
- While `screencapture -v` records, it adds a full-screen window at layer 1499 and a cursor window
  at the cursor layer. The overlay cuts no hole for a window at or above its own level.
- A title-bar drag produced about 9 `AXMoved` notifications per second. A corner resize produced
  about 57 `AXResized` per second. The tail keeps a drag burst alive between two notifications.
- Order front and Mission Control reported no event. The safety poll found them after 450 ms and
  1057 ms.
- At 100 ms into Mission Control, both `poll` and `events` left 108336 magenta pixels on the
  screen. At 1500 ms, 70 remained.
- During a minimize, CGWindowList reports the window at its animated frame. At 100 ms, both modes
  missed 256 to 363 border points on a minimize or a restore. At 1500 ms, both passed.
- Posted events reset the `HIDSystemState` idle counter through the HID tap and through the session
  tap. So the driver waits the full 30 s once. After that, a gate passes at once only while the
  counter still matches the time since the driver's own last posted event.
- An Accessory app activates itself once when `NSApplication.run` starts. The overlay uses the
  Prohibited activation policy, so it never takes focus.

## Deliberately out of scope

- Primary display only. A window on a second display is flipped wrong, because the coordinate map
  assumes one display.
- Borders every layer-0 window, not only the windows Dome manages.
- No space switch case. It needs a second space and changes the user's space state.

## Tests

`cargo test` covers the rectangle subtraction, the window list diff, the checker's point test, the
`top` and `ps` parsers, and the drag lag row scan.

## Third spike: AX geometry with rare CGWindowList calls

The plan is `plans/border-overlay-ax-geometry-spike.md` in the Dome repository. The second spike
still called `CGWindowListCopyWindowInfo` 60 times a second during a drag or a resize. This round
keeps the z-order, the levels, and the rects of all windows in a cache. A CGWindowList call fills
the cache after a z-order event and once a second. The frame of a managed window comes from an AX
read after each move or resize notification of that window. The spike uses public API only.

### Changes to every mode

- The overlay reads these commands on stdin, one per line:
  - `manage <pid> <window number> <title>` adds a managed window.
  - `unmanage <window number>` removes it.
  - `stats` logs the stats line at once and starts a new period. Each stats line now carries
    `period_ms`.
- Every mode borders only managed windows. This replaces the layer-0 filter. So the out-of-scope
  item "Borders every layer-0 window" above no longer holds.
- The driver sends `manage` for every fixture test window except the backdrop. The unmanaged window
  case leaves one more window out.
- The checker checks every managed window at any layer. So the floating window case now checks the
  floating window too, at 1238 points against 798.
- `event=redraw changed=1` now means that the path changed, in every mode. In the second spike it
  meant that the window list changed in `events`. The staleness column reads this field. So a list
  change that leaves the path alone, such as a menu over no ring, now shows "-".
- The driver sends `stats` at the start and at the end of each CPU measurement. So the rates cover
  the measured time, including the 5 s focus spam.

### The `ax-geometry` mode

- These events go to the z-order throttle: focused window changed, did activate application, window
  created, UI element destroyed, miniaturized, deminiaturized, application hidden, application
  shown, menu opened, menu closed, active space changed, screen parameters changed, application
  launched, and application terminated.
- The throttle in `src/throttle.rs` has a leading and a trailing edge with a 100 ms interval. The
  first event calls CGWindowList at once. Later events in the interval share one trailing call.
- A CGWindowList call replaces the whole cache. The once-a-second call skips when a z-order call ran
  in the last second. A change that the once-a-second call finds logs `event=unreported`.
- On `manage`, the overlay finds the window's AX element by its `AXTitle` among the app's
  `AXWindows`. The element gets a 50 ms messaging timeout.
- The overlay compares the element of a move or resize notification with the managed elements of
  that pid through `CFEqual`. A match reads `AXPosition` and `AXSize` and patches that one rect. A
  move of any other window reads nothing.
- New log records are `event=cglist cause=<zorder|safety>`, `event=axread window=<number>
  ok=<0|1> us=<duration>`, and the call, read, and failure counts in `event=stats`.

### Driver additions

- The fixture takes `minsize <label> <w> <h>` and `spam <label1> <label2> <interval ms> <count>`.
  The `spam` reply comes after the last key change.
- `src/bin/driver/dome.rs` acts as Dome. It writes each frame through `AXPosition`, then `AXSize`.
- `--modes` defaults to `events,ax-geometry`. Each CPU scenario runs twice.
- The CPU table gains the run number, the AX reads per second, the fixture CPU of A and B summed,
  and the 1 minute load average at the start and the end. "CGWindowList calls/s" replaces "Polls/s",
  because every poll is one call.
- The two new CPU scenarios are a Dome layout and a focus spam. In the Dome layout, the driver moves
  4 windows between two tilings every 500 ms for 20 s. The focus spam runs `spam a1 a2 10 500`.
- The new accuracy cases are these:
  - Dome layout. One layout change of the 4 windows.
  - Minimum size. `a1` gets a minimum content size of 800x500 and a 700x420 tile.
  - Focus spam end. `spam lo hi 10 101`. The count is odd, so `lo` ends in front, where `hi` was
    before. The early still is 150 ms after the last key change.
  - Unmanaged window moves. An unmanaged window of fixture B moves from the ring of `b1` to the
    ring of `hi`.

### Run

```
cd spike/border-occlusion
cargo build --release
./target/release/driver --out out/ax
```

The run took about 22 minutes.

### Drag lag scan check

Before this run, five decoded frames per mode of `out/final/lag/poll.mov` and
`out/final/lag/events.mov` were read by eye and compared to the scan.

- In every frame that the scan read as 7 pt, the ring stood one frame behind the window, with
  backdrop between them. In every frame that the scan read as 0 pt, the ring was flush with the
  window on both sides. So the second spike's 0 pt median for `events` was a true reading.
- The second spike's 95th percentile of 14 pt came mostly from the start of the drag. For the
  first 6 moving frames, the ring stayed where the drag started.
- The scan could not read a ring that leads the window. A leading ring covers the window's left
  edge. So a lead below 10 pt read as 0 pt, and a larger lead dropped the frame.
- The fix reads a lead on the right side, as the gap between the window's right edge and the ring.
  `row_lag` in `src/bin/driver/lag.rs` now returns a negative lag for a leading ring. A unit test
  covers it.
- No frame of the two recordings shows that gap, so no frame led. The fix leaves the second spike's
  numbers as they were. The recordings of this run show no leading frame either.
- The `events` median is 7 pt in this run and was 0 pt in the second spike, with the same scan. The
  spike does not measure why.

### Results

The tables below are copied from `out/ax/results.md`. The 1 minute load average was 3.4 at the start
and 5.4 at the end. Per CPU scenario it stayed between 2.5 and 7.5. Before the run, endpoint
security agents used about 2.5 cores. Treat a CPU difference below about 1 percentage point as
noise.

#### CPU per mode and scenario

| Mode | Scenario | Run | Overlay CPU % | Overlay idle wakeups/s | Overlay interrupt wakeups/s | CGWindowList calls/s | AX reads/s | Redraws/s | Triggers/s | Mean CGWindowList µs | Fixture CPU % | WindowServer CPU % | WindowServer idle wakeups/s | WindowServer power | Load start / end |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| events | idle | 1 | 0.05 | 0.00 | 1.1 | 1.0 | 0.0 | 0.0 | 0.0 | 1132 | 0.87 | 2.3 | 0.35 | 2.2 | 3.09 / 2.95 |
| events | drag | 1 | 3.17 | 0.55 | 60.7 | 58.9 | 0.0 | 53.7 | 8.8 | 2074 | 5.20 | 40.6 | 5.15 | 39.1 | 2.95 / 4.35 |
| events | resize | 1 | 2.92 | 0.35 | 60.1 | 59.1 | 0.0 | 51.4 | 56.3 | 1826 | 17.38 | 38.4 | 4.95 | 36.8 | 4.08 / 3.86 |
| events | app switch | 1 | 0.80 | 0.20 | 19.3 | 20.0 | 0.0 | 1.0 | 1.0 | 1549 | 2.25 | 5.8 | 0.45 | 5.6 | 3.71 / 2.95 |
| events | Dome layout | 1 | 1.95 | 0.15 | 38.6 | 40.2 | 0.0 | 3.3 | 12.1 | 1321 | 4.09 | 9.0 | 0.55 | 8.6 | 2.79 / 2.67 |
| events | focus spam | 1 | 2.18 | 0.00 | 60.8 | 58.9 | 0.0 | 20.5 | 99.0 | 668 | 30.31 | 42.6 | 1.80 | 37.8 | 2.61 / 3.13 |
| ax-geometry | idle | 1 | 0.07 | 0.00 | 1.1 | 1.0 | 0.0 | 0.0 | 0.0 | 1727 | 0.80 | 1.4 | 0.18 | 1.4 | 2.96 / 2.71 |
| ax-geometry | drag | 1 | 0.40 | 0.00 | 1.2 | 1.0 | 8.7 | 9.5 | 8.7 | 3216 | 5.20 | 31.4 | 4.65 | 30.3 | 2.49 / 2.53 |
| ax-geometry | resize | 1 | 1.83 | 0.00 | 1.2 | 1.0 | 56.7 | 56.7 | 56.9 | 1177 | 18.28 | 35.3 | 3.55 | 33.7 | 2.53 / 4.39 |
| ax-geometry | app switch | 1 | 0.10 | 0.00 | 1.2 | 1.0 | 0.0 | 0.9 | 1.0 | 5179 | 2.25 | 4.1 | 0.55 | 3.9 | 4.20 / 5.48 |
| ax-geometry | Dome layout | 1 | 0.50 | 0.00 | 1.2 | 1.0 | 12.0 | 7.9 | 12.1 | 2280 | 3.59 | 4.8 | 0.35 | 4.6 | 5.36 / 5.17 |
| ax-geometry | focus spam | 1 | 1.19 | 0.00 | 11.9 | 9.5 | 0.0 | 4.5 | 98.9 | 1685 | 30.07 | 39.6 | 1.20 | 35.1 | 5.16 / 5.15 |
| no overlay | idle | 1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 0.85 | 1.4 | 0.32 | 1.3 | 5.45 / 5.02 |
| no overlay | drag | 1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 5.55 | 28.8 | 4.35 | 27.8 | 5.02 / 5.38 |
| no overlay | resize | 1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 17.87 | 30.3 | 3.80 | 29.2 | 4.94 / 4.52 |
| no overlay | app switch | 1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 2.15 | 3.3 | 0.35 | 3.2 | 4.52 / 4.39 |
| no overlay | Dome layout | 1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 3.39 | 4.2 | 0.75 | 4.1 | 4.39 / 4.55 |
| no overlay | focus spam | 1 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 31.09 | 42.0 | 1.00 | 37.1 | 5.07 / 4.74 |
| events | idle | 2 | 0.07 | 0.02 | 1.1 | 1.0 | 0.0 | 0.0 | 0.0 | 2278 | 0.85 | 1.5 | 0.25 | 1.4 | 4.92 / 3.75 |
| events | drag | 2 | 3.42 | 0.35 | 59.4 | 58.9 | 0.0 | 54.7 | 8.9 | 3114 | 5.20 | 40.7 | 3.50 | 39.1 | 3.75 / 4.66 |
| events | resize | 2 | 2.87 | 0.70 | 59.7 | 59.1 | 0.0 | 55.0 | 56.6 | 1380 | 17.83 | 38.9 | 9.35 | 37.4 | 4.45 / 5.02 |
| events | app switch | 2 | 0.70 | 0.05 | 19.3 | 20.0 | 0.0 | 1.0 | 1.0 | 1414 | 2.00 | 4.6 | 0.40 | 4.4 | 5.10 / 5.73 |
| events | Dome layout | 2 | 1.70 | 0.50 | 38.5 | 40.1 | 0.0 | 3.2 | 12.1 | 1163 | 3.44 | 7.1 | 1.25 | 6.8 | 5.51 / 5.62 |
| events | focus spam | 2 | 2.18 | 0.20 | 60.9 | 58.9 | 0.0 | 19.4 | 98.9 | 3462 | 31.25 | 41.9 | 0.60 | 37.0 | 5.65 / 6.48 |
| ax-geometry | idle | 2 | 0.08 | 0.00 | 1.1 | 1.0 | 0.0 | 0.0 | 0.0 | 1872 | 0.87 | 1.5 | 0.13 | 1.5 | 7.48 / 5.17 |
| ax-geometry | drag | 2 | 0.40 | 0.00 | 1.2 | 1.0 | 8.6 | 9.5 | 8.6 | 1770 | 5.40 | 32.9 | 7.05 | 31.9 | 5.17 / 4.37 |
| ax-geometry | resize | 2 | 1.68 | 0.00 | 1.2 | 1.0 | 56.3 | 57.2 | 56.6 | 757 | 16.99 | 33.3 | 4.25 | 32.2 | 4.34 / 4.61 |
| ax-geometry | app switch | 2 | 0.10 | 0.05 | 1.3 | 1.0 | 0.0 | 0.9 | 1.0 | 4482 | 1.95 | 3.7 | 0.65 | 3.7 | 4.32 / 3.63 |
| ax-geometry | Dome layout | 2 | 0.50 | 0.10 | 1.2 | 1.0 | 12.0 | 7.9 | 12.1 | 1749 | 3.49 | 4.4 | 0.65 | 4.2 | 3.34 / 3.26 |
| ax-geometry | focus spam | 2 | 1.38 | 0.00 | 11.1 | 9.5 | 0.0 | 3.9 | 98.9 | 1868 | 31.05 | 41.3 | 1.40 | 36.5 | 3.16 / 2.99 |
| no overlay | idle | 2 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 0.88 | 1.3 | 0.23 | 1.3 | 3.31 / 3.16 |
| no overlay | drag | 2 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 5.45 | 28.3 | 5.65 | 27.3 | 3.15 / 4.07 |
| no overlay | resize | 2 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 19.60 | 31.6 | 3.50 | 30.3 | 3.91 / 5.21 |
| no overlay | app switch | 2 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 2.40 | 3.3 | 0.30 | 3.2 | 5.35 / 5.65 |
| no overlay | Dome layout | 2 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 3.69 | 6.6 | 1.15 | 6.4 | 5.65 / 5.91 |
| no overlay | focus spam | 2 | n/a | n/a | n/a | n/a | n/a | n/a | n/a | n/a | 31.07 | 39.2 | 2.00 | 34.5 | 5.91 / 5.84 |

#### Accuracy per case and mode

Each count reads missing / over content, out of the ring midline points checked, as in the second
spike. The early still is 100 ms after the action and the late one 1500 ms after it, except where
the note says otherwise.

| Case | Mode | Early | Late | Staleness ms | Unreported changes | Note |
|---|---|---|---|---|---|---|
| raise | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 57 | none |  |
| drag | events | 0 / 0 of 1237 | 0 / 0 of 1237 | 1 | none |  |
| resize | events | 0 / 0 of 1112 | 0 / 0 of 1112 | 7 | none |  |
| overlap | events | 0 / 0 of 1256 | 0 / 0 of 1256 | 44 | none |  |
| context menu open | events | 0 / 0 of 1221 | 0 / 0 of 1221 | - | none |  |
| context menu Escape | events | 0 / 0 of 1221 | 0 / 0 of 1238 | - | none |  |
| pop-up menu open | events | 0 / 0 of 1235 | 0 / 0 of 1235 | - | none |  |
| pop-up menu Escape | events | 0 / 0 of 1235 | 0 / 0 of 1238 | - | none |  |
| launcher open | events | 0 / 0 of 1229 | 0 / 0 of 1229 | - | none |  |
| launcher Escape | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 34 | none |  |
| QuickLook open | events | 0 / 296 of 1230 | 0 / 0 of 1230 | 725 | qlmanage appeared |  |
| QuickLook killed | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 10 | none |  |
| floating window, raise below it | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 74 | none |  |
| order front | events | 124 / 124 of 1238 | 0 / 0 of 1238 | 288 | fixture reordered |  |
| minimize | events | 376 / 0 of 1056 | 0 / 0 of 800 | 215 | fixture moved |  |
| restore | events | 298 / 0 of 1098 | 0 / 0 of 1238 | 381 | none |  |
| open | events | 0 / 0 of 1559 | 0 / 0 of 1559 | 52 | none |  |
| close | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 0 | none |  |
| app switch | events | 0 / 0 of 1264 | 0 / 0 of 1264 | 48 | none |  |
| Dome layout | events | 0 / 0 of 2240 | 0 / 0 of 2240 | 24 | none |  |
| minimum size | events | 0 / 0 of 2291 | 0 / 0 of 2291 | 25 | none | a1 requested 700x420, real 800x532 |
| focus spam end | events | 0 / 0 of 1238 | 0 / 0 of 1238 | 5 | none | early still 150 ms after the last key change |
| unmanaged window moves | events | 0 / 0 of 1232 | 0 / 0 of 1232 | 37 | none |  |
| notification banner | events | - | - | - | none | not exercised: no banner window appeared within 2 s |
| Mission Control | events | 108336 magenta px | 63 magenta px | 878 | none |  |
| Mission Control Escape | events | 0 / 0 of 1234 | 0 / 0 of 1238 | 269 | Dock appeared, fixture moved, fixture moved, fixture moved, fixture moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Google_Chrome moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Brave_Browser moved, Ghostty moved, Ghostty moved, Ghostty moved, Microsoft_Outlook moved, Slack moved |  |
| raise | ax-geometry | 124 / 124 of 1238 | 0 / 0 of 1238 | 1428 | fixture reordered |  |
| drag | ax-geometry | 0 / 0 of 1237 | 0 / 0 of 1237 | 6 | none |  |
| resize | ax-geometry | 0 / 0 of 1112 | 0 / 0 of 1112 | - | none |  |
| overlap | ax-geometry | 445 / 198 of 1256 | 445 / 198 of 1256 | - | none |  |
| context menu open | ax-geometry | 0 / 0 of 1221 | 0 / 0 of 1221 | - | none |  |
| context menu Escape | ax-geometry | 0 / 0 of 1221 | 0 / 0 of 1238 | - | fixture appeared |  |
| pop-up menu open | ax-geometry | 0 / 0 of 1235 | 0 / 0 of 1235 | - | fixture appeared |  |
| pop-up menu Escape | ax-geometry | 0 / 0 of 1235 | 0 / 0 of 1238 | - | none |  |
| launcher open | ax-geometry | 0 / 0 of 1229 | 0 / 0 of 1229 | 18 | none |  |
| launcher Escape | ax-geometry | 575 / 0 of 1238 | 0 / 0 of 1238 | 1488 | Raycast gone |  |
| QuickLook open | ax-geometry | 0 / 296 of 1230 | 0 / 0 of 1230 | 1028 | qlmanage appeared |  |
| QuickLook killed | ax-geometry | 0 / 0 of 1238 | 0 / 0 of 1238 | 12 | none |  |
| floating window, raise below it | ax-geometry | 0 / 0 of 1238 | 0 / 0 of 1238 | 108 | none |  |
| order front | ax-geometry | 124 / 124 of 1238 | 0 / 0 of 1238 | 563 | fixture reordered |  |
| minimize | ax-geometry | 334 / 0 of 1010 | 0 / 0 of 800 | 281 | none |  |
| restore | ax-geometry | 276 / 0 of 1076 | 0 / 0 of 1238 | 390 | none |  |
| open | ax-geometry | 266 / 98 of 1559 | 266 / 98 of 1559 | 1675 | fixture appeared |  |
| close | ax-geometry | 100 / 0 of 1238 | 100 / 0 of 1238 | 0 | none |  |
| app switch | ax-geometry | 0 / 0 of 1264 | 0 / 0 of 1264 | 110 | none |  |
| Dome layout | ax-geometry | 0 / 0 of 2240 | 0 / 0 of 2240 | 3 | none |  |
| minimum size | ax-geometry | 0 / 0 of 2291 | 0 / 0 of 2291 | 7 | none | a1 requested 700x420, real 800x532 |
| focus spam end | ax-geometry | 0 / 0 of 1238 | 0 / 0 of 1238 | - | none | early still 150 ms after the last key change |
| unmanaged window moves | ax-geometry | 79 / 73 of 1232 | 0 / 0 of 1232 | 845 | fixture moved |  |
| notification banner | ax-geometry | - | - | - | none | not exercised: no banner window appeared within 2 s |
| Mission Control | ax-geometry | 108336 magenta px | 63 magenta px | 896 | none |  |
| Mission Control Escape | ax-geometry | 0 / 0 of 1234 | 1114 / 0 of 1238 | - | Dock appeared, fixture moved, fixture moved, fixture moved, fixture moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Google_Chrome moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Ghostty moved, Brave_Browser moved, Ghostty moved, Ghostty moved, Ghostty moved, Microsoft_Outlook moved, Slack moved |  |

Stills with a failure are under `out/ax/fidelity/<mode>/`, one PNG per failed still, with a `.txt`
beside it that lists the window list and the failed points.

#### Drag lag per mode

| Mode | Moving frames | Median pt | 95th percentile pt | Note |
|---|---|---|---|---|
| events | 113 | 7.0 | 9.0 | 446 frames decoded |
| ax-geometry | 113 | 20.0 | 41.0 | 473 frames decoded |

### Verdict

Do not adopt `ax-geometry` as the plan specifies it. It cuts the WindowServer cost of a drag, a
resize, and a Dome layout. But its z-order schedule leaves wrong borders for up to 2 s after a
raise, an open, a close, and other common changes.

Cost:

- During a drag, CGWindowList calls drop from 58.9 to 1.0 per second. Overlay CPU drops from 3.17%
  and 3.42% to 0.40%. WindowServer drops from 40.6% and 40.7% to 31.4% and 32.9%, against 28.8% and
  28.3% with no overlay.
- During a resize, WindowServer drops from 38.4% and 38.9% to 35.3% and 33.3%, against 30.3% and
  31.6% with no overlay. The overlay still redraws 57 times a second, once per `AXResized`.
- During a Dome layout, CGWindowList calls drop from 40 to 1 per second. WindowServer drops from
  9.0% and 7.1% to 4.8% and 4.4%, against 4.2% and 6.6% with no overlay.
- Under focus spam, the throttle holds CGWindowList at 9.5 calls per second against 99 focus events
  per second. WindowServer stays near 40% in every mode, including no overlay.
- Idle is the same in both modes. An app switch drops from 20 to 1 CGWindowList call per second.
- The AX reads cost the fixtures nothing above the noise. No `ax-geometry` run shows more fixture CPU
  than the higher of the two no-overlay runs of the same scenario.

Accuracy:

- `ax-geometry` failed seven cases that `events` passed. These are raise, overlap, launcher Escape,
  open, close, unmanaged window moves, and Mission Control Escape. Overlap, open, close, and Mission
  Control Escape still failed at 1500 ms.
- The overlay log shows the cause. A single z-order event started a leading call 0 to 1 ms after
  its AX notification. That call still returned the old list. No second event came, so no trailing
  call ran. The next once-a-second call skipped, because the z-order call had run less than 1 s
  before. So the cache stayed stale for 1 to 2 s. In the raise case, the once-a-second call found
  the reorder 1427 ms after the notification.
- The drag lag grows from 7 pt to 20 pt at the median, and from 9 pt to 41 pt at the 95th
  percentile. `AXMoved` arrives about 8.7 times per second during a drag.
- An unmanaged window's old cut stays for 845 ms, against 37 ms in `events`.
- The Dome layout, minimum size, and focus spam end cases passed in both modes. `ax-geometry` drew
  the Dome layout 3 ms after the AX writes, against 24 ms for `events`.

A later spike could test a second call a fixed time after each leading call, so that a single
event also gets a read after WindowServer applies the change. This run does not measure it.

### Findings for Dome

- An AX z-order notification can arrive before CGWindowList shows the change. In the raise, open,
  close, launcher Escape, and Mission Control Escape cases, a CGWindowList call 0 to 1 ms after the
  notification returned the old list.
- An AX frame read took 0.41 ms and 0.45 ms at the mean during a drag, and 1.0 ms and 1.1 ms during
  a resize. The longest read took 7.1 ms, during a Dome layout. No read failed, and the title match
  found every managed window at once.
- The AX frame and the CGWindowList bounds agree once a window stops. During the Dome layout, the
  once-a-second call found no unreported change in either run. During a drag it found the dragged
  window moved 20 and 18 times. During a resize it found 0 and 19, because the frame keeps changing
  between two AX reads.
- A focus spam at a 10 ms interval produced about 99 `AXFocusedWindowChanged` per second.
- An AX size write that the app clamps to its minimum size still returns success. The window took
  800x532 for a 700x420 request.
- A layout change between the two tilings produced 6 move or resize notifications, not 8. A window
  whose position does not change sends no `AXMoved`.

### Tests

`cargo test` also covers the z-order throttle, the signed drag lag scan, and the overlay log parser
that the CPU table reads.

## Fourth spike: AX position reads at 60 Hz

The plan is `plans/border-overlay-ax-position-reads-spike.md` in the parent repository. It adds the
mode `ax-position-reads`. That mode starts from `ax-geometry` and makes two changes.

- A moved notification from a managed window starts a 16 ms read timer for that window
  (`on_read_tick` in `src/main.rs:632`). The timer stops 250 ms after the window's last moved
  notification.
- Each z-order call schedules one more call 50 ms later (`second_timer` in `src/main.rs:590`). The
  once-a-second call no longer skips.

Two new driver variants move 4 overlapping managed windows at the same time. The focus moves
between them every 150 ms. "Layout jumps" moves the windows to a new layout every 300 ms. "Smooth
moves" moves them with AX writes at 60 Hz. The per-frame check is `check_movie` in
`src/bin/driver/frames.rs:209`. It fails a frame where a magenta run has the same window color on
both sides.

The driver now waits up to 30 s for the overlay to start, up from 5 s. An overlay in an AX mode
takes about 7 s to start.

### Run

`./target/release/driver --out out/ax-reads` ran `events`, `ax-geometry`, `ax-position-reads`, and
the baseline with no overlay. Each CPU scenario ran twice. The 1 minute load average was between
about 2.8 and 7.6 at the start of each scenario. The full tables are in `out/ax-reads/results.md`.

### Results

Drag lag, from `out/ax-reads/results.md`:

| Mode | Moving frames | Median pt | 95th percentile pt | Move notifications/s | AX reads/s | AX reads with a new position/s |
|---|---|---|---|---|---|---|
| events | 113 | 7.0 | 9.0 | 8.5 | 0.0 | 0.0 |
| ax-geometry | 111 | 20.0 | 46.0 | 9.0 | 9.0 | 9.0 |
| ax-position-reads | 114 | 21.0 | 41.0 | 8.5 | 66.4 | 8.5 |

Per-frame check of the moving windows:

| Variant | Mode | Share of frames failing | Longest failing run ms |
|---|---|---|---|
| layout jumps | events | 25.0% | 42 |
| smooth moves | events | 10.0% | 25 |
| layout jumps | ax-geometry | 53.8% | 258 |
| smooth moves | ax-geometry | 57.6% | 242 |
| layout jumps | ax-position-reads | 47.3% | 75 |
| smooth moves | ax-position-reads | 28.1% | 58 |

CPU, with both runs in each cell:

| Scenario | Mode | CGWindowList calls/s | AX reads/s | Overlay CPU % | WindowServer CPU % |
|---|---|---|---|---|---|
| drag | events | 58.9 | 0 | 4.46, 1.83 | 41.3, 40.2 |
| drag | ax-geometry | 1.0 | 8.6 | 0.40, 0.40 | 31.4, 31.2 |
| drag | ax-position-reads | 1.0 | 70.0 | 1.54, 1.68 | 32.2, 32.4 |
| drag | no overlay | - | - | - | 29.0, 29.9 |
| layout jumps | events | 59.8 | 0 | 2.53, 2.63 | 18.4, 18.2 |
| layout jumps | ax-geometry | 8.8 | 26.5 | 1.19, 1.19 | 16.0, 16.6 |
| layout jumps | ax-position-reads | 19.3 | 238 | 4.17, 4.32 | 14.8, 16.1 |
| layout jumps | no overlay | - | - | - | 13.9, 15.1 |
| smooth moves | events | 59.8 | 0 | 3.14, 3.39 | 41.2, 35.8 |
| smooth moves | ax-geometry | 8.9 | 238 | 4.49, 4.64 | 40.5, 37.1 |
| smooth moves | ax-position-reads | 19.1 | 487 | 6.14, 6.19 | 41.0, 40.0 |
| smooth moves | no overlay | - | - | - | 36.4, 36.7 |
| focus spam | events | 58.9 | 0 | 2.38, 2.37 | 41.0, 42.3 |
| focus spam | ax-geometry | 9.5 | 0 | 1.39, 1.39 | 40.7, 40.0 |
| focus spam | ax-position-reads | 19.9 | 0 | 1.58, 1.58 | 42.0, 39.6 |
| focus spam | no overlay | - | - | - | 39.8, 40.8 |

Second z-order calls in `ax-position-reads`, with the count that found a change the first call
missed:

| Scenario | Second calls | Found a change |
|---|---|---|
| app switch | 21, 21 | 20, 20 |
| focus spam | 49, 49 | 13, 24 |
| layout jumps | 186, 187 | 94, 91 |
| smooth moves | 182, 180 | 87, 88 |

Still images:

- `ax-position-reads` passed every case that `ax-geometry` failed from the timing bug. These are
  raise, overlap, launcher Escape, open, close, Mission Control Escape, and smooth moves stop.
- Raise now takes 57 ms to draw correctly, compared to 1431 ms in `ax-geometry`. Open takes 83 ms.
  App switch takes 66 ms.
- Order front, minimize, restore, and Mission Control failed at 100 ms in every mode, as before.
- QuickLook open failed at 100 ms in both AX modes, with 296 points over content. The once-a-second
  call found the `qlmanage` window 931 ms after it appeared. `events` passed this case in this run.
- Unmanaged window moves failed at 100 ms in both AX modes, as the design allows.

### Verdict

- Drop the 60 Hz read timer. During a title-bar drag, only about 8.5 AX reads a second return a new
  position. That is the move notification rate. The drag lag stays at 21 pt at the median and 41 pt
  at the 95th percentile, the same as `ax-geometry`.
- The timer also adds nothing for moves made through AX writes. In smooth moves, `ax-geometry`
  already got 238 new frames a second from the notifications alone. The timer doubled the reads and
  raised the overlay CPU from about 4.6% to 6.2%.
- Keep the second z-order call and the once-a-second call that never skips. They remove the 1.4 s
  wrong borders after a raise, an open, or a close.
- With 4 windows moving and the focus changing every 150 ms, `ax-position-reads` still fails more
  frames than `events`. Each focus change leaves up to about 50 ms of wrong z-order, until the
  second call.
- The second call doubles the z-order calls under focus spam, from 9.5 to 19.9 a second.

### Findings for Dome

- During a drag that WindowServer drives, an app's AX position changes only at its move
  notification rate, about 8.5 to 9 times a second. A more frequent AX read returns the same
  position.
- For a move made through AX writes, each write produces a notification. An AX read after each
  notification then sees every frame.
- A CGWindowList call right after an activation notification missed the reorder in 20 of 21 app
  switches. The call 50 ms later found it every time.
- `qlmanage` owns the QuickLook window. The window appeared about 1.2 s after the launch event of
  `qlmanage`. No window event came for it. The once-a-second call found it.
- Even `events`, with CGWindowList at 60 Hz, fails 10% to 25% of frames when windows move and the
  focus changes every 150 ms. Its longest failing run is 42 ms.

## Fifth spike: CGWindowList at 60 Hz for the moving windows only

The plan is `plans/border-overlay-moving-window-list-spike.md` in the parent repository. It adds the
mode `moving-window-list`. That mode starts from `ax-position-reads` and keeps its timing fix. It
replaces the AX read timer with a CGWindowList read of the moving windows.

- A moved notification from a managed window adds it to the moving set (`MovingSet` in
  `src/moving.rs:8`). A window leaves the set `READ_TAIL` after its last moved notification.
- One 16 ms timer runs while the set is not empty (`keep_listing` in `src/main.rs:659`).
- Each tick makes one call for all windows in the set (`on_moving_tick` in `src/main.rs:670`,
  through `query_windows_by_id` in `src/windows.rs:110`). The call uses
  `CGWindowListCreateDescriptionFromArray`. It patches only those rects in the cache.

The driver gained `--no-idle-wait`. With it, a gate waits only for a large system window to clear.
The user was at the machine during this run, so user input may have disturbed a scenario.

### Run

`./target/release/driver --out out/moving-window --no-idle-wait` ran `events`,
`ax-position-reads`, `moving-window-list`, and the baseline with no overlay. Each CPU scenario ran
twice. The 1 minute load average was between about 2.5 and 9.5 at the start of each scenario. The
full tables are in `out/moving-window/results.md`.

### Results

Drag lag:

| Mode | Moving frames | Median pt | 95th percentile pt |
|---|---|---|---|
| events | 115 | 7.0 | 9.0 |
| ax-position-reads | 114 | 21.0 | 41.0 |
| moving-window-list | 113 | 0.0 | 9.0 |

CPU, with both runs in each cell:

| Scenario | Mode | CGWindowList calls/s | Mean CGWindowList µs | Overlay CPU % | WindowServer CPU % |
|---|---|---|---|---|---|
| drag | events | 58.9, 59.0 | 1167, 1985 | 3.71, 3.76 | 40.8, 39.0 |
| drag | ax-position-reads | 1.0, 1.0 | 964, 1286 | 1.73, 1.34 | 32.6, 29.3 |
| drag | moving-window-list | 62.3, 62.3 | 1051, 1002 | 2.87, 2.62 | 38.4, 37.1 |
| drag | no overlay | - | - | - | 28.0, 28.1 |
| layout jumps | events | 59.8, 59.8 | 1275, 1151 | 2.38, 2.28 | 19.2, 20.7 |
| layout jumps | ax-position-reads | 19.2, 19.1 | 1123, 1233 | 3.97, 3.72 | 15.7, 16.8 |
| layout jumps | moving-window-list | 69.0, 69.1 | 801, 775 | 2.63, 2.48 | 17.3, 17.4 |
| layout jumps | no overlay | - | - | - | 16.9, 15.7 |
| smooth moves | events | 59.7, 59.8 | 713, 1584 | 3.14, 3.15 | 40.4, 37.7 |
| smooth moves | ax-position-reads | 19.2, 19.2 | 1432, 1507 | 5.94, 5.94 | 42.9, 40.0 |
| smooth moves | moving-window-list | 81.0, 81.4 | 1112, 916 | 5.19, 5.09 | 42.2, 40.8 |
| smooth moves | no overlay | - | - | - | 41.0, 39.3 |

Per-frame check of the moving windows:

| Variant | Mode | Share of frames failing | Longest failing run ms |
|---|---|---|---|
| layout jumps | events | 27.4% | 42 |
| smooth moves | events | 6.9% | 17 |
| layout jumps | ax-position-reads | 50.8% | 67 |
| smooth moves | ax-position-reads | 28.1% | 50 |
| layout jumps | moving-window-list | 45.9% | 75 |
| smooth moves | moving-window-list | 28.8% | 50 |

Still images: `moving-window-list` failed the same cases as `ax-position-reads`. These are
QuickLook open, order front, minimize, restore, unmanaged window moves, and Mission Control at
100 ms. `events` failed order front, minimize, restore, and Mission Control at 100 ms.

### Verdict

- `moving-window-list` removes the drag lag. Its median lag is 0 pt and its 95th percentile is 9 pt,
  the same as `events`.
- It saves little of the WindowServer cost of a drag. WindowServer used 38.4% and 37.1%, compared to
  40.8% and 39.0% for `events` and 28.0% and 28.1% with no overlay. A call for the moving windows
  costs WindowServer about as much as a call for the full list. The cost follows the number of
  calls, not the number of windows in each call.
- It does not improve the frames with 4 windows moving and the focus changing every 150 ms. Its
  share of failing frames is the same as `ax-position-reads`. So those failures come from the
  z-order, which both modes refresh only after a z-order event and 50 ms later. `events` refreshes
  the full list, including the z-order, 60 times a second during a move.
- For about the same WindowServer cost during a move, `events` also refreshes the z-order. It had
  fewer failing frames and passed two more still cases. `moving-window-list` had a lower overlay CPU
  and a lower median drag lag.

### Findings for Dome

- A CGWindowList call for a few windows costs WindowServer about the same as a call for the full
  list. To cut the WindowServer cost, cut the number of calls.
- During a drag, only CGWindowList at 60 Hz removed the lag. AX reported the position about 9 times
  a second in every mode.
