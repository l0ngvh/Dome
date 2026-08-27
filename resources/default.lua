-- The default configuration Dome falls back to and exposes through
-- dome.defaults(). It returns a config table holding the default keymaps. A user
-- starts from dome.defaults(), overrides fields, and returns the table.
--
-- This file must not call dome.defaults(). That function re-evaluates this same
-- source, so a call here would not terminate.
--
-- Keymaps only. Static scalar defaults live in Rust field defaults, and the
-- window-ignore floor is applied by Rust regardless of this file.

return {
  keymaps = {
    ["meta+0"] = "focus workspace 0",
    ["meta+1"] = "focus workspace 1",
    ["meta+2"] = "focus workspace 2",
    ["meta+3"] = "focus workspace 3",
    ["meta+4"] = "focus workspace 4",
    ["meta+5"] = "focus workspace 5",
    ["meta+6"] = "focus workspace 6",
    ["meta+7"] = "focus workspace 7",
    ["meta+8"] = "focus workspace 8",
    ["meta+9"] = "focus workspace 9",
    ["meta+shift+0"] = "move workspace 0",
    ["meta+shift+1"] = "move workspace 1",
    ["meta+shift+2"] = "move workspace 2",
    ["meta+shift+3"] = "move workspace 3",
    ["meta+shift+4"] = "move workspace 4",
    ["meta+shift+5"] = "move workspace 5",
    ["meta+shift+6"] = "move workspace 6",
    ["meta+shift+7"] = "move workspace 7",
    ["meta+shift+8"] = "move workspace 8",
    ["meta+shift+9"] = "move workspace 9",
    ["meta+e"] = "toggle spawn",
    ["meta+d"] = "toggle direction",
    ["meta+b"] = "toggle layout",
    ["meta+p"] = "focus parent",
    ["meta+h"] = "focus left",
    ["meta+j"] = "focus down",
    ["meta+k"] = "focus up",
    ["meta+l"] = "focus right",
    ["meta+["] = "focus tab prev",
    ["meta+]"] = "focus tab next",
    ["meta+shift+h"] = "move left",
    ["meta+shift+j"] = "move down",
    ["meta+shift+k"] = "move up",
    ["meta+shift+l"] = "move right",
    ["meta+shift+f"] = "toggle float",
    ["meta+shift+q"] = "close",
    ["meta+alt+h"] = "focus monitor left",
    ["meta+alt+j"] = "focus monitor down",
    ["meta+alt+k"] = "focus monitor up",
    ["meta+alt+l"] = "focus monitor right",
    ["meta+alt+shift+h"] = "move monitor left",
    ["meta+alt+shift+j"] = "move monitor down",
    ["meta+alt+shift+k"] = "move monitor up",
    ["meta+alt+shift+l"] = "move monitor right",
  },
}
