# Preferred layout

The preferred layout defines how windows are arranged on each workspace when they
first appear. Once placed, you can still move and resize them with normal tiling
actions.

Dome reads the layout from:
- macOS: `~/.config/dome/layout.jsonc` (or `$XDG_CONFIG_HOME/dome/layout.jsonc`).
- Windows: `%APPDATA%\dome\layout.jsonc`.

The file is [JSONC](https://github.com/microsoft/node-jsonc-parser), JSON with
comments and trailing commas. It is hot reloaded on save. Only moving windows
within a workspace is supported during hot reload.

`dome export` rewrites this file from the current window state (see
[commands.md](commands.md)). It edits the file in place and keeps your comments,
indentation, and trailing commas. A comment inside a workspace entry is lost when
that entry changes.

## Defining a workspace

The file is one object with a `workspace` array. Each entry defines the window
layout for one workspace. It overrides the global defaults in `config.lua`, and an
unset field falls back to its global value.

```jsonc
{
  "workspace": [
    {
      "name": "3",
      "strategy": "master",
      "float": [{ "process": "calc.exe" }],
      "fullscreen": [{ "process": "player.exe" }]
    }
  ]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `name` | string (required) | Workspace name to match. |
| `strategy` | string (required) | Tiling strategy for this workspace. One of `"partition_tree"` or `"master"`. |
| `float` | array of matchers | Start matching windows as floating. |
| `fullscreen` | array of matchers | Start matching windows as fullscreen. |

All window matcher arrays use the same per-platform fields as
[window rules](configuration.md#window-rules). Wrap a value in forward slashes
(`/pattern/`) for regex matching or leave it bare for exact matching.

## Master and secondary placement

When `"strategy": "master"`, you can pin specific windows to the master or
secondary area. Windows are placed in the order they appear in each array. When no
entry matches a window, it goes to the master stack if there is still room, and to
the secondary stack otherwise.

You can also override the strategy defaults per workspace with `master_ratio` and
`master_count`.

| Field | Type | Description |
|-------|------|-------------|
| `master_ratio` | float | Override `master.master_ratio` for this workspace. |
| `master_count` | integer | Override `master.master_count` for this workspace. |
| `master` | array of matchers | Place matching windows in the master area. |
| `secondary` | array of matchers | Place matching windows in the secondary area. |

```jsonc
{
  "workspace": [
    {
      "name": "code",
      "strategy": "master",
      "master_ratio": 0.65,
      "master": [{ "process": "code.exe" }],
      "secondary": [
        { "process": "terminal.exe", "title": "build" },
        { "process": "terminal.exe", "title": "test" }
      ]
    }
  ]
}
```

## Defining a tree layout

When `"strategy": "partition_tree"`, you can define a predictable window
arrangement with a `tree` field.

| Field | Type | Description |
|-------|------|-------------|
| `tree` | object or array | Preferred window arrangement. |

```jsonc
{
  "workspace": [
    {
      "name": "code",
      "strategy": "partition_tree",
      "tree": {
        "split": "horizontal",
        "children": [
          { "process": "editor.exe" },
          {
            "split": "vertical",
            "children": [
              { "process": "terminal.exe" },
              { "process": "logs.exe" }
            ]
          },
          [
            { "process": "editor.exe" },
            { "process": "terminal.exe" }
          ]
        ]
      }
    }
  ]
}
```

An array `[...]` groups children into a container with the split direction decided
by Dome. To control the split direction yourself, use an object `{ "split":
"horizontal" | "vertical" | "tabbed", "children": [...] }`. When a parent and child
share the same split direction, the child is flipped.

A container with a single child is collapsed.

The preferred tree is built as windows are inserted. There are no gaps on screen,
but the tree does not match the preferred layout until every window is inserted.
