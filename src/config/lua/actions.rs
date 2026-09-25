//! Builds the per-call `actions` handle. `Lua::scope` expires the closures when
//! the handler returns, so a stashed handle cannot drive the hub later.

use std::cell::RefCell;

use crate::core::{Direction, MonitorSelector, StrategyAction, TilingAction};

use super::ActionContext;

/// `TilingAction` is not `Clone`, so the table below holds constructors.
type Make = fn() -> TilingAction;

/// Each name must be its CLI command name with every hyphen replaced by an
/// underscore.
const NO_ARG: [(&str, Make); 28] = [
    ("focus_left", || {
        focus_direction(Direction::Horizontal, false)
    }),
    ("focus_right", || {
        focus_direction(Direction::Horizontal, true)
    }),
    ("focus_up", || focus_direction(Direction::Vertical, false)),
    ("focus_down", || focus_direction(Direction::Vertical, true)),
    ("focus_parent", || StrategyAction::FocusParent.into()),
    ("focus_tab_next", || {
        StrategyAction::FocusTab { forward: true }.into()
    }),
    ("focus_tab_prev", || {
        StrategyAction::FocusTab { forward: false }.into()
    }),
    ("focus_monitor_left", || {
        focus_monitor(MonitorSelector::Left)
    }),
    ("focus_monitor_right", || {
        focus_monitor(MonitorSelector::Right)
    }),
    ("focus_monitor_up", || focus_monitor(MonitorSelector::Up)),
    ("focus_monitor_down", || {
        focus_monitor(MonitorSelector::Down)
    }),
    ("move_left", || move_direction(Direction::Horizontal, false)),
    ("move_right", || move_direction(Direction::Horizontal, true)),
    ("move_up", || move_direction(Direction::Vertical, false)),
    ("move_down", || move_direction(Direction::Vertical, true)),
    ("move_to_monitor_left", || {
        move_to_monitor(MonitorSelector::Left)
    }),
    ("move_to_monitor_right", || {
        move_to_monitor(MonitorSelector::Right)
    }),
    ("move_to_monitor_up", || {
        move_to_monitor(MonitorSelector::Up)
    }),
    ("move_to_monitor_down", || {
        move_to_monitor(MonitorSelector::Down)
    }),
    ("toggle_split", || StrategyAction::ToggleSpawnMode.into()),
    ("rotate", || StrategyAction::ToggleDirection.into()),
    ("toggle_tabbed", || {
        StrategyAction::ToggleContainerLayout.into()
    }),
    ("toggle_float", || TilingAction::ToggleFloat),
    ("toggle_fullscreen", || TilingAction::ToggleFullscreen),
    ("increase_master_ratio", || {
        StrategyAction::GrowMaster.into()
    }),
    ("decrease_master_ratio", || {
        StrategyAction::ShrinkMaster.into()
    }),
    ("increase_master_count", || {
        StrategyAction::MoreMaster.into()
    }),
    ("decrease_master_count", || {
        StrategyAction::FewerMaster.into()
    }),
];

pub(super) fn build_actions<'scope, 'env, 'a, 'b>(
    lua: &mlua::Lua,
    scope: &'scope mlua::Scope<'scope, 'env>,
    cx: &'scope RefCell<&'a mut ActionContext<'b>>,
) -> mlua::Result<mlua::Table>
where
    'a: 'scope,
    'b: 'scope,
{
    let actions = lua.create_table()?;

    for (name, make) in NO_ARG {
        actions.set(
            name,
            scope.create_function(move |_, ()| {
                cx.borrow_mut().hub.handle_tiling_action(make());
                Ok(())
            })?,
        )?;
    }

    actions.set(
        "focus_workspace",
        scope.create_function(move |_, name: String| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(TilingAction::FocusWorkspace {
                    name,
                    monitor: None,
                });
            Ok(())
        })?,
    )?;
    actions.set(
        "move_to_workspace",
        scope.create_function(move |_, name: String| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(TilingAction::MoveToWorkspace {
                    name,
                    monitor: None,
                });
            Ok(())
        })?,
    )?;
    actions.set(
        "focus_monitor",
        scope.create_function(move |_, name: String| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(focus_monitor(MonitorSelector::Name(name)));
            Ok(())
        })?,
    )?;
    actions.set(
        "move_to_monitor",
        scope.create_function(move |_, name: String| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(move_to_monitor(MonitorSelector::Name(name)));
            Ok(())
        })?,
    )?;

    actions.set(
        "execute",
        scope.create_function(move |_, command: String| {
            cx.borrow_mut().effects.execute(&command);
            Ok(())
        })?,
    )?;
    actions.set(
        "close",
        scope.create_function(move |_, ()| {
            let target = {
                let c = cx.borrow();
                c.hub.focused_window(c.hub.current_workspace())
            };
            if let Some(id) = target {
                cx.borrow_mut().effects.close(id);
            }
            Ok(())
        })?,
    )?;
    actions.set(
        "exit",
        scope.create_function(move |_, ()| {
            cx.borrow_mut().effects.exit();
            Ok(())
        })?,
    )?;
    actions.set(
        "mode",
        scope.create_function(move |_, name: String| {
            cx.borrow_mut().keymap_effects.switch_mode(&name);
            Ok(())
        })?,
    )?;

    Ok(actions)
}

