# Layout

Layout settings live in `layout.toml`, a sibling of `config.toml` in the same
directory. Dome hot-reloads the file on save.

Default paths:

- macOS: `~/.config/dome/layout.toml` (or `$XDG_CONFIG_HOME/dome/layout.toml`).
- Windows: `%APPDATA%\dome\layout.toml`.

The schema is flat. `strategy` and four window-size constraint fields sit at
the file root, alongside `[gaps]`, `[partition_tree]`, and `[master]`
sub-tables for spacing and strategy-specific options. Within `[master]`,
an optional `[[master.workspace]]` array sets per-workspace master defaults. A
separate `[[workspace]]` array overrides the strategy on a per-workspace basis.

```toml
strategy = "partition_tree"

min_width = "5%"
min_height = "5%"
max_width = 0
max_height = 0

[gaps]
inner.horizontal = 8.0
inner.vertical = 8.0
outer.top = 0.0
outer.right = 0.0
outer.bottom = 0.0
outer.left = 0.0

[partition_tree]
tab_bar_height = 24.0
automatic_tiling = true

[master]
master_ratio = 0.5
master_count = 1

[[master.workspace]]
name = "1"
master_count = 3

[[workspace]]
name = "3"
strategy = "master"
```

`strategy` selects the active tiling strategy, either `"partition_tree"` or
`"master"`. The default is `"partition_tree"`.

## Gaps

The optional `[gaps]` table controls spacing in logical pixels. Values default
to `0.0`.

- `inner.horizontal` separates side-by-side tiled windows.
- `inner.vertical` separates vertically stacked tiled windows.
- `outer.*` reserves monitor edges for external bars, launchers, or widgets.

Use `outer.top` for a top-aligned SketchyBar:

```toml
[gaps]
outer.top = 32.0
```

Outer gaps apply to the normal tiled work area. Inner gaps apply only between
tiled siblings, not at monitor edges. Fullscreen windows still use the
platform's real fullscreen bounds.

## Per-workspace overrides

The optional `[[workspace]]` array overrides the global strategy for individual
workspaces. Each entry needs a `name` (the workspace identifier) and a
`strategy` (same values as the global field). Workspaces without a matching
entry use the global `strategy`.

If the same `name` appears more than once, the last entry wins. Entries with
an empty `name` or an unrecognized `strategy` are dropped.

## Partition tree

The default strategy. i3-style manual tiling with split containers (horizontal,
vertical, tabbed), spawn-mode routing, and direction invariance. See
[architecture.md](architecture.md#tiling-strategies) for the full model.

`automatic_tiling` (default `true`) controls how Dome picks split direction for
new windows. When true, Dome chooses horizontal or vertical based on the
focused window's dimensions. When false, new windows split in the current
container's direction.

`tab_bar_height` (default `24.0`) sets the height of the tab bar in tabbed
containers, in logical pixels.

## Master

A two-area layout modeled on xmonad's `Tall`. The first `master_count` windows
fill a master pane on the left, and the rest stack vertically in a pane on the
right.

`master_ratio` (default `0.5`) sets the width of the master pane as a fraction
of the workspace width, constrained to `[0.1, 0.9]`. `master_count` (default
`1`) sets how many windows go in the master pane and must be at least 1.

Both values seed new workspaces on first attach. A reload does not push them
into existing workspaces. Runtime tuning via `master grow/shrink/more/fewer`
persists across reloads.

Both panes honor the global window-size constraints, and per-window constraints
reported by the OS take precedence. Each pane scrolls vertically when
per-window min heights push its content past the screen height, with focus
movement as the sole trigger.

### Per-workspace master overrides

The optional `[[master.workspace]]` array sets different seed values for
individual workspaces by name.

```toml
[master]
master_ratio = 0.5
master_count = 1

[[master.workspace]]
name = "1"
master_count = 3

[[master.workspace]]
name = "code"
master_ratio = 0.7
```

Each entry requires a `name` matching a workspace identifier. `master_count`
and `master_ratio` are both optional. Omitted fields fall back to the global
`[master]` defaults.

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | yes | Workspace name to match. |
| `master_ratio` | float | no | Initial master-pane width fraction, in `[0.1, 0.9]`. |
| `master_count` | integer | no | Initial master-pane window count, >= 1. |

Overrides apply only on first attach. Config reloads do not push new values
into workspaces that already have master state.

An entry referencing a workspace name that never materializes is harmless and
sits unused. Out-of-range values are dropped with a warning, falling back to
the global default for that field. If the same name appears more than once,
the last entry wins.

## Window size constraints

Both strategies enforce four global size constraints. A size value is either a
number (logical pixels) or a string ending in `%` (percentage of the screen
dimension). Per-window constraints reported by the OS always take precedence
over these globals.

`min_width` and `min_height` (both default `"5%"`) set the floor. `max_width`
and `max_height` (both default `0`) set the ceiling, where `0` means no limit.

## Error handling

One cross-field rule applies. `min_width` must not exceed `max_width`, and
`min_height` must not exceed `max_height`, when both sides are pixel values
and `max` is greater than 0. A violation makes `layout.toml` fail to load
entirely and Dome falls back to layout defaults.

Field-level recovery follows the same rules as
[`config.toml`](configuration.md#error-handling).
