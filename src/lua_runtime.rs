//! The `dome-lua` runtime thread. It owns the one persistent Luau VM and every
//! value that is not `Send`: the VM itself and the registered callback
//! functions. It talks to the rest of Dome only over plain-data channels, so
//! this module carries no platform type.
//!
//! A keybinding bound to a Lua function must stay callable after the load that
//! created it, so the VM that owns the function must outlive the load. This is
//! why config load and reload run here rather than on a fresh drop-after-load
//! VM.

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::thread::JoinHandle;

use anyhow::anyhow;

use crate::action::{
    Action, Actions, FocusTarget, MasterTarget, MonitorTarget, MoveTarget, TabDirection,
    ToggleTarget,
};
use crate::config::{
    CallbackId, Config, DEFAULT_LUA, Modifiers, load_config_into, load_default_config_into,
};

/// A message the runtime thread emits back to the platform. The platform's
/// `out` closure translates each variant into a hub event or a `KeymapState`
/// write. Only plain data crosses this boundary.
pub(crate) enum RuntimeOut {
    Actions(Actions),
    /// Mode state lives in `KeymapState`, not the hub, so a mode switch cannot
    /// ride the action path.
    SwitchMode(String),
    /// A reload produced a new config for the platform to apply.
    Reloaded(Box<Config>),
}

/// A message the platform sends into the runtime thread.
pub(crate) enum RuntimeMsg {
    RunCallback(CallbackId),
    Reload,
    Shutdown,
}

const MODIFIER_ADD_ERROR: &str = "a modifier joins only with another modifier or a key string";
const REVOKED_ERROR: &str = "this action handle is not valid outside its handler";

/// A modifier set exposed to Luau as a userdata constant. The `+` operator
/// unions two modifiers or attaches a key to produce a chord string.
#[derive(Clone, Copy)]
struct Modifier(Modifiers);

impl mlua::UserData for Modifier {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(
            mlua::MetaMethod::Add,
            |lua, this, rhs: mlua::Value| match rhs {
                mlua::Value::UserData(ud) => {
                    let other = ud
                        .borrow::<Modifier>()
                        .map_err(|_| mlua::Error::runtime(MODIFIER_ADD_ERROR))?;
                    lua.create_userdata(Modifier(this.0 | other.0))
                        .map(mlua::Value::UserData)
                }
                mlua::Value::String(key) => lua
                    .create_string(chord_string(this.0, &key.to_string_lossy()))
                    .map(mlua::Value::String),
                _ => Err(mlua::Error::runtime(MODIFIER_ADD_ERROR)),
            },
        );
    }
}

/// Build a chord string that `Keymap::from_str` accepts. `from_str` is
/// order-independent, so this order is only for a stable, readable result.
fn chord_string(mods: Modifiers, key: &str) -> String {
    let mut chord = String::new();
    for (bit, token) in [
        (Modifiers::META, "meta"),
        (Modifiers::CTRL, "ctrl"),
        (Modifiers::ALT, "alt"),
        (Modifiers::SHIFT, "shift"),
    ] {
        if mods.contains(bit) {
            chord.push_str(token);
            chord.push('+');
        }
    }
    chord.push_str(key);
    chord
}

/// Build the persistent VM with the frozen read-only surface installed once. The
/// surface persists across reloads because the same VM evaluates each reload.
pub(crate) fn build_vm() -> mlua::Result<mlua::Lua> {
    let lua = mlua::Lua::new();
    let globals = lua.globals();

    let dome = lua.create_table()?;
    dome.set(
        "os",
        if cfg!(target_os = "macos") {
            "macos"
        } else {
            "windows"
        },
    )?;
    dome.set(
        "executable",
        lua.create_function(|_, name: String| Ok(which::which(name).is_ok()))?,
    )?;
    dome.set(
        "defaults",
        lua.create_function(|lua, ()| {
            lua.load(DEFAULT_LUA)
                .set_name("dome.defaults")
                .eval::<mlua::Table>()
        })?,
    )?;
    let table_lib: mlua::Table = globals.get("table")?;
    let freeze: mlua::Function = table_lib.get("freeze")?;
    let dome: mlua::Table = freeze.call(dome)?;
    globals.set("dome", dome)?;

    globals.set("Meta", Modifier(Modifiers::META))?;
    globals.set("Alt", Modifier(Modifiers::ALT))?;
    globals.set("Ctrl", Modifier(Modifiers::CTRL))?;
    globals.set("Shift", Modifier(Modifiers::SHIFT))?;
    globals.set("Cmd", Modifier(Modifiers::META))?;
    globals.set("Win", Modifier(Modifiers::META))?;
    globals.set("Option", Modifier(Modifiers::ALT))?;
    globals.set("Opt", Modifier(Modifiers::ALT))?;
    globals.set("Control", Modifier(Modifiers::CTRL))?;

    // Sandbox after the surface is installed, so the constants and `dome` are
    // base globals the sandbox protects from in-place mutation.
    lua.sandbox(true)?;
    Ok(lua)
}

