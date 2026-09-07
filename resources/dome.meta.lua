---@meta

-- Type stubs for config.lua. Dome does not load this file at runtime.

---@class dome.Focus
---@field left fun()
---@field right fun()
---@field up fun()
---@field down fun()
---@field parent fun()
---@field workspace fun(name: string)
---@field monitor fun(target: string)
---@field tab dome.FocusTab

---@class dome.FocusTab
---@field next fun()
---@field prev fun()

---@class dome.Move
---@field left fun()
---@field right fun()
---@field up fun()
---@field down fun()
---@field workspace fun(name: string)
---@field monitor fun(target: string)

---@class dome.Toggle
---@field spawn fun()
---@field direction fun()
---@field layout fun()
---@field float fun()
---@field fullscreen fun()

---@class dome.Master
---@field grow fun()
---@field shrink fun()
---@field more fun()
---@field fewer fun()

---@class Actions
---@field focus dome.Focus
---@field move dome.Move
---@field toggle dome.Toggle
---@field master dome.Master
---@field exec fun(command: string)
---@field close fun()
---@field exit fun()
---@field mode fun(name: string)

---@alias Binding fun(actions: Actions)

-- All present fields must match. macOS reads app/bundle_id/title, Windows reads process/class/aumid/title.
---@class WindowMatcher
---@field app? string
---@field bundle_id? string
---@field title? string
---@field process? string
---@field class? string
---@field aumid? string

---@alias Keymaps table<string, table<string, Binding>>

---@class dome.PartitionTree
---@field tab_bar_height? number
---@field automatic_tiling? boolean

---@class dome.MasterConfig
---@field master_ratio? number
---@field master_count? number

---@class Config
---@field border_size? number
---@field theme? "latte" | "frappe" | "macchiato" | "mocha"
---@field log_level? "trace" | "debug" | "info" | "warn" | "error"
---@field start_at_login? boolean
---@field strategy? "partition_tree" | "master"
---@field minimum_width? number | string
---@field minimum_height? number | string
---@field maximum_width? number | string
---@field maximum_height? number | string
---@field partition_tree? dome.PartitionTree
---@field master? dome.MasterConfig
---@field font_size? number
---@field font_family? string
---@field float? WindowMatcher[]
---@field fullscreen? WindowMatcher[]
---@field ignore? WindowMatcher[]
---@field keymaps? Keymaps

-- A chord holds one key, so adding a second key is a type error.
---@class Modifier
---@operator add(string): string
---@operator add(Modifier): Modifier

---@type Modifier
Meta = nil
---@type Modifier
Alt = nil
---@type Modifier
Ctrl = nil
---@type Modifier
Shift = nil
---@type Modifier
Cmd = nil
---@type Modifier
Win = nil
---@type Modifier
Option = nil
---@type Modifier
Opt = nil
---@type Modifier
Control = nil

---@class dome.Builder
---@field defaults fun(): Config

---@class Dome
---@field os "macos" | "windows"
---@field executable fun(name: string): boolean
---@field defaults fun(): Config
---@field with_default_modifier fun(modifier: Modifier): dome.Builder

---@type Dome
dome = nil
