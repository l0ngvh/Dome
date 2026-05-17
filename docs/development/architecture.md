# Architecture

## Overview

Dome is split into a platform-independent **core** that owns all window-positioning
logic, and per-OS **platform shells** that talk to the operating system, render
overlays, and actually move windows on screen. The core knows nothing about the
OS. The shells know nothing about layout strategy. Everything in between flows
through a single chokepoint, the Hub.

```txt
resources/               # Icons, manifests, installer assets.
src/
  core/                  # Window positioning logic. Zero OS deps.
  platform/
    macos/               # macOS shell (Accessibility API + AppKit).
    windows/             # Windows shell (Win32 + wgpu).
  main.rs                # Binary entry point.
docs/
examples/
tests/                   # End-to-end tests.
```

## Core

The core decides *where* windows go. Platform shells decide *how* to put them
there.

### Hub: the single mutator

`Hub` is the only entry point into the core. A platform shell calls a hub
operation (`attach_window`, `set_focus`, `handle_action`, ...), receives the
resulting window placements plus any data needed to draw overlays, and applies
them. Nothing else in the core mutates state.

This single-mutator discipline pairs with fail-fast semantics. Because the core
has zero OS dependencies and a single chokepoint for mutation, any invariant
violation is a programmer error, not a recoverable runtime condition. The core
panics on broken invariants instead of papering over them, so bugs surface at
the call site instead of propagating corrupt state through the tree.

### Entities and IDs

Core state is modelled as a set of entities (windows, containers, monitors,
workspaces), each addressed by a typed ID (`WindowId`, `ContainerId`,
`MonitorId`, `WorkspaceId`). IDs are monotonically increasing and never reused,
so a stale reference fails loudly rather than silently aliasing into a recycled
slot.

### Window types

Every window the core knows about has one of three types:

- **Tiling** is the default. The active tiling strategy controls position
  and size.
- **Float** windows are exempt from tiling. They sit above tiling windows
  and ignore most tiling actions.
- **Fullscreen** windows take the entire visible viewport.

Minimized windows sit outside this classification. They are parked in a
scratchpad zone and re-enter the workspace through the minimized-window
picker, which opens next to the last-focused tiling window on the current
workspace.

Core is coordinate-system-agnostic. `Monitor.dimension` holds whatever rect
the platform shell supplies in its native frame, and all layout math works
in that frame without knowing the unit. On macOS the native frame is
logical points (AppKit, AX, and Core Graphics). On Windows under PMv2 it is
physical pixels, taken from the `rcWork` field of `GetMonitorInfoW`. Core
never converts between the two.

### Restrictions

Operating systems impose real constraints on certain windows. A Windows
exclusive-fullscreen process cannot be repositioned. A macOS native-fullscreen
window cannot move between monitors. The core models these as per-window
restrictions that block specific user commands:

- **None**: no restrictions.
- **ProtectFullscreen**: blocks display-mode changes and cross-monitor moves.
  Applied to macOS native and borderless fullscreen windows, and to Windows
  borderless fullscreen windows.
- **BlockAll**: blocks every user command. Reserved for Windows
  exclusive-fullscreen windows, which are delicate and demand undivided
  attention.

Because fullscreen windows own the workspace and user commands always target
the focused window, restrictions are checked against the focused window only.
Focus can still move away through `set_focus` triggered by an OS-level focus
change, which produces some interesting interactions with exclusive-fullscreen
windows.

### Size constraints

Each window may carry per-window min/max bounds, discovered by the platform
shell. The configuration supplies global min/max bounds. The two are reconciled
as follows:

- Per-window **max** overrides the global **min**. A fixed-size dialog should
  not be inflated to fit a global minimum, since that just produces dead space.
- Per-window **min** is floored by the global **min**.

### Tiling strategies

Tiling-window layout is delegated to a pluggable `TilingStrategy`. The trait
covers the window lifecycle (`attach_window`, `detach_window`, `set_focus`) and
command dispatch (`handle_action`). Float and fullscreen management stay on the
Hub.

User commands reach a strategy as a `TilingAction`: focus and movement, layout
toggles, parent and tab focus, tab clicks, and master-stack ratio adjustments.
Hub checks each action against the focused window's restrictions before
routing it to the strategy.

### PartitionTreeStrategy

