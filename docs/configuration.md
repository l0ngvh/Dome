# Configuration

Dome is configured by editing a single TOML file. Changes take effect when you save.

By default, Dome reads from:

- macOS: `~/.config/dome/config.toml` (or `$XDG_CONFIG_HOME/dome/config.toml`).
- Windows: `%APPDATA%\dome\config.toml`.

To use a different file, pass `dome launch -c <path>` (see [cli.md](cli.md)).

## General

Top-level appearance and operational settings.

```toml
border_size = 4.0
theme = "mocha"
log_level = "info"
start_at_login = false
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `border_size` | float | `4.0` | Border width around windows, in logical pixels. |
| `theme` | string | `"mocha"` | Color theme. One of `"latte"`, `"frappe"`, `"macchiato"`, `"mocha"` ([Catppuccin](https://catppuccin.com/) flavors). |
| `log_level` | string | `"info"` | Log verbosity. One of `trace`, `debug`, `info`, `warn`, `error`. |
| `start_at_login` | boolean | `false` | Launch Dome at user login. |

## Window size constraints

Top-level minimum and maximum window dimensions, enforced by both the partition tree and master strategies. A size value is either a number (logical pixels) or a string ending in `%` (percentage of the screen dimension). Per-window constraints reported by the OS take precedence over these global values.

```toml
min_width = "5%"
min_height = "5%"
max_width = 0       # 0 means no limit
max_height = 0
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `min_width` | float or string | `"5%"` | Minimum window width. |
| `min_height` | float or string | `"5%"` | Minimum window height. |
| `max_width` | float or string | `0` | Maximum window width. `0` means no limit. |
| `max_height` | float or string | `0` | Maximum window height. `0` means no limit. |

## Font

Font sizes live under the `[font]` table. Dome uses egui's built-in font stack (Ubuntu-Light proportional, Hack monospace, plus emoji fallbacks); custom font families are not configurable.

```toml
[font]
text_size = 14.0     # Body text: tab titles, picker labels.
subtext_size = 12.0  # Secondary text: picker app-name subtext.
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font.text_size` | float | `14.0` | Body text size in points. Must be in `[4.0, 128.0]`. |
| `font.subtext_size` | float | `12.0` | Secondary text size in points. Must be in `[4.0, 128.0]`. |

`tab_bar_height` (under `[layout.partition_tree]`) does not auto-scale with `text_size`, so long tab titles may truncate earlier as the body size grows.

## Layout

The `[layout]` table selects the tiling strategy and holds per-strategy parameters. Both sub-tables (`[layout.partition_tree]` and `[layout.master]`) are always parsed and validated regardless of which strategy is active, so a typo in the inactive block surfaces immediately rather than hiding until `strategy` is flipped.