fn focus_direction(direction: Direction, forward: bool) -> TilingAction {
    StrategyAction::FocusDirection { direction, forward }.into()
}

fn move_direction(direction: Direction, forward: bool) -> TilingAction {
    StrategyAction::MoveDirection { direction, forward }.into()
}

fn focus_monitor(selector: MonitorSelector) -> TilingAction {
    TilingAction::FocusMonitor { selector }
}

fn move_to_monitor(selector: MonitorSelector) -> TilingAction {
    TilingAction::MoveToMonitor { selector }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;
    use crate::config::lua::new_vm;
    use crate::config::lua::test_support::{
        RecordingEffects, RecordingKeymap, hub_with_focused_window, test_hub,
    };
    use crate::core::Hub;

    const META_LUA: &str = include_str!("../../../resources/dome.meta.lua");

    /// The stub is hand-maintained and never loaded, so only this test keeps it
    /// from drifting away from the registered names.
    #[test]
    fn the_luals_stub_lists_every_registered_action() {
        let stub: BTreeSet<&str> = META_LUA
            .lines()
            .skip_while(|line| *line != "---@class Actions")
            .skip(1)
            .take_while(|line| line.starts_with("---@field "))
            .filter_map(|line| line.split_whitespace().nth(1))
            .collect();
        assert_eq!(stub, registered_names());
    }

    #[test]
    fn execute_exit_and_mode_reach_the_recorded_effects() {
        let lua = new_vm().unwrap();
        let mut hub = test_hub();
        let mut effects = RecordingEffects::default();
        let mut keymap = RecordingKeymap::default();
        let handler: mlua::Function = lua
            .load(
                r#"return function(a)
                    a.execute("wt")
                    a.exit()
                    a.mode("resize")
                end"#,
            )
            .eval()
            .unwrap();
        {
            let mut cxv = ActionContext {
                hub: &mut hub,
                effects: &mut effects,
                keymap_effects: &mut keymap,
            };
            let cx = RefCell::new(&mut cxv);
            lua.scope(|scope| {
                let actions = build_actions(&lua, scope, &cx)?;
                handler.call::<()>(actions)
            })
            .unwrap();
        }
        assert_eq!(effects.executed, vec!["wt".to_string()]);
        assert!(effects.exited);
        assert_eq!(keymap.switched, vec!["resize".to_string()]);
    }

    #[test]
    fn actions_revoked_after_handler_returns() {
        let lua = new_vm().unwrap();
        let mut hub = test_hub();
        let mut effects = RecordingEffects::default();
        let mut keymap = RecordingKeymap::default();
        let handler: mlua::Function = lua
            .load("return function(a) return a.focus_left end")
            .eval()
            .unwrap();
        let mut cxv = ActionContext {
            hub: &mut hub,
            effects: &mut effects,
            keymap_effects: &mut keymap,
        };
        let cx = RefCell::new(&mut cxv);
        let escaped: mlua::Function = lua
            .scope(|scope| {
                let actions = build_actions(&lua, scope, &cx)?;
                handler.call::<mlua::Function>(actions)
            })
            .unwrap();
        assert!(escaped.call::<()>(()).is_err());
    }

    #[test]
    fn close_closes_the_focused_window() {
        let (mut hub, focused) = hub_with_focused_window();
        let mut effects = RecordingEffects::default();
        call_close(&mut hub, &mut effects);
        assert_eq!(effects.closed, vec![focused]);

        let mut empty = test_hub();
        let mut effects = RecordingEffects::default();
        call_close(&mut empty, &mut effects);
        assert!(effects.closed.is_empty());
    }

    fn call_close(hub: &mut Hub, effects: &mut RecordingEffects) {
        let lua = new_vm().unwrap();
        let handler: mlua::Function = lua.load("return function(a) a.close() end").eval().unwrap();
        let mut keymap = RecordingKeymap::default();
        let mut cxv = ActionContext {
            hub,
            effects,
            keymap_effects: &mut keymap,
        };
        let cx = RefCell::new(&mut cxv);
        lua.scope(|scope| {
            let actions = build_actions(&lua, scope, &cx)?;
            handler.call::<()>(actions)
        })
        .unwrap();
    }

    fn registered_names() -> BTreeSet<&'static str> {
        let lua = new_vm().unwrap();
        let mut hub = test_hub();
        let mut effects = RecordingEffects::default();
        let mut keymap = RecordingKeymap::default();
        let mut cxv = ActionContext {
            hub: &mut hub,
            effects: &mut effects,
            keymap_effects: &mut keymap,
        };
        let cx = RefCell::new(&mut cxv);
        lua.scope(|scope| {
            let actions = build_actions(&lua, scope, &cx)?;
            actions
                .pairs::<String, mlua::Value>()
                .map(|pair| pair.map(|(key, _)| key))
                .collect::<mlua::Result<BTreeSet<String>>>()
        })
        .unwrap()
        .into_iter()
        .map(|name| &*String::leak(name))
        .collect()
    }
}
