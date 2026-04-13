# Getting Started

## Prerequisites

A Rust toolchain is required. Install one via [rustup.rs](https://rustup.rs/). The project uses the nightly toolchain (as specified in `rust-toolchain.toml`), which Cargo will install automatically on first build.

**macOS:** Dome requires Accessibility permissions. After first launch, macOS will prompt to grant access in System Settings → Privacy & Security → Accessibility. Dome cannot manage windows without this permission.

## Homebrew (macOS)

```bash
brew tap longvh/dome
brew install --cask dome
```

This installs `Dome.app` to `/Applications/` and symlinks the `dome` CLI into your PATH.

## Scoop (Windows)

```powershell
scoop bucket add dome https://github.com/longvh/scoop-dome
scoop install dome
```

This installs `dome.exe` and adds it to your PATH via Scoop's shim.

## Building from Source

```bash
git clone https://github.com/longvh/dome
cd dome
cargo install --path .
```

This compiles Dome in release mode and installs the `dome` binary to `~/.cargo/bin/`, which should be in your `PATH` if Rust was installed via rustup.

## macOS App Bundle

To produce a `Dome.app` bundle with a dock icon:

```bash
cargo make bundle
```

The bundle is created at `target/bundle/Dome.app`. Copy it to `/Applications/` for a permanent install. The binary inside the bundle also works as a CLI tool — `Dome.app/Contents/MacOS/dome focus left` etc. all work when invoked directly.

To launch Dome automatically at login, add `start_at_login = true` to your config file. This registers a LaunchAgent plist; setting it to `false` removes it. This option only works on macOS.

## Launching Dome

```bash
dome
```

Running `dome` with no arguments starts the window manager using the default config at `~/.config/dome/config.toml` (on macOS/Linux) or `%APPDATA%\dome\config.toml` (on Windows). If the config file doesn't exist, Dome uses built-in defaults. `dome` (bare) and `dome launch` (with no flags) are equivalent — both start the window manager with the default config path.

To use a custom config path:

```bash
dome launch --config /path/to/config.toml
```

## What Happens on Launch

Dome takes over window management immediately. All existing windows on the current monitor are tiled automatically. Even as Dome discovers and tiles existing windows, keyboard input is processed independently from tiling work — your keypresses are never lost. New windows that open are inserted into the tiling tree next to the focused window. The config file is watched for changes and reloaded automatically (hot reload).

This immediate, zero-config experience is intentional — Dome is designed to stay out of your way.

## Sending Commands

While Dome is running, you can send commands from another terminal using the `dome` CLI. For example:

```bash
dome focus right
dome move workspace 2
dome toggle float
```

These commands communicate with the running Dome instance over IPC. See the [Command Reference](../commands/reference.md) for the full list. Most users bind commands to keyboard shortcuts instead of typing them — see [Keybindings](configuration.md#keybindings).

## Stopping Dome

Run `dome exit` from a terminal, or press the keybinding mapped to `exit` (default: `Cmd+Shift+Q`). On Windows, `cmd` maps to the Windows key (⊞), so the equivalent is `Win+Shift+Q`. Dome restores all windows to their original positions and exits.
