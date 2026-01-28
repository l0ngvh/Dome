# Dome

A cross-platform tiling window manager written in Rust.
Currently support macOS and Windows, with Linux next on the roadmap

## Features

- Automatic tiling
- Native tabbed layout
- Hot-reload configuration
- Support for multiple monitors
- Respect windows desired size
- Scrolling

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
dome focus workspace <name>
dome focus monitor up|down|left|right|<name>

dome move up|down|left|right
dome move workspace <name>
dome move monitor up|down|left|right|<name>

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

# Minimum window size - float for pixels, string for percentage (default: "5%")
min_width = 200     # 200 logical pixels
min_height = "10%"  # 10% of screen height

# Maximum window size - float for pixels, string for percentage (default: 0 = no limit)
# Windows that hit max constraints are centered within their allocated space
max_width = 800     # 800 logical pixels
max_height = "50%"  # 50% of screen height

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

"cmd+alt+h" = ["focus monitor left"]
"cmd+alt+j" = ["focus monitor down"]
"cmd+alt+k" = ["focus monitor up"]
"cmd+alt+l" = ["focus monitor right"]

"cmd+alt+shift+h" = ["move monitor left"]
"cmd+alt+shift+j" = ["move monitor down"]
"cmd+alt+shift+k" = ["move monitor up"]
"cmd+alt+shift+l" = ["move monitor right"]

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
[[macos.ignore]]
app = "System Preferences"

# Regex matching
[[macos.ignore]]
app = "/.*Preferences/"

# Match by bundle ID and title
[[macos.ignore]]
bundle_id = "com.apple.finder"
title = "Trash"

# Run actions on window open
[[macos.on_open]]
app = "Slack"
run = ["move workspace 3"]

[[macos.on_open]]
app = "Safari"
run = ["toggle float"]
```

#### Windows

```toml
# Ignore windows by process name
[[windows.ignore]]
process = "SystemSettings.exe"

# Regex matching
[[windows.ignore]]
process = "/.*Settings.*/"

# Match by title
[[windows.ignore]]
title = "Task Manager"

# Run actions on window open
[[windows.on_open]]
process = "slack.exe"
run = ["move workspace 3"]
```

## Development

```bash
# Run tests with coverage (requires cargo-make)
cargo install cargo-make
cargo make coverage
```

## License

MIT
