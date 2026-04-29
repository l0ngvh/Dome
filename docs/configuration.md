# Configuration

Create a config file at `~/.config/dome/config.toml` (macOS/Linux) or `%APPDATA%\dome\config.toml` (Windows). All settings are optional — Dome uses sensible defaults for anything not specified. Changes are applied automatically via hot reload.

The config format is TOML, and the same file works on both macOS and Windows. Changes are hot-reloaded on save, so you can tweak settings without restarting Dome. The one area where config is platform-specific is window rules: macOS and Windows identify applications differently (bundle IDs vs. process names), so matching fields differ by platform.

## Reference

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `border_size` | float | `4.0` | Border width around windows, in logical pixels. |
| `border_radius` | float | `12.0` | Corner radius for window and container borders, in logical pixels. `0` means sharp corners. Clamped at runtime to half the smallest window dimension, so large values are safe. |
| `tab_bar_height` | float | `24.0` | Height of the tab bar in tabbed containers, in logical pixels. |
| `automatic_tiling` | boolean | `true` | When `true`, Dome chooses horizontal or vertical split based on the focused window's dimensions. When `false`, new windows always split in the current container's direction. |
| `min_width` | float or string | `"5%"` | Minimum window width. A number means logical pixels (e.g., `200`). A string with `%` means percentage of screen width (e.g., `"10%"`). |
| `min_height` | float or string | `"5%"` | Minimum window height. Same format as `min_width`, relative to screen height. |
| `max_width` | float or string | `0` | Maximum window width. Same format as `min_width`. `0` means no limit. Windows that hit the max are centered within their allocated space. |
| `max_height` | float or string | `0` | Maximum window height. Same format as `min_height`. `0` means no limit. |
| `theme` | string | `"mocha"` | Color theme. One of `"latte"`, `"frappe"`, `"macchiato"`, `"mocha"` ([Catppuccin](https://catppuccin.com/) flavors, light to dark). Changes apply live via hot reload. |
| `log_level` | string | `"info"` | Log verbosity. One of: `trace`, `debug`, `info`, `warn`, `error`. |

## Size Constraints

Size values accept either a number (logical pixels) or a string ending in `%` (percentage of screen dimension). Percentages must be between 0 and 100. Pixel values must be non-negative. If both `min_width` and `max_width` are set as pixel values, `min_width` must not exceed `max_width` (same for height).

```toml
min_width = 200       # 200 logical pixels
min_height = "10%"    # 10% of screen height
max_width = "50%"     # 50% of screen width
max_height = 800      # 800 logical pixels
```

## Full Example

```toml
border_size = 2.0
border_radius = 0.0
tab_bar_height = 24.0
automatic_tiling = true
min_width = "5%"
min_height = "5%"
max_width = 0
max_height = 0
theme = "mocha"
log_level = "info"
```

## Upgrading from Per-Color Config

Older versions of Dome used five separate color fields (`focused_color`, `spawn_indicator_color`, `border_color`, `tab_bar_background_color`, `active_tab_background_color`). These have been replaced by the single `theme` field. If your config still contains any of the old color fields, Dome will fail to parse your config, print an error to stderr, and fall back to built-in defaults. This means your keybindings, window rules, and other settings silently revert to defaults. Remove the old fields and use `theme` instead.

## Log File

Dome writes logs to a single `dome.log` file, overwritten on each launch:

- macOS: `~/Library/Logs/dome/dome.log`
- Windows: `%APPDATA%\dome\logs\dome.log`
- Linux: `$XDG_STATE_HOME/dome/dome.log` (defaults to `~/.local/state/dome/dome.log`)

## Keybindings

Keybindings are defined in the `[keymaps]` section of the config file. Each entry maps a key combination to one or more commands. Dome ships with a full set of default keybindings (listed below) — if you define a `[keymaps]` section, it completely replaces the defaults rather than merging with them.

### Syntax

Keys are written as `"modifier+modifier+key"` (all lowercase). Available modifiers: `cmd`, `shift`, `alt`, `ctrl`. Multiple modifiers are joined with `+`. The key is the final segment after all modifiers. Examples: `"cmd+h"`, `"cmd+shift+return"`, `"ctrl+alt+1"`.

Platform note: `cmd` maps to the Command key (⌘) on macOS and the Windows key (⊞) on Windows. `ctrl` maps to Control on both platforms. `alt` maps to Option on macOS and Alt on Windows.

This means the same `[keymaps]` section works on both platforms without changes. Your muscle memory transfers between platforms — the same shortcuts, the same behavior.

Values are arrays of command strings. Each command is executed in order. Example with a single command:

```toml
"cmd+h" = ["focus left"]
```

Multi-command example (not a default binding):

```toml
"cmd+shift+1" = ["move workspace 1", "focus workspace 1"]
```

This would move the focused window to workspace 1, then switch to it.

### Default Keybindings

#### Workspace Focus

| Key | Command |
|-----|---------|
| `cmd+0` through `cmd+9` | `focus workspace 0` through `focus workspace 9` |

#### Workspace Move

| Key | Command |
|-----|---------|
| `cmd+shift+0` through `cmd+shift+9` | `move workspace 0` through `move workspace 9` |

#### Focus Navigation

| Key | Command | Description |
|-----|---------|-------------|
| `cmd+h` | `focus left` | Focus the window to the left |
| `cmd+j` | `focus down` | Focus the window below |
| `cmd+k` | `focus up` | Focus the window above |
| `cmd+l` | `focus right` | Focus the window to the right |
| `cmd+p` | `focus parent` | Focus the parent container |
| `cmd+[` | `focus prev_tab` | Focus the previous tab |
| `cmd+]` | `focus next_tab` | Focus the next tab |

#### Move Window

| Key | Command | Description |
|-----|---------|-------------|
| `cmd+shift+h` | `move left` | Move focused window left |
| `cmd+shift+j` | `move down` | Move focused window down |
| `cmd+shift+k` | `move up` | Move focused window up |
| `cmd+shift+l` | `move right` | Move focused window right |

#### Monitor Focus

| Key | Command |
|-----|---------|
| `cmd+alt+h` | `focus monitor left` |
| `cmd+alt+j` | `focus monitor down` |
| `cmd+alt+k` | `focus monitor up` |
| `cmd+alt+l` | `focus monitor right` |

#### Monitor Move

| Key | Command |
|-----|---------|
| `cmd+alt+shift+h` | `move monitor left` |
| `cmd+alt+shift+j` | `move monitor down` |
| `cmd+alt+shift+k` | `move monitor up` |
| `cmd+alt+shift+l` | `move monitor right` |

#### Toggles

| Key | Command | Description |
|-----|---------|-------------|
| `cmd+e` | `toggle spawn_direction` | Toggle the spawn direction between horizontal, vertical, and tabbed |
| `cmd+d` | `toggle direction` | Flip the split direction between horizontal and vertical |
| `cmd+b` | `toggle layout` | Toggle the parent container between split and tabbed |
| `cmd+shift+f` | `toggle float` | Toggle focused window between tiling and floating |

#### Other

| Key | Command |
|-----|---------|
| `cmd+shift+q` | `exit` |

Note: the default keybindings do not include `exec`, `toggle fullscreen`, or `toggle minimize_picker` bindings. The example config at `examples/config.toml` includes these as examples: `"cmd+return" = ["exec open -a Terminal"]` (macOS), `"cmd+shift+return" = ["toggle fullscreen"]`, and `"cmd+m" = ["toggle minimize_picker"]`. Users who want these bindings must add them to their `[keymaps]` section. Keeping the defaults focused on core navigation means Dome is useful immediately and customizable later.

## Window Rules

Window rules let you customize how Dome handles specific applications. Rules are defined under platform-specific sections (`[macos]` or `[windows]`) because the matching fields differ by platform. There are two types of rules: `ignore` (don't manage the window at all) and `on_open` (run commands when the window first appears).

This is the one area where Dome's config acknowledges platform differences directly. Everywhere else — keybindings, general settings, commands — the config is identical across platforms.

Window rules are the one area where Dome's config is platform-specific. macOS and Windows identify applications differently (bundle IDs vs. process names), and a cross-platform abstraction would be leaky. Dome surfaces this honestly rather than papering over it.

### Matching Logic

- All fields in a rule must match for the rule to apply (AND logic). If a rule specifies both `app` and `title`, the window must match both.
- Rules are evaluated in order. The first matching rule wins — subsequent rules are not checked.
- String values are matched exactly by default. To use a regular expression, wrap the pattern in forward slashes: `/pattern/`. The regex is matched against the full string.
- A rule must specify at least one matching field to be valid.

### macOS Rules

Available matching fields:

- `app` — the application name (e.g., `"System Preferences"`). Supports regex with `/pattern/`.
- `bundle_id` — the application's bundle identifier (e.g., `"com.apple.finder"`). Exact match only.
- `title` — the window title. Supports regex with `/pattern/`.

Ignore examples:

```toml
[macos]
ignore = [
  { app = "System Preferences" },
  { app = "/.*Preferences/" },
  { bundle_id = "com.apple.finder", title = "Trash" },
]
```

The first rule ignores all System Preferences windows by exact app name. The second ignores any app whose name ends with "Preferences" using regex. The third ignores only the Trash window in Finder (both `bundle_id` and `title` must match).

On-open examples:

```toml
[macos]
on_open = [
  { app = "Slack", run = ["move workspace 3"] },
  { app = "Safari", run = ["toggle float"] },
]
```

`on_open` rules run a list of commands when a matching window first appears. The `run` field takes the same command strings used in keybindings and the CLI. The first rule moves Slack windows to workspace 3 on open. The second makes Safari windows float on open.

### Windows Rules

Available matching fields:

- `process` — the process executable name (e.g., `"SystemSettings.exe"`). Supports regex with `/pattern/`.
- `title` — the window title. Supports regex with `/pattern/`.

Ignore examples:

```toml
[windows]
ignore = [
  { process = "SystemSettings.exe" },
  { process = "/.*Settings.*/" },
  { title = "Task Manager" },
]
```

Dome has built-in ignore rules for common Windows system windows (`LockApp.exe`, `SearchHost.exe`, `StartMenuExperienceHost.exe`, and certain internal UI windows). These are always active — you only need to add your own custom ignore rules under `[windows] ignore`. User-defined rules are checked in addition to the built-in rules.

On-open examples:

```toml
[windows]
on_open = [
  { process = "slack.exe", run = ["move workspace 3"] },
]
```

### Combining Rules

A complete example showing both `ignore` and `on_open` together:

```toml
[macos]
ignore = [
  { app = "System Preferences" },
  { bundle_id = "com.apple.finder", title = "Trash" },
]
on_open = [
  { app = "Slack", run = ["move workspace 3"] },
  { app = "Safari", run = ["toggle float"] },
]
```
