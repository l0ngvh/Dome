# CLI

The `dome` binary is both the daemon and its client. Running `dome launch` (or
just `dome`) starts the daemon. Every other invocation connects to a running
daemon over a local socket, sends a single message, and exits.

## `dome launch`

Starts Dome. Running `dome` on its own is shorthand for `dome launch`.

Two flags override the default file paths. `-c <path>` (or `--config <path>`)
sets the config file, and `-l <path>` (or `--layout <path>`) sets the layout
file. Without them, Dome reads the platform defaults documented in
[configuration.md](configuration.md) and [layout.md](layout.md).

## Actions

Every action listed in [commands.md](commands.md) is also a `dome` subcommand,
with the same word-for-word syntax used in `[keymaps]` bindings:

```
dome focus right
dome move workspace 2
dome toggle minimized
dome master grow
dome mode resize
dome exit
```

Action payloads with spaces need to be quoted on the command line. `exec` is
the main case, since its command string is taken verbatim:

```
dome exec "open -a Terminal"
```

The same payload in a `[keymaps]` entry lives in a TOML string and needs no
extra quoting (`"meta+return" = ["exec open -a Terminal"]`).

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
true for any workspace currently active on a monitor, one per monitor.
`window_count` totals tiling, float, and fullscreen windows without
double-counting. Empty workspaces stay in the output with a `window_count`
of 0 until Dome exits.
