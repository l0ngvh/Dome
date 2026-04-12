# Dome

**Dome** is a tiling window manager for macOS and Windows.

## Why

There are already plenty of window managers on each platform, but they all
behave slightly differently, just enough to trip up your muscle memory. Dome aims
to give a consistent experience across macOS and Windows.

Dome is inspired by how window managers on Wayland work, and strives to bring
the same level of control that Wayland compositors offer on Linux to macOS and Windows.
Dome uses a modified version of i3, which respects windows' minimum size
constraints and allows for scrolling when all windows approach their minimum size.

## Quick Start

### Install

```bash
git clone https://github.com/l0ngvh/Dome
cd Dome
cargo install --path .
dome
```

Requires a [Rust toolchain](https://rustup.rs/).

On macOS, Dome needs Accessibility permissions to manage windows, and Screen
Capture permissions to render float windows. macOS will prompt you both on
first launch.

### Key bindings

Dome ships with these default keybindings. Meta is ⌘ on macOS and ⊞ Win on Windows.

| Key | Action |
|-----|--------|
| <kbd>Meta</kbd> + <kbd>H</kbd> | Focus left |
| <kbd>Meta</kbd> + <kbd>J</kbd> | Focus down |
| <kbd>Meta</kbd> + <kbd>K</kbd> | Focus up |
| <kbd>Meta</kbd> + <kbd>L</kbd> | Focus right |
| <kbd>Meta</kbd> + <kbd>P</kbd> | Focus parent container |
| <kbd>Meta</kbd> + <kbd>[</kbd> | Focus previous tab |
| <kbd>Meta</kbd> + <kbd>]</kbd> | Focus next tab |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd> | Move window left |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>J</kbd> | Move window down |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>K</kbd> | Move window up |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>L</kbd> | Move window right |
| <kbd>Meta</kbd> + <kbd>0</kbd>–<kbd>9</kbd> | Focus workspace 0–9 |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>0</kbd>–<kbd>9</kbd> | Move window to workspace 0–9 |
| <kbd>Meta</kbd> + <kbd>Alt</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Focus monitor left/down/up/right |
| <kbd>Meta</kbd> + <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Move window to monitor left/down/up/right |
| <kbd>Meta</kbd> + <kbd>E</kbd> | Toggle spawn direction |
| <kbd>Meta</kbd> + <kbd>D</kbd> | Toggle split direction |
| <kbd>Meta</kbd> + <kbd>B</kbd> | Toggle split/tabbed layout |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>F</kbd> | Toggle floating |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>Q</kbd> | Exit Dome |

See the [keybinding configuration](docs/configuration.md#keybindings) for the complete list and customization options.

### CLI

Dome can also be controlled through CLI. A few commands to get started:

```bash
dome focus left|down|up|right    # Move focus
dome move left|down|up|right     # Move window
dome toggle float                # Toggle floating
dome toggle fullscreen           # Toggle fullscreen
dome toggle layout               # Toggle split/tabbed
dome focus workspace <name>      # Switch workspace
dome exit                        # Quit Dome
```

See the [command reference](docs/commands.md) for the full list.

## Documentation

- [Getting started](docs/getting-started.md) — platform-specific setup details
- [Configuration](docs/configuration.md) — config file reference, keybindings, window rules
- [Commands](docs/commands.md) — full command reference
- [Architecture](docs/development/architecture.md) — how Dome is built (for contributors)

## Development

Dome is written in Rust with a platform-independent core and platform-specific shells for macOS and Windows. See the [architecture docs](docs/development/architecture.md) for details.

```bash
cargo install cargo-make         # Task runner (required)
cargo make setup                 # Install dev tools
cargo make coverage              # Run tests with coverage
```

## License

MIT
