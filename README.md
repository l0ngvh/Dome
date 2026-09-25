# Dome

**Dome** is my take on a tiling window manager for macOS and Windows, with a
bunch of features that might only matter and make sense to me.

## Install

**Windows (Scoop)**
```bash
scoop bucket add dome https://github.com/l0ngvh/scoop-dome
scoop install dome-nightly
```

**macOS (Homebrew)**
```bash
brew install --cask l0ngvh/homebrew-dome/dome-nightly
```

Or build from source with a [Rust toolchain](https://rustup.rs/):

```bash
git clone https://github.com/l0ngvh/Dome
cd Dome
cargo install --path .
```

Once installed, start Dome from your system launcher, or run `dome` from the
command line.

## Default key bindings

| Key | Action |
|-----|--------|
| <kbd>Alt</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Focus left/down/up/right |
| <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Move window left/down/up/right |
| <kbd>Alt</kbd> + <kbd>0</kbd>-<kbd>9</kbd> | Focus workspace 0-9 |
| <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>0</kbd>-<kbd>9</kbd> | Move window to workspace 0-9 |
| <kbd>Alt</kbd> + <kbd>Ctrl</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Focus monitor left/down/up/right |
| <kbd>Alt</kbd> + <kbd>Ctrl</kbd> + <kbd>Shift</kbd> + <kbd>H</kbd> / <kbd>J</kbd> / <kbd>K</kbd> / <kbd>L</kbd> | Move window to monitor left/down/up/right |
| <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Space</kbd> | Toggle floating |
| <kbd>Alt</kbd> + <kbd>F</kbd> | Toggle fullscreen |
| <kbd>Alt</kbd> + <kbd>W</kbd> | Toggle split/tabbed layout |
| <kbd>Alt</kbd> + <kbd>Return</kbd> | Open a terminal |
| <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>Q</kbd> | Close focused window |
| <kbd>Alt</kbd> + <kbd>Shift</kbd> + <kbd>E</kbd> | Exit Dome |

## Configuring Dome

See [Configuration](docs/configuration.md).

## Preferred layout

Dome can remember how a workspace is arranged and restore it the next time those
windows open:

```bash
dome export
```

That writes the arrangement to the `layout.lua` file in your configuration
folder:

```lua
---@type dome.Layout
return {
  ["Built-in Retina Display"] = {
    ["dev"] = {
      layout = "partition_tree",
      tree = {
        split = "horizontal",
        children = {
          { app = "Ghostty" },
          { app = "Firefox" },
        },
      },
      float = { { app = "System Settings" } },
    },
  },
}
```

You can edit it by hand to refine a rule, for example `title = "/Picture in
Picture/"` to match a window with a regular expression.

## Credits

Dome draws a lot of inspiration from these awesome WMs and likely wouldn't
exist without them:
- [AeroSpace](https://github.com/nikitabobko/AeroSpace)
- [GlazeWM](https://github.com/glzr-io/glazewm)
- [komorebi](https://github.com/LGUG2Z/komorebi)
- [Sway](https://github.com/swaywm/sway)
- [niri](https://github.com/niri-wm/niri)

## License

Dome is released under the [MIT License](LICENSE).
