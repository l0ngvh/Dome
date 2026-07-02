# Layout

Layout settings live in `layout.toml`, a sibling of `config.toml` in the same
directory. Dome hot-reloads the layout config on save.

Default paths:

- macOS: `~/.config/dome/layout.toml` (or `$XDG_CONFIG_HOME/dome/layout.toml`).
- Windows: `%APPDATA%\dome\layout.toml`.

```toml
strategy = "partition_tree"

min_width = "5%"
min_height = "5%"
max_width = 0
max_height = 0

[partition_tree]
tab_bar_height = 24.0
automatic_tiling = true

[master]
master_ratio = 0.5
master_count = 1

[[workspace]]
name = "3"
strategy = "master"
```


## Window size constraints

At the top level, we can set the desired minimum/maximum width/height for all
windows. Windows can override these with their own native minimum/maximum size
however.

Each constraint is a number for logical pixels, a `%` string for percentage of
screen, or `0` for no limit.

```toml
min_width = "5%"
min_height = 200
max_width = "100%"
max_height = 0
```

## Forcing display mode

Matcher fields are the same cross-platform set:

- `app`: macOS application name.
- `bundle_id`: macOS bundle identifier.
- `title`: window title (both platforms).
- `process`: Windows executable name.
- `class`: Win32 window class name.
- `aumid`: AppUserModelID.

Wrap a value in forward slashes (`/pattern/`) for regex matching. Without
slashes, strings match exactly. An empty matcher never matches.

```toml
[[workspace]]
name = "3"
float = [
  { process = "calculator.exe" },
  { app = "Calculator" },
]
fullscreen = [
  { process = "slides.exe" },
]
```

## Tiling strategy

`strategy` selects the active tiling strategy, either `"partition_tree"` or
`"master"`. The default is `"partition_tree"`.

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

These values can be overridden at runtime via `master grow/shrink/more/fewer`,
which persists even after config reloads.

Each pane scrolls vertically when per-window min heights push its content past
the screen height, with focus movement as the sole trigger.

`master_ratio` (in `[0.1, 0.9]`) and `master_count` (>= 1) seed the workspace
on first attach only. Omitted fields fall back to the global `[master]`
defaults. Workspaces without a matching entry use the global `strategy`.

## Per-workspace override

The global configuration can be overridden per workspace using the
`[[workspace]]`. Here, we can instruct Dome to float windows matching
pre-defined predicates, or force the tiling strategy to place windows in the
specified layout.

```toml
[[workspace]]
name = "3"
strategy = "master"
master_ratio = 0.5
master_count = 3
float = [
  { process = "calculator.exe" },
  { app = "Calculator" },
]
fullscreen = [
  { process = "slides.exe" },
]
master = [
  { process = "code.exe" },
]
secondary = [
  { process = "browser.exe" },
]
```
