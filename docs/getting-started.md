# Getting started

## Install

### Homebrew (macOS)

```bash
brew tap l0ngvh/dome
brew install --cask dome
```

### Scoop (Windows)

```powershell
scoop bucket add dome https://github.com/l0ngvh/scoop-dome
scoop install dome
```

### Building from source

A Rust toolchain is required to build from source.

```bash
git clone https://github.com/l0ngvh/Dome
cd Dome
cargo install --path .
```

This compiles Dome in release mode and installs the `dome` binary to `~/.cargo/bin/`.

## macOS permissions

Dome requires two permissions on macOS. A restart is required after granting either one.

**Accessibility:** System Settings > Privacy & Security > Accessibility > add or enable Dome. Dome cannot manage windows without this permission.

**Screen Recording:** System Settings > Privacy & Security > Screen Recording > add or enable Dome. Dome logs an error and exits at startup if Screen Recording is denied. The symptom is Dome exiting immediately after Accessibility is already granted.

## Logs

Dome writes a single `dome.log`, overwritten on each launch:

- macOS: `~/Library/Logs/dome/dome.log`.
- Windows: `%APPDATA%\dome\logs\dome.log`.

Set `log_level` in the config (see [configuration.md](configuration.md)) to control verbosity. The `RUST_LOG` environment variable overrides it.

## Stopping Dome

Run `dome exit` from a terminal, or press the keybinding mapped to `exit` (default: `cmd+shift+q`).
