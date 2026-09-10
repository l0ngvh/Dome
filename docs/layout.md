# Layout

Dome puts every window it manages into one of three modes: tiling, floating, or
fullscreen.

## Tiling

A tiling window has its position and size controlled by Dome through a
strategy, which packs windows to fill the screen without overlap and honors any
minimum or maximum size the window carries. Two strategies are available,
[Partition Tree](#partition-tree) and [Master](#master).

## Floating

A float window has its position and size controlled by the operating system,
not by Dome, the same as an ignored window. Unlike an ignored window, though,
it stays pinned always on top.

## Fullscreen

A fullscreen window takes over the whole monitor.

An application can go fullscreen on its own, the way a game or a media player
does. Such a window can react badly to being moved or resized, so Dome leaves
it alone instead of managing it through a strategy.

## Preferred layout

Dome can preserve a workspace's layout in a file and apply it the first time the
workspace's windows appear. The preserved layout is stored at:
- macOS: `~/.config/dome/layout.lua` (or `$XDG_CONFIG_HOME/dome/layout.lua`).
- Windows: `%APPDATA%\dome\layout.lua`.

`dome export` writes the current layout into the file. You're encouraged to
edit this file, for example to replace a matcher with a regular expression.
Each export overwrites the file and saves the previous version to
`layout.lua.bak`. Copy any edits you want to keep elsewhere first, because the
next export replaces that backup too.

An example file:
```lua
---@type dome.Layout
return {
  workspace = {
    {
      -- workspace to match
      name = "3",
      -- "partition_tree" or "master"
      strategy = "master",
      -- These windows will be float on insert
      float = { { process = "calc.exe" } },
      -- wrap a value in /.../ to match it as a regular expression
      fullscreen = { { title = "/Media Player/" } },
    },
  },
}
```

## Partition Tree

Partition Tree tiles windows like i3. Windows are the leaves of a tree, and the
containers above them arrange their children horizontally, vertically, or as
tabs. Dome keeps the tree normalized, so a non-tabbed container and its non-tabbed parent
never share a direction. When the windows don't all fit at their minimum widths,
the workspace scrolls to bring the offscreen ones into view.

Preserved layout:
```lua
---@type dome.Layout
return {
  workspace = {
    {
      name = "code",
      strategy = "partition_tree",
      tree = {
        -- "horizontal", "vertical", or "tabbed"
        split = "horizontal",
        children = {
          { process = "editor.exe" },
          {
            split = "vertical",
            children = {
              { process = "terminal.exe" },
              { process = "logs.exe" },
            },
          },
          -- shorthand, where Dome can choose the split based on parent container.
          { { process = "editor.exe" }, { process = "terminal.exe" } },
        },
      },
    },
  },
}
```

Dome grows the tree as windows appear, so a workspace matches its `tree` only
once every window is present.

## Master

Master splits the monitor into two side-by-side panes. Each pane is a single
container that stacks its windows vertically.
The first `master_count` windows fill the master pane on the left, and the rest
go to the secondary pane on the right, with `master_ratio` setting where the split
falls. Each pane scrolls on its own when its windows overflow.

Preserved layout:

```lua
---@type dome.Layout
return {
  workspace = {
    {
      name = "code",
      strategy = "master",
      master_ratio = 0.65,
      master_count = 1,
      master = { { process = "code.exe" } },
      secondary = {
        { process = "terminal.exe", title = "build" },
        { process = "terminal.exe", title = "test" },
      },
    },
  },
}
```

Dome fills the arrays in order, then places any unmatched window in the master
pane while it has room and in the secondary pane after that.
