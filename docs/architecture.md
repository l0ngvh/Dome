# Architecture

At the center of Dome sits `Hub`, the logical model of every monitor, workspace
and window Dome manages. Around `Hub` are the OS shells, which translate `Hub`
decisions into OS API calls.

## Hub

`Hub` is made of three kinds of entities: windows, workspaces and monitors.
Each carries its own typed ID, monotonically increasing and never reused.
Reusing IDs causes too many problems, and we are not going to run out, nobody
can open two billion windows in a single session.

A monitor corresponds to an actual physical display. A workspace lives in a
monitor and houses windows, and only one workspace is shown on a monitor at a
time. A window corresponds to an application window and is assigned to a
workspace.

`Hub` defines three window types. Tiling windows, the default, have their
position and size controlled by `Hub` through a tiling strategy, and may carry
minimum/maximum size constraints that prevent the strategy from stretching or
shrinking them too much. Float windows sit above all tiling windows, with
position and size left to the OS. Fullscreen windows mirror the OS fullscreen
window mode, taking over the entire monitor and inheriting OS-imposed
restrictions, so the user can't move them across monitors or convert them to a
different type, among other restrictions.

A window can also be minimized, in which case it leaves its workspace and sits
in a Hub-wide scratchpad list until the user restores it through the picker.
Minimize preserves the window's prior mode (tiling, float, or fullscreen),
restrictions, and float geometry, so restoration returns the window to the
current workspace in the same mode it had before.

`Hub` is unit-agnostic about coordinates. Every position and size it stores is
in whichever unit the OS shell hands back, physical pixels or logical points,
and the arithmetic works the same. The alternative, requiring the shells to
convert physical pixels to logical points before calling `Hub`, isn't viable.
In a multi-monitor setting where each monitor has its own scale, no single
logical plane can hold all monitors without overlaps or gaps.

`Hub` exposes operations the OS shells can use to mutate it (`insert_window`,
`set_focus`, `handle_tiling_action`, ...) and to query the layout they should
apply.

### Tiling strategies

Two tiling strategies are implemented. The default one, Partition Tree, tiles
windows like i3. All tiling windows are leaves of a tree rooted in a workspace,
with intermediate nodes called containers. A container lays out its children in
one of three modes. Horizontal and vertical lay children out in a row or column
respectively. Tabbed stacks children into a single slot and shows only one at a
time. The trees are normalized, meaning a non-tabbed container and its
non-tabbed parent never share the same direction. If a workspace's windows
don't all fit in the visible viewport because of minimum-width constraints, the
workspace can scroll to bring offscreen windows into view. Float windows aren't
managed by the tiling strategy, so workspace scrolling doesn't apply to them.
Same for fullscreen windows, which already take over the whole monitor.

The other implemented tiling strategy, Master, splits the monitor into two
side-by-side panes with no containers and no tabs. The first `master_count`
windows go in the master pane on the left, and the rest go in the stack pane on
the right. Each pane stacks its windows vertically, with `master_ratio` setting
where the split lands. Each pane scrolls vertically and independently when its
content overflows, with focus movement as the sole trigger. Both panes honor
per-window min/max size constraints the same way Partition Tree does, but when
the panes' combined min widths exceed the screen, the layout overflows past the
edge rather than scrolling horizontally.

## macOS

The macOS shell listens for window events through the Accessibility (AX) API,
forwards them to `Hub`, and applies `Hub`'s layout back through the same API.
The shell also owns a few borderless windows, used for visual indicators and a
handful of related shell-side responsibilities.

### Listening to window lifecycle events

Through the AX API, the shell subscribes to window lifecycle events (opened,
closed, moved, resized) and forwards them to `Hub`. AX speaks logical points
with a top-left origin, so no coordinate translation is needed.

AX notifications are quite unreliable, however. They can arrive attached to the
wrong window, and might even be dropped. To solve the first problem, on every
notification the shell extracts all windows from the app's pid and reconciles
them to figure out which windows were created or deleted. To make up for
dropped notifications, the shell does a sweep every five seconds to detect any
created or deleted windows that might have been missed. The sweep also rebuilds
every observer from scratch, since observers can go dead silently and a fresh
registration is the only reliable way to bring them back.

