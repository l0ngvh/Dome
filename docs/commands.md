# Commands

An action is a single instruction that mutates Dome's window state, like `focus right` or `toggle float`. The action surface is identical on macOS and Windows, and the same syntax appears everywhere actions are used: the `[keymaps]` table in the config file (see [configuration.md](configuration.md#keybindings)) and the `dome` CLI (see [cli.md](cli.md)).

## Focus

Move keyboard focus to a different window, container, tab, workspace, or monitor.

| Action | Effect |
|--------|--------|
| `focus up`, `focus down`, `focus left`, `focus right` | Focus the neighboring window in the tiling tree. In the master layout, vertical focus movement inside a pane scrolls the pane to keep the focused window visible when content overflows. |
| `focus parent` | Focus the parent container. Subsequent `move` and `toggle` actions then target the whole group. |
| `focus tab next` | Focus the next tab in a tabbed container. |
| `focus tab prev` | Focus the previous tab. |
| `focus workspace <name>` | Switch to the named workspace, e.g. `focus workspace 2`. Workspaces are created on demand and any string is a valid name. |
| `focus monitor up`, `focus monitor down`, `focus monitor left`, `focus monitor right` | Focus the nearest monitor in that direction. |
| `focus monitor <name>` | Focus the monitor with the given name. |

## Move

Move the focused window or container within the tiling tree, or to a different workspace or monitor.

| Action | Effect |
|--------|--------|
| `move up`, `move down`, `move left`, `move right` | Move the focused window in the tiling tree. |
| `move workspace <name>` | Move the focused window to the named workspace. |
| `move monitor up`, `move monitor down`, `move monitor left`, `move monitor right` | Move the focused window to the nearest monitor in that direction. |
| `move monitor <name>` | Move the focused window to the named monitor. |

## Container layout

These actions reshape the container holding the focused window. To target a specific ancestor instead of the immediate parent, run `focus parent` first.

| Action | Effect |
|--------|--------|
| `toggle spawn` | Cycle the spawn direction between horizontal, vertical, and tabbed. The spawn direction decides where the next new window lands relative to the focused one. |
| `toggle direction` | Flip the parent container's split direction between horizontal and vertical. |
| `toggle layout` | Toggle the parent container between split and tabbed layout. |

## Window state

These actions change the focused window's display mode.

| Action | Effect |
|--------|--------|
| `toggle float` | Toggle the focused window between tiling and floating. No effect on fullscreen windows. |
| `toggle fullscreen` | Toggle the focused window between normal and fullscreen. Works for both tiling and float windows. |
| `toggle minimized` | Open or close the minimized window picker. |
| `close` | Close the focused window. Sends the platform-native close request. The app decides whether to prompt the user or exit immediately. |

A window toggled into floating or fullscreen is placed next to the last-focused tiling window on the current workspace. A floated window toggled back to tiling restores its previous tiling dimension. Toggling fullscreen off reveals the next lower fullscreen window, if any.

Fullscreen integrates with each platform's native fullscreen behavior: macOS Spaces and Windows borderless or exclusive fullscreen are detected and respected. While a native fullscreen window is focused, `toggle float`, `toggle fullscreen`, and `move monitor` have no effect. Windows exclusive fullscreen additionally blocks every other action, including tiling navigation, workspace moves, and master-area adjustments.

## Master area

The master-stack layout reserves a configurable area on one side for `master_count` windows. These actions adjust that area at runtime, and have effect only when the master-stack layout is active. Changes are per-workspace and persist across config reloads.

Per-window min-width constraints can override the ratio when honoring them requires a wider pane than the ratio would allow.

| Action | Effect |
|--------|--------|
| `master grow` | Increase the master area by 5 percentage points, clamped to `0.1..=0.9` of the workspace. |
| `master shrink` | Decrease the master area by the same step. |
| `master more` | Add one window slot to the master area. |
| `master fewer` | Remove one window slot from the master area, with a minimum of 1. |

## Other commands

These actions do not target windows.

| Action | Effect |
|--------|--------|
| `exec <command>` | Run a shell command. The payload after `exec ` is passed verbatim to the system shell. |
| `mode <name>` | Switch to a named keybinding mode. `mode default` returns to the default keybindings. See [configuration.md](configuration.md#modes). |
| `exit` | Stop Dome and restore all windows. |

> **Note**
> 
> Do not run Dome with elevated privileges. `exec` runs arbitrary shell commands, so
> anyone with access to the user's shell or Dome's IPC socket would inherit
> those privileges.