The default strategy. Tiling in the i3 style: windows live in a container tree
whose internal nodes are split-horizontal, split-vertical, or tabbed. Every
container tracks its last-focused descendant, container or window.

A split container never shares its direction with its parent split. Without
this rule the same visual layout maps to multiple trees, and "move right"
becomes ambiguous. Toggle and restructure operations enforce the invariant by
walking child containers and flipping any that match their parent's direction.

If the focused workspace's tiling windows cannot all fit at their minimum
widths, the workspace scrolls horizontally to keep the focused window in view.
Off-screen windows are skipped during placement, partially visible windows are
clipped to the screen rect, and floats are pinned to the viewport.

### MasterStackStrategy

An alternative strategy modeled on xmonad's `Tall` layout. Each workspace owns
a flat, ordered list of tiling windows: the first `master_count` entries occupy
a master area on the left, and the rest stack vertically on the right.


## macOS

### Threading model

The macOS shell runs two long-lived threads. The **main thread** hosts
NSApplication, taps keyboard events via a CGEvent tap, observes window lifecycle
through AX observers, and renders overlays. The **hub thread** is a
calloop event loop that manages and places windows, routing layout decisions to
the core.

This split keeps the main thread responsive for overlay rendering and prevents
the CGEvent tap from blocking. A stalled tap holds up the OS until the handler
returns, producing visible stutter on every subsequent keystroke. Overlay
rendering itself has to live on the main thread because macOS requires all UI
rendering there.

CGEvent tap and AX observers could in principle move to separate threads,
leaving the main thread to overlay rendering only. So far the current
architecture shows no measurable regression, so the split is unrealized.


### Accessibility API

The macOS shell talks to managed windows through the Accessibility API. Each
call crosses a process boundary by IPC and is safe to issue from any thread, so
the shell dispatches AX work on a GCD queue and keeps the hub thread free of
blocking calls.

The Accessibility API stops responding while the machine sleeps or the
screen is locked, and every call returns an error in that state. The shell
cannot tell those errors apart from the ones that normally signal a
destroyed window, so it suspends all AX traffic for the duration of the
lock and resumes once the screen unlocks.

AX speaks logical points with a top-left origin, the same unit core uses on
macOS. The shell does no conversion at this boundary.

### Event sources and reconciliation

The macOS shell wires up two event sources. `AXObserver` delivers per-app
notifications for window creation, destruction, move, resize, focus change,
and title change. `NSWorkspace` delivers app lifecycle events and Space
switches.

AX notifications are unreliable. Events go missing, arrive duplicated, or
attach to the wrong window. To compensate, a sync timer fires every five
seconds and reconciles the tracked windows against live AX state. The pass
catches windows that were added, removed, or minimized since the last sync,
but it does not try to recover focus changes.

Each sync also rebuilds every observer from scratch. Observers go dead
silently and stop emitting, and a fresh registration is the only reliable
way to bring them back. The same rebuild drops failed registrations and
cleans up after apps that have since terminated. When a new app launches,
the shell registers its observers on the spot rather than waiting for the
next tick.

Reconciliation makes a lot of slow AX calls, so it runs on a GCD background
queue. Results dispatch back to the hub thread synchronously, keeping all
tree mutations on a single thread.

### Focus and motion tracking

Focus changes can feed back on themselves. When Dome focuses window A, the
OS may already have a focus event for B sitting in its queue from earlier
input. The focus action itself also causes the OS to queue another focus
event for A in response. Dome processes them in order, refocuses B, then
refocuses A, and the cycle keeps going. A focus throttle drops redundant
focus changes to break the loop.

macOS does not expose a way to ask whether a window is currently being
dragged. The shell debounces move and resize events to infer when motion
has stopped. This matters because Dome reads per-window size constraints
from the window's rest position, and reading them mid-drag returns the
wrong values.

Debouncing happens per process, not per window. The AX move and resize
notifications carry an unreliable window element, so the shell cannot tell
which window in the process actually moved. Once the event stream settles,
the shell reads positions for every window owned by that PID. The entire
app has to go quiet before that read, not just a single window, because
per-window debouncing would catch other windows mid-motion and record
stale positions.

### Window states

