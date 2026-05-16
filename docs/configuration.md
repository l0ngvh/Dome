# Configuration

Dome reads a single TOML file at startup and watches it for changes. Every setting is optional and falls back to a built-in default. The `dome launch -c <path>` flag overrides the location (see [cli.md](cli.md)).

Default paths:

- macOS: `~/.config/dome/config.toml` (or `$XDG_CONFIG_HOME/dome/config.toml`).
- Windows: `%APPDATA%\dome\config.toml`.

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

Top-level minimum and maximum window dimensions. A size value is either a number (logical pixels) or a string ending in `%` (percentage of the screen dimension).

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

The `[layout]` table selects the tiling strategy and holds per-strategy parameters. Both sub-tables (`[layout.partition_tree]` and `[layout.master_stack]`) are always parsed and validated regardless of which strategy is active, so a typo in the inactive block surfaces immediately rather than hiding until `active` is flipped.

```toml
[layout]
active = "partition_tree"   # or "master_stack"

[layout.partition_tree]
tab_bar_height = 24.0
auto_tile = true

[layout.master_stack]
master_ratio = 0.5
master_count = 1
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `layout.active` | string | `"partition_tree"` | Active layout strategy. One of `partition_tree` or `master_stack`. |
| `layout.partition_tree.auto_tile` | boolean | `true` | When true, Dome picks horizontal or vertical split based on the focused window's dimensions. When false, new windows split in the current container's direction. |
| `layout.partition_tree.tab_bar_height` | float | `24.0` | Height of the tab bar in tabbed containers, in logical pixels. |
| `layout.master_stack.master_ratio` | float | `0.5` | Width of the master area as a fraction of the workspace width. Must be in `[0.1, 0.9]`. |
| `layout.master_stack.master_count` | integer | `1` | Number of windows in the master area. Must be `>= 1`. |

### Partition tree

The default strategy. i3-style manual tiling with split containers (horizontal, vertical, tabbed), spawn-mode routing, and direction invariance. See [architecture.md](development/architecture.md#partitiontreestrategy) for the full model.

### Master stack

A two-area layout: the first `master_count` windows fill a master area on the left, and the rest stack vertically on the right. Modeled on xmonad's `Tall` layout.

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

