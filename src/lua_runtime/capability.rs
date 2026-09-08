//! Turns a handler's action calls into `Action`s on a per-call sink.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::action::{
    Action, Actions, FocusTarget, MasterTarget, MonitorTarget, MoveTarget, TabDirection,
    ToggleTarget,
};

use super::RuntimeOut;

const REVOKED_ERROR: &str = "this action handle is not valid outside its handler";

pub(super) type LiveCell = Rc<Cell<bool>>;
pub(super) type Sink = Rc<RefCell<Vec<RuntimeOut>>>;

fn guard(cell: &LiveCell) -> mlua::Result<()> {
    if cell.get() {
        Ok(())
    } else {
        Err(mlua::Error::runtime(REVOKED_ERROR))
    }
}

fn monitor_target(s: &str) -> MonitorTarget {
    match s {
        "up" => MonitorTarget::Up,
        "down" => MonitorTarget::Down,
        "left" => MonitorTarget::Left,
        "right" => MonitorTarget::Right,
        other => MonitorTarget::Name(other.to_string()),
    }
}

fn action_fn(
    lua: &mlua::Lua,
    cell: &LiveCell,
    sink: &Sink,
    make: impl Fn() -> Action + 'static,
) -> mlua::Result<mlua::Function> {
    let cell = cell.clone();
    let sink = sink.clone();
    lua.create_function(move |_, ()| {
        guard(&cell)?;
        sink.borrow_mut()
            .push(RuntimeOut::Actions(Actions::new(vec![make()])));
        Ok(())
    })
}

fn action_fn_str(
    lua: &mlua::Lua,
    cell: &LiveCell,
    sink: &Sink,
    make: impl Fn(String) -> RuntimeOut + 'static,
) -> mlua::Result<mlua::Function> {
    let cell = cell.clone();
    let sink = sink.clone();
    lua.create_function(move |_, arg: String| {
        guard(&cell)?;
        sink.borrow_mut().push(make(arg));
        Ok(())
    })
}

