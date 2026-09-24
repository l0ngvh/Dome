---@meta

-- Type stubs for config.lua and layout.lua.

-- The handle Dome passes to each binding.
---@class Actions
---@field focus_left fun()
---@field focus_right fun()
---@field focus_up fun()
---@field focus_down fun()
---@field focus_parent fun()
---@field focus_tab_next fun()
---@field focus_tab_prev fun()
---@field focus_workspace fun(name: string)
---@field focus_monitor_left fun()
---@field focus_monitor_right fun()
---@field focus_monitor_up fun()
---@field focus_monitor_down fun()
---@field focus_monitor fun(name: string) Focus the monitor named name.
---@field move_left fun()
---@field move_right fun()
---@field move_up fun()
---@field move_down fun()
---@field move_to_workspace fun(name: string)
---@field move_to_monitor_left fun()
---@field move_to_monitor_right fun()
---@field move_to_monitor_up fun()
---@field move_to_monitor_down fun()
---@field move_to_monitor fun(name: string) Move the focused window to the monitor named name.
---@field toggle_split fun() Toggle the spawn direction between horizontal and vertical.
---@field rotate fun() Flip the parent container's split between horizontal and vertical.
---@field toggle_tabbed fun() Switch the parent container between tiled and tabbed.
---@field toggle_float fun()
---@field toggle_fullscreen fun()
---@field increase_master_ratio fun() Widen the master area by 5 percentage points, clamped to 0.1 through 0.9.
---@field decrease_master_ratio fun() Narrow the master area by 5 percentage points, clamped to 0.1 through 0.9.
---@field increase_master_count fun()
---@field decrease_master_count fun() Clamped to a minimum of 1.
---@field execute fun(command: string) Through /bin/sh -c on macOS and cmd.exe /C on Windows.
---@field close fun()
---@field exit fun() Stop Dome and restore all windows.
---@field mode fun(name: string) Switch to the keymap named name.

---@alias Binding fun(actions: Actions)

-- All present fields must match, and the first matching rule wins. Wrap a value in /pattern/ for a regex match.
---@class WindowMatcher
---@field app? string On Windows this is the executable's file description, often absent or localized, so prefer process there.
---@field bundle_id? string macOS only, for example com.apple.finder.
---@field title? string
---@field process? string Windows only, for example explorer.exe.
---@field class? string Windows only, for example Shell_TrayWnd.
---@field aumid? string Windows only. Application User Model ID, for example Microsoft.WindowsCalculator_8wekyb3d8bbwe!App.

-- A keymaps table in the config replaces every default with no merge.
---@alias Keymaps table<string, table<Keystroke, Binding>>

---@class dome.PartitionTree
---@field tab_bar_height? number Logical pixels, and it does not scale with font_size.
---@field automatic_tiling? boolean Take the split direction from the focused window's shape.

---@class dome.MasterConfig
---@field master_ratio? number
---@field master_count? number

---@class Config
---@field border_size? number Logical pixels.
---@field theme? "latte" | "frappe" | "macchiato" | "mocha" Catppuccin flavor.
---@field log_level? "trace" | "debug" | "info" | "warn" | "error"
---@field start_at_login? boolean
---@field layout? "partition_tree" | "master"
---@field minimum_width? number | string Logical pixels, or "<number>%" of the work area. 0 means unconstrained.
---@field minimum_height? number | string Logical pixels, or "<number>%" of the work area. 0 means unconstrained.
---@field maximum_width? number | string Logical pixels, or "<number>%" of the work area. 0 means unconstrained.
---@field maximum_height? number | string Logical pixels, or "<number>%" of the work area. 0 means unconstrained.
---@field partition_tree? dome.PartitionTree
---@field master? dome.MasterConfig
---@field font_size? number Widget text size, in logical pixels.
---@field font_family? string
---@field float? WindowMatcher[]
---@field fullscreen? WindowMatcher[]
---@field ignore? WindowMatcher[]
---@field env? table<string, string> Extra variables for actions.execute commands.
---@field keymaps? Keymaps

-- Keyed by monitor name.
---@alias dome.Layout table<string, dome.MonitorLayout>

-- Keyed by workspace name.
---@alias dome.MonitorLayout table<string, dome.LayoutWorkspace>

---@class dome.LayoutWorkspace
---@field layout "partition_tree" | "master"
---@field tree? dome.TreeNode partition_tree only.
---@field master_ratio? number master only.
---@field master_count? number master only.
---@field master? dome.Pane master only.
---@field secondary? dome.Pane master only.
---@field float? WindowMatcher[]
---@field fullscreen? WindowMatcher[]

---@alias dome.TreeNode WindowMatcher | dome.TreeNode[] | dome.TreeContainer

---@class dome.TreeContainer
---@field split? "horizontal" | "vertical" | "tabbed"
---@field children dome.TreeNode[]

---@alias dome.Pane WindowMatcher[] | dome.PaneContainer

---@class dome.PaneContainer
---@field display? "tiled" | "tabbed"
---@field children WindowMatcher[]

-- Built by adding a key to a modifier, for example Meta + "h", or named on its own as Space, Enter, or Escape.
---@class Keystroke

-- Meta (or Cmd, Win) is Command on macOS and the Windows key on Windows. Alt (or Opt, Option) is Option on macOS.
---@class Modifier
---@operator add(string): Keystroke
---@operator add(Keystroke): Keystroke
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

---@type Keystroke
Space = nil
---@type Keystroke
Enter = nil
---@type Keystroke
Return = nil
---@type Keystroke
Escape = nil
---@type Keystroke
Esc = nil
---@type Keystroke
Tab = nil
---@type Keystroke
Backspace = nil
---@type Keystroke
Up = nil
---@type Keystroke
Down = nil
---@type Keystroke
Left = nil
---@type Keystroke
Right = nil

---@class dome.Builder
---@field defaults fun(): Config

---@class Dome
---@field os "macos" | "windows"
---@field env table<string, string> The environment that started Dome.
---@field executable fun(name: string): boolean True when name resolves on PATH.
---@field defaults fun(): Config
---@field with_default_modifier fun(modifier: Modifier): dome.Builder Meta or Alt only.

---@type Dome
dome = nil
