-- Typecheck fixture: extend the defaults. Dome does not ship or load this file.
-- CI runs lua-language-server --check on it against resources/dome.meta.lua.

local terminal = dome.os == "macos" and "open -a Terminal" or "wt"

local config = dome.with_default_modifier(Alt).defaults()

config.theme = "mocha"
config.border_size = 4
config.log_level = "info"
config.start_at_login = false
config.layout = "partition_tree"
config.minimum_width = "5%"
config.minimum_height = 200
config.maximum_width = 0
config.maximum_height = "50%"
config.partition_tree = { tab_bar_height = 24, automatic_tiling = true }
config.master = { master_ratio = 0.5, master_count = 1 }
config.scrolling = { default_column_width = "60%" }
config.font_size = 14.0
config.font_family = dome.env.DOME_FONT or "PingFang SC"
config.float = { { process = "calculator.exe" } }
config.fullscreen = { { process = "slides.exe" } }
config.env = { EDITOR = "nvim" }

-- A typed local keeps a mixed-shape matcher list from being inferred against
-- its first element.
---@type WindowMatcher[]
local ignore = {
  { app = "System Preferences" },
  { class = "Shell_TrayWnd" },
}
config.ignore = ignore

-- keymaps is optional on Config, so annotate the local before indexing.
---@type Keymaps
local keymaps = config.keymaps
keymaps.main[Meta + Return] = function(actions) actions.execute(terminal) end
keymaps.main[Alt + "return"] = function(actions) actions.execute(terminal) end
keymaps.main[Meta + Shift + "1"] = function(actions)
  actions.move_to_workspace("1")
  actions.focus_workspace("1")
end
keymaps.main["meta+r"] = function(actions) actions.mode("resize") end
keymaps.main[Meta + Ctrl + "h"] = function(actions) actions.focus_monitor_left() end
keymaps.main[Meta + Ctrl + Shift + "h"] = function(actions) actions.move_to_monitor_left() end
keymaps.main[Meta + "n"] = function(actions) actions.focus_tab_next() end
keymaps.main[Meta + "p"] = function(actions) actions.focus_tab_prev() end
keymaps.main[Meta + Shift + Enter] = function(actions)
  actions.execute(terminal)
  actions.focus_right()
  actions.move_to_workspace("2")
  actions.mode("resize")
end
keymaps.resize = {
  ["h"] = function(actions) actions.decrease_master_ratio() end,
  ["l"] = function(actions) actions.increase_master_ratio() end,
  ["j"] = function(actions) actions.decrease_master_count() end,
  ["k"] = function(actions) actions.increase_master_count() end,
  [Escape] = function(actions) actions.mode("main") end,
}

return config
