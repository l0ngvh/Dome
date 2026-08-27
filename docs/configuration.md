# Configuration

Dome reads `config.lua` from one of these locations:

- macOS: `~/.config/dome/config.lua` (or `$XDG_CONFIG_HOME/dome/config.lua`).
- Windows: `%APPDATA%\dome\config.lua`.

Use `dome launch -c <path>` to point to a different file (see [cli.md](cli.md)).

`config.lua` is a [Luau](https://luau.org) script that returns one table. Dome
runs it at startup and again on every save. A syntax error keeps the last good
config, and a missing file uses the built-in defaults.

Per-workspace preferred layout lives in a separate JSONC file, `layout.jsonc`
(see [preferred-layout.md](preferred-layout.md)).

The snippets below show fields of the one table `config.lua` returns. The
bundled `examples/config.lua` is a complete file.

## Editor typechecking

Dome ships a Luau type-definition file at `resources/dome.d.luau`. Point your
editor's Luau language server at it to check `config.lua` as you type. It carries
the config table shape, the `dome` API, and the modifier constants, so a
misspelled field or a bad `dome` call shows up in the editor. For
[luau-lsp](https://github.com/JohnnyMorganz/luau-lsp), add the file to the
`luau-lsp.types.definitionFiles` setting.

## General

```lua
border_size = 4,
theme = "mocha",
log_level = "info",
start_at_login = false,
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `border_size` | integer | `4` | Border width around windows, in logical pixels. |
| `theme` | string | `"mocha"` | Color theme. One of `"latte"`, `"frappe"`, `"macchiato"`, `"mocha"` ([Catppuccin](https://catppuccin.com/) flavors). |
| `log_level` | string | `"info"` | Log verbosity. One of `trace`, `debug`, `info`, `warn`, `error`. |
| `start_at_login` | boolean | `false` | Launch Dome at user login. |

## Starting from the defaults

Dome bundles a default config. `dome.defaults()` returns a fresh copy of it as a
table. Start from that table, override fields, and return it to keep the default
keybindings and add your own:

```lua
local config = dome.defaults()
config.theme = "latte"
config.keymaps["meta+return"] = "exec open -a Terminal"
return config
```

A config that returns its own table without `dome.defaults()` gets no default
keybindings. Static settings still fall back to their built-in values, and the
window-ignore floor (see [Window rules](#window-rules)) applies to every config.

## Tiling layout

Controls how windows are tiled on screen.

```lua
strategy = "partition_tree",
minimum_width = "5%",
minimum_height = "5%",
maximum_width = 0,
maximum_height = 0,

partition_tree = {
  tab_bar_height = 24,
  automatic_tiling = true,
},

master = {
  master_ratio = 0.5,
  master_count = 1,
},
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `strategy` | string | `"partition_tree"` | Default tiling strategy. One of `"partition_tree"` or `"master"`. A per-workspace preferred layout in `layout.jsonc` can set this per workspace. |
| `minimum_width` / `minimum_height` | size | `"5%"` | Minimum window size. A whole number (e.g. `200`) parses as logical pixels. A string with a `%` suffix (e.g. `"10%"`) parses as a percentage of the workspace dimension. Use `0` to disable. |
| `maximum_width` / `maximum_height` | size | `0` | Maximum window size. Same parsing as min. `0` means no limit. A window clamped by max is centered within its allocated space. |
| `partition_tree.tab_bar_height` | integer | `24` | Height of the tab bar in tabbed containers, logical pixels, `>= 1`. This value does not auto-scale with `font.text_size`, so a long tab title may truncate earlier as the body size grows. |
| `partition_tree.automatic_tiling` | boolean | `true` | Pick split direction from the focused window's dimensions. |
| `master.master_ratio` | float | `0.5` | Width of the master area, in `[0.1, 0.9]`. |
| `master.master_count` | integer | `1` | Number of master windows, `>= 1`. |

The master strategy splits the screen into a master area (left or top) and a
secondary stack area (right or bottom). `master.master_ratio` controls the master
area's width and `master.master_count` sets how many windows go there. The rest
of the windows stack in the secondary area.

The partition-tree strategy fills the screen by arranging windows in a tree of
nested containers. Each container is either a split (horizontal or vertical) or
tabbed. Unlike i3, Dome automatically removes single-child containers and
alternates nested split direction, similar to Aerospace's normalized mode.

## Window rules

Match windows by their attributes to ignore, float, or fullscreen them. All
fields in a rule must match (AND) and the first matching rule wins. Wrap a value
in `/pattern/` for regex matching or leave it bare for exact matching.

Dome always applies a built-in window-ignore floor. It covers platform windows a
user never wants tiled, the macOS dock and the Windows taskbar and shell. Your
`ignore` rules add to the floor. You cannot remove a floor rule. Dome logs the
floor at startup, so you can see what it ignores.

| Key | Semantics |
|-----|-----------|
| `ignore` | Do not manage matching windows. |
| `float` | Start matching windows as floating. |
| `fullscreen` | Start matching windows as fullscreen. |

| Platform | Matching fields |
|----------|-----------------|
| macOS | `app`, `bundle_id` (exact only), `title` |
| Windows | `process`, `title`, `class` (Win32), `aumid` |

```lua
ignore = {
  { app = "System Preferences" },
  { bundle_id = "com.apple.finder", title = "Trash" },
  { process = "SystemSettings.exe" },
},
float = {
  { process = "calculator.exe" },
},
fullscreen = {
  { process = "slides.exe" },
},
```

A per-workspace `float` or `fullscreen` rule in `layout.jsonc` takes priority over
these global rules (see [preferred-layout.md](preferred-layout.md)).

## Keybindings

Keybindings go in the `keymaps` table. A key is a chord and its value is one of:

- a string action, like `"focus left"`,
- a list of string actions that fire in order on one press, like `{ "move workspace 1", "focus workspace 1" }`,
- a Lua function that runs on the press (see [Function bindings](#function-bindings)).

```lua
keymaps = {
  ["meta+h"] = "focus left",
  ["meta+shift+1"] = { "move workspace 1", "focus workspace 1" },
},
```

A `keymaps` table you define is the whole keymap. It does not merge with the
defaults. To keep the default bindings, start from `dome.defaults()` and add to
its `keymaps` table (see [Starting from the defaults](#starting-from-the-defaults)).

### Chords

A chord is a string of modifiers and one key joined by `+`, like `"meta+h"`.
Modifiers are `meta`, `shift`, `alt`, and `ctrl`. `cmd` and `win` are aliases for
`meta`.

You can also build a chord from modifier constants with the `+` operator:

```lua
keymaps = {
  [Meta + "h"] = "focus left",
  [Meta + Shift + "q"] = "close",
  [Meta + Alt + Ctrl + Shift + "x"] = "exit",
},
```

The constants are `Meta`, `Alt`, `Ctrl`, and `Shift`, with `Cmd` and `Win` as
aliases for `Meta`, `Option` and `Opt` for `Alt`, and `Control` for `Ctrl`.
`Modifier + Modifier` composes modifiers and `Modifier + key` attaches the one
key. A chord holds one key, so `Meta + "h" + "j"` is a type error the shipped
types catch. Dome has no `Hyper` constant. Build an all-modifiers chord with
`Meta + Alt + Ctrl + Shift`.

### Function bindings

A binding value can be a Lua function. Dome runs it off the keyboard thread and
passes it an `actions` handle:

```lua
keymaps = {
  ["meta+return"] = function(actions)
    actions.exec("open -a Terminal")
    actions.focus.right()
  end,
},
```

The handle mirrors the actions in [commands.md](commands.md), grouped as
`actions.focus.left()`, `actions.move.workspace(name)`, `actions.toggle.float()`,
`actions.exec(command)`, `actions.close()`, and so on. The handle is valid only
while the handler runs. A call on it after the handler returns errors and issues
no action.

### Default bindings

These are the bindings `dome.defaults()` returns.

| Key | Action |
|-----|--------|
| <kbd>meta</kbd>+<kbd>0</kbd> through <kbd>meta</kbd>+<kbd>9</kbd> | `focus workspace 0` through `focus workspace 9` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>0</kbd> through <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>9</kbd> | `move workspace 0` through `move workspace 9` |
| <kbd>meta</kbd>+<kbd>h</kbd> | `focus left` |
| <kbd>meta</kbd>+<kbd>j</kbd> | `focus down` |
| <kbd>meta</kbd>+<kbd>k</kbd> | `focus up` |
| <kbd>meta</kbd>+<kbd>l</kbd> | `focus right` |
| <kbd>meta</kbd>+<kbd>p</kbd> | `focus parent` |
| <kbd>meta</kbd>+<kbd>[</kbd> | `focus tab prev` |
| <kbd>meta</kbd>+<kbd>]</kbd> | `focus tab next` |
| <kbd>meta</kbd>+<kbd>e</kbd> | `toggle spawn` |
| <kbd>meta</kbd>+<kbd>d</kbd> | `toggle direction` |
| <kbd>meta</kbd>+<kbd>b</kbd> | `toggle layout` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>f</kbd> | `toggle float` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move left` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move down` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move up` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move right` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>h</kbd> | `focus monitor left` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>j</kbd> | `focus monitor down` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>k</kbd> | `focus monitor up` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>l</kbd> | `focus monitor right` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move monitor left` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move monitor down` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move monitor up` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move monitor right` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>q</kbd> | `close` |

### Modes

Additional sets of bindings go in `keymaps.mode.<name>`. Switch between them with
the `mode <name>` action or `dome mode <name>`. An unknown mode name is rejected.

```lua
keymaps = {
  ["meta+h"] = "focus left",
  ["meta+r"] = "mode resize",

  mode = {
    resize = {
      ["h"] = "master shrink",
      ["l"] = "master grow",
      ["escape"] = "mode default",
    },
  },
},
```

Always include an escape binding (like `["escape"] = "mode default"`) or your
keyboard stays in that mode until Dome exits. A config reload preserves the active
mode, but Dome falls back to the default keybindings on the next keypress if the
new config removes it.

## OS branching

`dome.os` is `"macos"` or `"windows"`. Branch on it to vary a setting or a binding
by platform:

```lua
local terminal = dome.os == "macos" and "exec open -a Terminal" or "exec wt"

return {
  keymaps = {
    ["meta+return"] = terminal,
  },
}
```

`dome.executable(name)` returns `true` when `name` resolves to an executable on
`PATH`, and `false` otherwise. Use it to gate a setting on whether a program is
installed, for example a status bar.

## Font

```lua
font = {
  text_size = 14.0, -- Body text: tab titles.
  -- family = "PingFang SC",
},
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font.text_size` | float | `14.0` | Body text size in points (`4.0` to `128.0`). |
| `font.family` | string | unset | System font for rendering. When unset, egui's built-in Ubuntu-Light is used. Dome logs a warning and falls back to built-in fonts when a commercial font cannot be used. |