```toml
[layout]
strategy = "partition_tree"   # or "master"

[layout.partition_tree]
tab_bar_height = 24.0
automatic_tiling = true

[layout.master]
master_ratio = 0.5
master_count = 1
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `layout.strategy` | string | `"partition_tree"` | Active layout strategy. One of `partition_tree` or `master`. |
| `layout.partition_tree.automatic_tiling` | boolean | `true` | When true, Dome picks horizontal or vertical split based on the focused window's dimensions. When false, new windows split in the current container's direction. |
| `layout.partition_tree.tab_bar_height` | float | `24.0` | Height of the tab bar in tabbed containers, in logical pixels. |
| `layout.master.master_ratio` | float | `0.5` | Width of the master area as a fraction of the workspace width. Must be in `[0.1, 0.9]`. |
| `layout.master.master_count` | integer | `1` | Number of windows in the master area. Must be `>= 1`. |

### Partition tree

The default strategy. i3-style manual tiling with split containers (horizontal, vertical, tabbed), spawn-mode routing, and direction invariance. See [architecture.md](development/architecture.md#partitiontreestrategy) for the full model.

### Master

A two-area layout: the first `master_count` windows fill a master pane on the left, and the rest stack vertically in a pane on the right. Modeled on xmonad's `Tall` layout.

Both panes honor the global `min_width`, `min_height`, `max_width`, and `max_height` constraints above, and per-window constraints reported by the OS take precedence. Each pane scrolls vertically when per-window min heights push the pane's content past the screen height. Scroll is focus-driven, meaning that moving focus inside a pane brings the focused window into view. No new keybindings or actions are needed.

## Window rules

These hooks run once a window matching the criteria is identified. Each hook is one of two kinds:

- `ignore`: do not manage the window at all.
- `on_open`: run a list of actions when the window first appears.

### Matching

All fields in a rule must match for the rule to apply (AND logic). Rules are evaluated in order, and the first matching rule wins. String values are matched exactly by default. To match a regular expression instead, wrap the pattern in forward slashes (`/pattern/`). The regex is matched against the full string. A rule must specify at least one matching field.

### macOS

macOS rules match on `app` (the application name, regex-capable), `bundle_id` (exact match only), and `title` (regex-capable).

```toml
[macos]
ignore = [
  { app = "System Preferences" },                       # exact app name
  { app = "/.*Preferences/" },                          # regex on app name
  { bundle_id = "com.apple.finder", title = "Trash" },  # bundle and title (AND)
]
on_open = [
  { app = "Slack", run = ["move workspace 3"] },
  { app = "Safari", run = ["toggle float"] },
]
```

### Windows

Windows rules match on `process` (the executable name, regex-capable) and `title` (regex-capable).

```toml
[windows]
ignore = [
  { process = "SystemSettings.exe" },
  { process = "/.*Settings.*/" },
  { title = "Task Manager" },
]
on_open = [
  { process = "slack.exe", run = ["move workspace 3"] },
]
```

## Keybindings

Keybindings live in the `[keymaps]` table of the config file. The same file works on macOS and Windows without changes. Defining a `[keymaps]` table **replaces the defaults wholesale**. There is no merge. To keep any default binding and add to it, copy the defaults below and edit the copy.

### Syntax

Each entry maps a key combination to one or more action strings:

```toml
"mods+...+key" = ["<action>", ...]
```

Tokens are lowercase. Accepted modifier tokens: `meta`, `shift`, `alt`, `ctrl`. `cmd` and `win` are platform-flavored aliases for `meta`. Multiple modifiers are joined with `+`. The key is the final segment after all modifiers.

Values are arrays of action strings. Single-element arrays are fine. Multi-action arrays fire in order.

Examples:

```toml
"meta+h" = ["focus left"]
"meta+shift+1" = ["move workspace 1", "focus workspace 1"]
```

### Modifier mapping

The literal config token is `meta`. `cmd` and `win` are accepted as aliases and behave identically. It maps to the platform key shown in the table below.

| Token | macOS | Windows |
|-------|-------|---------|
| `meta` (alias: `cmd`, `win`) | <kbd>Command</kbd> | <kbd>Windows</kbd> |
| `shift` | <kbd>Shift</kbd> | <kbd>Shift</kbd> |
| `alt` | <kbd>Option</kbd> | <kbd>Alt</kbd> |
| `ctrl` | <kbd>Control</kbd> | <kbd>Control</kbd> |

### Default bindings

Dome ships 44 default bindings.

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
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>q</kbd> | `exit` |

### Customising

Define a `[keymaps]` section to replace the defaults with your own bindings. The entire default set is discarded when you do this, so copy any defaults you want to keep.

```toml
[keymaps]
"meta+h" = ["focus left"]
"meta+l" = ["focus right"]
"meta+return" = ["exec open -a Terminal"]
```

### Multi-action bindings

A binding's value is an array of action strings. All actions fire in order on a single keypress:

```toml
"meta+shift+1" = ["move workspace 1", "focus workspace 1"]
```

### Modes

Dome supports modal keybindings. The `[keymaps]` section defines the **default** mode. Additional modes are defined with `[keymaps.mode.<name>]` sections. Switch between modes using the `mode <name>` action in a binding or via `dome mode <name>` over CLI/IPC.

```toml
[keymaps]
"meta+h" = ["focus left"]
"meta+r" = ["mode resize"]

[keymaps.mode.resize]
"h" = ["master shrink"]
"l" = ["master grow"]
"escape" = ["mode default"]
```

In this example, `meta+r` enters resize mode. Inside resize mode, `h` and `l` adjust the master area without modifiers, and `escape` returns to the default keybindings. The special name `"default"` always refers to the top-level `[keymaps]` table.

Mode switching is instant. When a binding contains `mode <name>`, the switch takes effect before the next keypress. A binding can combine a mode switch with other actions:

```toml
"meta+r" = ["focus left", "mode resize"]
```

This focuses left and enters resize mode in one keypress. If a binding lists multiple `mode` actions, the last one wins.

#### Reserved names

- `"default"` refers to the top-level `[keymaps]` section. Using it as a `[keymaps.mode.default]` section name causes a config validation error.
- Empty string `""` is rejected as a mode name.

#### Gotchas

**No automatic escape binding.** Dome does not enforce that a mode has a binding back to `default`. If you define a mode with no way out, your keyboard will only have the bindings in that mode until config reload or process exit. Always include an escape binding (like `"escape" = ["mode default"]`) in every custom mode.

**Config reload preserves active mode.** If you edit your config while in a non-default mode, hot reload keeps you in that mode as long as it still exists in the new config. If the reloaded config removes the active mode, Dome falls back to the default keybindings on the next keypress and logs a warning on each keystroke until you switch back to an existing mode (for example, `dome mode default`).

**Unknown mode names are rejected.** Running `dome mode typo` or pressing a binding with `mode typo` logs a warning and leaves your current mode unchanged.

**Mode state is global.** Modes are per-process, not per-workspace or per-monitor. Switching workspaces does not change the active mode.

