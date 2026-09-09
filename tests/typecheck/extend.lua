-- Typecheck fixture: extend the defaults. Dome does not ship or load this file.
-- CI runs lua-language-server --check on it against resources/dome.meta.lua, so a
-- break in the Config, Binding, Actions, Keymaps, or WindowMatcher types fails
-- the build. It exercises the "start from dome.defaults() and override" idiom.

local terminal = dome.os == "macos" and "open -a Terminal" or "wt"

local config = dome.with_default_modifier(Alt).defaults()

config.theme = "mocha"
config.border_size = 4
config.log_level = "info"
config.start_at_login = false
config.strategy = "partition_tree"
config.minimum_width = "5%"
config.minimum_height = 200
config.maximum_width = 0
config.maximum_height = "50%"
config.partition_tree = { tab_bar_height = 24, automatic_tiling = true }
config.master = { master_ratio = 0.5, master_count = 1 }
config.font_size = 14.0
config.font_family = "PingFang SC"
config.float = { { process = "calculator.exe" } }
config.fullscreen = { { process = "slides.exe" } }

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
keymaps.main[Meta + "return"] = function(actions) actions.execute(terminal) end
keymaps.main[Alt + "return"] = function(actions) actions.execute(terminal) end
keymaps.main[Meta + Shift + "1"] = function(actions)
  actions.move.workspace("1")
  actions.focus.workspace("1")
end
keymaps.main["meta+r"] = function(actions) actions.mode("resize") end
keymaps.main[Meta + Shift + "return"] = function(actions)
  actions.execute(terminal)
  actions.focus.right()
  actions.move.workspace("2")
  actions.mode("resize")
end
keymaps.resize = {
  ["h"] = function(actions) actions.master.shrink() end,
  ["l"] = function(actions) actions.master.grow() end,
  ["escape"] = function(actions) actions.mode("main") end,
}

return config
