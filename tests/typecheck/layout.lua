-- Typecheck fixture: a preferred layout file. Dome writes this file with
-- `dome export`, but CI runs lua-language-server --check on it against
-- resources/dome.meta.lua so the dome.Layout types stay honest. The leading
-- annotation is what turns on field checking for the returned table.

---@type dome.Layout
return {
  workspace = {
    {
      name = "solo",
      strategy = "partition_tree",
      tree = { app = "Ghostty" },
    },
    {
      name = "dev",
      strategy = "partition_tree",
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
    {
      name = "work",
      strategy = "master",
      master_ratio = 0.6,
      master_count = 1,
      master = { { app = "Ghostty" } },
      secondary = { display = "tabbed", children = { { app = "Firefox" } } },
    },
  },
}
