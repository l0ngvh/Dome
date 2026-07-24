# CLI

The `dome` binary is both the daemon and its client. Running `dome launch` (or
just `dome`) starts the daemon. Every other invocation connects to a running
daemon over a local socket, sends a single message, and exits.

## `dome launch`

Starts Dome. Running `dome` on its own is shorthand for `dome launch`.

Two flags override the default file paths. `-c <path>` (or `--config <path>`)
sets the config file, and `-l <path>` (or `--layout <path>`) sets the layout
file. Without them, Dome reads the platform defaults documented in
[configuration.md](configuration.md) and [preferred-layout.md](preferred-layout.md).

## Actions

Every action listed in [commands.md](commands.md) is also a `dome` subcommand,
with the same word-for-word syntax used in `[keymaps]` bindings:

```bash
dome focus right
dome move workspace 2
dome toggle minimized
dome master grow
dome mode resize
dome exit
```

Action payloads with spaces need to be quoted on the command line. `exec` is
the main case, since its command string is taken verbatim:

```bash
dome exec "open -a Terminal"
```

The same payload in a `[keymaps]` entry lives in a TOML string and needs no
extra quoting (`"meta+return" = ["exec open -a Terminal"]`).

## `dome query workspaces`

Prints one JSON entry per active workspace, ordered by creation:

```json
[
  {
    "name": "0",           // workspace name from the config
    "is_focused": true,    // true for the workspace on the focused monitor
    "is_visible": true,    // true for the workspace shown on each monitor, one per monitor
    "window_count": 3      // tiling + float + fullscreen, no double-count, stays 0 for empty workspaces until Dome exits
  },
  {
    "name": "web",
    "is_focused": false,
    "is_visible": false,
    "window_count": 1
  }
]
```

## `dome query minimized`

Prints one JSON entry per minimized window, in the order they were minimized:

```json
[
  {
    "id": 7,                   // bare integer (not a wrapped object), pair with `dome unminimize-window <id>` to restore
    "title": "draft.md - Zed", // window title
    "app_id": "dev.zed.Zed",   // nullable, on macOS the app's bundle identifier for Raycast and similar launcher icon APIs
    "app_name": "Zed"          // nullable
  },
  {
    "id": 12,
    "title": "inbox",
    "app_id": null,
    "app_name": null
  }
]
```

## `dome unminimize-window <id>`

Restores a specific minimized window by id. External callers pair
`dome query minimized` with this command to build their own picker.

Keymaps cannot bind to this action. `WindowId`s are not stable across daemon
restarts, so a bound id would refer to a different window (or no window)
after a reload.
