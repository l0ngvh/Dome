# Configuration

Dome can be configured with Lua. It reads the config file from one of these
locations, which will be created on first launch:

- macOS: `~/.config/dome/config.lua` (or `$XDG_CONFIG_HOME/dome/config.lua`).
- Windows: `%APPDATA%\dome\config.lua`.

The default config path can be overridden with `dome launch -c <path>`

## Config Options

### `border_size`

Integer, default `4`.

Sets the border width around windows, in logical pixels.

### `theme`

`"latte"`, `"frappe"`, `"macchiato"`, or `"mocha"`, default `"mocha"`.

Sets the color theme, a [Catppuccin](https://catppuccin.com/) flavor.

### `log_level`

`"trace"`, `"debug"`, `"info"`, `"warn"`, or `"error"`, default `"info"`.

Sets log verbosity. Dome writes it to one of these locations:

- macOS: `~/Library/Logs/dome/dome.log`.
- Windows: `%APPDATA%\dome\logs\dome.log`.

### `start_at_login`

Boolean, default `false`.

Launches Dome when you log in.

### `font_size`

Float, default `14.0`.

Sets the text size in logical pixels.

### `font_family`

String, default `nil`.

Sets the font Dome renders its widget in.

### `strategy`

`"partition_tree"` or `"master"`, default `"partition_tree"`.

Sets the default tiling strategy. It can be overridden per workspace by the
preferred layout. See [Layout](layout.md).

### `minimum_width` / `minimum_height`

Integer or `"<number>%"`, default `"5%"`.

Sets the minimum window size. Set to an integer to specify logical pixels. Use
the `"<number>%"` format instead to set it as a percentage of the available work
area. `0` disables it.

### `maximum_width` / `maximum_height`

Integer or `"<number>%"`, default `0`.

Sets the maximum window size. Same parsing as the minimum. `0` means no limit. A
window clamped by the maximum is centered in its allocated space.

### `partition_tree.tab_bar_height`

Positive integer, default `24`.

Sets the tab bar height in tabbed containers, in logical pixels. It does not
scale with `font_size`, so a long tab title may truncate earlier as the body
size grows.

### `partition_tree.automatic_tiling`

Boolean, default `true`.

Picks the split direction from the focused window's dimensions.

### `master.master_ratio`

Float in `[0.1, 0.9]`, default `0.5`.

Sets the width of the master area.

### `master.master_count`

Positive integer, default `1`.

Sets the number of master windows.

### `ignore`

`{ WindowMatcher }`, default `{}`.

Ignores matching windows. Dome already ignores a built-in list, and these
rules add to it. See [WindowMatcher](#windowmatcher).

### `float`

`{ WindowMatcher }`, default `{}`.

Floats matching windows. See [WindowMatcher](#windowmatcher).

### `fullscreen`

`{ WindowMatcher }`, default `{}`.

Fullscreens matching windows. See [WindowMatcher](#windowmatcher).

### `env`

`table<string, string>`, default `{}`.

Sets environment variables for the commands [`actions.execute`](#actionsexecutecommand)
spawns, layered over Dome's own environment. On macOS, launchd starts Dome with
a minimal environment, so set `PATH` here for a spawned command to find its
binary. Each entry replaces the inherited value for that variable.

### `keymaps`

Table of keymaps keyed by name. The default holds a single built-in keymap,
`main`, listed below.

Only one keymap is active at a time. To switch the active keymap, use
`actions.mode(name)`.

| Key | Action |
|-----|--------|
| <kbd>alt</kbd>+<kbd>0</kbd> through <kbd>alt</kbd>+<kbd>9</kbd> | `focus workspace 0` through `focus workspace 9` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>0</kbd> through <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>9</kbd> | `move workspace 0` through `move workspace 9` |
| <kbd>alt</kbd>+<kbd>h</kbd> | `focus left` |
| <kbd>alt</kbd>+<kbd>j</kbd> | `focus down` |
| <kbd>alt</kbd>+<kbd>k</kbd> | `focus up` |
| <kbd>alt</kbd>+<kbd>l</kbd> | `focus right` |
| <kbd>alt</kbd>+<kbd>p</kbd> | `focus parent` |
| <kbd>alt</kbd>+<kbd>[</kbd> | `focus tab prev` |
| <kbd>alt</kbd>+<kbd>]</kbd> | `focus tab next` |
| <kbd>alt</kbd>+<kbd>e</kbd> | `toggle spawn` |
| <kbd>alt</kbd>+<kbd>d</kbd> | `toggle direction` |
| <kbd>alt</kbd>+<kbd>b</kbd> | `toggle layout` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>f</kbd> | `toggle float` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move left` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move down` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move up` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move right` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>h</kbd> | `focus monitor left` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>j</kbd> | `focus monitor down` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>k</kbd> | `focus monitor up` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>l</kbd> | `focus monitor right` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move monitor left` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move monitor down` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move monitor up` |
| <kbd>alt</kbd>+<kbd>ctrl</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move monitor right` |
| <kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>q</kbd> | `close` |


## Built-in helpers

### `dome.os`

Returns the platform Dome runs on, `"macos"` or `"windows"`.

### `dome.defaults()`

Returns the default config.

### `dome.executable(name)`

Returns `true` when `name` resolves on `PATH`.

### `dome.with_default_modifier(m)`

Returns a builder whose `defaults()` builds the default keymap on `m` as the
primary modifier. `m` must be `Meta` or `Alt`.

### `Meta`/`Alt`/`Ctrl`/`Shift`

Combine a modifier with a key using `+`, for example `Meta + "h"`. Modifiers
can be chained together, for example `Meta + Shift + "1"`. `Meta` (or `Cmd`,
`Win`) is Command on macOS and the Windows key on Windows. `Alt` (or `Opt`,
`Option`) is the Option key on macOS.

### `WindowMatcher`

A table of keys that describe a window, used by `ignore`, `float`, and
`fullscreen`. Each value matches exactly, or wrap it in `/pattern/` for a regex
match. All present keys must match, and the first matching rule wins.

| Key | Platform | Matches |
|-----|----------|---------|
| `app` | Both | Application name, for example `Finder`. |
| `title` | Both | Window title. |
| `bundle_id` | macOS | Bundle identifier, for example `com.apple.finder`. Exact match, no regex. |
| `process` | Windows | Process executable, for example `explorer.exe`. |
| `class` | Windows | Window class, for example `#32770`. |
| `aumid` | Windows | Application User Model ID. |

## Actions

Dome passes an `actions` handle to each binding function.

### `actions.focus.left()`

Focus the neighboring window to the left in the tiling tree.

### `actions.focus.right()`

Focus the neighboring window to the right.

### `actions.focus.up()`

Focus the neighboring window above.

### `actions.focus.down()`

Focus the neighboring window below.

### `actions.focus.parent()`

Focus the parent container. Later `move` and `toggle` actions target the whole
group.

### `actions.focus.workspace(name)`

Switch to the workspace named `name`. Any string is valid, and the workspace is
created on demand.

### `actions.focus.monitor(target)`

Focus a monitor. `target` is `"up"`, `"down"`, `"left"`, `"right"`, or a monitor
name.

### `actions.focus.tab.next()`

Focus the next tab in a tabbed container.

### `actions.focus.tab.prev()`

Focus the previous tab.

### `actions.move.left()`

Move the focused window one step left in the tiling tree.

### `actions.move.right()`

Move the focused window one step right.

### `actions.move.up()`

Move the focused window one step up.

### `actions.move.down()`

Move the focused window one step down.

### `actions.move.workspace(name)`

Move the focused window to the workspace named `name`.

### `actions.move.monitor(target)`

Move the focused window to a monitor. `target` takes the same values as
`actions.focus.monitor`.

### `actions.toggle.spawn()`

Cycle the spawn direction between horizontal, vertical, and tabbed.

### `actions.toggle.direction()`

Flip the parent container's split direction between horizontal and vertical.

### `actions.toggle.layout()`

Toggle the parent container between split and tabbed layout.

### `actions.toggle.float()`

Toggle the focused window between tiling and floating.

### `actions.toggle.fullscreen()`

Toggle the focused window between normal and fullscreen.

### `actions.master.grow()`

Increase the master area by 5 percentage points, clamped to `0.1` through `0.9`
of the workspace.

### `actions.master.shrink()`

Decrease the master area by the same step.

### `actions.master.more()`

Add one window slot to the master area.

### `actions.master.fewer()`

Remove one window slot from the master area, with a minimum of 1.

### `actions.execute(command)`

Run `command` as a shell command line. macOS runs it through `/bin/sh -c` and
Windows through `cmd.exe /C`, so pipes, `&&`, and redirects work. On Windows
there is no "open" verb, so open a URL, document, or folder with `start`, for
example `start https://example.com`.

### `actions.close()`

Close the focused window.

### `actions.exit()`

Stop Dome and restore all windows.

### `actions.mode(name)`

Switch to the keymap named `name`. `actions.mode("main")` returns to the `main`
keymap.
