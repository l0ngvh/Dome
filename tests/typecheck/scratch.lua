-- Typecheck fixture: build a config from scratch. Dome does not ship or load
-- this file. CI runs lua-language-server --check on it against
-- resources/dome.meta.lua, the same role as extend.lua, for the "return your own
-- table" idiom.

local terminal = dome.os == "macos" and "open -a Terminal" or "wt"

---@type Config
local config = {
  theme = "latte",
  border_size = 2,
  keymaps = {
    main = {
      [Meta + "h"] = function(actions) actions.focus.left() end,
      [Meta + "l"] = function(actions) actions.focus.right() end,
      [Meta + "return"] = function(actions) actions.exec(terminal) end,
      [Meta + Shift + "1"] = function(actions)
        actions.move.workspace("1")
        actions.focus.workspace("1")
      end,
      ["meta+r"] = function(actions) actions.mode("resize") end,
      [Meta + Shift + "q"] = function(actions) actions.close() end,
    },
    resize = {
      ["h"] = function(actions) actions.master.shrink() end,
      ["l"] = function(actions) actions.master.grow() end,
      ["escape"] = function(actions) actions.mode("main") end,
    },
  },
}

return config
