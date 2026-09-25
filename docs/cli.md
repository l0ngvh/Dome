# CLI

## `dome launch`

Starts Dome. `dome` on its own does the same.

- `-c <path>`, `--config <path>`: override the default config file (see [configuration.md](configuration.md)).
- `-l <path>`, `--layout <path>`: override the default layout file.

Dome writes its log to `~/Library/Logs/dome/dome.log` on macOS and
`%APPDATA%\dome\logs\dome.log` on Windows.

## `dome focus-left`, `focus-right`, `focus-up`, `focus-down`

Focus the window in that direction.

## `dome focus-parent`

Focus the parent container.

## `dome focus-tab-next`, `focus-tab-prev`

Focus the next or previous tab in a tabbed container.

## `dome focus-workspace <name>`

Switch to the named workspace.

- `--monitor <monitor>`: target that workspace on a specific monitor.

## `dome focus-monitor-left`, `focus-monitor-right`, `focus-monitor-up`, `focus-monitor-down`

Focus the monitor in that direction.

## `dome focus-monitor <name>`

Focus the named monitor.

## `dome move-left`, `move-right`, `move-up`, `move-down`

Move the focused window one step in that direction.

## `dome move-to-workspace <name>`

Move the focused window to the named workspace.

- `--monitor <monitor>`: target that workspace on a specific monitor instead of the focused one.

## `dome move-to-monitor-left`, `move-to-monitor-right`, `move-to-monitor-up`, `move-to-monitor-down`

Move the focused window to the monitor in that direction.

## `dome move-to-monitor <name>`

Move the focused window to the named monitor.

## `dome toggle-split`

Toggle the direction, horizontal or vertical, in which the next new window
opens relative to the focused window.

## `dome rotate`

Flip the parent container's split direction between horizontal and vertical.

## `dome toggle-tabbed`

Toggle the parent container between split and tabbed layout. A new window does
not join a tabbed container.

## `dome toggle-float`

Float or re-tile the focused window.

## `dome toggle-fullscreen`

Fullscreen the focused window, or restore it.

## `dome increase-master-ratio`

Grow the master area by 5 percentage points, clamped to 0.1 through 0.9.

## `dome decrease-master-ratio`

Shrink the master area by the same step.

## `dome increase-master-count`

Add one window slot to the master area.

## `dome decrease-master-count`

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

Switch to the named keymap.

## `dome exit`

Stop Dome and restore all windows.

## `dome export`

Export the current window layout to the layout file. See
[Preferred layout](../README.md#preferred-layout).

## `dome query workspaces`

Prints one JSON entry per active workspace, ordered by creation:

```jsonc
[
  {
    "name": "0",
    // owning monitor, matches unique_name in query monitors
    "monitor": "DELL P2419H",
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
    "monitor": "DELL P2419H",
    "state": "Attached",
    "is_focused": false,
    "is_visible": false,
    "window_count": 1
  }
]
```

## `dome query minimized`

Prints one JSON entry per minimized window, in the order they were minimized:

```jsonc
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

```jsonc
[
  {
    // monitor name, can repeat when multiple monitors of the same model exist
    "device_name": "DELL P2419H",
    // device_name when it is unique, otherwise device_name plus a #N suffix
    "unique_name": "DELL P2419H",
    // populated on macOS, the display's CGDirectDisplayID
    "cg_display_id": 1,
    // the GDI device name on Windows
    "gdi_device": null,
    // usable area, excluding docks and taskbars
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

Restore a minimized window by id.

## `dome generate yasb`

Generate the YASB status-bar integration. Windows. See
[integration.md](integration.md).

## `dome generate sketchybar`

Generate the SketchyBar status-bar integration. macOS. See
[integration.md](integration.md).

## `dome generate zebar`

Generate the Zebar status-bar integration. Windows. See
[integration.md](integration.md).
