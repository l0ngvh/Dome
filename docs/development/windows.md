# Windows Platform Details

Details specific to the Windows shell that don't fit in the main [architecture doc](architecture.md).

## Startup Awareness Contract

`ensure_per_monitor_v2_awareness()` runs at the top of `run_app`, before any HWND is created. It calls `SetProcessDpiAwarenessContext(PMv2)` and, on failure, probes the current process awareness with `GetDpiAwarenessContextForProcess` + `AreDpiAwarenessContextsEqual`. If the process is already at PMv2 (set by a manifest, user compatibility shim, or prior call), startup continues with an `info` log. If awareness is pinned to a different level, startup aborts with an `anyhow::bail!` because every downstream geometry and rendering assumption depends on PMv2. This closes BRD risk #6 (silent awareness downgrade producing systemic wrong geometry). The abort is cheap -- no user state has been committed at this point.

## DPI Scale Map

`Dome` stores per-monitor state in a `pub(super) monitors: HashMap<MonitorId, MonitorState>`. `MonitorState` bundles `dimension: Dimension`, `scale: f32`, and `displayed: Option<DisplayedMonitor>` into a single struct.

Core is coordinate-system-agnostic: `Monitor.dimension` holds whatever rect the platform supplies in its native frame. On Windows under PMv2, that frame is physical pixels. `MonitorState.scale` is the config-to-frame-unit multiplier -- the monitor's DPI scale (e.g. 1.25 at 125%, 1.5 at 150%, 2.0 at 200%). Config values like `border_size` are multiplied by `scale` before use in placement math. On macOS, `scale` is always 1.0 because AppKit geometry is already in the same logical-point unit as config values.

- **Populated** in `Dome::new` from `ScreenInfo` (primary insert + non-primary loop).
- **Maintained** by `reconcile_monitors`: inserted on new monitor, removed on dropped monitor, updated when either dimension or scale changes. The changed-screen predicate triggers on a dimension diff OR a scale diff, so a scale-only change (same physical `rcWork`, different effective DPI) is handled.
- **Read** via direct indexing: `self.monitors[&id].scale`. `HashMap::Index` panics on lookup miss, preserving the fail-fast contract -- a miss means a bug in monitor lifecycle management (every caller passes a `MonitorId` obtained from Hub, which only returns IDs for registered monitors).

`self.monitors[&id].scale` is the single read path for all downstream DPI consumers (placement border scaling, overlay sizing, renderer resize, icon capture). It is used in `show_tiling`, `show_float`, and `window_drifted` in `dome/window.rs` for border scaling, in the overlay/picker creation and update sites for render-surface sizing, and indirectly via `Dome::picker_scale()` for icon capture density. `monitor_dpi_changed` does not use this path; it writes `MonitorState.scale` directly because it needs the old value for same-scale dedup before updating.

### Scale guarantees

Scale values originate from `dpi::scale_for_monitor` (calls `GetDpiForMonitor`). The wrapper:

- Returns a strictly positive `f32` (minimum 1.0 on API failure).
- Logs `tracing::warn!` on failure so operators can diagnose from `dome.log`.
- Never returns 0, so the `debug_assert!(scale > 0.0)` in the conversion helpers holds at every production call site.

These wrapper-level fallbacks are distinct from the `monitors[&id]` indexing, which panics on unknown monitor ID. The wrapper handles Win32 API failures (rare, hardware-level); the map indexing enforces that callers only ask about monitors Dome knows about (a logic invariant).

## DPI Conversion Module

`src/platform/windows/dpi.rs` contains DPI-related arithmetic. Because core is coordinate-system-agnostic and delivers physical pixels on Windows, most frame conversion helpers are gone. The remaining helpers handle config-to-physical scaling and cast-only surface sizing:

| Function | Purpose | Consumed by |
|---|---|---|
| `logical_to_physical(v, scale)` | Rounded cast from `f32` to physical `i32` (multiply by scale, round) | `TilingOverlay::rerender` (cached monitor dimension to physical surface) |
| `constraints_to_physical(min, max, border)` | WM_GETMINMAXINFO constraints minus invisible borders, cast to f32 | `handle::get_size_constraints` |
| `surface_size_from_physical(dim)` | Physical `Dimension` to `(x, y, w, h)` with unsigned width/height | `TilingOverlay::new`, `TilingOverlay::update`, `FloatOverlay::update` |
| `scale_for_monitor(hmonitor)` | `GetDpiForMonitor` wrapper | `display::get_all_screens` |
| `picker_physical_rect(scale, monitor_physical)` | Centre 400x300 logical picker on physical monitor, scale and clamp | `PickerWindow::new`, `PickerWindow::show` |
| `icon_px_for_scale(scale)` | `ICON_PX_LOGICAL` (24) x scale, floor 16 physical px | `load_app_icon` |

