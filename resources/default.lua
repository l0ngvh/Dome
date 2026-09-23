---@param mod1? Modifier
---@return Config
return function(mod1)
	mod1 = mod1 or Alt

	---@type Config
	return {
		-- Logical pixels.
		---@type number
		border_size = 4,
		-- Catppuccin flavor.
		---@type "latte" | "frappe" | "macchiato" | "mocha"
		theme = "mocha",
		---@type "trace" | "debug" | "info" | "warn" | "error"
		log_level = "info",
		---@type boolean
		start_at_login = false,
		-- Widget text size, in logical pixels.
		---@type number
		font_size = 14.0,
		---@type string?
		font_family = nil,
		---@type WindowMatcher[]
		ignore = {},
		---@type WindowMatcher[]
		float = {},
		---@type WindowMatcher[]
		fullscreen = {},
		-- Extra variables for actions.execute commands.
		---@type table<string, string>
		env = {},
		---@type "partition_tree" | "master" | "scrolling"
		layout = "partition_tree",
		-- Logical pixels, or "<number>%" of the work area. 0 means unconstrained.
		---@type number | string
		minimum_width = "5%",
		---@type number | string
		minimum_height = "5%",
		---@type number | string
		maximum_width = 0,
		---@type number | string
		maximum_height = 0,
		---@type dome.PartitionTree
		partition_tree = {
			---@type number
			tab_bar_height = 24,
			---@type boolean
			automatic_tiling = true,
		},
		---@type dome.MasterConfig
		master = {
			---@type number
			master_ratio = 0.5,
			---@type number
			master_count = 1,
		},
		---@type dome.ScrollingConfig
		scrolling = {
			---@type number | string
			default_column_width = "50%",
		},
		---@type Keymaps
		keymaps = {
			main = {
				[mod1 + "0"] = function(a)
					a.focus_workspace("0")
				end,
				[mod1 + "1"] = function(a)
					a.focus_workspace("1")
				end,
				[mod1 + "2"] = function(a)
					a.focus_workspace("2")
				end,
				[mod1 + "3"] = function(a)
					a.focus_workspace("3")
				end,
				[mod1 + "4"] = function(a)
					a.focus_workspace("4")
				end,
				[mod1 + "5"] = function(a)
					a.focus_workspace("5")
				end,
				[mod1 + "6"] = function(a)
					a.focus_workspace("6")
				end,
				[mod1 + "7"] = function(a)
					a.focus_workspace("7")
				end,
				[mod1 + "8"] = function(a)
					a.focus_workspace("8")
				end,
				[mod1 + "9"] = function(a)
					a.focus_workspace("9")
				end,
				[mod1 + Shift + "0"] = function(a)
					a.move_to_workspace("0")
				end,
				[mod1 + Shift + "1"] = function(a)
					a.move_to_workspace("1")
				end,
				[mod1 + Shift + "2"] = function(a)
					a.move_to_workspace("2")
				end,
				[mod1 + Shift + "3"] = function(a)
					a.move_to_workspace("3")
				end,
				[mod1 + Shift + "4"] = function(a)
					a.move_to_workspace("4")
				end,
				[mod1 + Shift + "5"] = function(a)
					a.move_to_workspace("5")
				end,
				[mod1 + Shift + "6"] = function(a)
					a.move_to_workspace("6")
				end,
				[mod1 + Shift + "7"] = function(a)
					a.move_to_workspace("7")
				end,
				[mod1 + Shift + "8"] = function(a)
					a.move_to_workspace("8")
				end,
				[mod1 + Shift + "9"] = function(a)
					a.move_to_workspace("9")
				end,
				[mod1 + "e"] = function(a)
					a.toggle_split()
				end,
				[mod1 + "d"] = function(a)
					a.rotate()
				end,
				[mod1 + "w"] = function(a)
					a.toggle_tabbed()
				end,
				[mod1 + "a"] = function(a)
					a.focus_parent()
				end,
				[mod1 + "h"] = function(a)
					a.focus_left()
				end,
				[mod1 + "j"] = function(a)
					a.focus_down()
				end,
				[mod1 + "k"] = function(a)
					a.focus_up()
				end,
				[mod1 + "l"] = function(a)
					a.focus_right()
				end,
				[mod1 + "["] = function(a)
					a.focus_tab_prev()
				end,
				[mod1 + "]"] = function(a)
					a.focus_tab_next()
				end,
				[mod1 + Shift + "h"] = function(a)
					a.move_left()
				end,
				[mod1 + Shift + "j"] = function(a)
					a.move_down()
				end,
				[mod1 + Shift + "k"] = function(a)
					a.move_up()
				end,
				[mod1 + Shift + "l"] = function(a)
					a.move_right()
				end,
				[mod1 + Shift + Space] = function(a)
					a.toggle_float()
				end,
				[mod1 + "f"] = function(a)
					a.toggle_fullscreen()
				end,
				[mod1 + Shift + "q"] = function(a)
					a.close()
				end,
				[mod1 + Ctrl + "h"] = function(a)
					a.focus_monitor_left()
				end,
				[mod1 + Ctrl + "j"] = function(a)
					a.focus_monitor_down()
				end,
				[mod1 + Ctrl + "k"] = function(a)
					a.focus_monitor_up()
				end,
				[mod1 + Ctrl + "l"] = function(a)
					a.focus_monitor_right()
				end,
				[mod1 + Ctrl + Shift + "h"] = function(a)
					a.move_to_monitor_left()
				end,
				[mod1 + Ctrl + Shift + "j"] = function(a)
					a.move_to_monitor_down()
				end,
				[mod1 + Ctrl + Shift + "k"] = function(a)
					a.move_to_monitor_up()
				end,
				[mod1 + Ctrl + Shift + "l"] = function(a)
					a.move_to_monitor_right()
				end,
			},
		},
	}
end
