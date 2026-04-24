# Architecture

## Overview

Dome has a platform-independent core and platform-specific shells. The core models the window tree and computes layout with zero OS dependencies, tested in isolation with snapshot tests, no OS mocking. Each platform structures its internals independently; the shared contract is Hub operations + placement structs.

```
src/
├── core/           # Tree model, layout, hub — zero OS dependencies
├── platform/
│   ├── macos/      # AX API, Metal overlays, CGEvent tap
│   └── windows/    # Win32, WinEvent hooks, OpenGL overlays
└── ...             # overlay, config, action, ipc, logging
```

## Core

### Tree Model

```text
Monitor
  └── Workspace (one visible per monitor, created lazily by name)
        ├── root: Container tree (tiling)
        ├── float_windows: [(WindowId, Dimension)]
        └── fullscreen_windows: [WindowId]

Container (split horizontal | split vertical | tabbed)
  └── children: [Child]   where Child = Container(id) | Window(id)
```

Windows are always leaves. Workspaces are created on demand by name — no fixed set.

Nodes are stored in a hash map with monotonically increasing typed IDs (`WindowId`, `ContainerId`, `MonitorId`, `WorkspaceId`). Hash map over Vec because nodes are frequently deleted (windows close, containers merge) and IDs must remain stable. Typed IDs prevent mixing at compile time. IDs are never reused, so a stale ID can't refer to a new node.

The shared `Window` struct holds only mode, workspace, restrictions, title, and size constraints. Tiling-specific per-window state (parent, dimension, spawn_mode) lives in `PartitionTreeStrategy`'s `HashMap<WindowId, TilingWindowData>`, not on `Window`. This parallels how containers store their state in the strategy's `Allocator<Container>`. Float and fullscreen windows have no tiling data.

**Direction invariance.** A split container never has the same direction as its parent split container. Without this, the same visual layout maps to multiple trees, making "move right" ambiguous. Enforced during toggle and restructure by walking child containers and flipping any that match their parent's direction.

### Window Modes

**Tiling** — the default. Windows live in the container tree and participate in layout.

**Float** — separate list on the workspace, outside the container tree. Keeping them out of the tree avoids layout needing to skip them and directional nav needing to special-case them.

- Directional focus/move are no-ops on floats.
- Toggle float→tiling: reattach next to last focused tiling window. Tiling→float: keep current screen position.

