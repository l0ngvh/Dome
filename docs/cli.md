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
dome toggle float
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
    "monitor": "DELL #1",  // owning monitor's unique name
    "state": "Attached",   // "Attached" or "Parked" (origin monitor gone)
    "is_focused": true,    // true for the workspace on the focused monitor
    "is_visible": true,    // true for the workspace shown on each monitor, one per monitor
    "window_count": 3      // tiling + float + fullscreen, no double-count, stays 0 for empty workspaces until Dome exits
  },
  {
    "name": "web",
    "monitor": "DELL #1",
    "state": "Attached",
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
    "id": 7,                            // bare integer (not a wrapped object), pair with `dome unminimize-window <id>` to restore
    "title": "draft.md - Zed",          // window title
    "app_name": "Zed",                  // nullable
    "bundle_id": "dev.zed.Zed",         // nullable, populated on macOS, for resolving the app icon
    "executable_path": null             // nullable, populated on Windows, for resolving the app icon
  },
  {
    "id": 12,
    "title": "Untitled - Notepad",
    "app_name": null,
    "bundle_id": null,
    "executable_path": "C:\\Windows\\System32\\notepad.exe"
  }
]
```

## `dome query monitors`

Prints one JSON entry per connected monitor, ordered left to right:

```json
[
  {
    "device_name": "DELL P2419H",  // raw platform name, can repeat across monitors
    "unique_name": "DELL P2419H",  // `device_name` plus a `#N` that reranks on topology change, matches `monitor` in `query workspaces`
    "cg_display_id": 1,            // nullable, populated on macOS, the display's `CGDirectDisplayID`
    "gdi_device": null,            // nullable, populated on Windows, GDI device name, can move to another display on topology change
    "work_area": { "x": 0, "y": 0, "width": 1920, "height": 1080 }  // excludes docks, taskbars, and reserved bars
  },
  {
    "device_name": "LG HDR 4K",
    "unique_name": "LG HDR 4K",
    "cg_display_id": null,
    "gdi_device": "\\\\.\\DISPLAY2",
    "work_area": { "x": 1920, "y": 0, "width": 3840, "height": 2160 }
  }
]
```

## `dome unminimize-window <id>`

Restores a specific minimized window by id. External callers pair
`dome query minimized` with this command to build their own picker.

Keymaps cannot bind to this action. `WindowId`s are not stable across daemon
restarts, so a bound id would refer to a different window (or no window)
after a reload.