`scale_for_monitor` is `#[cfg(target_os = "windows")]`; the other helpers are pure and compile on all targets. `picker_physical_rect`, `icon_px_for_scale`, and `ICON_PX_LOGICAL` are `pub(in crate::platform::windows)`. The rest are `pub(super)`. `PICKER_WIDTH_LOGICAL` / `PICKER_HEIGHT_LOGICAL` are module-private (only consumed by `picker_physical_rect`).

## Core-to-Shell Boundary

Core is coordinate-system-agnostic. On Windows, both core and Win32 APIs work in physical pixels under PMv2, so there is no unit conversion at the core-to-shell boundary. Hub delivers placement frames (from `get_visible_placements`) in physical pixels; `show_tiling`, `show_float`, and `show_fullscreen_window` round and cast these to `i32` for `SetWindowPos` directly. Float drift observations (physical DWM extended frame bounds) are stored directly and synced back to Hub without conversion. Size constraints from `WM_GETMINMAXINFO` are already physical; `constraints_to_physical` subtracts invisible borders and casts to `f32` for Hub.

The remaining DPI-sensitive boundaries are:

- **Config-sourced lengths** (e.g. `border_size`). These are logical (config-denominated) values, multiplied by `MonitorState.scale` at the point of use in `show_tiling`, `show_float`, and `window_drifted`.
- **wgpu surface sizing.** `surface_size_from_physical` casts the physical `Dimension` to `(i32, i32, u32, u32)` for `Renderer::resize`. `TilingOverlay::rerender` uses `logical_to_physical` to derive physical dimensions from its cached monitor + scale.
- **Picker window sizing.** `picker_physical_rect` scales the 400x300 logical picker size by the monitor's DPI scale and centres it within the physical monitor rect.
- **Icon capture.** `icon_px_for_scale` multiplies `ICON_PX_LOGICAL` (24) by the monitor scale for capture density.

`DriftState.target` stays in physical pixels. Both sides of the drift comparison (target and observation) are physical, so the tolerance threshold works without conversion.

Functions that stay physical (no conversion needed): `get_visible_rect`, `get_invisible_border`, `get_dimension`. Their doc comments pin the unit. `OFFSCREEN_POS` is also physical -- it is a Windows shell convention not scaled by PMv2.

Live `WM_DPICHANGED` handling does not add a new boundary concern. The wnd-proc arms extract DPI from `WPARAM` and monitor handle from `MonitorFromWindow`, then post `WM_APP_DPI_CHANGE` as a thread message. The dome-thread decode calls `Runner::handle_dpi_change` which updates `MonitorState.scale` via `Dome::monitor_dpi_changed` and re-runs `apply_layout`. All downstream placement and overlay sizing reads the updated scale from `self.monitors[&id].scale`.

## Live DPI Transitions

When a user changes a monitor's display scale in Settings, Windows posts `WM_DPICHANGED` to affected top-level windows. Unlike `WM_DISPLAYCHANGE` (which is broadcast to every top-level window), `WM_DPICHANGED` is per-window: only HWNDs whose hosting monitor's DPI changed receive it. This means a single wnd-proc on the primary monitor cannot observe a DPI change on a secondary monitor. Every Dome-owned wnd-proc must handle the message independently.

### Entry path

All four Dome-owned wnd-procs (`app_wnd_proc`, `float_overlay_wnd_proc`, `tiling_overlay_wnd_proc`, `picker_wnd_proc`) handle `WM_DPICHANGED` identically:

1. Extract new DPI from `LOWORD(wparam)`. X and Y DPI are equal on conforming displays; `HIWORD` is discarded.
2. Resolve the reporting monitor via `MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST)`.
3. Post `WM_APP_DPI_CHANGE` (WPARAM = DPI, LPARAM = HMONITOR handle) to the dome thread.
4. Return `LRESULT(0)` to suppress `DefWindowProcW`'s auto-resize to the suggested RECT.