type LiveCell = Rc<Cell<bool>>;
type Sink = Rc<RefCell<Vec<RuntimeOut>>>;

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

/// A no-argument accessor that emits one fixed `Action`.
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

/// A one-string-argument accessor. The `make` closure decides whether the
/// argument becomes an action or a mode switch.
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

/// Build the mutating action capability passed to a handler as its `actions`
/// argument. Each accessor is gated by `cell` and errors once the handler
/// returns, so a stashed handle cannot drive the hub later (R10).
fn build_capability(lua: &mlua::Lua, cell: LiveCell, sink: Sink) -> mlua::Result<mlua::Table> {
    let actions = lua.create_table()?;

    let focus = lua.create_table()?;
    focus.set(
        "left",
        action_fn(lua, &cell, &sink, || Action::Focus(FocusTarget::Left))?,
    )?;
    focus.set(
        "right",
        action_fn(lua, &cell, &sink, || Action::Focus(FocusTarget::Right))?,
    )?;
    focus.set(
        "up",
        action_fn(lua, &cell, &sink, || Action::Focus(FocusTarget::Up))?,
    )?;
    focus.set(
        "down",
        action_fn(lua, &cell, &sink, || Action::Focus(FocusTarget::Down))?,
    )?;
    focus.set(
        "parent",
        action_fn(lua, &cell, &sink, || Action::Focus(FocusTarget::Parent))?,
    )?;
    focus.set(
        "workspace",
        action_fn_str(lua, &cell, &sink, |name| {
            RuntimeOut::Actions(Actions::new(vec![Action::Focus(FocusTarget::Workspace {
                name,
                monitor: None,
            })]))
        })?,
    )?;
    focus.set(
        "monitor",
        action_fn_str(lua, &cell, &sink, |s| {
            RuntimeOut::Actions(Actions::new(vec![Action::Focus(FocusTarget::Monitor {
                target: monitor_target(&s),
            })]))
        })?,
    )?;
    let tab = lua.create_table()?;
    tab.set(
        "next",
        action_fn(lua, &cell, &sink, || {
            Action::Focus(FocusTarget::Tab {
                direction: TabDirection::Next,
            })
        })?,
    )?;
    tab.set(
        "prev",
        action_fn(lua, &cell, &sink, || {
            Action::Focus(FocusTarget::Tab {
                direction: TabDirection::Prev,
            })
        })?,
    )?;
    focus.set("tab", tab)?;
    actions.set("focus", focus)?;

    let move_tbl = lua.create_table()?;
    move_tbl.set(
        "left",
        action_fn(lua, &cell, &sink, || Action::Move(MoveTarget::Left))?,
    )?;
    move_tbl.set(
        "right",
        action_fn(lua, &cell, &sink, || Action::Move(MoveTarget::Right))?,
    )?;
    move_tbl.set(
        "up",
        action_fn(lua, &cell, &sink, || Action::Move(MoveTarget::Up))?,
    )?;
    move_tbl.set(
        "down",
        action_fn(lua, &cell, &sink, || Action::Move(MoveTarget::Down))?,
    )?;
    move_tbl.set(
        "workspace",
        action_fn_str(lua, &cell, &sink, |name| {
            RuntimeOut::Actions(Actions::new(vec![Action::Move(MoveTarget::Workspace {
                name,
                monitor: None,
            })]))
        })?,
    )?;
    move_tbl.set(
        "monitor",
        action_fn_str(lua, &cell, &sink, |s| {
            RuntimeOut::Actions(Actions::new(vec![Action::Move(MoveTarget::Monitor {
                target: monitor_target(&s),
            })]))
        })?,
    )?;
    actions.set("move", move_tbl)?;

    let toggle = lua.create_table()?;
    toggle.set(
        "spawn",
        action_fn(lua, &cell, &sink, || Action::Toggle(ToggleTarget::Spawn))?,
    )?;
    toggle.set(
        "direction",
        action_fn(lua, &cell, &sink, || {
            Action::Toggle(ToggleTarget::Direction)
        })?,
    )?;
    toggle.set(
        "layout",
        action_fn(lua, &cell, &sink, || Action::Toggle(ToggleTarget::Layout))?,
    )?;
    toggle.set(
        "float",
        action_fn(lua, &cell, &sink, || Action::Toggle(ToggleTarget::Float))?,
    )?;
    toggle.set(
        "fullscreen",
        action_fn(lua, &cell, &sink, || {
            Action::Toggle(ToggleTarget::Fullscreen)
        })?,
    )?;
    actions.set("toggle", toggle)?;

    let master = lua.create_table()?;
    master.set(
        "grow",
        action_fn(lua, &cell, &sink, || Action::Master(MasterTarget::Grow))?,
    )?;
    master.set(
        "shrink",
        action_fn(lua, &cell, &sink, || Action::Master(MasterTarget::Shrink))?,
    )?;
    master.set(
        "more",
        action_fn(lua, &cell, &sink, || Action::Master(MasterTarget::More))?,
    )?;
    master.set(
        "fewer",
        action_fn(lua, &cell, &sink, || Action::Master(MasterTarget::Fewer))?,
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
        action_fn_str(lua, &cell, &sink, RuntimeOut::SwitchMode)?,
    )?;

    Ok(actions)
}

