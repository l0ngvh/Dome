# Architecture

## Overview

Dome has a platform-independent core and platform-specific shells. The core models the window tree and computes layout with zero OS dependencies, tested in isolation with snapshot tests, no OS mocking. Each platform structures its internals independently; the shared contract is Hub operations + placement structs.

```
src/
├── core/           # Tree model, layout, hub — zero OS dependencies
├── platform/
│   ├── macos/      # AX API, Metal overlays, CGEvent tap
│   └── windows/    # Win32, WinEvent hooks, wgpu/DX12 overlays
└── ...             # overlay, picker, config, action, ipc, logging
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

**Minimized** — global list on Hub (`minimized_windows: Vec<WindowId>`), outside any workspace. When a window is minimized (via platform events or the picker), it is detached from its current layout (tiling tree, float list, or fullscreen list), its mode set to `Minimized`, and the workspace pruned if empty. The window's `workspace` field becomes stale after minimize since the workspace may be pruned. Code must check `window.mode != DisplayMode::Minimized` before using `window.workspace` to index into the workspace allocator. Unminimize always restores to the current workspace as tiling (scratchpad model), regardless of the window's original workspace or mode. `minimize_window` and `unminimize_window` live in `src/core/minimize.rs`, following the same pattern as `float.rs` and `fullscreen.rs`.

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
3. **Tiling focus** uses a dedicated `focused_tiling: Option<Child>` pointer on `WorkspaceTilingState` (owned by the strategy). Primarily set by the strategy's `set_focus` method.

The `focused()` accessor on Workspace computes effective focus by checking in priority order: fullscreen > float > tiling. All external reads go through this accessor. The three mechanisms are independent -- `focused_tiling` persists even when fullscreen or float windows are active, serving as "tiling focus memory." When fullscreen is unset or float is unfocused, tiling focus is restored without recomputing.

**Focus chain invariant.** `container.focused` stores the last focused node in that subtree, not the immediate child. This node can be a `Child::Window` or a `Child::Container` (e.g. after `focus_parent`). `set_focus_child` writes the same target to every ancestor container from the target up to the workspace root. This means if `focused_tiling == Some(X)`, every ancestor of X has `focused == X`, and walking `container.focused` from root reaches X directly in one hop per level. `replace_split_child_focus` preserves this invariant during tree mutations by replacing old references with new ones along the same scope. The test validator (`validate_workspace_focus`) checks reachability from root via the focus chain.

**Container highlight.** When `focused_tiling` is `Child::Container` (after `focus_parent`), `focused_tiling_window()` returns `None` rather than walking to a descendant window. This means `hub.focused_window()` returns `None` when a container is highlighted (assuming no fullscreen/float focus), which makes `toggle_float` and `toggle_fullscreen` no-ops and causes the platform to receive `focused_window: None` in placements (see Empty workspace focus in the platform docs for per-platform behavior). Move-to-workspace and move-to-monitor bypass `focused_window()` in this case and call the strategy directly to move the whole container.

**Invariant:** `is_float_focused` must be false when `float_windows` is empty. The test validator enforces this. The `focused()` accessor also handles it gracefully as defense-in-depth, falling through to `focused_tiling`.

**Write paths.** `set_focus_child` is the internal workhorse: it walks up from the target child, writing the child (the original argument, which can be a window or container) to `container.focused` on every ancestor, calling `set_active_tab(current)` on tabbed containers (where `current` is the direct child for correct tab activation), and finally setting `focused_tiling` and clearing `is_float_focused` at the workspace level. The public `set_focus` wraps a `WindowId` and delegates here. Hub's `set_focus` (the entry point from the platform layer) branches by display mode: fullscreen promotes to top of the z-order stack, float sets `is_float_focused` and moves to end of `float_windows`, tiling delegates to `strategy.set_focus`.

**Focus replacement during tree mutations.** `replace_split_child_focus` uses a two-walk algorithm. Walk 1 (scope): walks up from old_child, finding the highest ancestor with `focused == old_child`. If the walk reaches the workspace, the scope covers the entire path. Walk 2 (replace): walks up from new_child, replacing `focused` values and updating active tabs within the scope. When the scope reaches the workspace, `focused_tiling` is also updated if it pointed to old_child.

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

**Keyboard sink.** A 1x1 invisible HWND (`WS_POPUP | WS_EX_TOOLWINDOW`, moved offscreen) holds Win32 foreground when no managed window is focused (empty workspace, `focus_parent` container highlight). Activating this HWND instead of the tiling overlay avoids disturbing tiling window z-order, since `SetForegroundWindow` raises the target window. macOS doesn't need this because tiling overlay windows there are one window level behind normal windows.

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

- Each positioned window carries `DriftState` with target vs. actual and retry logic. `DriftState` also tracks `monitor`, the monitor the window was last placed on; compared in `show_tiling` to detect cross-monitor moves.
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

`SetWindowPos` sets z-order of any window -- no mirroring needed. Both tiling and float overlays sit behind their managed windows (mirroring macOS), so managed windows naturally occlude the overlay interior and no region clipping is needed. For topmost floats, the overlay is placed just below the managed window in the topmost band.

**Tiling z-order.** Windows tiling z-order is state-driven in `show_tiling`: newly-appearing tiling windows raise to `Top` above the overlay; same-monitor stable windows early-return with no `SetWindowPos`. The overlay is positioned once at creation and never re-raised. Sibling order follows Win32 foreground activation.

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

Both platforms use egui for borders, tab bars, and the minimized window picker. Shared tiling/container painting logic lives in `src/overlay.rs` (takes placements + config, draws into egui). Shared picker painting logic lives in `src/picker.rs`. Platform code handles windowing and GPU backend.

UI colors are resolved via `src/theme.rs`. The `Flavor` enum (latte/frappe/macchiato/mocha) is the config-facing type; `Theme` is a DTO of 13 semantic-role `Color32` fields built by `Theme::from_flavor()`. Overlay and picker code reads from `Theme`, never from raw palette constants. `Flavor::catppuccin_egui()` maps to the `catppuccin_egui` crate's theme constants for egui widget chrome (scrollbars, focus rings). `catppuccin_egui::set_theme` is called in `Renderer::new` on both platforms and re-applied via `Renderer::apply_theme` when a config reload changes the flavor. Theme lives outside `src/core/` because it depends on `egui::Color32` and `catppuccin_egui`.

Each platform has a `Renderer` struct (macOS: Metal backend, Windows: wgpu/DX12 via DirectComposition) that owns the GPU context, egui state, and texture cache. `Renderer::new()` takes an `opaque: bool` parameter: `false` for transparent overlays (clear-to-transparent, alpha blending), `true` for opaque UI windows like the picker (clear-to-black, no alpha). All tiling and float overlays pass `false`; the picker window passes `true`.

One tiling overlay per monitor draws all tiling borders and container overlays including tab bars. Float windows get separate overlays (they need z-ordering, mirroring on macOS). Each float overlay sized to `visible_frame`.

Borders are drawn with `rect_stroke` + `StrokeKind::Inside` (stroke stays within the frame rect), clipped to per-edge and per-corner regions to support per-piece coloring. The same full-rect stroke is drawn multiple times, each clipped to a different region with a different color, so the pieces join seamlessly. When `border_radius > 0`, corners get rounded arcs; when `0`, the result is identical to sharp corners. Radius is clamped at runtime to half the window dimension via `effective_radius` so it can't produce negative clip rects. On both platforms, overlays sit behind managed windows, so managed windows naturally occlude the overlay interior. Only border strips between windows and tab bar areas remain visible. No region clipping is needed. On macOS, clicks in the rounded corner area (transparent pixels) pass through to the window beneath; the corner area is small enough that this is acceptable. Tab bars are interactive (click sends event to hub thread). Focused window border shows spawn-mode indicator: one edge colored for next spawn direction, with corners colored based on their two adjacent edges.

**Minimized window picker.** A separate OS window (not part of the tiling overlay) that shows minimized windows with application icons. Triggered by `Action::ToggleMinimizePicker`. The picker is a borderless, topmost, opaque window centered on the focused monitor (400x300 logical pixels, clamped). It uses egui `CentralPanel` with `ScrollArea` for the list UI. The picker's `Renderer` receives `catppuccin_egui::set_theme` at construction (like all renderers), then immediately overwrites `Visuals` with `picker_visuals(&theme)` for picker-specific styling. This override is intentional: it keeps renderer construction uniform while giving the picker its own look. `PickerEntry`, `build_picker_entries()`, `PickerResult`, and `paint_picker()` live in `src/picker.rs`. Platform picker window code lives in `src/platform/macos/ui/picker.rs` and `src/platform/windows/dome/picker.rs`.

`PickerEntry` holds `id: WindowId`, `title: String`, and `app_id: Option<String>`. `build_picker_entries()` maps the raw `(WindowId, String)` list from `hub.minimized_window_entries()` into `Vec<PickerEntry>`, resolving `app_id` via a platform-provided closure. On macOS the closure looks up `bundle_id` from the registry; on Windows it looks up the process name. Registry lookups (`Registry::by_id`/`by_id_mut` on macOS, `WindowRegistry::get`/`get_mut` on Windows) return `Option` because a window can become unmanaged between picker open and icon dispatch. All callers handle the `None` case with early returns or `continue`.

**Icon loading.** Each row shows a 24x24 application icon (loaded at 48x48, downscaled by egui). Icon loading differs by platform because of threading constraints:

- **macOS** (synchronous, `src/platform/macos/ui/icon.rs`): `load_app_icon(bundle_id)` runs on the main thread during `render_now`. NSImage is thread-unsafe per Apple docs, so GCD dispatch is not an option. Loading is fast (< 1ms per icon, system-cached), so synchronous loading for the typical < 10 minimized windows is imperceptible. Icons are cached in `icon_textures: RefCell<HashMap<String, Option<TextureHandle>>>` on the picker view. `None` entries are failed-load sentinels, never retried within a session. On reopen, `None` entries are cleared so relaunched apps can be retried.
- **Windows** (background dispatch, `src/platform/windows/dome/icon.rs`): `load_app_icon(hwnd)` uses `SendMessageTimeoutW(WM_GETICON)` with a 100ms timeout, which can block, so it runs on the thread pool via `ReadDispatcher`. `collect_icons_to_load()` determines which `app_id`s need loading and inserts `None` sentinels into `icon_textures` to prevent duplicate dispatches. Background threads return `ColorImage` results into `pending_icons: Vec<(String, ColorImage)>` because `TextureHandle` creation requires the egui context during render. The next `rerender` drains `pending_icons`, converts to `TextureHandle`s inside the render closure, and inserts into `icon_textures`. `pending_icons` is cleared on reopen to discard stale in-flight results.

`paint_picker()` takes `&[PickerEntry]` and `&HashMap<String, Option<TextureHandle>>`. For each entry, it looks up the icon by `app_id`. Present icons render as `Image`; missing icons get `allocate_space` for stable layout alignment.

Picker state (selected index, entries list, icon caches) is owned by the platform picker window, not by Dome. When the picker opens, Dome builds `Vec<PickerEntry>` and passes the snapshot to the picker window. This is a one-shot read, not a persistent reference. The owning struct holds the picker as an `Option` (`Option<Box<PickerWindow>>` on Windows, `RefCell<Option<PickerPopup>>` on macOS) but never sets it back to `None` after creation. The picker window is created lazily on first toggle and reused via show/hide.

Only two events cross the picker boundary. `HubMessage::PickerToggle` (Dome to UI, macOS only since Windows is single-threaded) carries entries and monitor info. The picker sends `HubEvent::Action` with `Action::UnminimizeWindow(WindowId)` to trigger unminimize, the same channel used by all other actions. There is no picker-specific event variant. There is no `PickerClosed` or `PickerClose` event. Keyboard input (arrow keys, Return, Escape), focus loss, and close are handled entirely within the picker window with no round-trip through the hub thread. The picker hides itself directly (`orderOut` on macOS, `OwnedHwnd::hide` on Windows). Both hide calls are no-ops when already hidden, so `resignKeyWindow`/`WM_KILLFOCUS` firing after an explicit hide causes no double-action. This works because all default Dome keybindings require Cmd/Meta modifier, so bare arrow/enter/escape keys pass through the global keyboard hooks to the focused picker window.

**Empty workspace focus.** When no managed window is focused (empty workspace, container highlight), the platform must hold keyboard focus to prevent it from landing on offscreen windows. On macOS, Dome focuses the tiling overlay NSWindow. On Windows, Dome activates the keyboard sink HWND (see [Window Manipulation > Windows](#windows)) instead of the overlay, avoiding z-order disruption.

#### macOS

Borderless transparent NSWindows with CAMetalLayer. Shared Metal backend: device, command queue, two pipelines -- egui (premultiplied alpha for text blending) and mirror (passthrough). Each overlay has its own `Renderer` instance.

ScreenCaptureKit captures IOSurface for mirroring, rendered as textured quad. Captures start/stop as floats gain/lose focus.

#### Windows

Win32 windows with DirectComposition, egui_wgpu (wgpu/DX12). Tab bars use DWM blur-behind for frosted glass. Dome thread manages overlays directly -- no cross-thread dispatch.

#### Windows Rendering Model

**DWM + wgpu/DX12.** With DWM (always on since Windows 8), each window renders to an offscreen surface via DirectComposition. wgpu presents to a DComp swap chain, which DWM composites to screen at vsync. `WM_PAINT`/`BeginPaint`/`EndPaint` are irrelevant to actual screen content -- DWM picks up content from the swap chain, not from the GDI paint cycle.

**Per-pixel alpha.** Transparent overlays use `WS_EX_NOREDIRECTIONBITMAP` + DirectComposition. The window has no GDI surface; wgpu renders to a DComp swap chain with `DXGI_ALPHA_MODE_PREMULTIPLIED` (via `CompositeAlphaMode::PreMultiplied`). DWM composites the swap chain output directly, giving native per-pixel alpha without the old `DwmEnableBlurBehindWindow` hack.

**Render-last invariant.** All overlay updates follow this sequence: data assignments, `SetWindowPos(SWP_NOREDRAW)`, `ShowWindow` (first show only), wgpu render + present. Positioning calls with redraw flags can trigger synchronous quick-repaints that interfere with rendered content, which is why `SWP_NOREDRAW` is used.

**WM_ERASEBKGND / WM_PAINT handlers.** All window classes (app, float overlay, tiling overlay, picker overlay) handle `WM_ERASEBKGND` by returning `LRESULT(1)` to suppress background erase. Overlay and picker window classes call `BeginPaint`/`EndPaint` in their `WM_PAINT` handlers to validate dirty regions as a safety net (the app HWND is a 1x1 keyboard sink and WndProc host (handles WM_DISPLAYCHANGE) that never repaints). Neither handler renders wgpu content. Removing `WM_PAINT` entirely from overlay windows could cause infinite WM_PAINT loops if something unexpected invalidates them.

**No CS_HREDRAW|CS_VREDRAW.** Overlay windows don't use these class styles. They cause full-client invalidation on any size change, which is counterproductive when the application controls all rendering via wgpu present.

**Float overlay focus update.** In `show_float`, the `settled` check skips both positioning and overlay update when position is unchanged and no topmost change is needed. A separate `focus_changed` branch re-renders the overlay (without repositioning) when focus changes, so the border color updates even when the float's position hasn't moved.

### Recovery

#### macOS

POSIX signal handlers (SIGINT, SIGTERM, SIGHUP) + `catch_unwind` on both threads. All tracked windows moved to centered positions on primary monitor at original size.

#### Windows

Console control handler (Ctrl+C, Ctrl+Break, console close) posts `WM_QUIT` to the main thread, reusing the normal shutdown path rather than calling recovery directly from the handler thread. This avoids duplicating shutdown logic and needing thread-local global state in the handler. For `CTRL_CLOSE_EVENT`, the handler sleeps 2s after posting because Windows terminates the process shortly after the handler returns for close events. `catch_unwind` on dome thread. All tracked windows restored to (100, 100). Previously-maximized windows re-maximized. Taskbar tabs restored.

## Shared Subsystems

### IPC

The binary serves dual purpose: `dome`/`dome launch` starts the WM, `dome <action>` sends a command. Action variants (Focus, Move, Toggle, Exec, Exit, ToggleMinimizePicker) are clap subcommands and serde IPC payloads.

- macOS: Unix domain socket (`/tmp/dome.sock`), stale socket auto-cleaned.
- Windows: named pipe (`\\.\pipe\dome`).
- Protocol: one JSON-serialized `IpcMessage` per line, text response. `IpcMessage` is an enum with `Action` and `Query` variants, separating mutations from reads on the wire.
- Server on dedicated thread, forwards to hub thread.
- Startup connects to existing socket to detect running instance.

**Actions vs queries.** Actions (`&mut self` on Hub) are fire-and-forget: the IPC server sends them to the hub thread and returns `"ok"` immediately. Queries (`&self` on Hub) are synchronous round-trips: the IPC server sends a query to the hub thread via a `sync_channel(1)`, blocks on `recv_timeout(1s)`, and returns the JSON result. `dome query workspaces` is the first query, returning workspace metadata as JSON.

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