/// Accessors are gated by `cell` and error once the handler returns, so a
/// stashed handle cannot drive the hub later (R10).
pub(super) fn build_capability(
    lua: &mlua::Lua,
    cell: LiveCell,
    sink: Sink,
) -> mlua::Result<mlua::Table> {
    let actions = lua.create_table()?;

    let focus = lua.create_table()?;
    focus.set(
        "left",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Left,
        })?,
    )?;
    focus.set(
        "right",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Right,
        })?,
    )?;
    focus.set(
        "up",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Up,
        })?,
    )?;
    focus.set(
        "down",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Down,
        })?,
    )?;
    focus.set(
        "parent",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Parent,
        })?,
    )?;
    focus.set(
        "workspace",
        action_fn_str(lua, &cell, &sink, |name| {
            RuntimeOut::Actions(Actions::new(vec![Action::Focus {
                target: FocusTarget::Workspace {
                    name,
                    monitor: None,
                },
            }]))
        })?,
    )?;
    focus.set(
        "monitor",
        action_fn_str(lua, &cell, &sink, |s| {
            RuntimeOut::Actions(Actions::new(vec![Action::Focus {
                target: FocusTarget::Monitor {
                    target: monitor_target(&s),
                },
            }]))
        })?,
    )?;
    let tab = lua.create_table()?;
    tab.set(
        "next",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Tab {
                direction: TabDirection::Next,
            },
        })?,
    )?;
    tab.set(
        "prev",
        action_fn(lua, &cell, &sink, || Action::Focus {
            target: FocusTarget::Tab {
                direction: TabDirection::Prev,
            },
        })?,
    )?;
    focus.set("tab", tab)?;
    actions.set("focus", focus)?;

    let move_tbl = lua.create_table()?;
    move_tbl.set(
        "left",
        action_fn(lua, &cell, &sink, || Action::Move {
            target: MoveTarget::Left,
        })?,
    )?;
    move_tbl.set(
        "right",
        action_fn(lua, &cell, &sink, || Action::Move {
            target: MoveTarget::Right,
        })?,
    )?;
    move_tbl.set(
        "up",
        action_fn(lua, &cell, &sink, || Action::Move {
            target: MoveTarget::Up,
        })?,
    )?;
    move_tbl.set(
        "down",
        action_fn(lua, &cell, &sink, || Action::Move {
            target: MoveTarget::Down,
        })?,
    )?;
    move_tbl.set(
        "workspace",
        action_fn_str(lua, &cell, &sink, |name| {
            RuntimeOut::Actions(Actions::new(vec![Action::Move {
                target: MoveTarget::Workspace {
                    name,
                    monitor: None,
                },
            }]))
        })?,
    )?;
    move_tbl.set(
        "monitor",
        action_fn_str(lua, &cell, &sink, |s| {
            RuntimeOut::Actions(Actions::new(vec![Action::Move {
                target: MoveTarget::Monitor {
                    target: monitor_target(&s),
                },
            }]))
        })?,
    )?;
    actions.set("move", move_tbl)?;

    let toggle = lua.create_table()?;
    toggle.set(
        "spawn",
        action_fn(lua, &cell, &sink, || Action::Toggle {
            target: ToggleTarget::Spawn,
        })?,
    )?;
    toggle.set(
        "direction",
        action_fn(lua, &cell, &sink, || Action::Toggle {
            target: ToggleTarget::Direction,
        })?,
    )?;
    toggle.set(
        "layout",
        action_fn(lua, &cell, &sink, || Action::Toggle {
            target: ToggleTarget::Layout,
        })?,
    )?;
    toggle.set(
        "float",
        action_fn(lua, &cell, &sink, || Action::Toggle {
            target: ToggleTarget::Float,
        })?,
    )?;
    toggle.set(
        "fullscreen",
        action_fn(lua, &cell, &sink, || Action::Toggle {
            target: ToggleTarget::Fullscreen,
        })?,
    )?;
    actions.set("toggle", toggle)?;

    let master = lua.create_table()?;
    master.set(
        "grow",
        action_fn(lua, &cell, &sink, || Action::Master {
            target: MasterTarget::Grow,
        })?,
    )?;
    master.set(
        "shrink",
        action_fn(lua, &cell, &sink, || Action::Master {
            target: MasterTarget::Shrink,
        })?,
    )?;
    master.set(
        "more",
        action_fn(lua, &cell, &sink, || Action::Master {
            target: MasterTarget::More,
        })?,
    )?;
    master.set(
        "fewer",
        action_fn(lua, &cell, &sink, || Action::Master {
            target: MasterTarget::Fewer,
        })?,
    )?;
    actions.set("master", master)?;

    actions.set(
        "exec",
        action_fn_str(lua, &cell, &sink, |command| {
            RuntimeOut::Actions(Actions::new(vec![Action::Exec { command }]))
        })?,
    )?;
    actions.set("close", action_fn(lua, &cell, &sink, || Action::Close)?)?;
    actions.set("exit", action_fn(lua, &cell, &sink, || Action::Exit)?)?;
    actions.set(
        "mode",
        action_fn_str(lua, &cell, &sink, |name| {
            RuntimeOut::Actions(Actions::new(vec![Action::Mode { name }]))
        })?,
    )?;

    Ok(actions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lua_runtime::build_vm;

    #[test]
    fn capability_maps_each_action_group() {
        let lua = build_vm().unwrap();
        let cell: LiveCell = Rc::new(Cell::new(true));
        let sink: Sink = Rc::new(RefCell::new(Vec::new()));
        let handler: mlua::Function = lua
            .load(
                r#"return function(a)
                    a.focus.left()
                    a.focus.workspace("3")
                    a.move.monitor("left")
                    a.toggle.float()
                    a.master.grow()
                    a.exec("wt")
                    a.close()
                    a.mode("resize")
                end"#,
            )
            .eval()
            .unwrap();
        let capability = build_capability(&lua, cell.clone(), sink.clone()).unwrap();
        handler.call::<()>(capability).unwrap();
        cell.set(false);
        let out = sink.borrow();
        assert_eq!(out.len(), 8);
        assert!(matches!(&out[0], RuntimeOut::Actions(a) if a.to_string() == "[focus left]"));
        assert!(
            matches!(&out[1], RuntimeOut::Actions(a) if a.to_string() == "[focus workspace 3]")
        );
        assert!(
            matches!(&out[2], RuntimeOut::Actions(a) if a.to_string() == "[move monitor left]")
        );
        assert!(matches!(&out[3], RuntimeOut::Actions(a) if a.to_string() == "[toggle float]"));
        assert!(matches!(&out[4], RuntimeOut::Actions(a) if a.to_string() == "[master grow]"));
        assert!(matches!(&out[5], RuntimeOut::Actions(a) if a.to_string() == "[exec wt]"));
        assert!(matches!(&out[6], RuntimeOut::Actions(a) if a.to_string() == "[close]"));
        assert!(matches!(&out[7], RuntimeOut::Actions(a) if a.to_string() == "[mode resize]"));
    }

    #[test]
    fn capability_revoked_after_handler_returns() {
        let lua = build_vm().unwrap();
        let cell: LiveCell = Rc::new(Cell::new(true));
        let sink: Sink = Rc::new(RefCell::new(Vec::new()));
        let handler: mlua::Function = lua
            .load("return function(a) return a.focus.left end")
            .eval()
            .unwrap();
        let capability = build_capability(&lua, cell.clone(), sink.clone()).unwrap();
        let escaped: mlua::Function = handler.call(capability).unwrap();
        cell.set(false);
        assert!(escaped.call::<()>(()).is_err());
        assert!(sink.borrow().is_empty());
    }
}