/// Spawn the runtime thread. It builds the VM, does the initial load on the new
/// thread, and returns the initial `Config` before entering the message loop.
/// The only error is a failure to spawn the thread itself. A bad or missing
/// config file yields the bundled default config, matching the startup fallback.
pub(crate) fn spawn(
    config_path: String,
    out: Box<dyn Fn(RuntimeOut) + Send>,
) -> anyhow::Result<(JoinHandle<()>, Sender<RuntimeMsg>, Config)> {
    let (msg_tx, msg_rx) = std::sync::mpsc::channel::<RuntimeMsg>();
    let (init_tx, init_rx) = std::sync::mpsc::channel::<Config>();
    let handle = std::thread::Builder::new()
        .name("dome-lua".to_owned())
        .spawn(move || run(config_path, out, &msg_rx, &init_tx))?;
    let config = init_rx
        .recv()
        .map_err(|_| anyhow!("dome-lua thread exited before the initial config load"))?;
    Ok((handle, msg_tx, config))
}

fn run(
    config_path: String,
    out: Box<dyn Fn(RuntimeOut) + Send>,
    msg_rx: &Receiver<RuntimeMsg>,
    init_tx: &Sender<Config>,
) {
    let lua = match build_vm() {
        Ok(lua) => lua,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build the Lua VM, Lua config is disabled");
            init_tx.send(Config::default()).ok();
            return;
        }
    };

    let mut callbacks: Vec<mlua::Function> = Vec::new();
    let config = match load_config_into(&lua, &config_path, &mut callbacks) {
        Ok(config) => config,
        Err(e) => {
            log_initial_load_error(&config_path, &e);
            callbacks.clear();
            load_default_config_into(&lua, &mut callbacks).unwrap_or_else(|e| {
                tracing::error!(error = %e, "Failed to load bundled default config");
                Config::default()
            })
        }
    };
    init_tx.send(config).ok();

    while let Ok(msg) = msg_rx.recv() {
        match msg {
            RuntimeMsg::RunCallback(id) => run_callback(&lua, &callbacks, id, out.as_ref()),
            RuntimeMsg::Reload => reload(&lua, &config_path, &mut callbacks, out.as_ref()),
            RuntimeMsg::Shutdown => break,
        }
    }
}

fn run_callback(
    lua: &mlua::Lua,
    callbacks: &[mlua::Function],
    id: CallbackId,
    out: &dyn Fn(RuntimeOut),
) {
    // A reload can rebuild the registry while a keypress for an old id is still
    // in flight, so a stale id is expected rather than a bug.
    let Some(func) = callbacks.get(id.0) else {
        tracing::warn!(id = id.0, "Callback id out of range, dropping");
        return;
    };
    let cell: LiveCell = Rc::new(Cell::new(true));
    let sink: Sink = Rc::new(RefCell::new(Vec::new()));
    let capability = match build_capability(lua, cell.clone(), sink.clone()) {
        Ok(capability) => capability,
        Err(e) => {
            tracing::error!(error = %e, "Failed to build the action capability");
            return;
        }
    };
    let result: mlua::Result<()> = func.call(capability);
    cell.set(false);
    if let Err(e) = result {
        tracing::warn!(error = %e, "Callback handler errored");
    }
    for msg in sink.borrow_mut().drain(..) {
        out(msg);
    }
}

