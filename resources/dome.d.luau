-- Type definitions for Dome's config.lua surface.
--
-- Point your editor's Luau language server at this file to typecheck config.lua
-- as you edit. Dome does not load this file at runtime. Typechecking happens in
-- your editor and in CI, never in the running window manager.
--
-- These types describe config.lua only. layout.jsonc is JSONC data with its own
-- shape and is not covered here.

-- The mutating action capability Dome passes into a function-valued keybinding.
-- It is valid only while the handler runs. A call after the handler returns
-- errors at runtime.
export type Actions = {
  focus: {
    left: () -> (),
    right: () -> (),
    up: () -> (),
    down: () -> (),
    parent: () -> (),
    workspace: (name: string) -> (),
    monitor: (target: string) -> (),
    tab: {
      next: () -> (),
      prev: () -> (),
    },
  },
  move: {
    left: () -> (),
    right: () -> (),
    up: () -> (),
    down: () -> (),
    workspace: (name: string) -> (),
    monitor: (target: string) -> (),
  },
  toggle: {
    spawn: () -> (),
    direction: () -> (),
    layout: () -> (),
    float: () -> (),
    fullscreen: () -> (),
  },
  master: {
    grow: () -> (),
    shrink: () -> (),
    more: () -> (),
    fewer: () -> (),
  },
  exec: (command: string) -> (),
  close: () -> (),
  exit: () -> (),
  mode: (name: string) -> (),
}

-- A keymap value: one string action, a list of string actions, or a handler.
export type Binding = string | { string } | (actions: Actions) -> ()

-- A window match. All present fields must match. macOS reads app, bundle_id,
-- title. Windows reads process, class, aumid, title.
export type WindowMatcher = {
  app: string?,
  bundle_id: string?,
  title: string?,
  process: string?,
  class: string?,
  aumid: string?,
}

-- The keymaps table. A string key is a chord bound to a Binding. The reserved
-- key "mode" holds the modal keymaps, each a named mode of chord to Binding.
export type Keymaps = {
  [string]: Binding,
  mode: { [string]: { [string]: Binding } }?,
}

export type Config = {
  border_size: number?,
  theme: ("latte" | "frappe" | "macchiato" | "mocha")?,
  log_level: ("trace" | "debug" | "info" | "warn" | "error")?,
  start_at_login: boolean?,
  strategy: ("partition_tree" | "master")?,
  minimum_width: (number | string)?,
  minimum_height: (number | string)?,
  maximum_width: (number | string)?,
  maximum_height: (number | string)?,
  partition_tree: {
    tab_bar_height: number?,
    automatic_tiling: boolean?,
  }?,
  master: {
    master_ratio: number?,
    master_count: number?,
  }?,
  font: {
    family: string?,
    text_size: number?,
  }?,
  float: { WindowMatcher }?,
  fullscreen: { WindowMatcher }?,
  ignore: { WindowMatcher }?,
  keymaps: Keymaps?,
}

-- A modifier constant. Modifier + Modifier composes into a Modifier. Modifier +
-- key attaches the one key and yields a chord string. A chord holds one key, so
-- adding a second key is a type error.
declare extern type Modifier with
  function __add(self, other: Modifier): Modifier
  function __add(self, key: string): string
end

declare Meta: Modifier
declare Alt: Modifier
declare Ctrl: Modifier
declare Shift: Modifier
declare Cmd: Modifier
declare Win: Modifier
declare Option: Modifier
declare Opt: Modifier
declare Control: Modifier

declare dome: {
  os: "macos" | "windows",
  executable: (name: string) -> boolean,
  defaults: () -> Config,
}
