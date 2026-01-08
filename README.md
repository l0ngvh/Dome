# Dome

A cross-platform tiling window manager written in Rust.
Currently only support macOS, with Window and Linux next on the roadmap

## Features

- Automatic tiling
- Native tabbed layout
- Hot-reload configuration

## Installation

```bash
cargo install --path .
```

## Usage

### Launch

```bash
# Launch with default config (~/.config/dome/config.toml)
dome

# Launch with custom config
dome launch --config /path/to/config.toml
```

### Commands

Dome supports IPC commands that can be sent while the window manager is running:

```bash
dome focus up|down|left|right
dome focus parent
dome focus next_tab|prev_tab
dome focus workspace <0-9>

dome move up|down|left|right
dome move workspace <0-9>

dome toggle spawn_direction  # Toggle between horizontal/vertical/tabbed
dome toggle direction
dome toggle layout           # Toggle between split/tabbed layout
dome toggle float

dome exec <command>
dome exit
```

## Configuration

Create a config file at `~/.config/dome/config.toml`:

```toml
# Border size in pixels (default: 2.0)
border_size = 5.0

# Automatic tiling - determine split direction based on window dimensions (default: true)
automatic_tiling = true

# Focused window border color (default: light blue)
focused_color = "#6699ff"

# Unfocused window border color (default: gray)
border_color = "#4d4d4d"

# Tab bar background color
tab_bar_background_color = "#262633"

# Active tab background color
active_tab_background_color = "#4d4d66"
```

### Keybindings

```toml
[keymaps]
"cmd+0" = ["focus workspace 0"]
"cmd+1" = ["focus workspace 1"]

"cmd+shift+0" = ["move workspace 0"]
"cmd+shift+1" = ["move workspace 1"]

"cmd+h" = ["focus left"]
"cmd+j" = ["focus down"]
"cmd+k" = ["focus up"]
"cmd+l" = ["focus right"]
"cmd+p" = ["focus parent"]
"cmd+[" = ["focus prev_tab"]
"cmd+]" = ["focus next_tab"]

"cmd+shift+h" = ["move left"]
"cmd+shift+j" = ["move down"]
"cmd+shift+k" = ["move up"]
"cmd+shift+l" = ["move right"]

"cmd+e" = ["toggle spawn_direction"]
"cmd+d" = ["toggle direction"]
"cmd+b" = ["toggle layout"]
"cmd+shift+f" = ["toggle float"]

"cmd+return" = ["exec open -a Terminal"]
"cmd+shift+q" = ["exit"]
```

### Window Rules

Window rules let you customize behavior for specific applications. Rules are platform-specific.

#### macOS

```toml
# Ignore windows (don't manage)
[[macos.window_rules]]
app = "System Preferences"
manage = false

# Regex matching
[[macos.window_rules]]
app = "/.*Preferences/"
manage = false

# Match by bundle ID and title
[[macos.window_rules]]
bundle_id = "com.apple.finder"
title = "Trash"
manage = false

# Run actions on window open
[[macos.window_rules]]
app = "Slack"
run = ["move workspace 3"]

[[macos.window_rules]]
app = "Safari"
run = ["toggle float"]
```

#### Windows

```toml
# Ignore windows by process name
[[windows.window_rules]]
process = "SystemSettings.exe"
manage = false

# Regex matching
[[windows.window_rules]]
process = "/.*Settings.*/"
manage = false

# Match by title
[[windows.window_rules]]
title = "Task Manager"
manage = false

# Run actions on window open
[[windows.window_rules]]
process = "slack.exe"
run = ["move workspace 3"]
```

## License

MIT