fn reload(
    lua: &mlua::Lua,
    config_path: &str,
    callbacks: &mut Vec<mlua::Function>,
    out: &dyn Fn(RuntimeOut),
) {
    let mut new_callbacks: Vec<mlua::Function> = Vec::new();
    match load_config_into(lua, config_path, &mut new_callbacks) {
        Ok(config) => {
            // Swap only after a successful rebuild, so a failure keeps the
            // running registry and config (R8).
            *callbacks = new_callbacks;
            out(RuntimeOut::Reloaded(Box::new(config)));
        }
        Err(e) => {
            tracing::warn!(path = %config_path, error = %format!("{e:#}"), "Reload failed, keeping current config");
        }
    }
}

fn log_initial_load_error(path: &str, e: &anyhow::Error) {
    if e.downcast_ref::<std::io::Error>()
        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
    {
        tracing::info!(%path, "Config file not found, using defaults");
    } else {
        tracing::warn!(%path, error = %format!("{e:#}"), "Failed to load config, using defaults");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(std::path::PathBuf);
    impl Drop for TempFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }
    fn write_temp(tag: &str, src: &str) -> TempFile {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("dome_rt_{tag}_{nanos}.lua"));
        std::fs::write(&path, src).unwrap();
        TempFile(path)
    }
    fn path_str(f: &TempFile) -> String {
        f.0.to_str().unwrap().to_string()
    }

    #[test]
    fn modifier_add_unions_and_attaches_key() {
        let lua = build_vm().unwrap();
        let one: String = lua.load(r#"return Meta + "h""#).eval().unwrap();
        assert_eq!(one, "meta+h");
        let two: String = lua.load(r#"return (Meta + Ctrl) + "y""#).eval().unwrap();
        assert_eq!(two, "meta+ctrl+y");
        let four: String = lua
            .load(r#"return (Meta + Alt + Ctrl + Shift) + "x""#)
            .eval()
            .unwrap();
        assert_eq!(four, "meta+ctrl+alt+shift+x");
    }

    #[test]
    fn modifier_second_key_errors() {
        let lua = build_vm().unwrap();
        assert!(
            lua.load(r#"return Meta + "h" + "j""#)
                .eval::<String>()
                .is_err()
        );
    }

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
        assert!(matches!(&out[7], RuntimeOut::SwitchMode(name) if name == "resize"));
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

    #[test]
    fn dome_surface_is_frozen() {
        let lua = build_vm().unwrap();
        let os: String = lua.load("return dome.os").eval().unwrap();
        assert!(os == "macos" || os == "windows");
        assert!(lua.load(r#"dome.os = "x""#).exec().is_err());
        assert!(lua.load("dome.new_field = 1").exec().is_err());
    }

    #[test]
    fn executable_query_returns_boolean() {
        let lua = build_vm().unwrap();
        let missing: bool = lua
            .load(r#"return dome.executable("dome-no-such-binary-xyzzy")"#)
            .eval()
            .unwrap();
        assert!(!missing);

        #[cfg(unix)]
        {
            let present: bool = lua
                .load(r#"return dome.executable("/bin/sh")"#)
                .eval()
                .unwrap();
            assert!(present);
        }
    }

    #[test]
    fn reload_keeps_last_good_on_error() {
        let lua = build_vm().unwrap();
        let good = write_temp(
            "good",
            "return { keymaps = { ['meta+c'] = function() end } }",
        );
        let mut callbacks = Vec::new();
        load_config_into(&lua, &path_str(&good), &mut callbacks).unwrap();
        assert_eq!(callbacks.len(), 1);

        let bad = write_temp("bad", "this is not lua {{{");
        reload(&lua, &path_str(&bad), &mut callbacks, &|_| {});
        assert_eq!(callbacks.len(), 1);
    }

    #[test]
    fn load_default_config_provides_default_keymaps() {
        let lua = build_vm().unwrap();
        let mut callbacks = Vec::new();
        let config = load_default_config_into(&lua, &mut callbacks).unwrap();
        assert_eq!(config.keymaps.default.len(), 44);
        assert!(callbacks.is_empty());
    }

    #[test]
    fn dome_defaults_returns_a_fresh_mutable_table() {
        let lua = build_vm().unwrap();
        let leaked: bool = lua
            .load(
                r#"local a = dome.defaults()
a.keymaps["meta+x"] = "close"
local b = dome.defaults()
return b.keymaps["meta+x"] ~= nil"#,
            )
            .eval()
            .unwrap();
        assert!(!leaked);
    }
}
