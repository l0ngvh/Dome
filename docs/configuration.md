# Configuration

Dome reads `config.toml` for general settings (theme, borders, font, keybindings, window rules) and hot-reloads it on save. Layout settings (strategy, window-size constraints, per-strategy parameters) live in a separate file. See [layout.md](layout.md) for the layout reference.

Default path:

- macOS: `~/.config/dome/config.toml` (or `$XDG_CONFIG_HOME/dome/config.toml`).
- Windows: `%APPDATA%\dome\config.toml`.

Pass `dome launch -c <path>` to override (see [cli.md](cli.md)).

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

## Font

Dome ships with egui's built-in font stack (Ubuntu-Light proportional, Hack monospace, plus emoji fallbacks). Set `family` to render glyphs that the built-in fonts do not cover (CJK, Cyrillic, Arabic, Hebrew, Thai, Devanagari, etc.) using a font already installed on the system.

```toml
[font]
text_size = 14.0     # Body text: tab titles, picker labels.
subtext_size = 12.0  # Secondary text: picker app-name subtext.
family = "PingFang SC"            # macOS: render Chinese tab text via PingFang SC.
# family = "Microsoft YaHei UI"   # Windows: English family name.
# family = "微软雅黑"              # Windows: localized name also works.
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `font.text_size` | float | `14.0` | Body text size in points. Must be in `[4.0, 128.0]`. |
| `font.subtext_size` | float | `12.0` | Secondary text size in points. Must be in `[4.0, 128.0]`. |
| `font.family` | string (optional) | unset | Optional system-installed proportional font, used as a fallback after egui's built-in Ubuntu-Light. When unset, non-Latin glyphs render as tofu. |

`tab_bar_height` (under `[partition_tree]` in [layout.md](layout.md)) does not auto-scale with `text_size`, so long tab titles may truncate earlier as the body size grows.

### Caveats

Dome does not validate the family name. Windows substitutes a lookalike silently on a miss, so a typo or uninstalled name can still render something other than what you intended. Double-check the name in Font Book on macOS or `Settings > Personalization > Fonts` on Windows.

English and localized names both work. `"Microsoft YaHei UI"` and `"微软雅黑"` resolve to the same font on Windows, and macOS behaves the same way.

For fonts that bundle multiple variants in one file (TrueType collections like `msyh.ttc`), Dome picks the regular weight. Selecting a specific weight or italic from a collection is not yet supported.

A small number of third-party commercial fonts are marked non-embeddable in their license metadata, and the OS will not hand their bytes to applications. Dome logs a warning and falls back to its built-in fonts. System fonts (Microsoft, Apple) and Google Noto fonts are always embeddable.

Changing `family` to a different value takes effect on the next render. Removing the line entirely keeps the previously-loaded font active until Dome restarts.

## Window rules

Window rules come in two kinds:

- `ignore`: do not manage the window at all.
- `on_open`: apply initial settings (mode and workspace) when a matching window first appears. Valid `mode` values: `tiling`, `float`, `fullscreen`.

### Matching

All fields in a rule must match for the rule to apply. Rules are evaluated in order, and the first matching rule wins. A rule with no fields never matches.

Wrap a value in forward slashes (`/pattern/`) for regex matching. Without slashes, strings match exactly.

If a window attribute is unavailable, any rule that specifies that field does not match.

Both platforms ship built-in `ignore` entries that are always active. User rules add to the defaults, never replace them.

### macOS

macOS rules match on three fields:

- `app`: the application name.
- `bundle_id`: the CFBundleIdentifier. Always matched by exact equality (no regex support).
- `title`: the window title.

The built-in macOS ignore list covers `com.apple.dock`, `com.apple.controlcenter`, `com.apple.notificationcenterui`, and `com.apple.loginwindow`.

```toml
[macos]
ignore = [
  { app = "System Preferences" },                       # exact app name
  { app = "/.*Preferences/" },                          # regex on app name
  { bundle_id = "com.apple.finder", title = "Trash" },  # bundle and title (AND)
]
on_open = [
  { app = "Slack", workspace = "3" },
  { app = "Safari", mode = "float" },
]
```

### Windows

Windows rules match on four fields:

- `process`: the executable name.
- `title`: the window title.
- `class`: the Win32 window class name (from `GetClassNameW`).
- `aumid`: the AppUserModelID, useful for distinguishing UWP apps that share `ApplicationFrameHost.exe` as their host process.

```toml
[windows]
ignore = [
  { process = "SystemSettings.exe" },
  { process = "/.*Settings.*/" },
  { title = "Task Manager" },
  { class = "Shell_TrayWnd" },                                     # taskbar
  { aumid = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App" },     # by AUMID
]
on_open = [
  { process = "slack.exe", workspace = "3" },
  { class = "Chrome_WidgetWin_1", mode = "float" },                # by class
]
```

## Keybindings

Keybindings live in the `[keymaps]` table. The same file works on macOS and Windows without changes. Defining a `[keymaps]` table **replaces the defaults wholesale**, with no merge. To keep any default binding while adding your own, copy the defaults and edit the copy.

### Syntax

Each entry maps a key combination to one or more action strings:

```toml
"mods+...+key" = ["<action>", ...]
```

Tokens are lowercase. Accepted modifier tokens: `meta`, `shift`, `alt`, `ctrl`. `cmd` and `win` are platform-flavored aliases for `meta`. Multiple modifiers are joined with `+`. The key is the final segment after all modifiers.

Values are arrays of action strings. Single-element arrays are fine. Multi-action arrays fire in order.

```toml
"meta+h" = ["focus left"]
"meta+shift+1" = ["move workspace 1", "focus workspace 1"]
```

### Modifier mapping

The literal config token is `meta`. `cmd` and `win` are accepted as aliases and behave identically.

| Token | macOS | Windows |
|-------|-------|---------|
| `meta` (alias: `cmd`, `win`) | <kbd>Command</kbd> | <kbd>Windows</kbd> |
| `shift` | <kbd>Shift</kbd> | <kbd>Shift</kbd> |
| `alt` | <kbd>Option</kbd> | <kbd>Alt</kbd> |
| `ctrl` | <kbd>Control</kbd> | <kbd>Control</kbd> |

### Default bindings

Dome ships 44 default bindings.

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
| <kbd>meta</kbd>+<kbd>shift</kbd>+<kbd>q</kbd> | `exit` |

### Customising

Define a `[keymaps]` section to replace the defaults with your own bindings. The entire default set is discarded, so copy any defaults you want to keep.

```toml
[keymaps]
"meta+h" = ["focus left"]
"meta+l" = ["focus right"]
"meta+return" = ["exec open -a Terminal"]
```

### Multi-action bindings

A binding's value is an array of action strings. All actions fire in order on a single keypress:

```toml
"meta+shift+1" = ["move workspace 1", "focus workspace 1"]
```

### Modes

Dome supports modal keybindings. The `[keymaps]` section defines the **default** mode. Additional modes are defined with `[keymaps.mode.<name>]` sections. Switch between modes using the `mode <name>` action in a binding or via `dome mode <name>` over CLI/IPC.

```toml
[keymaps]
"meta+h" = ["focus left"]
"meta+r" = ["mode resize"]

[keymaps.mode.resize]
"h" = ["master shrink"]
"l" = ["master grow"]
"escape" = ["mode default"]
```

Here `meta+r` enters resize mode. Inside resize mode, `h` and `l` adjust the master area without modifiers, and `escape` returns to the default keybindings. The special name `"default"` always refers to the top-level `[keymaps]` table.

Mode switching is instant. When a binding contains `mode <name>`, the switch takes effect before the next keypress. A binding can combine a mode switch with other actions:

```toml
"meta+r" = ["focus left", "mode resize"]
```

This focuses left and enters resize mode in one keypress. If a binding lists multiple `mode` actions, the last one wins.

#### Reserved names

`"default"` refers to the top-level `[keymaps]` section. A `[keymaps.mode.default]` section is dropped with a warning in `dome.log`. Empty string `""` is also dropped with a warning.

#### Gotchas

**No automatic escape binding.** Dome does not enforce that a mode has a binding back to `default`. If you define a mode with no way out, your keyboard will only have the bindings in that mode until config reload or process exit. Always include an escape binding (like `"escape" = ["mode default"]`) in every custom mode.

**Config reload preserves active mode.** If you edit your config while in a non-default mode, hot reload keeps you in that mode as long as it still exists in the new config. If the reloaded config removes the active mode, Dome falls back to the default keybindings on the next keypress and logs a warning on each keystroke until you switch back to an existing mode (for example, `dome mode default`).

**Unknown mode names are rejected.** Running `dome mode typo` or pressing a binding with `mode typo` logs a warning and leaves your current mode unchanged.

**Mode state is global.** Modes are per-process, not per-workspace or per-monitor. Switching workspaces does not change the active mode.

## Error handling

Dome recovers from config errors at field granularity. A wrong type, out-of-range value, or unknown field does not invalidate the rest of the config. Each broken field falls back to its default, and Dome logs a warning to `dome.log` with the dotted field path (for example, `field=master.master_ratio`) and the reason for the fallback.

Per-field recovery covers unknown fields at any nesting level, wrong types or shapes, out-of-range values (`master_ratio`, `master_count`, `text_size`, `subtext_size`, blank `font.family`), bad keybindings (unparseable key or invalid action), bad entries in window-rule arrays, and reserved or empty mode names. In each case the offending item is dropped and the surrounding config survives.

One condition causes the entire config to fall back to defaults:

- TOML syntax errors (missing quotes, unmatched brackets, etc.).
