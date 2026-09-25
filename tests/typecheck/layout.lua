-- Typecheck fixture: a preferred layout file. Dome writes this file with
-- `dome export`, but CI runs lua-language-server --check on it against
-- resources/dome.meta.lua, which catches a field name or a value type that
-- dome.Layout does not accept.

---@type dome.MonitorLayout
local shared = {
  ["solo"] = {
    layout = "partition_tree",
    tree = { app = "Ghostty" },
  },
  ["dev"] = {
    layout = "partition_tree",
    tree = {
      split = "horizontal",
      children = {
        { app = "Ghostty" },
        { split = "vertical", children = { { app = "Firefox" }, { app = "Slack" } } },
      },
    },
    float = { { app = "System Settings" } },
    fullscreen = { { title = "Picture in Picture" } },
  },
  ["work"] = {
    layout = "master",
    master_ratio = 0.6,
    master_count = 1,
    master = { { app = "Ghostty" } },
    secondary = { display = "tabbed", children = { { app = "Firefox" } } },
  },
  ["scroll"] = {
    layout = "scrolling",
    columns = {
      { app = "Ghostty" },
      { width = "40%", children = { { app = "Firefox" } } },
      { width = 800, children = { { app = "Slack" } } },
    },
    float = { { app = "mpv" } },
  },
}

---@type dome.Layout
return {
  ["Built-in Retina Display"] = shared,
  ["PC Monitor"] = shared,
}
