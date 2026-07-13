# Preferred layout

The preferred layout defines how windows are arranged on each workspace
when they first appear. Once placed, you can still move and resize them
with normal tiling actions.

Dome reads the layout from:
- macOS: `~/.config/dome/layout.toml` (or `$XDG_CONFIG_HOME/dome/layout.toml`).
- Windows: `%APPDATA%\dome\layout.toml`.

The file is hot reloaded on save. Currently, only moving windows within
a workspace is supported during hot reload.

## Defining a workspace

Each `[[workspace]]` block defines the window layout for a workspace. It
overrides the global defaults in `config.toml`, with any unset fields falling
back to their global values.

```toml
[[workspace]]
name = "3"
strategy = "master"
float = [{ process = "calc.exe" }]
fullscreen = [{ process = "player.exe" }]
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

When `strategy = "master"`, you can pin specific windows to the master or
secondary area. Windows are placed in the order they appear in each array.
When no entry matches a window, it goes to the master stack if there is still
room, and to the secondary stack otherwise.

You can also override the strategy defaults per workspace using
`master_ratio` and `master_count`.

| Field | Type | Description |
|-------|------|-------------|
| `master_ratio` | float | Override `master.master_ratio` for this workspace. |
| `master_count` | integer | Override `master.master_count` for this workspace. |
| `master` | array of matchers | Place matching windows in the master area. |
| `secondary` | array of matchers | Place matching windows in the secondary area. |

```toml
[[workspace]]
name = "code"
strategy = "master"
master_ratio = 0.65
master = [{ process = "code.exe" }]
secondary = [
  { process = "terminal.exe", title = "build" },
  { process = "terminal.exe", title = "test" },
]
```

## Defining a tree layout

When `strategy = "partition_tree"`, you can define a predictable window
arrangement using a `tree` field.

| Field | Type | Description |
|-------|------|-------------|
| `tree` | object or array | Preferred window arrangement. |

```toml
[[workspace]]
name = "code"
strategy = "partition_tree"
tree = { split = "horizontal", children = [
  { process = "editor.exe" },
  { split = "vertical", children = [
    { process = "terminal.exe" },
    { process = "logs.exe" },
  ]},
  [
    { process = "editor.exe" },
    { process = "terminal.exe" },
  ],
]}
```

Here, an array `[...]` groups children into a container with the split
direction decided by Dome. To control the split direction yourself, use the `{
split = "horizontal" | "vertical" | "tabbed", children = [...] }` syntax. Note
that, when a parent and child share the same split direction, the child will be
flipped.

Containers with a single child are collapsed.

The preferred tree is built incrementally as windows are inserted. This means
no gaps on screen, but the tree does not match the preferred layout until all
windows have been inserted.

