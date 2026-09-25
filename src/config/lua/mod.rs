//! The hub thread owns the persistent Luau VM and the registered callbacks,
//! none of which are `Send`. The VM outlives each load so a function-valued
//! binding stays callable after the load that created it.

use std::cell::RefCell;

use super::{Config, Keystroke, ModalKeymaps};
use crate::core::{Hub, WindowId};

mod actions;
pub(crate) mod deserializer;
mod globals;

use actions::build_actions;
pub(crate) use globals::new_vm;

pub(super) fn evaluate<T: deserializer::FromLuaValue>(
    lua: &mlua::Lua,
    name: &str,
    src: &str,
) -> mlua::Result<T> {
    let mut cx = deserializer::LoadContext::new();
    evaluate_with(lua, name, src, &mut cx)
}

/// The caller owns the `LoadContext` and can reuse it across calls, so later
/// warnings share the same field-path root.
pub(super) fn evaluate_with<T: deserializer::FromLuaValue>(
    lua: &mlua::Lua,
    name: &str,
    src: &str,
    cx: &mut deserializer::LoadContext,
) -> mlua::Result<T> {
    let value: mlua::Value = lua.load(src).set_name(name).eval()?;
    T::from_lua_value(&value, cx)
}

// Raise through Lua's `error` at level 2 so the message names the config line
// that made the call. An mlua::Error carries no position.
pub(super) fn caller_error(lua: &mlua::Lua, message: &str) -> mlua::Error {
    let raised = lua
        .globals()
        .get::<mlua::Function>("error")
        .and_then(|error| error.call::<()>((message, 2)));
    match raised {
        Err(error) => error,
        Ok(()) => mlua::Error::runtime(message),
    }
}

/// Verbs that need the OS, so the neutral hub cannot serve them.
pub(crate) trait PlatformEffects {
    fn close(&mut self, id: WindowId);
    fn execute(&mut self, command: &str);
    fn exit(&mut self);
}

/// Verbs that need the keyboard thread, so the config layer cannot serve them.
pub(crate) trait KeymapEffects {
    fn switch_mode(&mut self, name: &str);
    fn update_keymaps(&mut self, keymaps: &ModalKeymaps);
}

pub(crate) struct ActionContext<'a> {
    pub(crate) hub: &'a mut Hub,
    pub(crate) effects: &'a mut dyn PlatformEffects,
    pub(crate) keymap_effects: &'a mut dyn KeymapEffects,
}

/// Owns the VM and the keymaps together, so a reload swaps both or neither.
pub(crate) struct KeymapRuntime {
    runtime: LuaRuntime,
    keymap_effects: Box<dyn KeymapEffects>,
}

impl KeymapRuntime {
    pub(crate) fn new(runtime: LuaRuntime, keymap_effects: Box<dyn KeymapEffects>) -> Self {
        Self {
            runtime,
            keymap_effects,
        }
    }

    pub(crate) fn dispatch(
        &mut self,
        keymap: &str,
        keystroke: &Keystroke,
        hub: &mut Hub,
        effects: &mut dyn PlatformEffects,
    ) {
        let mut cx = ActionContext {
            hub,
            effects,
            keymap_effects: self.keymap_effects.as_mut(),
        };
        self.runtime.run_binding(keymap, keystroke, &mut cx);
    }

    pub(crate) fn reload(&mut self) -> Option<Box<Config>> {
        let config = self.runtime.reload()?;
        self.keymap_effects.update_keymaps(&config.keymaps);
        Some(config)
    }

    pub(crate) fn switch_mode(&mut self, name: &str) {
        self.keymap_effects.switch_mode(name);
    }
}

pub(crate) struct LuaRuntime {
    lua: mlua::Lua,
    keymaps: ModalKeymaps,
    config_path: String,
}

impl LuaRuntime {
    pub(crate) fn new(config_path: String) -> mlua::Result<Self> {
        Ok(Self {
            lua: new_vm()?,
            keymaps: ModalKeymaps::default(),
            config_path,
        })
    }

    pub(crate) fn load(&mut self) -> Config {
        let config = match Config::load(&self.lua, &self.config_path) {
            Ok(config) => config,
            Err(e) => {
                log_initial_load_error(&self.config_path, &e);
                // Every load already reads the bundled file for its defaults, so
                // a failure here means that file is broken.
                Config::load_default(&self.lua).expect("the bundled default config must load")
            }
        };
        self.keymaps = config.keymaps.clone();
        config
    }

    fn run_binding(&self, keymap: &str, keystroke: &Keystroke, cx: &mut ActionContext) {
        // A reload between the keypress and here can drop the binding, so a miss
        // is expected rather than a bug.
        let Some(func) = self
            .keymaps
            .modes
            .get(keymap)
            .and_then(|b| b.get(keystroke))
        else {
            tracing::warn!(%keymap, %keystroke, "Binding is gone, dropping");
            return;
        };
        let cx = RefCell::new(cx);
        let result = self.lua.scope(|scope| {
            let actions = build_actions(&self.lua, scope, &cx)?;
            func.call::<()>(actions)
        });
        if let Err(e) = result {
            tracing::warn!(%keymap, %keystroke, error = %e, "Callback handler errored");
        }
    }