```text
WindowState
├── Positioned             -- subject to a max retry limit
│   ├── InView             -- tiled or floating, active placement target
│   └── Offscreen
├── NativeFullscreen       -- lives in a separate macOS Space
├── BorderlessFullscreen   -- covers the entire monitor (zoom, app shortcut)
└── Minimized              -- borderless fullscreen that cannot be moved offscreen
```

To park a window offscreen, Dome moves it to the bottom-right corner of the
furthest monitor and leaves one pixel visible. macOS refuses to position a
window fully offscreen, so that single-pixel sliver is what keeps the
placement from snapping back. The technique comes from AeroSpace's
virtual-workspace implementation.

Native fullscreen and borderless fullscreen need different detection paths.
A native fullscreen window lives in its own Space. The primary detection
path is the periodic reconcile cycle, which compares the AX fullscreen
attribute against tracked state for every window each tick. A secondary,
faster path fires on SpaceChanged and checks the focused window only. A
borderless fullscreen window just covers the monitor (zoom button or an
app-defined shortcut), so the shell catches it by checking position and
size after every move or resize.

Borderless fullscreen windows on a different workspace are stored as
`Minimized` rather than parked offscreen. Moving them would falsely trip the
fullscreen-exit detector, and macOS overrides positioning requests on zoomed
windows.

### Constraint detection

macOS does not expose a way to query a window's minimum and maximum size
up front. The shell has to place the window, wait for the OS to snap it
back within the app's allowed range, and read the result from the move
and resize notifications that follow.

To keep a user reposition or an app-driven resize from being mistaken
for the OS enforcing a constraint, the constraint and drift check keys
off the first timestamp in each coalesced debounce burst. If that
timestamp lands within 1 second of the last placement, the burst counts
as the app reacting to the placement, and the shell records a possible
constraint hit or edge drift. Bursts that start more than 1 second
after placement are treated as late-event drift and trigger a
corrective `set_frame` against the shared 5-retry budget. The 1-second
window is kept short because move and resize events arrive constantly
during a session, so later bursts give the shell plenty of follow-up
chances to detect a constraint.

The tradeoff is at least one "wrong" frame for new windows, because
constraint detection only resolves once the OS has had time to snap
back. Debouncing is also scoped per app rather than per window, because
one window's resize can emit move and resize events while another
window in the same app is still being dragged. A per-window scope would
let those events bleed into the wrong constraint reading.

### Overlay rendering

Overlays are borderless transparent `NSWindow`s backed by a `CAMetalLayer`.
A single Metal backend feeds all of them: one device, one command queue,
and two pipelines. One pipeline drives egui with premultiplied alpha so
text blends cleanly. The other is a passthrough that takes IOSurface
frames from ScreenCaptureKit and draws them as a textured quad.

Overlays come in two flavors and the shell renders them on separate
paths. Tiled windows dominate any normal session and floats are rare,
so tiling overlays are batched into a single overlay window per monitor
while floats render individually.

The per-monitor tiling overlay does more than draw the layout. It
absorbs keyboard focus whenever no managed window should be receiving
input, which keeps offscreen or hidden windows from picking up stray
keystrokes. It also acts as the anchor when the user exits a native
fullscreen window into an otherwise empty workspace. Click handling has
to stay clear of managed windows, and macOS offers no per-region
click-through, so the overlay parks at `NSNormalWindowLevel - 1`, just
below the windows it decorates. Managed windows receive every click
that lands on them, and the overlay only catches what falls into the
gaps.

Floats follow a different path. macOS does not let an application set
the window level of windows it does not own, so always-on-top floats
have to be emulated. Dome hides the real window, captures its contents
through ScreenCaptureKit, and draws the resulting frames into a
Dome-owned mirror window at `NSFloatingWindowLevel`. The mirror
pipeline introduced earlier is what feeds these windows. When input
needs to reach the underlying window, Dome swaps the real window back
into place and stops the mirror.

## Windows

### Threading model

The shell runs on three long-lived threads. The main thread runs the IPC
server, watches the config file, and listens to window lifecycle events
through `SetWinEventHook`. A dedicated low-level keyboard hook thread
receives raw keystrokes, matches them against the configured keymap, and
forwards matched actions onward. The orchestration thread routes layout
decisions into core and renders the UI.

