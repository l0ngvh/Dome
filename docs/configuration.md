# Configuration

Dome reads `config.toml` from one of these locations:

- macOS: `~/.config/dome/config.toml` (or `$XDG_CONFIG_HOME/dome/config.toml`).
- Windows: `%APPDATA%\dome\config.toml`.

Use `dome launch -c <path>` to point to a different file (see [cli.md](cli.md)).

All settings are hot-reloaded on save.

## General

```toml
border_size = 4.0
theme = "mocha"
log_level = "info"
start_at_login = false
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `border_size` | float | `4.0` | Border width around windows, in logical pixels. |
| `theme` | string | `"mocha"` | Color theme. One of `"latte"`, `"frappe"`, `"macchiato"`, `"mocha"` ([Catppuccin](https://catppuccin.com/) flavors). |
| `log_level` | string | `"info"` | Log verbosity. One of `trace`, `debug`, `info`, `warn`, `error`. |
| `start_at_login` | boolean | `false` | Launch Dome at user login. |

## Tiling layout

Controls how windows are tiled on screen.

```toml
strategy = "partition_tree"
minimum_width = "5%"
minimum_height = "5%"
maximum_width = 0
maximum_height = 0

[partition_tree]
tab_bar_height = 24.0
automatic_tiling = true

[master]
master_ratio = 0.5
master_count = 1
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `strategy` | string | `"partition_tree"` | Default tiling strategy. One of `"partition_tree"` or `"master"`. Per-workspace preferred layouts in `layout.toml` can set this per workspace. |
| `minimum_width` / `minimum_height` | size | `"5%"` | Minimum window size. Float (e.g. `200`) parses as logical pixels. String with `%` suffix (e.g. `"10%"`) parses as percentage of workspace dimension. Use `0` to disable. |
| `maximum_width` / `maximum_height` | size | `0` | Maximum window size. Same parsing rules as min. `0` means no limit. Windows clamped by max are centered within their allocated space. |
| `partition_tree.tab_bar_height` | float | `24.0` | Height of the tab bar in tabbed containers, logical pixels. This value does not auto-scale with `font.text_size`, so long tab titles may truncate earlier as the body size grows. |
| `partition_tree.automatic_tiling` | boolean | `true` | Pick split direction based on the focused window's dimensions. |
| `master.master_ratio` | float | `0.5` | Width of the master area, in `[0.1, 0.9]`. |
| `master.master_count` | integer | `1` | Number of master windows, `>= 1`. |

The master strategy splits the screen into a master area (left or top) and
a secondary stack area (right or bottom). `master.master_ratio` controls the
master area's width and `master.master_count` sets how many windows go there.
The rest of the windows stack in the secondary area.

The partition-tree strategy fills the screen by arranging windows in a tree of
nested containers. Each container is either a split (horizontal or vertical) or
tabbed. Unlike i3, Dome automatically removes single-child containers and
alternates nested split direction, similar to Aerospace's normalized mode.

`partition_tree.automatic_tiling` lets the runtime choose the split direction
based on the focused window's dimensions.

## Window rules

Match windows by their attributes to ignore, float, or fullscreen them.
All fields in a rule must match (AND) and the first matching rule wins.
Wrap a value in `/pattern/` for regex matching or leave it bare for exact
matching. Built-in `ignore` rules are always active and user rules add to
them.

| Key | Semantics |
|-----|-----------|
| `ignore` | Do not manage matching windows. |
| `float` | Start matching windows as floating. |
| `fullscreen` | Start matching windows as fullscreen. |

| Platform | Matching fields |
|----------|-----------------|
| macOS | `app`, `bundle_id` (exact only), `title` |
| Windows | `process`, `title`, `class` (Win32), `aumid` |

```toml
ignore = [
  # macOS
  { app = "System Preferences" },
  { bundle_id = "com.apple.finder", title = "Trash" },
  # Windows
  { process = "SystemSettings.exe" },
  { class = "Shell_TrayWnd" },
]
float = [
  { process = "calculator.exe" },
]
fullscreen = [
  { process = "slides.exe" },
]
```

## Keybindings

Keybindings go in the `[keymaps]` table. Defining `[keymaps]` **replaces all
default bindings** with no merge, so copy any defaults you want to keep if
you are adding your own.

```toml
"mods+...+key" = ["<action>", ...]
```

Modifiers are `meta`, `shift`, `alt`, and `ctrl` (`cmd` and `win` work as
aliases for `meta`), combined with `+`. A keymap can trigger one or more
actions that fire in order on a single press.

```toml
"meta+h" = ["focus left"]
"meta+shift+1" = ["move workspace 1", "focus workspace 1"]
```

### Default bindings

| Key | Action |
|-----|--------|
| <kbd>meta</kbd>+<kbd>0</kbd> through <kbd>meta</kbd>+<kbd>9</kbd> | `focus workspace 0` through `focus workspace 9` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>0</kbd> through <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>9</kbd> | `move workspace 0` through `move workspace 9` |
| <kbd>meta</kbd>+<kbd>h</kbd> | `focus left` |
| <kbd>meta</kbd>+<kbd>j</kbd> | `focus down` |
| <kbd>meta</kbd>+<kbd>k</kbd> | `focus up` |
| <kbd>meta</kbd>+<kbd>l</kbd> | `focus right` |
| <kbd>meta</kbd>+<kbd>p</kbd> | `focus parent` |
| <kbd>meta</kbd>+<kbd>[</kbd> | `focus tab prev` |
| <kbd>meta</kbd>+<kbd>]</kbd> | `focus tab next` |
| <kbd>meta</kbd>+<kbd>e</kbd> | `toggle spawn` |
| <kbd>meta</kbd>+<kbd>d</kbd> | `toggle direction` |
| <kbd>meta</kbd>+<kbd>b</kbd> | `toggle layout` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>f</kbd> | `toggle float` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move left` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move down` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move up` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move right` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>h</kbd> | `focus monitor left` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>j</kbd> | `focus monitor down` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>k</kbd> | `focus monitor up` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>l</kbd> | `focus monitor right` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move monitor left` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move monitor down` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move monitor up` |
| <kbd>meta</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move monitor right` |
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>q</kbd> | `close` |

### Modes

Additional sets of bindings go in `[keymaps.mode.<name>]`. Switch between them
with the `mode <name>` action or `dome mode <name>`. Unknown mode names are
rejected.

```toml
[keymaps]
"meta+h" = ["focus left"]
"meta+r" = ["mode resize"]

[keymaps.mode.resize]
"h" = ["master shrink"]
"l" = ["master grow"]
"escape" = ["mode default"]
```

Always include an escape binding (like `"escape" = ["mode default"]`) or
your keyboard stays in that mode until Dome exits. Config reload preserves
the active mode, but Dome falls back to defaults on the next keypress if the
new config removes it.

## Font

```toml
[font]
text_size = 14.0       # Body text: tab titles, picker labels.
subtext_size = 12.0    # Secondary text: picker app-name subtext.
# family = "PingFang SC"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font.text_size` | float | `14.0` | Body text size in points (`4.0` to `128.0`). |
| `font.subtext_size` | float | `12.0` | Secondary text size in points (`4.0` to `128.0`). |
| `font.family` | string | unset | System font to use for rendering. When unset, egui's built-in Ubuntu-Light is used. Dome logs a warning and falls back to built-in fonts when a commercial font cannot be used. |