    fn reload(&mut self) -> Option<Box<Config>> {
        match Config::load(&self.lua, &self.config_path) {
            Ok(config) => {
                self.keymaps = config.keymaps.clone();
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
    use super::{KeymapEffects, PlatformEffects};
    use crate::config::ModalKeymaps;
    use crate::core::{
        Hub, PixelRect, PreferredLayouts, ReportedMonitor, WindowId, WindowMatcher, WindowMetadata,
        WindowRestrictions,
    };

    pub(crate) fn test_hub() -> Hub {
        let monitor = ReportedMonitor {
            device_name: "test".to_string(),
            work_area: PixelRect::new(0, 0, 1920, 1080),
            scale: 1.0,
            cg_display_id: None,
            gdi_device: None,
        };
        Hub::new(
            monitor,
            crate::config::tests::tiling_config(),
            PreferredLayouts::default(),
        )
    }

    /// The inserted window is the workspace's only one, so it holds the focus.
    pub(crate) fn hub_with_focused_window() -> (Hub, WindowId) {
        let mut hub = test_hub();
        let id = hub
            .insert_window(
                Box::new(TestMetadata),
                PixelRect::new(0, 0, 800, 600),
                WindowRestrictions::None,
            )
            .expect("the test window should insert");
        (hub, id)
    }

    #[derive(Debug, Clone)]
    pub(crate) struct TestMetadata;

    impl std::fmt::Display for TestMetadata {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            f.write_str("test window")
        }
    }

    impl WindowMetadata for TestMetadata {
        fn app_name(&self) -> Option<String> {
            None
        }
        fn title(&self) -> Option<&str> {
            None
        }
        fn set_title(&mut self, _title: String) {}
        fn clone_box(&self) -> Box<dyn WindowMetadata> {
            Box::new(self.clone())
        }
        fn matches_window_matcher(&self, _matcher: &WindowMatcher) -> bool {
            false
        }
        fn to_window_matcher(&self) -> WindowMatcher {
            WindowMatcher::default()
        }
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

    #[derive(Default)]
    pub(crate) struct RecordingKeymap {
        pub switched: Vec<String>,
    }

    impl KeymapEffects for RecordingKeymap {
        fn switch_mode(&mut self, name: &str) {
            self.switched.push(name.to_string());
        }
        fn update_keymaps(&mut self, _keymaps: &ModalKeymaps) {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    use crate::config::BASE_MODE;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    use crate::platform::keymap::{KeymapPublisher, KeymapView};

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

    fn keymap_runtime(runtime: LuaRuntime) -> KeymapRuntime {
        KeymapRuntime::new(runtime, Box::new(test_support::RecordingKeymap::default()))
    }

    fn meta_c() -> Keystroke {
        "meta+c".parse().unwrap()
    }

    #[test]
    fn reload_keeps_last_good_bindings_runnable_on_error() {
        let good = write_temp(
            "good",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('ok') end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&good)).unwrap();
        runtime.load();
        let mut keymaps = keymap_runtime(runtime);

        std::fs::write(&good.0, "this is not lua {{{").unwrap();
        assert!(keymaps.reload().is_none());

        let mut hub = test_support::test_hub();
        let mut effects = test_support::RecordingEffects::default();
        keymaps.dispatch("main", &meta_c(), &mut hub, &mut effects);
        assert_eq!(effects.executed, vec!["ok".to_string()]);
    }

    #[test]
    fn dispatch_drives_context() {
        let cfg = write_temp(
            "cb",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('wt') end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&cfg)).unwrap();
        runtime.load();
        let mut keymaps = keymap_runtime(runtime);
        let mut hub = test_support::test_hub();
        let mut effects = test_support::RecordingEffects::default();
        keymaps.dispatch("main", &meta_c(), &mut hub, &mut effects);
        assert_eq!(effects.executed, vec!["wt".to_string()]);
    }

    #[test]
    fn a_reload_before_an_in_flight_keypress_runs_the_new_binding() {
        let cfg = write_temp(
            "inflight",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('old') end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&cfg)).unwrap();
        runtime.load();
        let mut keymaps = keymap_runtime(runtime);

        std::fs::write(
            &cfg.0,
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('new') end } } }",
        )
        .unwrap();
        assert!(keymaps.reload().is_some());

        let mut hub = test_support::test_hub();
        let mut effects = test_support::RecordingEffects::default();
        keymaps.dispatch("main", &meta_c(), &mut hub, &mut effects);
        assert_eq!(effects.executed, vec!["new".to_string()]);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn a_reload_publishes_the_new_bound_keys_to_the_keyboard() {
        let cfg = write_temp(
            "publish",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('old') end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&cfg)).unwrap();
        runtime.load();
        let (tx, rx) = std::sync::mpsc::channel();
        let publisher = KeymapPublisher::new(KeymapView::new(), tx);
        let mut keymaps = KeymapRuntime::new(runtime, Box::new(publisher));

        std::fs::write(
            &cfg.0,
            "return { keymaps = { main = { ['meta+x'] = function(a) a.execute('new') end } } }",
        )
        .unwrap();
        assert!(keymaps.reload().is_some());

        let view = rx
            .try_iter()
            .last()
            .expect("the reload publishes a keymap view");
        assert_eq!(view.resolve(&"meta+x".parse().unwrap()), Some(BASE_MODE));
        assert_eq!(view.resolve(&meta_c()), None);
    }

    #[test]
    fn a_reload_that_drops_the_binding_runs_nothing() {
        let cfg = write_temp(
            "dropped",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('old') end } } }",
        );
        let mut runtime = LuaRuntime::new(path_str(&cfg)).unwrap();
        runtime.load();
        let mut keymaps = keymap_runtime(runtime);

        std::fs::write(
            &cfg.0,
            "return { keymaps = { main = { ['meta+x'] = function(a) a.execute('other') end } } }",
        )
        .unwrap();
        assert!(keymaps.reload().is_some());

        let mut hub = test_support::test_hub();
        let mut effects = test_support::RecordingEffects::default();
        keymaps.dispatch("main", &meta_c(), &mut hub, &mut effects);
        assert!(effects.executed.is_empty());
    }
}
