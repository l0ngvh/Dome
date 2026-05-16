# Keybindings

## Overview

Keybindings are defined in the `[keymaps]` section of the config file. The same file works on macOS and Windows without changes. Defining a `[keymaps]` section **replaces the defaults wholesale**. There is no merge. Users who want the defaults plus their own bindings must copy the default table and add to it.

## Syntax

Each entry maps a key combination to one or more action strings:

```toml
"mods+...+key" = ["<action>", ...]
```

Tokens are lowercase. Accepted modifier tokens: `cmd`, `shift`, `alt`, `ctrl`. Multiple modifiers are joined with `+`. The key is the final segment after all modifiers.

Values are arrays of action strings. Single-element arrays are fine. Multi-action arrays fire in order.

Examples:

```toml
"cmd+h" = ["focus left"]
"cmd+shift+1" = ["move workspace 1", "focus workspace 1"]
```

## Modifier mapping

The literal config token is always `cmd`. "Meta" is display shorthand used in the README and never appears in config files.

| Token | macOS | Windows |
|-------|-------|---------|
| `cmd` | <kbd>Command</kbd> | <kbd>Windows</kbd> |
| `shift` | <kbd>Shift</kbd> | <kbd>Shift</kbd> |
| `alt` | <kbd>Option</kbd> | <kbd>Alt</kbd> |
| `ctrl` | <kbd>Control</kbd> | <kbd>Control</kbd> |

## Default bindings

Dome ships 44 default bindings.

| Key | Action |
|-----|--------|
| <kbd>cmd</kbd>+<kbd>0</kbd> through <kbd>cmd</kbd>+<kbd>9</kbd> | `focus workspace 0` through `focus workspace 9` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>0</kbd> through <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>9</kbd> | `move workspace 0` through `move workspace 9` |
| <kbd>cmd</kbd>+<kbd>h</kbd> | `focus left` |
| <kbd>cmd</kbd>+<kbd>j</kbd> | `focus down` |
| <kbd>cmd</kbd>+<kbd>k</kbd> | `focus up` |
| <kbd>cmd</kbd>+<kbd>l</kbd> | `focus right` |
| <kbd>cmd</kbd>+<kbd>p</kbd> | `focus parent` |
| <kbd>cmd</kbd>+<kbd>[</kbd> | `focus tab prev` |
| <kbd>cmd</kbd>+<kbd>]</kbd> | `focus tab next` |
| <kbd>cmd</kbd>+<kbd>e</kbd> | `toggle spawn` |
| <kbd>cmd</kbd>+<kbd>d</kbd> | `toggle direction` |
| <kbd>cmd</kbd>+<kbd>b</kbd> | `toggle layout` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>f</kbd> | `toggle float` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move left` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move down` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move up` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move right` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>h</kbd> | `focus monitor left` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>j</kbd> | `focus monitor down` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>k</kbd> | `focus monitor up` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>l</kbd> | `focus monitor right` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>h</kbd> | `move monitor left` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>j</kbd> | `move monitor down` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>k</kbd> | `move monitor up` |
| <kbd>cmd</kbd>+<kbd>alt</kbd>+<kbd>shift</kbd>+<kbd>l</kbd> | `move monitor right` |
| <kbd>cmd</kbd>+<kbd>shift</kbd>+<kbd>q</kbd> | `exit` |

## Customising

Define a `[keymaps]` section to replace the defaults with your own bindings. The entire default set is discarded when you do this, so copy any defaults you want to keep.

```toml
[keymaps]
"cmd+h" = ["focus left"]
"cmd+l" = ["focus right"]
"cmd+return" = ["exec open -a Terminal"]
```

## Multi-action bindings

A binding's value is an array of action strings. All actions fire in order on a single keypress:

```toml
"cmd+shift+1" = ["move workspace 1", "focus workspace 1"]
```

## Modes

Dome supports modal keybindings. The `[keymaps]` section defines the **default** mode. Additional modes are defined with `[keymaps.mode.<name>]` sections. Switch between modes using the `mode <name>` action in a binding or via `dome mode <name>` over CLI/IPC.

```toml
[keymaps]
"cmd+h" = ["focus left"]
"cmd+r" = ["mode resize"]

[keymaps.mode.resize]
"h" = ["master shrink"]
"l" = ["master grow"]
"escape" = ["mode default"]
```

In this example, `cmd+r` enters resize mode. Inside resize mode, `h` and `l` adjust the master area without modifiers, and `escape` returns to the default keybindings. The special name `"default"` always refers to the top-level `[keymaps]` table.

Mode switching is instant. When a binding contains `mode <name>`, the switch takes effect before the next keypress. A binding can combine a mode switch with other actions:

```toml
"cmd+r" = ["focus left", "mode resize"]
```

This focuses left and enters resize mode in one keypress. If a binding lists multiple `mode` actions, the last one wins.

### Reserved names

- `"default"` refers to the top-level `[keymaps]` section. Using it as a `[keymaps.mode.default]` section name causes a config validation error.
- Empty string `""` is rejected as a mode name.

### Gotchas

**No automatic escape binding.** Dome does not enforce that a mode has a binding back to `default`. If you define a mode with no way out, your keyboard will only have the bindings in that mode until config reload or process exit. Always include an escape binding (like `"escape" = ["mode default"]`) in every custom mode.

**Config reload preserves active mode.** If you edit your config while in a non-default mode, hot reload keeps you in that mode as long as it still exists in the new config. If the reloaded config removes the active mode, Dome falls back to the default keybindings on the next keypress and logs a warning on each keystroke until you switch back to an existing mode (for example, `dome mode default`).

**Unknown mode names are rejected.** Running `dome mode typo` or pressing a binding with `mode typo` logs a warning and leaves your current mode unchanged.

**Mode state is global.** Modes are per-process, not per-workspace or per-monitor. Switching workspaces does not change the active mode.
