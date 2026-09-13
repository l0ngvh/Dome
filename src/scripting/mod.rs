//! The hub thread owns the persistent Luau VM and the registered callbacks,
//! none of which are `Send`. The VM outlives each load so a function-valued
//! binding stays callable after the load that created it.

use std::cell::RefCell;

use crate::config::Config;
use crate::core::{Hub, WindowId};
use crate::keybinding::{CallbackId, KeymapPublisher};

mod actions;

use actions::build_actions;

/// Verbs that need the OS, so the neutral hub cannot serve them.
pub(crate) trait PlatformEffects {
    fn close(&mut self, id: WindowId);
    fn execute(&mut self, command: &str);
    fn exit(&mut self);
}

/// Lent to a handler for one call, as three disjoint borrows so a handler can
/// drive the hub and reach the effects and keymap at once.
pub(crate) struct ActionContext<'a> {
    pub(crate) hub: &'a mut Hub,
    pub(crate) effects: &'a mut dyn PlatformEffects,
    pub(crate) keymap: &'a mut KeymapPublisher,
}

pub(crate) struct LuaRuntime {
    lua: mlua::Lua,
    callbacks: Vec<mlua::Function>,
    config_path: String,
}

impl LuaRuntime {
    pub(crate) fn new(config_path: String) -> mlua::Result<Self> {
        Ok(Self {
            lua: crate::config::lua::new_vm()?,
            callbacks: Vec::new(),
            config_path,
        })
    }

    pub(crate) fn load(&mut self) -> Config {
        let mut callbacks: Vec<mlua::Function> = Vec::new();
        let config = match Config::load_into(&self.lua, &self.config_path, &mut callbacks) {
            Ok(config) => config,
            Err(e) => {
                log_initial_load_error(&self.config_path, &e);
                callbacks.clear();
                Config::load_default_into(&self.lua, &mut callbacks).unwrap_or_else(|e| {
                    tracing::error!(error = %e, "Failed to load bundled default config");
                    Config::default()
                })
            }
        };
        self.callbacks = callbacks;
        config
    }

    pub(crate) fn run_callback(&self, id: CallbackId, cx: &mut ActionContext) {
        // A reload can rebuild the registry while a keypress for an old id is
        // still in flight, so a miss is expected rather than a bug.
        let Some(func) = self.callbacks.get(id.0) else {
            tracing::warn!(id = id.0, "Callback id out of range, dropping");
            return;
        };
        let cx = RefCell::new(cx);
        let result = self.lua.scope(|scope| {
            let actions = build_actions(&self.lua, scope, &cx)?;
            func.call::<()>(actions)
        });
        if let Err(e) = result {
            tracing::warn!(error = %e, "Callback handler errored");
        }
    }

    pub(crate) fn reload(&mut self) -> Option<Box<Config>> {
        let mut new_callbacks: Vec<mlua::Function> = Vec::new();
        match Config::load_into(&self.lua, &self.config_path, &mut new_callbacks) {
            Ok(config) => {
                // Swap only after a successful rebuild, so a failure keeps the
                // running registry and config.
                self.callbacks = new_callbacks;
                Some(Box::new(config))
            }
            Err(e) => {
                tracing::warn!(path = %self.config_path, error = %format!("{e:#}"), "Reload failed, keeping current config");
                None
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
pub(crate) mod test_support {
    use super::PlatformEffects;
    use crate::core::{Hub, LayoutOptions, PixelRect, ReportedMonitor, WindowId};

    pub(crate) fn test_hub() -> Hub {
        let monitor = ReportedMonitor {
            device_name: "test".to_string(),
            work_area: PixelRect::new(0, 0, 1920, 1080),
            scale: 1.0,
            cg_display_id: None,
            gdi_device: None,
        };
        Hub::new(monitor, LayoutOptions::default(), Vec::new())
    }

    #[derive(Default)]
    pub(crate) struct RecordingEffects {
        pub closed: Vec<WindowId>,
        pub executed: Vec<String>,
        pub exited: bool,
    }

    impl PlatformEffects for RecordingEffects {
        fn close(&mut self, id: WindowId) {
            self.closed.push(id);
        }
        fn execute(&mut self, command: &str) {
            self.executed.push(command.to_string());
        }
        fn exit(&mut self) {
            self.exited = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinding::{KeymapPublisher, KeymapState, ModalKeymaps};

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
        assert!(out.is_none());
        assert_eq!(runtime.callbacks.len(), 1);
    }

    #[test]
    fn run_callback_drives_context() {
        let cfg = write_temp(
            "cb",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('wt') end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&cfg)).unwrap();
        runtime.load();
        let mut hub = test_support::test_hub();
        let mut effects = test_support::RecordingEffects::default();
        let (tx, _rx) = std::sync::mpsc::channel();
        let mut keymap = KeymapPublisher::new(KeymapState::new(ModalKeymaps::default()), tx);
        {
            let mut cx = ActionContext {
                hub: &mut hub,
                effects: &mut effects,
                keymap: &mut keymap,
            };
            runtime.run_callback(CallbackId(0), &mut cx);
        }
        assert_eq!(effects.executed, vec!["wt".to_string()]);
    }
}