Reconciling windows to detect deletions is also not simple. Getting the list of
windows through the AX API only returns the windows visible in the current
space, which causes a problem if we rely on it alone: focus can jump between
spaces when a window becomes fullscreen (see [Fullscreen
windows](#macos-fullscreen-windows)). For that reason, to detect deleted
windows, we check whether the AXUIElement backing the window has already been
invalidated, by calling any method on it. This itself creates another problem,
since AX windows can also be invalidated right before the screen is locked, so
we need a check for that case as well. We previously used a simpler method,
trying to correlate against the CGWindowID list, but it didn't work out, since
the list isn't updated by the time the notification fires.

Handling focus changes also requires care, since focus events can feed back on
themselves. When `Hub` focuses window A, the OS may already have a focus event
for B sitting in its queue from earlier input, and the focus action itself
causes the OS to queue another focus event for A in response. Without
intervention, the shell processes them in order, refocuses B, then refocuses A,
and the cycle keeps going. A focus throttle ignores focus events that arrive
immediately after a shell-driven focus call.

### Placing windows

The shell places tiling windows through AX calls. Because macOS or the owning
app can override a placement, the shell reissues the call up to five times,
until the window lands at the requested frame or the retry budget is exhausted.
Dome-initiated fullscreen uses the same path with the monitor's bounds as the
target. Fullscreen initiated by the OS or the owning app comes with its own
rules and is covered in [Fullscreen windows](#macos-fullscreen-windows).

Placing float windows needs a little twist. Even though the shell can place
them anywhere through AX, there is no public API that lets an application set
the window level (`NSFloatingWindowLevel`) of windows it does not own. Without
it, a float window would drop behind whichever window currently takes focus. To
emulate this, given that float windows are mostly used for reading and rarely
interacted with, the shell captures the real window through `ScreenCaptureKit`
and presents the resulting frames in a Dome-owned window. Each captured frame
is an `IOSurface` set directly as a `CALayer`'s contents, so Core Animation
composites the pixels without any Dome-side GPU work. When the float does need
input, the shell swaps the real window back into place and stops the mirror,
rather than forwarding events from the mirror to the real window. This has two
consequences. Captured frames carry the source monitor's pixel density,
so the mirror can look blurry when the real window is parked on a lower-DPI
monitor than the one showing it. Each swap also produces a brief flicker.

### Virtual workspaces {#macos-virtual-workspaces}

macOS has its own workspace concept called Spaces, but the public API is too
restrictive to drive from a process, so the shell implements virtual workspaces
of its own. The obvious AX options for taking a window out of view don't fit
either. `hide` affects every window in the application rather than just the one
being moved, and `minimize` animates each window separately, so switching a
multi-window workspace becomes slow enough to stall the `CGEvent` tap and time
out the keystroke that triggered it (see [Handling
keymaps](#macos-handling-keymaps)). The shell instead parks windows offscreen
when their workspace becomes inactive. macOS refuses to position a window fully
offscreen and snaps it back to the nearest monitor, so the shell moves the
window to the bottom-right corner of the furthest monitor and leaves a
one-pixel sliver visible, the same trick AeroSpace uses for its virtual
workspaces.

Parking alone is not enough when the destination workspace has no managed
window to take focus. macOS leaves focus on the previously focused window,
which now sits offscreen in another workspace, so a stray keystroke after the
switch can land in an app the user can no longer see. The shell handles this by
focusing one of its own overlay windows whenever no managed window is eligible
(see [Displaying visual indicators](#macos-displaying-visual-indicators)).

Because workspace switching stays within a single Space, each monitor lives on
one Space throughout the session, except when native fullscreen windows are
involved (see [Fullscreen windows](#macos-fullscreen-windows)).

### Fullscreen windows {#macos-fullscreen-windows}

On top of the placement rules above, macOS imposes more restrictions on
fullscreen windows, and the exact set depends on how the window entered
fullscreen. Dome recognizes two ways in, native and borderless.

Native fullscreen is triggered by clicking the green traffic-light button.
macOS moves the window into its own Space, hiding everything else, and the
shell can only focus it, not move or resize it. The shell detects this by
reading the AX fullscreen attribute, both on `SpaceChanged` and during the sync
sweep. Switching workspaces in this case means switching Spaces. macOS switches
to whichever Space contains the focused window, so the shell enters the
fullscreen workspace by focusing the fullscreen window, and leaves it by
focusing a window in the destination workspace. If the destination workspace is
empty, the shell focuses the overlay instead.

Borderless fullscreen, a term borrowed from gaming, is triggered by zooming the
window or by an app's own fullscreen action. The window covers the monitor
without leaving the current Space, which the shell detects by checking the
frame after every move or resize. macOS always blocks moves on zoom-triggered
fullscreen, and apps that control their own fullscreen state vary, some
blocking and others letting the move through, in which case the
size-and-position check reads it as a fullscreen exit. Inactive borderless
windows are therefore minimized rather than parked offscreen.

### Tracking motion and constraints

A placement does not always land where the shell asked. Apps often set minimum
size constraints on their windows, and macOS refuses to resize past those
limits. A placement that would violate them still lands at the requested
origin, but macOS clamps the size to the app's allowed range. A tight tiling
layout that took the placement at face value would end up with overlapping
windows or gaps the shell never asked for.

macOS does not expose an API to query those limits up front, so the shell
tracks constraints by reading where each placement actually lands. macOS also
lacks any "drag finished" or "resize finished" signal, so the shell infers when
motion has stopped by debouncing move and resize events into coalesced bursts
and treating the settled frame at the end of each burst as the placement
result.

AX move and resize notifications signal a change but don't include the new
position or size, so the shell queries AX after each notification to read the
current frame. AX queries are blocking IPC and offer no async variant, so the
shell dispatches every query to a GCD queue rather than calling AX from the
event handler.

To keep a user-driven move or an app-driven resize from being mistaken for the
OS enforcing a constraint, the shell keys off the first timestamp in each
coalesced burst. If that timestamp lands within 1 second of the last placement,
the burst counts as the app reacting to the placement, and the shell records a
possible constraint hit. Bursts that start more than 1 second after placement
are treated as late-event drift and trigger a corrective placement against the
same five-retry budget. The 1-second window is kept short because move and
resize events arrive constantly during a session, so later bursts give the
shell plenty of follow-up chances to detect a constraint.

### Handling keymaps {#macos-handling-keymaps}

The shell handles user commands by registering a `CGEvent` tap and matching
each keystroke against the configured keymaps in the tap's callback. macOS
holds each keystroke until the callback returns or the tap's timeout fires. If
the callback's thread is busy with other work, every keystroke arrives late and
the user sees visible stutter. Long enough delays trip the timeout and cause
macOS to disable the tap (see [Virtual workspaces](#macos-virtual-workspaces)).

The callback therefore does only matching work and dispatches every action to
the background thread that owns `Hub`. UI rendering stays on the main thread
because AppKit requires it.

The tap could run on its own thread to isolate it from rendering, but the shell
keeps it on the main thread with rendering and the AX listener. Main-thread
contention has not been a problem in practice, and projects that use `CGEvent`
taps typically keep them on the main thread. Splitting the tap off remains an
option if that changes.

### Displaying visual indicators {#macos-displaying-visual-indicators}

To render visual indicators, the macOS shell owns borderless transparent
`NSWindow`s called overlay windows. Tiled windows dominate any normal session
and floats are rare, so the shell batches tiling overlays into one overlay
window per monitor, while each float gets its own. Tiling overlays are backed
by a single `CAMetalLayer`. Float overlays use a two-sublayer stack: a plain
`CALayer` below that presents the captured `IOSurface`, and a `CAMetalLayer`
above for the border decoration only.

Click handling has to stay clear of managed windows, and macOS offers no
per-region click-through, so the overlay parks at `NSNormalWindowLevel - 1`,
just below the windows it decorates. Managed windows receive every click that
lands on them, and the overlay only catches what falls into the gaps.

`NSWindow` uses Cocoa screen coordinates, with the origin at the bottom-left of
the primary monitor and Y increasing upward. The rest of the shell uses
top-left coordinates, so the shell flips the Y axis against the primary monitor
height before passing any overlay frame to `NSWindow`.

The per-monitor tiling overlay also absorbs keyboard focus when no managed
window is eligible to receive it, which keeps stray keystrokes from landing on
offscreen windows during workspace switches (see [Virtual
workspaces](#macos-virtual-workspaces)). The same overlay anchors focus when
the user exits a native fullscreen window into an empty workspace (see
[Fullscreen windows](#macos-fullscreen-windows)).

## Windows

The Windows shell listens for window events through `SetWinEventHook`, forwards
them to `Hub`, and applies `Hub`'s layout back through Win32. The shell also
owns a few borderless windows, used for visual indicators and a handful of
related shell-side responsibilities.

### Managing windows {#windows-managing-windows}

Through `SetWinEventHook`, the shell subscribes to window lifecycle events
(opened, closed, moved, resized) from every process and forwards them to `Hub`.
Unlike AX on macOS, the hook is reliable enough that the shell does not run a
periodic resync.

As on macOS, focus changes can feed back on themselves, so a 500 ms throttle
ignores events arriving within that window after a shell-driven focus call. The
window is long enough to break the cycle and short enough to stay invisible to
the user.

Unlike macOS, where the AX API is the only sanctioned way to drive another
application's windows, Win32 lets any process move, resize, restack, or
activate any window in the system. The shell calls the placement APIs directly
on foreign windows, and floats need no mirroring trick to stay above other apps
because `SetWindowPos` accepts the topmost band on a foreign window the same
way it does on its own. Apps can still push back on a placement by reasserting
their own size or position, so each placement runs against a five-retry budget
like on macOS. Care must also be taken when reading or writing window rects,
since Windows reports an outer rect that includes an invisible resize border
around the visible frame.

Unlike macOS, Windows exposes constraints directly. `WM_GETMINMAXINFO` reports
a window's minimum and maximum size before the first placement, so the shell
reads them up front without the place-and-observe dance the macOS shell needs.

Like AX on macOS, querying state on a foreign window is synchronous IPC that
blocks the calling thread until the target responds, so reads such as the
constraint query and window-text lookups run on a thread pool rather than on
the event handler, and go through `SendMessageTimeoutW` with `SMTO_ABORTIFHUNG`
so a hung target releases the call instead of stalling the shell.

### Virtual workspaces {#windows-virtual-workspaces}

Windows has its own workspace concept called Virtual Desktops, but the public
API is too limited to drive from a process, so the shell implements virtual
workspaces of its own. The approach mirrors macOS, parking windows offscreen
when their workspace becomes inactive. Unlike macOS, Windows imposes no
restrictions on where a window can land, so parking goes straight to `(-32000,
-32000)` without the one-pixel-sliver trick the macOS shell needs.

Parking has a few side effects the shell has to handle. When a foreground
window closes, Windows hands focus to the next window in the global z-order,
and that next window may well be an offscreen parked window belonging to a
different workspace, causing that workspace to be activated. To prevent that,
the shell drops every parked window to `HWND_BOTTOM` so the close-time focus
walk skips them and lands on a Dome-owned window instead (see [Displaying
visual indicators](#windows-displaying-visual-indicators)).

Switching to an empty workspace causes a similar problem from the opposite
direction. Focus stays on whichever window held it before the switch, which
after parking sits offscreen in another workspace, so a stray keystroke can
land somewhere the user can no longer see. The shell handles this with a
dedicated focus sink, a small offscreen window that absorbs Win32 foreground
when no managed window is eligible (empty workspace, `focus_parent`
container-highlight).

### Fullscreen windows {#windows-fullscreen-windows}

Fullscreen windows don't fit the placement or parking patterns above. Dome
handles two modes, borderless and exclusive (terms borrowed from gaming), and
windows in either push back when the shell tries to move, resize, or restack
them, each in their own way.

Borderless fullscreen is recognized when a window's frame matches its monitor
exactly. Parking such a window offscreen does not work. Apps that own their
fullscreen state fight back hard against any move, and even if they didn't, the
shell would read its own move as the window leaving fullscreen. The shell
therefore minimizes the window when its workspace becomes inactive.

Exclusive fullscreen connects the application's render surface directly to the
display and bypasses the desktop compositor. Touching such a window does not
work either. The window has to keep the foreground role with nothing topmost
above it or the OS drops it out of exclusive mode, and apps in this state fight
to stay there, responding to any interference either by reasserting the
foreground in a tight loop that can freeze the display or by minimizing
themselves. The shell therefore stays hands off entirely while exclusive
fullscreen is up, querying `SHQueryUserNotificationState` in response to
`WM_DISPLAYCHANGE` to detect the state.

Exclusive fullscreen is the sole reason Dome has a fullscreen restriction at
all. Recent Windows versions have started extending some of these optimizations
to ordinary borderless windows, so Dome should see fewer exclusive-fullscreen
apps over time.

### Handling keymaps {#windows-handling-keymaps}

The shell registers a low-level keyboard hook and matches each keystroke
against the configured keymaps in the hook procedure. Windows holds each
keystroke until the procedure returns, so heavy work on this path shows up as
visible input lag. The hook procedure therefore does only the cheap
match-and-dispatch step, dispatching every action to the orchestration thread
that owns `Hub`. The hook itself runs on its own thread to keep it isolated
from anything that could delay the callback.

### Displaying visual indicators {#windows-displaying-visual-indicators}

To render visual indicators, the Windows shell owns borderless transparent
windows backed by `wgpu` surfaces, called overlay windows. As on macOS, the
shell batches tiling overlays into one overlay window per monitor, while each
float gets its own. These float overlays sit inside the topmost band
themselves, just below their float. Overlay rendering stays on the
orchestration thread because Win32 has no AppKit-style main-thread rule, only a
same-thread one.

[Similar to macOS](#macos-displaying-visual-indicators), Windows doesn't have
per-region click-through, so the tiling overlay window has to stay at the
bottom of all managed windows. Unlike on macOS, however, even though we can
place a window at the bottom of the z-order stack through `HWND_BOTTOM`,
Windows doesn't guarantee that the window stays there, and it can gradually be
raised up the z-order as other windows get pushed to the bottom (see [Virtual
workspaces](#windows-virtual-workspaces)). To ensure that at least managed
windows in the current workspace don't get occluded by the tiling overlay,
whenever a managed window is displayed as part of the current workspace, the
shell restacks it above the overlay before showing it. The overlay also
shouldn't receive activation or absorb keyboard focus the way the macOS overlay
does (see [Virtual workspaces](#windows-virtual-workspaces)). Instead, a
separate focus sink window holds focus whenever no managed window is eligible.