The keyboard hook lives on its own thread because Windows blocks the next
keystroke until the hook procedure returns. Heavy work on this path shows
up as visible input lag, so the hook does only the cheap match-and-dispatch
step and hands the action off elsewhere. UI rendering is fine on the
orchestration thread because Win32 imposes no main-thread rule of the kind
macOS does, as long as every render call originates from the same thread.
Blocking Win32 reads, such as window titles, are pushed to a thread pool
so they never stall orchestration.

### Win32 API

Dome runs as Per-Monitor DPI Aware v2 and refuses to start if the OS has
downgraded the process to a lower awareness level. Every Win32 call goes
through `src/platform/windows/handle.rs`, which works exclusively in
physical pixels: `RECT`s plus the underlying `HWND` and `HMONITOR` handles
in, `Dimension<Physical>` out. The same module handles non-client border
compensation, child-window propagation, and the offscreen-placement
convention.

Outgoing Win32 messages aimed at foreign processes always go through
`SendMessageTimeoutW` with `SMTO_ABORTIFHUNG` and the `MSG_TIMEOUT_MS`
budget (100 ms). Plain `SendMessage`, `GetWindowText`, or any variant
that can block indefinitely on a hung target counts as a bug.

Incoming window events arrive through `SetWinEventHook`, which delivers
every event from every process. Coverage has been reliable enough in
practice that Dome does not run a periodic resync.

### Focus and motion tracking

Focus changes are throttled by 500 ms to break feedback loops, where Dome
focuses A, the OS queues a focus event for B, processing B refocuses B,
and so on. The throttle is long enough to break the cycle and short
enough to stay invisible to the user.

When no managed window should have focus (empty workspace, or a
`focus_parent` container highlight), some Win32 window must still hold
foreground or offscreen windows will pick up keystrokes. Activating the
tiling overlay would raise it above the managed windows it sits behind,
so the shell instead activates a dedicated keyboard sink: a 1x1
`WS_POPUP | WS_EX_TOOLWINDOW` window parked offscreen whose only job is
to absorb foreground. macOS solves the same problem by focusing the
tiling overlay directly, since overlay windows there sit one level below
normal windows and cannot be raised by focus.

Motion arrives as standard Win32 move and resize notifications. Windows
surfaces enough information to distinguish user-driven motion from
programmatic motion, so the user path bypasses debouncing entirely.
Programmatic events still need coalescing because some apps emit a burst
of resizes after a property change.

### Window states

A managed window is always in exactly one state:

```text
WindowState
├── Positioned
│   ├── Tiling      visible, tiled, drift-tracked
│   ├── Float       visible, floating, drift-tracked
│   └── Offscreen   hidden by Dome
├── FullscreenBorderless   covers the whole monitor
├── FullscreenExclusive    D3D or Vulkan
└── Minimized
```

Borderless fullscreen is recognized when a window's frame matches its
monitor exactly. Exclusive fullscreen is detected by polling
`SHQueryUserNotificationState` in response to `WM_DISPLAYCHANGE`.
Offscreen windows are parked at `(-32000, -32000)`, dropped to
`HWND_BOTTOM` in the z-order, and have their taskbar tab removed through
`ITaskbarList::DeleteTab`. The tab is restored when the window becomes
visible again.

### Constraint detection

`WM_GETMINMAXINFO` reports minimum and maximum size before the first
placement, so Dome can apply constraints up front. The probe-and-snap-back
machinery the macOS shell needs has no Windows counterpart, and there is
no first-frame visual hiccup while constraints settle.

### Overlay rendering

Each monitor owns a single tiling overlay and floats render individually,
mirroring the macOS split. The overlay also sits behind the managed windows
because Windows has no GPU path that combines per-pixel alpha with per-region
click-through. Managed windows on top occlude the parts of the overlay they
cover, so no per-region clipping is needed.

Where the Windows shell does diverge from macOS is in how that layering
is enforced. There is no equivalent of `NSNormalWindowLevel - 1`, so the
shell pins each overlay's z-order just behind its managed windows on
spawn and never focuses or otherwise touches it afterwards.
`SetForegroundWindow` and similar calls would raise the overlay above
the managed windows and break the layering.

For floats, `SetWindowPos` can change the z-order of any window
regardless of process, so always-on-top floats need no mirror. Each
float overlay is placed just below its managed window inside the
topmost band.
