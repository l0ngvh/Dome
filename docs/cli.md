# CLI

Action commands such as `dome focus left` mirror their `keymaps` bindings in
[configuration.md](configuration.md#actions).

## `dome launch`

Starts Dome. `dome` on its own does the same.

- `-c <path>`, `--config <path>`: override the default config file (see [configuration.md](configuration.md)).
- `-l <path>`, `--layout <path>`: override the default layout file (see [layout.md](layout.md)).

## `dome focus left`

Focus the window to the left.

## `dome focus right`

Focus the window to the right.

## `dome focus up`

Focus the window above, scrolling the pane if needed.

## `dome focus down`

Focus the window below, scrolling the pane if needed.

## `dome focus parent`

Focus the parent container.

## `dome focus workspace <name>`

Switch to the named workspace.

- `--monitor <monitor>`: target that workspace on a specific monitor.

## `dome focus monitor left`

Focus the monitor to the left.

## `dome focus monitor right`

Focus the monitor to the right.

## `dome focus monitor up`

Focus the monitor above.

## `dome focus monitor down`

Focus the monitor below.

## `dome focus monitor <name>`

Focus the named monitor.

## `dome focus tab next`

Focus the next tab in a tabbed container.

## `dome focus tab prev`

Focus the previous tab.

## `dome move left`

Move the focused window one step left.

## `dome move right`

Move the focused window one step right.

## `dome move up`

Move the focused window one step up.

## `dome move down`

Move the focused window one step down.

## `dome move workspace <name>`

Move the focused window to the named workspace.

- `--monitor <monitor>`: target that workspace on a specific monitor instead of the focused one.

## `dome move monitor left`

Move the focused window to the monitor on the left.

## `dome move monitor right`

Move the focused window to the monitor on the right.

## `dome move monitor up`

Move the focused window to the monitor above.

## `dome move monitor down`

Move the focused window to the monitor below.

## `dome move monitor <name>`

Move the focused window to the named monitor.

## `dome toggle spawn`

Set where the next new window lands relative to the focused window. Cycles
between horizontal, vertical, and tabbed.

## `dome toggle direction`

Flip the parent container's split direction between horizontal and vertical.

## `dome toggle layout`

Toggle the parent container between split and tabbed layout.

## `dome toggle float`

Float/unfloat focused window.

## `dome toggle fullscreen`

Fullscreen/unfullscreen focused window.

## `dome master grow`

Grow the master area.

## `dome master shrink`

Decrease the master area by the same step.

## `dome master more`

Add one window slot to the master area.

## `dome master fewer`

Remove one window slot from the master area, with a minimum of 1.

## `dome execute <command>`

Run a shell command, passed verbatim to the system shell. Quote a command that
contains spaces, for example `dome execute "open -a Terminal"`.

> **Note**
> 
> Do not run Dome with elevated privileges. `execute` runs arbitrary shell commands, so
> anyone with access to the user's shell or Dome's IPC socket would inherit
> those privileges.

## `dome close`

Close the focused window.

## `dome mode <name>`

Switch to the named keymap. See
[keymaps](configuration.md#keymaps).

## `dome exit`

Stop Dome and restore all windows.

## `dome export`

Export the current window layout to the layout file. See
[layout.md](layout.md#preferred-layout).

## `dome query workspaces`

Prints one JSON entry per active workspace, ordered by creation:

```json
[
  {
    "name": "0",
    // owning monitor, matches unique_name in query monitors
    "monitor": "DELL #1",
    // or "Parked" when the origin monitor is gone
    "state": "Attached",
    // the focused monitor's workspace
    "is_focused": true,
    // the visible workspace, one per monitor
    "is_visible": true,
    "window_count": 3
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
    // pair with dome unminimize-window <id> to restore
    "id": 7,
    "title": "draft.md - Zed",
    "app_name": "Zed",
    // populated on macOS, for resolving the app icon
    "bundle_id": "dev.zed.Zed",
    // populated on Windows, for resolving the app icon
    "executable_path": null
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
    // monitor name, can repeat when multiple monitors of the same model exist
    "device_name": "DELL P2419H",
    // stable unique monitor name. when multiple monitors of the same model exist
    "unique_name": "DELL P2419H",
    // populated on macOS, the display's CGDirectDisplayID
    "cg_display_id": 1,
    // populated on Windows, the GDI device name, can move to another display on topology change
    "gdi_device": null,
    // usable area, excluding docks, taskbars
    "work_area": { "x": 0, "y": 0, "width": 1920, "height": 1080 }
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

Restores a specific minimized window by id.

## `dome generate yasb`

Generate the YASB status-bar integration. Windows. See
[integration.md](integration.md).

## `dome generate sketchybar`

Generate the SketchyBar status-bar integration. macOS. See
[integration.md](integration.md).

## `dome generate zebar`

Generate the Zebar status-bar integration. Windows. See
[integration.md](integration.md).
