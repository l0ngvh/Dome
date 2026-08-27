-- Dome configuration. Reloaded automatically on save.
-- Return one table. `dome.os` is "macos" or "windows" for OS branching.

-- One terminal binding, chosen per OS.
local terminal = dome.os == "macos" and "exec open -a Terminal" or "exec wt"

return {
  border_size = 4,
  theme = "mocha", -- latte | frappe | macchiato | mocha
  log_level = "debug", -- trace | debug | info | warn | error
  start_at_login = false,
  strategy = "partition_tree", -- partition_tree | master

  -- Global float/fullscreen matchers apply to all workspaces.
  -- Per-workspace matchers in layout.jsonc take priority.
  float = { { process = "calculator.exe" } },
  fullscreen = { { process = "slides.exe" } },

  -- Window size: a whole number is pixels, "N%" is percent.
  minimum_width = 200,
  minimum_height = "10%",
  maximum_width = 800,
  maximum_height = "50%",

  partition_tree = {
    tab_bar_height = 24, -- tab bar height in tabbed containers, logical pixels
    automatic_tiling = true, -- split direction follows the focused window shape
  },

  master = {
    master_ratio = 0.5, -- master area width, in [0.1, 0.9]
    master_count = 1, -- master windows, >= 1
  },

  font = {
    text_size = 14.0,
  },

  -- A binding value is a string or a list of strings.
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
    ["meta+alt+h"] = "focus monitor left",
    ["meta+alt+j"] = "focus monitor down",
    ["meta+alt+k"] = "focus monitor up",
    ["meta+alt+l"] = "focus monitor right",
    ["meta+alt+shift+h"] = "move monitor left",
    ["meta+alt+shift+j"] = "move monitor down",
    ["meta+alt+shift+k"] = "move monitor up",
    ["meta+alt+shift+l"] = "move monitor right",
    ["meta+shift+f"] = "toggle float",
    ["meta+shift+return"] = "toggle fullscreen",
    ["meta+shift+q"] = "close",
    ["meta+return"] = terminal,

    mode = {
      resize = {
        ["h"] = "master shrink",
        ["l"] = "master grow",
        ["j"] = "master more",
        ["k"] = "master fewer",
        ["escape"] = "mode default",
      },
    },
  },

  -- Ignore rules. All fields must match (AND). First match wins.
  -- Use /pattern/ for a regex on app, process, or title.
  -- macOS fields: app, bundle_id, title. Windows fields: process, class, aumid, title.
  -- ignore = { { app = "System Preferences" }, { title = "Task Manager" } },
}