**Fullscreen** — separate list on the workspace. If non-empty, tiling and float windows are skipped in placements. Detection is platform-specific (see [Fullscreen Detection](#fullscreen-detection)).

**Window restrictions.** The platform sets a `WindowRestrictions` enum on fullscreen windows; core checks it without knowing why.

- `None` — no restrictions.
- `BlockAll` — Windows exclusive fullscreen. Blocks all user commands including focus and workspace switching.
- `ProtectFullscreen` — macOS native/borderless, Windows borderless. Blocks display mode changes and cross-monitor moves; allows workspace moves and navigation.

Checks run at the top of each user-facing command against the *focused* window only. Unfocused `BlockAll` windows don't block commands — on Windows, exclusive fullscreen windows that lose focus exit exclusive mode. Focus can move away via `set_focus`, a lifecycle operation not guarded by restrictions.

Getters and tree helpers are pure. Lifecycle operations (`insert_tiling`, `delete_window`, `set_focus`, etc.) are never restricted.

### Focus Model

Focus is split into three independent mechanisms, one per window mode:

1. **Fullscreen focus** is implicit. If `fullscreen_windows` is non-empty, the last element is the focused fullscreen window. Focusing a fullscreen window moves it to the end of the vec.
2. **Float focus** uses z-order. `float_windows` is z-ordered (last = topmost = focused). Focusing a float moves it to the end of the vec. A separate `is_float_focused` bool on Workspace tracks whether float mode has focus.
3. **Tiling focus** uses a dedicated `focused_tiling: Option<Child>` pointer on Workspace. Only set by the strategy's `set_focus` method.

The `focused()` accessor on Workspace computes effective focus by checking in priority order: fullscreen > float > tiling. All external reads go through this accessor. The three mechanisms are independent -- `focused_tiling` persists even when fullscreen or float windows are active, serving as "tiling focus memory." When fullscreen is unset or float is unfocused, tiling focus is restored without recomputing.

**Container highlight.** When `focused_tiling` is `Child::Container` (after `focus_parent`), `focused_tiling_window()` returns `None` rather than walking to a leaf. This means `hub.focused_window()` returns `None` when a container is highlighted (assuming no fullscreen/float focus), which makes `toggle_float` and `toggle_fullscreen` no-ops and causes the platform to receive `focused_window: None` in placements, focusing the tiling overlay. Move-to-workspace and move-to-monitor bypass `focused_window()` in this case and call the strategy directly to move the whole container.

**Invariant:** `is_float_focused` must be false when `float_windows` is empty. The test validator enforces this. The `focused()` accessor also handles it gracefully as defense-in-depth, falling through to `focused_tiling`.

**Write paths.** The strategy's `set_focus` is purely tiling: it walks up the container tree updating `container.focused` and active tabs, sets `focused_tiling`, and clears `is_float_focused`. `set_focus` (the entry point from the platform layer when the OS reports a focus change) branches by display mode: fullscreen promotes to top of the z-order stack, float sets `is_float_focused` and moves to end of `float_windows`, tiling delegates to `strategy.set_focus`.

**Detach cleanup.** Each detach function only cleans up its own mode's focus state. Cross-mode priority resolution happens at read time via `focused()`. For example, detaching the last float sets `is_float_focused = false` and `focused()` falls through to `focused_tiling`. Detaching the last tiling child with floats present sets `is_float_focused = true`. Detaching the last fullscreen window falls back directly to `focused_tiling`, skipping float. Users rarely focus float windows explicitly, so falling back to float would be surprising. There's no cross-mode fallback chain from fullscreen to float.

### Hub

Hub is the single entry point for all tree mutations, preventing scattered mutation sites that could violate invariants. The platform calls Hub operations, then `get_visible_placements()` for a flat list of `WindowPlacement` and `ContainerPlacement` with screen coordinates. The platform positions windows and renders overlays from those placements.

Hub never knows about OS handles, AX elements, or HWNDs — state changes are deterministic and testable.

### Tiling Strategy

Hub delegates all tiling-specific operations to a `TilingStrategy` trait (`src/core/strategy.rs`). This separates generic window management (monitors, workspaces, float, fullscreen, focus priority) from tiling behavior (container tree, spawn modes, split directions, layout).

The trait has a minimal surface: `attach_child`, `detach_child`, `handle_action`, `layout_workspace`, `set_focus`, `collect_tiling_placements`, `focused_tiling_child`, and `validate_tree` (test-only). Everything else (scroll, viewport clamping, container parent lookups, tree restructuring) is private to the strategy implementation.

`PartitionTreeStrategy` (`src/core/partition_tree/`) is the default and currently only implementation. It owns the container allocator and per-window tiling state (`HashMap<WindowId, TilingWindowData>` for parent, dimension, spawn_mode), and implements i3-style manual tiling: container tree with split horizontal/vertical/tabbed layout, spawn mode routing, direction invariance. All logic that was previously in `split.rs` (deleted) and the layout portion of `workspace.rs` now lives here.

Hub holds `access: HubAccess` (monitors, focused_monitor, workspaces, windows, config) and `strategy: Box<dyn TilingStrategy>` as disjoint fields. Strategy methods receive `&mut HubAccess` so they can read/write shared state without borrowing Hub. This solves the split-borrow problem.

`TilingAction` is an enum of tiling-specific commands (focus/move direction, toggle spawn mode, toggle direction, toggle layout, focus parent, focus tab). Hub's `command.rs` does restriction checks then delegates to `strategy.handle_action`. Float and fullscreen management stay on Hub.

### Layout

Two passes after every tree mutation: minimum sizes bottom-up, then space distribution top-down. You need the total minimum before distributing remaining space. Windows hitting max get centered in their allocated slot.

**Constraints.** Per-window min/max (set by platform) and global min/max (from config). Per-window max overrides global min — a fixed-size dialog shouldn't be forced larger, wasting dead space. Per-window min is floored by global min. Discovery is platform-specific (see [Constraints](#constraints)).

**Scrolling.** When min-constrained windows exceed screen width, a viewport enables scrolling. The focused window is scrolled into view (right edge → shift right, left edge → shift left, same vertical). Offset clamped to content bounds.

Placements are translated by viewport offset and clipped to screen bounds. Windows entirely outside are omitted; partially visible get a clipped `visible_frame`. Floats scroll with the viewport.

### Design Rules

- **Bounded loops.** Iterative with 10,000 upper bound, no recursion. A bug panics with a clear message instead of stack-overflowing the user's machine.
- **Core infallibility.** No `Result` types. Invariant violations panic immediately.

## Platform Layer

### Overview

The platform layer converts OS events into Hub operations, calls Hub for layout, then positions windows and renders overlays from placements.

**macOS — 2 threads:**

- **Main thread**: NSApplication, keyboard events (CGEvent tap), AX observers, overlay rendering (Metal).
- **Hub thread**: calloop event loop, owns `Dome`, processes events, sends rendering data back via abstracted transport.

The split prevents blocking the CGEvent tap, which would throttle keyboard input. Overlay rendering must happen on the main thread (macOS requirement). AX queries are slow IPC to the accessibility server, so they're dispatched to the GCD pool to avoid blocking the hub thread.

`GcdDispatcher` dispatches closures to `DispatchQueue::global_queue` (`UserInitiated` QoS). A zero-sized `DispatcherMarker` token enforces at compile time that blocking AX calls (position, size, title, validity, fullscreen status) only run on GCD queues. Results return to the hub thread via `Scheduler<ApplyFn>`, keeping result handling synchronous.

**Windows — 3 threads:**

- **Main thread**: bare Win32 message pump for hooks, WinEvent hooks (`SetWinEventHook`), IPC, config watching.
- **Keyboard hook thread**: minimal message pump for `WH_KEYBOARD_LL`. Looks up keymap, sends matched actions to dome thread. Isolated because Windows skips slow hook callbacks.
- **Dome thread**: `GetMessageW` pump, owns `Dome`, processes events, positions windows and overlays. Events arrive via `PostThreadMessageW` with custom `WM_APP` carrying a boxed `HubEvent`.

The dome thread dispatches blocking Win32 reads to a thread pool, receives results via `PostThreadMessageW` — analogous to macOS GCD dispatch.

### Window Manipulation

#### macOS

Accessibility API (`AXUIElement`) via `AXWindowApi` trait for position, size, focus, and property queries. AX calls are IPC to the accessibility server, dispatched to the GCD pool to avoid blocking the hub thread.

#### Windows

`SetWindowPos` for positioning, `ShowWindow` for minimize/restore. Focus uses simulated ALT + `SetForegroundWindow` — ALT satisfies the foreground lock exception.

DWM invisible borders affect positioning. `DwmGetWindowAttribute` queries extended frame bounds; `SetWindowPos` compensates.

### Window State Machines

Both platforms track window state for valid operations. Shared concepts: positioned/offscreen/fullscreen states, drift correction (target vs. actual position with up to 5 retries).

#### macOS

```text
WindowState
├── Positioned
│   ├── InView       — tiled/floating, active placement target
│   └── Offscreen    — hidden by Dome
├── NativeFullscreen  — in a separate macOS Space
├── BorderlessFullscreen — covers entire monitor (zoom, app shortcut)
└── Minimized         — borderless fullscreen that can't be moved offscreen
```

- `InView` carries target vs. actual position (integer coords for pixel-exact comparison). Stale observations filtered by coalesced timestamps from debouncing.
- `Offscreen` has its own drift detection and retry logic.
- All windows except borderless fullscreen start as `Offscreen` after discovery.
- Transitions: `Offscreen`↔`InView`, Any→`NativeFullscreen` (space-change + AX fullscreen attr), `NativeFullscreen`→`InView`/`BorderlessFullscreen`, `InView`↔`BorderlessFullscreen` (covers monitor, confirmed not Dome's placement), `BorderlessFullscreen`→`Minimized` (when hiding).
- User-minimized windows (not by Dome) are untracked and removed.

#### Windows

```text
WindowState
├── Positioned
│   ├── Tiling    — visible, tiled, with drift tracking
│   ├── Float     — visible, floating, with drift tracking
│   └── Offscreen — hidden by Dome
├── FullscreenBorderless  — covers entire monitor
├── FullscreenExclusive   — D3D/Vulkan (SHQueryUserNotificationState)
└── Minimized
```

- Each positioned window carries `DriftState` with target vs. actual and retry logic.
- User drags detected via `EVENT_OBJECT_LOCATIONCHANGE` bracketed by move/size start/end events. Drift correction suppressed during drags. 60-second safety timeout for missed drag-end.
- `FullscreenExclusive` bypasses the compositor; Dome skips overlay rendering.

### Hiding Windows

#### macOS

No API to hide another app's window without minimizing (dock animation). Dome moves windows to the bottom-right corner of the furthest monitor, 1px visible — macOS disallows fully offscreen. Borrowed from AeroSpace's virtual workspace approach.

Crash recovery must move all tracked windows back — can't distinguish "hidden by Dome" from "user-placed."

Borderless fullscreen windows are minimized instead: moving offscreen triggers false fullscreen-exit detection, and macOS zoomed windows ignore Dome's positioning requests.

#### Windows

Fully offscreen placement to -32000,-32000. Hidden windows set to `HWND_BOTTOM` z-order. Taskbar tabs removed via `ITaskbarList::DeleteTab`; restored when visible.

### Float Windows

#### macOS

Float windows are persistent overlays users rarely type into. macOS can't control window level of non-owned windows, so Dome hides the real window, captures via ScreenCaptureKit, and renders in an overlay at `NSFloatingWindowLevel`. When focused, the real window returns (keyboard events need it) and mirroring stops.

#### Windows

`SetWindowPos` sets z-order of any window — no mirroring needed. Float overlay set to `HWND_TOPMOST`.

Auto-float: windows without `WS_THICKFRAME`, or with `WS_POPUP`, `WS_EX_TOPMOST`, `WS_EX_DLGMODALFRAME` are inserted as float.

### Constraints

Both platforms discover size limits and report to Hub via `set_window_constraint()`, triggering relayout.

#### macOS

No API to query limits upfront — must place, wait, read back. AX fires moved/resized per-window, but the window element attached to the notification is unreliable (you can't trust which window it refers to), so reading too early returns stale values.

1. Hub computes layout, platform positions the window.
2. Window snaps to its own min/max.
3. AX fires notifications. Debounce timer resets on each.
4. Events go quiet, debounce fires. Platform reads actual size.
5. If actual differs from target, reports constraint via `set_window_constraint()`.
6. Hub relayouts.

On macOS, the constraint/drift check uses the first observed timestamp of the coalesced debounce burst: if it falls within 1s of the last placement, the burst is treated as the app reacting to that placement (possible constraint or edge drift). Bursts that start later than 1s after placement are treated as late-event drift and trigger a corrective `set_frame` via the shared 5-retry budget. We only limit the window 1s as there would be plenty move/resize events during a single Dome's session, plenty of oppotunities for constraint detections, so it's fine if we miss a few.

This causes at least one "wrong" frame for new windows as constraint detection takes time. Per-app debouncing to prevent incorrect constraint check, one window's resize can trigger the move/resize events while another in the same app is still moving.

#### Windows

`WM_GETMINMAXINFO` returns min/max without resizing — constraints known before first placement. Queried with `SendMessageTimeout` (`SMTO_ABORTIFHUNG`). Invisible border compensation subtracted from reported values. Set at insertion, refreshed on display change.

### Event Observation

#### macOS

AXObserver: per-app notifications (created, destroyed, moved, resized, focused, title changed). NSWorkspace: app lifecycle and space changes.

AX notifications are unreliable — missed, duplicated, or wrong window. A 5s sync timer reconciles all windows against AX state (add/remove, not focus). Each sync rebuilds all observers from scratch — they go dead and stop emitting. Rebuilding also handles failed registrations and terminated app cleanup. App launches register observers immediately; next sync rebuilds everything.

Reconciliation involves slow AX calls, so it runs on a GCD background queue. Results return to the hub thread synchronously.

#### Windows

WinEvent hooks (`SetWinEventHook`) fire for all window events across all processes. Reliable enough that periodic sync hasn't been needed.

### Throttling

**Focus throttling** prevents feedback loops where Dome focuses A, the OS queues a focus event for B, processing B focuses B, queuing A, etc. A throttle interval breaks the cycle. Windows uses 500ms.

**Resize debounce** waits for move/resize events to settle. Per-app on macOS, per-window on Windows.

macOS debounce is per-PID (per-app) because the window element in AX notifications is unreliable. You can't trust which window actually moved, so when events settle you must query all windows for the PID to get actual positions. This means you need to wait until the entire app goes quiet, not just one window. If you debounced per-window, some windows in the app might still be moving when others report stopping, and you'd read stale positions. On Windows, WinEvent carries a reliable HWND, so per-window debouncing works.

Pure debounce (not throttle) because constraint detection compares actual vs. target size, and during a drag the window is still being repositioned. A throttle would fire checks mid-drag, causing constraint detection to fight with the ongoing operation. Debounce waits until events stop (100ms quiet), then checks once after the window settles. The `set_pid_moving` flag suppresses layout corrections during the debounce window so the platform doesn't reposition a window the user is actively dragging.

On macOS, only the first and last timestamps of the debounce burst are tracked, as a single `(Instant, Instant)` tuple. The stale check uses `.1` (last): if even the most recent notification predates the last placement, the burst is discarded. The constraint/drift check uses `.0` (first): if the burst started within 1s of placement, constraint detection runs; otherwise the late-event drift path re-issues `set_frame` and consumes one retry.

### Suspend (macOS)

AX is unusable during sleep/screen lock — calls hang or error. A suspend flag causes all callbacks to bail early.

Resume on screen unlock only, not screen wake (screen can wake while locked). Keyboard action also clears the flag. Full sync on resume.

**CGWindowID reuse.** macOS reuses IDs of deleted windows. Validity checked via AX element query — `InvalidUIElement` means destroyed. During screen lock, AX errors for everything, so validity checks assume all windows valid. The suspend flag prevents reconciliation during lock; the validity guard is a safety net.

### Coordinate System (macOS)

Cocoa uses bottom-left origin; Hub uses top-left. Conversion via primary monitor height. AX already uses top-left, so only overlay positioning needs the flip.

### Fullscreen Detection

#### macOS

Native fullscreen: separate Space, detected via space-change + AX fullscreen attribute. Borderless: covers monitor, detected by position/size after move/resize.

#### Windows

Borderless: position/size covers monitor. Exclusive: D3D/Vulkan, detected via `SHQueryUserNotificationState`.

### Rendering

Both platforms use egui for borders and tab bars. Shared painting logic takes placements + config, draws into egui. Platform code handles windowing and GPU backend.

One tiling overlay per monitor draws all tiling borders and container overlays including tab bars. Float windows get separate overlays (they need z-ordering, mirroring on macOS). Each float overlay sized to `visible_frame`.

Border edges offset from full frame — clipped edges fall outside overlay bounds, clipped by egui. Tab bars are interactive (click sends event to hub thread). Focused window border shows spawn-mode indicator: one edge colored for next spawn direction.

**Empty workspace focus.** Dome focuses its own tiling overlay on empty workspaces to prevent keyboard focus landing on offscreen windows. On macOS, needed when switching to an empty workspace from another Space. On Windows, prevents unwanted workspace switches when destroying a window hands focus to an offscreen window.

#### macOS

Borderless transparent NSWindows with CAMetalLayer. Shared Metal backend: device, command queue, two pipelines — egui (premultiplied alpha for text blending) and mirror (passthrough). Each overlay has its own renderer.

ScreenCaptureKit captures IOSurface for mirroring, rendered as textured quad. Captures start/stop as floats gain/lose focus.

#### Windows

Win32 layered windows, egui_glow (OpenGL via glutin). Tab bars use DWM blur-behind for frosted glass. Dome thread manages overlays directly -- no cross-thread dispatch.

#### Windows Rendering Model

**DWM + OpenGL.** With DWM (always on since Windows 8), each window renders to an offscreen surface. `SwapBuffers` copies the GL front buffer to a DWM-managed D3D surface. DWM composites all surfaces to screen at vsync. `WM_PAINT`/`BeginPaint`/`EndPaint` are irrelevant to actual screen content -- DWM picks up content from `SwapBuffers`, not from the GDI paint cycle.

**Render-last invariant.** All overlay updates follow this sequence: data assignments, `SetWindowRgn(FALSE)`, `SetWindowPos(SWP_NOREDRAW)`, `ShowWindow` (first show only), GL render + `SwapBuffers`. Positioning calls (`SetWindowPos`, `SetWindowRgn` with `bRedraw=TRUE`) can trigger synchronous quick-repaints (WM_NCPAINT + WM_ERASEBKGND) that overwrite GL content. Rendering last ensures the GL content goes straight to DWM without being clobbered.

**WM_ERASEBKGND / WM_PAINT handlers.** All three window classes (app, float overlay, tiling overlay) handle `WM_ERASEBKGND` by returning `LRESULT(1)` to suppress background erase. `WM_PAINT` handlers only call `BeginPaint`/`EndPaint` to validate dirty regions as a safety net. Neither handler renders GL content. Removing `WM_PAINT` entirely could cause infinite WM_PAINT loops if something unexpected invalidates the window.

**No CS_HREDRAW|CS_VREDRAW.** GL windows don't use these class styles. They cause full-client invalidation on any size change, which is counterproductive when the application controls all rendering via `SwapBuffers`.

**Float overlay focus update.** In `show_float`, the `settled` check skips both positioning and overlay update when position is unchanged and no topmost change is needed. A separate `focus_changed` branch re-renders the overlay (without repositioning) when focus changes, so the border color updates even when the float's position hasn't moved.

### Recovery

#### macOS

POSIX signal handlers (SIGINT, SIGTERM, SIGHUP) + `catch_unwind` on both threads. All tracked windows moved to centered positions on primary monitor at original size.

#### Windows

Console control handler (Ctrl+C, Ctrl+Break, console close) + `catch_unwind` on dome thread. All tracked windows restored to (100, 100). Previously-maximized windows re-maximized. Taskbar tabs restored.

## Shared Subsystems

### IPC

The binary serves dual purpose: `dome`/`dome launch` starts the WM, `dome <action>` sends a command. Action variants (Focus, Move, Toggle, Exec, Exit) are clap subcommands and serde IPC payloads.

- macOS: Unix domain socket (`/tmp/dome.sock`), stale socket auto-cleaned.
- Windows: named pipe (`\\.\pipe\dome`).
- Protocol: one JSON-serialized Action per line, text response.
- Server on dedicated thread, forwards to hub thread.
- Startup connects to existing socket to detect running instance.

### Configuration

TOML config parsed with serde. Hot-reload via `notify` file watcher — changes sent as `HubEvent`.

- macOS: hub thread relayouts; main thread updates overlay config.
- Windows: dome thread does both.

Keymaps shared between keyboard listener and config update path with synchronization. macOS: `Arc<RwLock>`. Windows: `keyboard::update_config()` on config watcher thread.

### Launch at Login

`src/platform/macos/login_item.rs` manages a LaunchAgent plist (`~/Library/LaunchAgents/com.dome-wm.dome.plist`) for start-at-login on macOS. Uses `launchctl bootstrap`/`bootout` (not the deprecated `load`/`unload`). LaunchAgent chosen over `SMAppService` because `SMAppService` requires code signing.

`src/platform/windows/login_item.rs` manages the `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` registry key on Windows via the `windows-registry` crate. Registry chosen over Task Scheduler (overkill) and Startup folder shortcuts (requires COM `IShellLink`).

Both are synced on startup and on config hot-reload.

## Testing

See [Testing](testing.md) for test rules, patterns, and commands.
