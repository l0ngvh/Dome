//! The hub thread owns the persistent Luau VM and the registered callbacks,
//! none of which are `Send`. The VM outlives each load so a function-valued
//! binding stays callable after the load that created it.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::action::Actions;
use crate::config::{CallbackId, Config, load_config_into, load_default_config_into};

mod capability;
mod vm;

use capability::{LiveCell, Sink, build_capability};
pub(crate) use vm::build_vm;

pub(crate) enum RuntimeOut {
    Actions(Actions),
    Reloaded(Box<Config>),
}

pub(crate) struct LuaRuntime {
    lua: mlua::Lua,
    callbacks: Vec<mlua::Function>,
    config_path: String,
}

impl LuaRuntime {
    pub(crate) fn new(config_path: String) -> mlua::Result<Self> {
        Ok(Self {
            lua: build_vm()?,
            callbacks: Vec::new(),
            config_path,
        })
    }

    pub(crate) fn load(&mut self) -> Config {
        let mut callbacks: Vec<mlua::Function> = Vec::new();
        let config = match load_config_into(&self.lua, &self.config_path, &mut callbacks) {
            Ok(config) => config,
            Err(e) => {
                log_initial_load_error(&self.config_path, &e);
                callbacks.clear();
                load_default_config_into(&self.lua, &mut callbacks).unwrap_or_else(|e| {
                    tracing::error!(error = %e, "Failed to load bundled default config");
                    Config::default()
                })
            }
        };
        self.callbacks = callbacks;
        config
    }

    pub(crate) fn run_callback(&self, id: CallbackId) -> Vec<RuntimeOut> {
        // A reload can rebuild the registry while a keypress for an old id is
        // still in flight, so a miss is expected rather than a bug.
        let Some(func) = self.callbacks.get(id.0) else {
            tracing::warn!(id = id.0, "Callback id out of range, dropping");
            return Vec::new();
        };
        let cell: LiveCell = Rc::new(Cell::new(true));
        let sink: Sink = Rc::new(RefCell::new(Vec::new()));
        let capability = match build_capability(&self.lua, cell.clone(), sink.clone()) {
            Ok(capability) => capability,
            Err(e) => {
                tracing::error!(error = %e, "Failed to build the action capability");
                return Vec::new();
            }
        };
        let result: mlua::Result<()> = func.call(capability);
        cell.set(false);
        if let Err(e) = result {
            tracing::warn!(error = %e, "Callback handler errored");
        }
        std::mem::take(&mut *sink.borrow_mut())
    }

    pub(crate) fn reload(&mut self) -> Vec<RuntimeOut> {
        let mut new_callbacks: Vec<mlua::Function> = Vec::new();
        match load_config_into(&self.lua, &self.config_path, &mut new_callbacks) {
            Ok(config) => {
                // Swap only after a successful rebuild, so a failure keeps the
                // running registry and config (R8).
                self.callbacks = new_callbacks;
                vec![RuntimeOut::Reloaded(Box::new(config))]
            }
            Err(e) => {
                tracing::warn!(path = %self.config_path, error = %format!("{e:#}"), "Reload failed, keeping current config");
                Vec::new()
            }
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
    fn reload_keeps_last_good_on_error() {
        let good = write_temp(
            "good",
            "return { keymaps = { main = { ['meta+c'] = function() end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&good)).unwrap();
        runtime.load();
        assert_eq!(runtime.callbacks.len(), 1);

        std::fs::write(&good.0, "this is not lua {{{").unwrap();
        let out = runtime.reload();
        assert!(out.is_empty());
        assert_eq!(runtime.callbacks.len(), 1);
    }

    #[test]
    fn run_callback_returns_actions() {
        let cfg = write_temp(
            "cb",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.focus.left() end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&cfg)).unwrap();
        runtime.load();
        let out = runtime.run_callback(CallbackId(0));
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], RuntimeOut::Actions(a) if a.to_string() == "[focus left]"));
    }

    #[test]
    fn load_default_config_provides_default_keymaps() {
        let lua = build_vm().unwrap();
        let mut callbacks = Vec::new();
        let config = load_default_config_into(&lua, &mut callbacks).unwrap();
        assert_eq!(config.keymaps.modes["main"].len(), 44);
        assert_eq!(callbacks.len(), 44);
    }
}
