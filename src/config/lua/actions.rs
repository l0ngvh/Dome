use std::cell::RefCell;

use crate::core::{Direction, MonitorSelector, StrategyAction, TilingAction};

use super::ActionContext;

/// The returned handle's closures expire with `scope`, so a stashed handle
/// cannot drive the hub after the scope ends.
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
            let mut guard = cx.borrow_mut();
            let c = &mut **guard;
            c.hub.close_focused_window(c.effects);
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
    ("toggle_float", || StrategyAction::ToggleFloat.into()),
    ("toggle_fullscreen", || {
        StrategyAction::ToggleFullscreen.into()
    }),
    ("grow", || StrategyAction::Grow.into()),
    ("shrink", || StrategyAction::Shrink.into()),
    ("increase_master_count", || {
        StrategyAction::MoreMaster.into()
    }),
    ("decrease_master_count", || {
        StrategyAction::FewerMaster.into()
    }),
];

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
