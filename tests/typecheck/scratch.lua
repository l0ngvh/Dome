-- Typecheck fixture: build a config from scratch. Dome does not ship or load
-- this file. CI runs lua-language-server --check on it against
-- resources/dome.meta.lua. The fixture covers the "return your own table"
-- idiom.

local terminal = dome.os == "macos" and "open -a Terminal" or "wt"

---@type Config
local config = {
  theme = "latte",
  border_size = 2,
  env = { EDITOR = "nvim" },
  keymaps = {
    main = {
      [Meta + "h"] = function(actions) actions.focus_left() end,
      [Meta + "l"] = function(actions) actions.focus_right() end,
      [Meta + "j"] = function(actions) actions.focus_down() end,
      [Meta + "k"] = function(actions) actions.focus_up() end,
      [Meta + "a"] = function(actions) actions.focus_parent() end,
      [Meta + Shift + "h"] = function(actions) actions.move_left() end,
      [Meta + Shift + "l"] = function(actions) actions.move_right() end,
      [Meta + Shift + "j"] = function(actions) actions.move_down() end,
      [Meta + Shift + "k"] = function(actions) actions.move_up() end,
      [Meta + "s"] = function(actions) actions.toggle_split() end,
      [Meta + "d"] = function(actions) actions.rotate() end,
      [Meta + "b"] = function(actions) actions.toggle_tabbed() end,
      [Meta + Shift + "f"] = function(actions) actions.toggle_float() end,
      [Meta + "f"] = function(actions) actions.toggle_fullscreen() end,
      [Meta + Return] = function(actions) actions.execute(terminal) end,
      [Meta + Shift + "1"] = function(actions)
        actions.move_to_workspace("1")
        actions.focus_workspace("1")
      end,
      ["meta+r"] = function(actions) actions.mode("resize") end,
      [Meta + Shift + "q"] = function(actions) actions.close() end,
      [Meta + Shift + "e"] = function(actions) actions.exit() end,
    },
    resize = {
      ["h"] = function(actions) actions.decrease_master_ratio() end,
      ["l"] = function(actions) actions.increase_master_ratio() end,
      [Space] = function(actions) actions.toggle_float() end,
      [Up] = function(actions) actions.increase_master_count() end,
      [Esc] = function(actions) actions.mode("main") end,
    },
  },
}

return config
