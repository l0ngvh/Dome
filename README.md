# Dome

**Dome** is a constraint-aware scrollable tiling window manager that works on
Windows and macOS.

## Background

Before Dome, there were already plenty of excellent window managers for macOS
and Windows. The problem is, I'd like to enjoy gaming with friends on Windows,
while still having to ship code handed to me by my manager on macOS, and
maintaining two sets of configuration and getting them to behave consistently
takes a lot of work. So I just decided to put in even more work and build Dome.

## Install

Currently, to install Dome, you have to build from source, which requires a
[Rust toolchain](https://rustup.rs/).

```bash
git clone https://github.com/l0ngvh/Dome
cd Dome
cargo install --path .
dome
```

We'll have proper one-click installers once the project reaches a stable point.

On macOS, Dome needs Accessibility permissions to manage windows, and Screen
Capture permissions to render float windows. macOS will prompt you for both on
first launch. No extra permissions are required on Windows.

## Usage

By default, Dome uses a modified version of the i3 layout, where each window is
a leaf of a layout tree rooted at the workspace. Each window has a cap on how
small (or big) it can be, and thus the whole workspace can be scrolled when the
windows can't all fit on the screen. Dome ships with the following default
keybindings:

| Key | Action |
|-----|--------|
| <kbd>Meta</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Focus left/down/up/right |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Move window left/down/up/right |
| <kbd>Meta</kbd> + <kbd>0</kbd>–<kbd>9</kbd> | Focus workspace 0–9 |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>0</kbd>–<kbd>9</kbd> | Move window to workspace 0–9 |
| <kbd>Meta</kbd> + <kbd>Alt</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Focus monitor left/down/up/right |
| <kbd>Meta</kbd> + <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Move window to monitor left/down/up/right |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>F</kbd> | Toggle floating |
| <kbd>Meta</kbd> + <kbd>B</kbd> | Toggle split/tabbed layout |
| <kbd>Meta</kbd> + <kbd>Shift</kbd> + <kbd>Q</kbd> | Exit Dome |

Dome can also be controlled via the CLI:

```bash
dome focus left|down|up|right    # Move focus
dome move left|down|up|right     # Move window
dome toggle float|fullscreen     # Toggle floating or fullscreen
dome toggle layout               # Toggle split/tabbed
dome toggle minimized            # Open minimized window picker
dome focus workspace <name>      # Switch workspace
dome exit                        # Quit Dome
```

See the [keybinding configuration](docs/configuration.md#keybindings) and
[command reference](docs/commands.md) for the complete list.

To check why Dome did what it did, look at `dome.log`. It's written fresh on each
launch to `~/Library/Logs/dome/dome.log` on macOS, or
`%APPDATA%\dome\logs\dome.log` on Windows.

## Configuring Dome

Dome is configured by editing two TOML files. The default locations are:

- macOS: `~/.config/dome/config.toml` and `~/.config/dome/layout.toml` (or under `$XDG_CONFIG_HOME/dome/`).
- Windows: `%APPDATA%\dome\config.toml` and `%APPDATA%\dome\layout.toml`.

`config.toml` covers general settings, keybindings, and window rules. `layout.toml` covers tiling strategy, window-size constraints, and per-strategy parameters. Changes take effect when you save.

## Documentation

- [Configuration](docs/configuration.md): config file reference, window rules, and keybindings
- [Layout](docs/layout.md): layout strategy, window-size constraints, and per-strategy parameters
- [Commands](docs/commands.md): full command reference
- [CLI](docs/cli.md): command-line interface usage
- [FQG](docs/faq.md): command-line interface usage

## Credits

Dome draws a lot of inspiration from these awesome WMs and likely wouldn't
exist without them:
- [AeroSpace](https://github.com/nikitabobko/AeroSpace)
- [GlazeWM](https://github.com/glzr-io/glazewm)
- [komorebi](https://github.com/LGUG2Z/komorebi)
- [Sway](https://github.com/swaywm/sway)
- [niri](https://github.com/niri-wm/niri)

## License

Dome is released under the [MIT License](LICENSE).