The dome-thread message loop decodes `WM_APP_DPI_CHANGE` and calls `Runner::handle_dpi_change(handle, dpi)`, which delegates to `Dome::monitor_dpi_changed` then calls `apply_layout`.

`monitor_dpi_changed` looks up the HMONITOR handle in `monitor_handles`, computes the new scale from the DPI value, and updates `MonitorState.scale`. It then calls `self.hub.update_monitor(id, current_dimension, new_scale)` to propagate the new scale into core so layout math uses the updated multiplier when the caller-scheduled `apply_layout` reruns. It does not touch `MonitorState.dimension` because a scale-only change does not alter the physical `rcWork` rect.

### Same-scale dedup

On the primary monitor, all four Dome-owned HWNDs receive `WM_DPICHANGED` and each posts `WM_APP_DPI_CHANGE` with the same `(dpi, handle)` pair. `monitor_dpi_changed` early-returns when the computed scale equals the stored scale, so the first post updates state and logs; subsequent posts are silent no-ops. `apply_layout` still runs on each post (it is unconditional in `Runner::handle_dpi_change`) but is idempotent.

On secondary monitors, only the overlay HWNDs sitting on that monitor receive the message, so this dedup rarely triggers there.

### WM_GETDPISCALEDSIZE suppression

Windows 11 (23H2+) auto-resizes PMv2 windows to a scaled suggested RECT on `WM_DPICHANGED`, even when the handler returns 0 without calling `DefWindowProcW`. The suggested RECT is derived from the size reported in `WM_GETDPISCALEDSIZE`, which arrives before `WM_DPICHANGED`.

Every Dome-owned wnd-proc handles `WM_GETDPISCALEDSIZE` by writing the current window size (via `GetClientRect`) into the reply `SIZE*` and returning `LRESULT(1)`. This identity reply is correct under the agnostic-core model: core stores physical pixels on Windows, so the OS's suggested physical size is already the final frame unit and no rescaling is needed. The effect is that Windows resizes to the "current" size, holding geometry stable until `apply_layout` produces the correct new-scale placement.

The helper `wm_getdpiscaledsize_reply(current: SIZE) -> SIZE` returns its argument unchanged. It exists as a unit-testable seam documenting Dome's "suppress OS auto-resize" intent. Because all four Dome HWNDs are borderless `WS_POPUP` with no non-client area, `GetClientRect` equals window size. Future window classes with a title bar must not copy this pattern without adding the non-client delta.

Windows 10 also delivers `WM_GETDPISCALEDSIZE` (introduced in 1703) but does not auto-resize on `WM_DPICHANGED`, so the reply has no visible effect there. The arm is cheap to carry and keeps behavior uniform.

### WM_GETDPISCALEDSIZE constant

`WM_GETDPISCALEDSIZE` (0x02E4) is manually defined in `src/platform/windows/mod.rs` because the `windows` crate (v0.62) does not export it. The constant is defined in `WinUser.h`. If a future `windows` crate version adds it, the manual definition should be removed to avoid drift.

### Thread-message design

`WM_APP_DPI_CHANGE` (`WM_APP + 3`) follows the same pattern as `WM_APP_DISPLAY_CHANGE`: the wnd-proc posts, the dome-thread message loop decodes, and the runner mutates state. Direct state mutation inside a wnd-proc arm is unsafe because `apply_layout` calls `SetWindowPos`, which can dispatch synchronous messages back into the wnd-proc and produce unbounded re-entry. The thread-message hop breaks this re-entrancy chain. DPI events mutate only Dome's own state and enqueue `apply_layout`; no cross-process `SendMessage` variants are used in this path.

No `HubEvent` variant was added. DPI change is platform-owned and carries Windows-specific payload (HMONITOR + raw DPI) that has no meaning on macOS. The dome-thread message loop already demultiplexes platform-only events; `WM_APP_DPI_CHANGE` fits the same slot.

### Cross-monitor tile moves

When a tiled window moves between monitors with different scales, the move flows through `HubAction::Move` and inherits the destination monitor's scale via the normal placement path (`show_tiling` indexing `self.monitors[&id].scale`). There is no DPI-specific code path for cross-monitor moves; the existing boundary-site table above covers it.

