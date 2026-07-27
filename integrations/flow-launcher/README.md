# Dome Flow Launcher plugin

A Flow Launcher plugin that lists Dome's minimized windows and restores the one you pick. Type the `dome` action keyword in Flow Launcher to see the list, then click a row to restore that window.

## Prerequisites

The Dome daemon must be running and `dome.exe` must be on your global `PATH`. Install via `winget` or add the install directory to `PATH` in the user environment variables.

## Install from release

1. Download `dome-<version>-flow-launcher-plugin.zip` from the latest Dome release at <https://github.com/l0ngvh/Dome/releases>. Use `dome-nightly-flow-launcher-plugin.zip` from the `nightly` release for the nightly channel.
2. Unzip. Move the resulting `flow-launcher/` folder into `%APPDATA%\FlowLauncher\Plugins\`. The final path must be `%APPDATA%\FlowLauncher\Plugins\flow-launcher\plugin.json` (or any folder name under `Plugins\` containing `plugin.json`).
3. Restart Flow Launcher. First launch triggers Flow's Python 3.11 embeddable install if not already present. The plugin registers under the `dome` action keyword.

## Install from source

The plugin is not on the Flow Launcher plugin store yet. Zip the `integrations/flow-launcher/` folder, drop the zip into `%APPDATA%\FlowLauncher\Plugins\` and unzip in place, then restart Flow Launcher.
