# Dome · Zebar widget

Per-monitor workspaces for [Dome](https://github.com/l0ngvh/Dome). Each monitor's bar
shows only that monitor's workspaces, resolved automatically.

## Install

Copy this directory to `%USERPROFILE%\.glzr\zebar\dome\` and enable the pack in the
Zebar GUI (**My widgets** tab).

## Run from source

Set `program` in `zpack.json` and the two `shellExec` calls in `workspaces/index.html`
to your dome binary, for example `%USERPROFILE%\src\dome\target\debug\dome.exe`.
