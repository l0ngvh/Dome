# Commands

Every Dome command works identically on macOS and Windows — the command set is part of the platform-independent core.

There are three ways to send commands:

1. **CLI** — `dome <command>` sends the command to a running Dome instance via IPC.
2. **Keybindings** — bind commands to keyboard shortcuts in the `[keymaps]` section of the config file.
3. **Window rules** — use commands in `on_open` rules to run actions when a window opens.

All three use the same command syntax and produce the same result.

> **Syntax note:** In config files, keybindings, and `on_open` rules, multi-word arguments use underscores (e.g., `next_tab`, `prev_tab`, `spawn_direction`). On the CLI, clap auto-converts these to kebab-case (e.g., `next-tab`, `prev-tab`, `spawn-direction`). The tables below use the config/keybinding form (underscores) since that is the canonical form.

## Focus

Move keyboard focus to a different window, tab, workspace, or monitor.

| Command | Description |
|---------|-------------|
| `focus up` | Focus the window above the current one in the tiling tree. |
| `focus down` | Focus the window below. |
| `focus left` | Focus the window to the left. |
| `focus right` | Focus the window to the right. |
| `focus parent` | Focus the parent container. Useful for selecting a group of windows to move or toggle. |
| `focus next_tab` | Focus the next tab in a tabbed container. |
| `focus prev_tab` | Focus the previous tab in a tabbed container. |
| `focus workspace <name>` | Switch to the named workspace (e.g., `focus workspace 2`). Workspaces are created on demand — any string is a valid name. |
| `focus monitor up\|down\|left\|right` | Focus the nearest monitor in the given direction. |
| `focus monitor <name>` | Focus the monitor with the given name. |

## Move

Move the focused window or container within the tiling tree, or to a different workspace or monitor.

| Command | Description |
|---------|-------------|
| `move up` | Move the focused window up in the tiling tree. |
| `move down` | Move the focused window down. |
| `move left` | Move the focused window left. |
| `move right` | Move the focused window right. |
| `move workspace <name>` | Move the focused window to the named workspace. |
| `move monitor up\|down\|left\|right` | Move the focused window to the nearest monitor in the given direction. |
| `move monitor <name>` | Move the focused window to the named monitor. |

## Toggle

Toggle window or container properties.

| Command | Description |
|---------|-------------|
| `toggle spawn_direction` | Cycle the spawn direction of the focused window/container between horizontal, vertical, and tabbed. Controls where the next window opens relative to the focused one. |
| `toggle direction` | Flip the parent container's split direction between horizontal and vertical. |
| `toggle layout` | Toggle the parent container between split and tabbed layout. |
| `toggle float` | Toggle the focused window between tiling and floating. Floating windows are not part of the tiling tree and can be freely positioned. Has no effect on fullscreen windows. |
| `toggle fullscreen` | Toggle the focused window between normal and fullscreen. A fullscreen window covers the entire monitor. Works from both tiling and floating states. |

Fullscreen integrates with each platform's native fullscreen behavior — macOS Spaces and Windows borderless/exclusive fullscreen are detected and respected rather than overridden.

## Other

| Command | Description |
|---------|-------------|
| `exec <command>` | Run a shell command. Everything after `exec ` is passed to the system shell. Example: `exec open -a Terminal` (macOS) or `exec cmd /c start notepad` (Windows). |
| `exit` | Stop Dome and restore all windows. |

## Launching Dome

These are not commands sent to a running instance — they control how Dome starts.

| CLI | Description |
|-----|-------------|
| `dome` | Start Dome with the default config path. |
| `dome launch` | Same as bare `dome`. |
| `dome launch --config <path>` | Start Dome with a custom config file. |

## Queries

Queries read state from the hub without modifying it. Unlike fire-and-forget action commands, queries block until the hub responds with JSON.

| Command | Description |
|---------|-------------|
| `dome query workspaces` | Returns a JSON array of workspace metadata. |

### `dome query workspaces`

Returns a JSON array with one entry per active workspace, ordered by creation order:

```json
[
  {
    "name": "0",
    "is_focused": true,
    "is_visible": true,
    "window_count": 3
  },
  {
    "name": "web",
    "is_focused": false,
    "is_visible": false,
    "window_count": 1
  }
]
```

- `is_focused` — true for the workspace on the focused monitor.
- `is_visible` — true for workspaces active on any monitor (one per monitor).
- `window_count` — total windows (tiling + float + fullscreen), no double-counting.

Empty workspaces that are not active on any monitor are pruned and never appear.

## CLI Examples

```bash
# Focus the window to the right
dome focus right

# Focus the next tab (CLI uses kebab-case)
dome focus next-tab

# Move focused window to workspace 3
dome move workspace 3

# Toggle spawn direction (CLI uses kebab-case)
dome toggle spawn-direction

# Toggle floating mode
dome toggle float

# Launch a terminal
dome exec open -a Terminal

# Stop Dome
dome exit
```

## Restrictions

Some commands are blocked when the focused window has restrictions (e.g., a window in native fullscreen). The behavior depends on the restriction level:

- **Tiling navigation** (`focus`/`move` directional, `focus parent`, tab navigation, `toggle spawn_direction`, `toggle direction`, `toggle layout`) — blocked when the window has full restrictions.
- **Display mode changes** (`toggle float`, `toggle fullscreen`) — blocked when the window has any restriction (full or fullscreen-protected).
- **Workspace moves** (`move workspace`) — blocked only by full restrictions. Fullscreen windows can move across workspaces.
- **Monitor moves** (`move monitor`) — blocked when the window has any restriction. Fullscreen windows are bound to their monitor.

When a command is blocked, it silently does nothing.
