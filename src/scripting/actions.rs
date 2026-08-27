//! Builds the per-call `actions` handle. Each leaf runs its effect inline
//! against the live `ActionContext`, and `Lua::scope` expires the closures when
//! the handler returns, so a stashed handle cannot drive the hub later.

use std::cell::RefCell;

use crate::core::{Direction, MonitorSelector, StrategyAction, TilingAction};

use super::ActionContext;

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

    let focus = lua.create_table()?;
    focus.set(
        "left",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusDirection {
                    direction: Direction::Horizontal,
                    forward: false,
                });
            Ok(())
        })?,
    )?;
    focus.set(
        "right",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusDirection {
                    direction: Direction::Horizontal,
                    forward: true,
                });
            Ok(())
        })?,
    )?;
    focus.set(
        "up",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusDirection {
                    direction: Direction::Vertical,
                    forward: false,
                });
            Ok(())
        })?,
    )?;
    focus.set(
        "down",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusDirection {
                    direction: Direction::Vertical,
                    forward: true,
                });
            Ok(())
        })?,
    )?;
    focus.set(
        "parent",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusParent);
            Ok(())
        })?,
    )?;
    focus.set(
        "workspace",
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
    focus.set(
        "monitor",
        scope.create_function(move |_, s: String| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(TilingAction::FocusMonitor {
                    selector: monitor_selector(&s),
                });
            Ok(())
        })?,
    )?;
    let tab = lua.create_table()?;
    tab.set(
        "next",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusTab { forward: true });
            Ok(())
        })?,
    )?;
    tab.set(
        "prev",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FocusTab { forward: false });
            Ok(())
        })?,
    )?;
    focus.set("tab", tab)?;
    actions.set("focus", focus)?;

    let move_tbl = lua.create_table()?;
    move_tbl.set(
        "left",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::MoveDirection {
                    direction: Direction::Horizontal,
                    forward: false,
                });
            Ok(())
        })?,
    )?;
    move_tbl.set(
        "right",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::MoveDirection {
                    direction: Direction::Horizontal,
                    forward: true,
                });
            Ok(())
        })?,
    )?;
    move_tbl.set(
        "up",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::MoveDirection {
                    direction: Direction::Vertical,
                    forward: false,
                });
            Ok(())
        })?,
    )?;
    move_tbl.set(
        "down",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::MoveDirection {
                    direction: Direction::Vertical,
                    forward: true,
                });
            Ok(())
        })?,
    )?;
    move_tbl.set(
        "workspace",
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
    move_tbl.set(
        "monitor",
        scope.create_function(move |_, s: String| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(TilingAction::MoveToMonitor {
                    selector: monitor_selector(&s),
                });
            Ok(())
        })?,
    )?;
    actions.set("move", move_tbl)?;

    let toggle = lua.create_table()?;
    toggle.set(
        "spawn",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::ToggleSpawnMode);
            Ok(())
        })?,
    )?;
    toggle.set(
        "direction",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::ToggleDirection);
            Ok(())
        })?,
    )?;
    toggle.set(
        "layout",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::ToggleContainerLayout);
            Ok(())
        })?,
    )?;
    toggle.set(
        "float",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(TilingAction::ToggleFloat);
            Ok(())
        })?,
    )?;
    toggle.set(
        "fullscreen",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(TilingAction::ToggleFullscreen);
            Ok(())
        })?,
    )?;
    actions.set("toggle", toggle)?;

    let master = lua.create_table()?;
    master.set(
        "grow",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::GrowMaster);
            Ok(())
        })?,
    )?;
    master.set(
        "shrink",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::ShrinkMaster);
            Ok(())
        })?,
    )?;
    master.set(
        "more",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::MoreMaster);
            Ok(())
        })?,
    )?;
    master.set(
        "fewer",
        scope.create_function(move |_, ()| {
            cx.borrow_mut()
                .hub
                .handle_tiling_action(StrategyAction::FewerMaster);
            Ok(())
        })?,
    )?;
    actions.set("master", master)?;

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
            cx.borrow_mut().keymap.switch_mode(&name);
            Ok(())
        })?,
    )?;

    Ok(actions)
}

fn monitor_selector(s: &str) -> MonitorSelector {
    match s {
        "up" => MonitorSelector::Up,
        "down" => MonitorSelector::Down,
        "left" => MonitorSelector::Left,
        "right" => MonitorSelector::Right,
        other => MonitorSelector::Name(other.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::lua::new_vm;
    use crate::keybinding::{KeymapPublisher, KeymapState, ModalKeymaps};
    use crate::scripting::test_support::{RecordingEffects, test_hub};

    #[test]
    fn actions_drive_context() {
        let lua = new_vm().unwrap();
        let mut hub = test_hub();
        let mut effects = RecordingEffects::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut keymap = KeymapPublisher::new(KeymapState::new(ModalKeymaps::default()), tx);
        let handler: mlua::Function = lua
            .load(
                r#"return function(a)
                    a.focus.left()
                    a.move.monitor("left")
                    a.toggle.float()
                    a.master.grow()
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
                keymap: &mut keymap,
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
    }

    #[test]
    fn actions_revoked_after_handler_returns() {
        let lua = new_vm().unwrap();
        let mut hub = test_hub();
        let mut effects = RecordingEffects::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut keymap = KeymapPublisher::new(KeymapState::new(ModalKeymaps::default()), tx);
        let handler: mlua::Function = lua
            .load("return function(a) return a.focus.left end")
            .eval()
            .unwrap();
        let mut cxv = ActionContext {
            hub: &mut hub,
            effects: &mut effects,
            keymap: &mut keymap,
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
}