### Wnd-proc maintenance rule

Every new Dome-owned window class must add both `WM_DPICHANGED` (post `WM_APP_DPI_CHANGE` and return 0) and `WM_GETDPISCALEDSIZE` (reply current size and return 1) arms. A forgotten arm silently drops DPI changes for that class's hosting monitor or produces a wrongly-sized frame flash on Windows 11.

## Picker & Icon Capture

The picker window and its icon captures are scale-aware.

**Sizing.** `PICKER_WIDTH_LOGICAL` (400) and `PICKER_HEIGHT_LOGICAL` (300) are module-private constants in `dpi.rs`. `dpi::picker_physical_rect(scale, monitor_physical)` scales the logical picker size to physical, then centres and clamps within the physical monitor rect. Both `PickerWindow::new` and `PickerWindow::show` call this helper instead of using fixed physical constants.

**Icon capture.** `load_app_icon(hwnd, scale)` captures at `dpi::icon_px_for_scale(scale)` physical pixels per edge. The helper multiplies the logical icon size (`ICON_PX_LOGICAL` = 24, matching `ICON_SIZE` in `src/picker.rs`) by the monitor scale and applies a 16-physical-pixel floor (below 16px, shared HICONs lose recognisable shape). `Dome::picker_scale()` returns the visible picker's monitor scale (`None` when hidden); `dispatch_picker_icons` in `runner.rs` passes it to `load_app_icon`, falling back to `2.0` when the picker is hidden (preserves the legacy 48px capture quality in the narrow race window between action-fire and rayon dispatch).

**Cache invalidation.** `PickerWindow::show` clears `icon_textures` when the incoming scale differs from the stored `pixels_per_point`. This forces re-capture at the new density when the picker moves between monitors of different scales. No multi-resolution cache: the one-frame re-capture cost at scale boundaries is acceptable and keeps the cache shape matching macOS.

**HICON source cap.** `DrawIconEx` captures from the shared HICON returned by `WM_GETICON(ICON_BIG)`, typically a 32×32 handle. Captures above ~48 physical pixels (scale > 2.0) show interpolation blur because `DrawIconEx` has only that 32×32 source to work from. This is a known limitation tracked against BRD risk #7. The follow-up APIs are `PrivateExtractIconsW` (user32) and `LoadIconWithScaleDown` (commctrl.h), which can select higher-resolution authored variants from the PE resources.

## EXE Resources

`build.rs` compiles `resources/windows/dome.rc` into a linkable resource object via the [`embed-resource`](https://crates.io/crates/embed-resource) crate (v3). The resource script is the single source of truth for what gets embedded into `dome.exe`.

### Resource files

| File | Purpose |
|---|---|
| `resources/windows/dome.rc` | Resource script. References manifest and icon by filename. |
| `resources/windows/dome.manifest` | PMv2 application manifest (XML). Sets DPI awareness (with `PerMonitorV2,PerMonitor` graceful-degradation fallback), supported OS GUIDs, and `asInvoker` execution level. |
| `resources/windows/Dome.ico` | Application icon. Placeholder until final artwork lands. Swap is a single-file change, no build-pipeline change needed. |

### Why both manifest and runtime check

The manifest and `ensure_per_monitor_v2_awareness()` (see [Startup Awareness Contract](#startup-awareness-contract)) cover disjoint failure modes:

- **Manifest**: read by the Windows loader before any code runs, including DLL-injected code in `DllMain`. Covers HWNDs created by screen readers, input methods, AV hooks, etc. before `main`.
- **Runtime check**: catches AppCompat shims, group-policy overrides, and per-exe user overrides that can override a manifest after process init.

Both stay.

### Cross-compile prerequisite

`embed-resource` invokes `windres` for `*-pc-windows-gnu` targets. Cross-compiling from macOS requires mingw-w64:

```bash
brew install mingw-w64
```

This puts `x86_64-w64-mingw32-windres` on `PATH`. Without it, `build.rs` fails fast (via `.manifest_required().unwrap()`) with a diagnostic error. This prerequisite applies to `cargo clippy --target=x86_64-pc-windows-gnu --tests` as well, since the build script runs during that check.

### Version drift

`assemblyIdentity/@version` in `dome.manifest` is hand-synced with `Cargo.toml`. Drift is a known cold bug until a release-tooling plan automates it.
