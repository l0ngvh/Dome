# CLI

## `dome launch`

Starts Dome. Running `dome` on its own is shorthand for `dome launch`.

Pass `-c <path>` (or `--config <path>`) to override the config path. Without
the flag, Dome reads the default location for your platform. See
[configuration.md](configuration.md).

Every other subcommand connects to a running Dome, sends one message, and
exits.

## `dome exit`

Stops Dome and restores all windows.

## Actions

Every action listed in [commands.md](commands.md) is also a CLI subcommand. The
syntax matches what you write in `[keymaps]` bindings: `dome focus right`,
`dome move workspace 2`, `dome toggle minimized`, `dome exec "open -a Terminal"`.

## `dome query workspaces`

Prints one JSON entry per active workspace, ordered by creation:

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

`is_focused` is true for the workspace on the focused monitor. `is_visible` is
true for any workspace currently active on a monitor (one per monitor).
`window_count` totals tiling, float, and fullscreen windows without
double-counting. Empty workspaces that are not active on any monitor are pruned
and never appear.
