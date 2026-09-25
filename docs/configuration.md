# Configuration

On first launch, Dome writes its own configuration file to these locations:

- macOS: `~/.config/dome/config.lua` (or `$XDG_CONFIG_HOME/dome/config.lua`).
- Windows: `%APPDATA%\dome\config.lua`.

Both can be overridden with `dome launch -c <path>`. The configuration is
automatically applied on save.

```lua
local config = dome.defaults()

config.theme = "latte"
config.font_size = 15.0

config.keymaps.main[Meta + Space] = function(actions)
  actions.execute("open -a Raycast")
end

local terminal = dome.os == "macos" and "open -a Terminal" or "wt"
config.keymaps.main[Meta + Return] = function(actions)
  actions.execute(terminal)
end

-- Ignore, float, or fullscreen windows that match a rule.
-- All keys must match, and the first matching rule wins.
config.ignore = { { app = "Finder", title = "Trash" } }
config.float = { { app = "System Settings" } }
config.fullscreen = { { app = "/IINA|VLC/" } }  -- wrap a value in /.../ for a regex match

return config
```

## Defining key bindings

Bindings live in named keymaps, and the default has only `main`.
[`resources/default.lua`](../resources/default.lua) holds the default bindings.

```lua
local config = dome.defaults()

config.keymaps.main[Meta + "h"] = function(actions) actions.focus_left() end
config.keymaps.main[Meta + Shift + "1"] = function(actions)
  actions.move_to_workspace("1")
  actions.focus_workspace("1")
end
local hyper = Meta + Alt + Shift + Ctrl
config.keymaps.main[hyper + Space] = function(actions)
  actions.execute("open -a Terminal")
end
-- a plain string works as well
config.keymaps.main["meta+return"] = function(actions) actions.execute("open -a Terminal") end
```

To add another keymap table:

```lua
config.keymaps.main[Meta + "m"] = function(actions) actions.mode("monitor") end
config.keymaps.monitor = {
  ["h"] = function(actions) actions.focus_monitor_left() end,
  ["l"] = function(actions) actions.focus_monitor_right() end,
  -- keep a binding back to main, or the keyboard stays in this keymap until Dome restarts
  ["escape"] = function(actions) actions.mode("main") end,
}
```

To replace the defaults outright:

```lua
config.keymaps = {
  main = {
    [Meta + "h"] = function(actions) actions.focus_left() end,
    [Meta + "r"] = function(actions) actions.mode("resize") end,
  },
  resize = {
    ["h"] = function(actions) actions.decrease_master_ratio() end,
    ["l"] = function(actions) actions.increase_master_ratio() end,
    ["escape"] = function(actions) actions.mode("main") end,
  },
}
```

## Built-in helpers

### `dome.os`

The platform Dome runs on, `"macos"` or `"windows"`.

### `dome.env`

The environment variables that Dome started with.

### `dome.defaults()`

Build the default config table. See [`resources/default.lua`](../resources/default.lua).

### `dome.with_default_modifier(m)`

Change the modifier in the default keymap that `defaults()` returns to `m`.
`m` must be `Meta` or `Alt`.

Example:
```lua
local config = dome.with_default_modifier(Meta).defaults()
```

### `dome.executable(name)`

Returns `true` when `name` resolves on `PATH`.

### `Meta`/`Alt`/`Ctrl`/`Shift`

Combines with a key or another modifier using `+`, for example
`Meta + Shift + "1"`. Each modifier also has a string form:

- `Meta`, `Cmd`, or `Win`:
  `"meta"`, `"cmd"`, or `"win"`
- `Alt`, `Opt`, or `Option`: `"alt"`, `"opt"`, or `"option"`
- `Ctrl` or `Control`: `"ctrl"` or `"control"`
- `Shift`: `"shift"`

### `Space`/`Enter`/`Escape`/`Tab`/`Backspace`/`Up`/`Down`/`Left`/`Right`

Represents non-character keys. Each key also has a string form:

- `Space`: `"space"`
- `Enter` or `Return` (also numpad Enter): `"return"` or `"enter"`
- `Escape` or `Esc`: `"escape"`
- `Tab`: `"tab"`
- `Backspace`: `"backspace"`
- `Up`, `Down`, `Left`, `Right`: `"up"`, `"down"`, `"left"`, `"right"`

## Actions

Dome passes an `actions` handle to each binding function.

### `actions.focus_left()`, `focus_right()`, `focus_up()`, `focus_down()`

Focus the neighboring window in that direction.

### `actions.focus_parent()`

Focus the parent container.

### `actions.focus_workspace(name)`

Switch to the workspace named `name`. Any string is valid.

### `actions.focus_monitor_left()`, `focus_monitor_right()`, `focus_monitor_up()`, `focus_monitor_down()`

Focus the nearest monitor in that direction.

### `actions.focus_monitor(name)`

Focus the monitor named `name`. `dome query monitors` lists the monitor names.

### `actions.focus_tab_next()`, `focus_tab_prev()`

Focus the next or previous tab in a tabbed container.

### `actions.move_left()`, `move_right()`, `move_up()`, `move_down()`

Move the focused window one step in that direction.

### `actions.move_to_workspace(name)`

Move the focused window to the workspace named `name`.

### `actions.move_to_monitor_left()`, `move_to_monitor_right()`, `move_to_monitor_up()`, `move_to_monitor_down()`

Move the focused window to the nearest monitor in that direction.

### `actions.move_to_monitor(name)`

Move the focused window to the monitor named `name`.

### `actions.toggle_split()`

Toggle the spawn direction between horizontal and vertical.

### `actions.rotate()`

Flip the parent container's split direction between horizontal and vertical.

### `actions.toggle_tabbed()`

Toggle the parent container between split and tabbed layout. A new window does
not join a tabbed container.

### `actions.toggle_float()`

Toggle the focused window between tiling and floating.

### `actions.toggle_fullscreen()`

Toggle the focused window between normal and fullscreen.

### `actions.increase_master_ratio()`, `decrease_master_ratio()`

Change the master area by 5 percentage points, clamped to `0.1` through `0.9`
of the workspace.

### `actions.increase_master_count()`, `decrease_master_count()`

Add or remove one window slot in the master area, with a minimum of 1.

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
