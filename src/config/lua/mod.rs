//! The hub thread owns the persistent Luau VM and the registered callbacks,
//! none of which are `Send`. The VM outlives each load so a function-valued
//! binding stays callable after the load that created it.

use std::cell::RefCell;

use super::{Config, Keystroke, ModalKeymaps};
use crate::core::{Hub, ReservedArea, WindowId};

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
    let value: mlua::Value = lua.load(src).set_name(name).eval()?;
    T::from_lua_value(&value, &mut deserializer::LoadContext::new())
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
    fn unminimize(&mut self, id: WindowId);
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

pub(crate) struct LuaRuntime {
    lua: mlua::Lua,
    keymaps: ModalKeymaps,
    reserved_area: mlua::Function,
    config_path: String,
}

impl LuaRuntime {
    #[tracing::instrument(skip_all, fields(path = %config_path))]
    pub(crate) fn load(
        config_path: String,
        keymap_effects: &mut dyn KeymapEffects,
    ) -> mlua::Result<(Self, Config)> {
        let lua = new_vm()?;
        let config = match Config::load(&lua, &config_path) {
            Ok(config) => config,
            Err(e) => {
                log_initial_load_error(&e);
                Config::load_default(&lua).expect("the bundled default config must load")
            }
        };
        keymap_effects.update_keymaps(&config.keymaps);
        let runtime = Self {
            lua,
            keymaps: config.keymaps.clone(),
            reserved_area: config.reserved_area.clone(),
            config_path,
        };
        Ok((runtime, config))
    }

    #[tracing::instrument(skip_all, fields(%keymap, %keystroke))]
    pub(crate) fn binding(&self, keymap: &str, keystroke: &Keystroke) -> Option<Binding> {
        let function = self.keymaps.modes.get(keymap)?.get(keystroke)?.clone();
        Some(Binding {
            lua: self.lua.clone(),
            function,
        })
    }

    #[tracing::instrument(skip_all, fields(monitor_name = %name), ret(level = "debug"))]
    pub(crate) fn reserved_area(&self, name: &str) -> ReservedArea {
        let returned = self.lua.create_table().and_then(|table| {
            table.set("name", name)?;
            self.reserved_area.call::<mlua::Value>(table)
        });
        match returned {
            Ok(mlua::Value::Nil) => ReservedArea::ZERO,
            Ok(value) => deserializer::LoadContext::new()
                .read("reserved_area", &value, || ReservedArea::ZERO),
            Err(e) => {
                tracing::warn!(error = %e, "reserved_area errored, using default");
                ReservedArea::ZERO
            }
        }
    }

    #[tracing::instrument(skip_all, fields(path = %self.config_path))]
    pub(crate) fn reload(&mut self, keymap_effects: &mut dyn KeymapEffects) -> Option<Box<Config>> {
        match Config::load(&self.lua, &self.config_path) {
            Ok(config) => {
                self.install(&config, keymap_effects);
                Some(Box::new(config))
            }
            Err(e) => {
                tracing::warn!(error = %format!("{e:#}"), "Reload failed, keeping current config");
                None
            }
        }
    }

    fn install(&mut self, config: &Config, keymap_effects: &mut dyn KeymapEffects) {
        self.keymaps = config.keymaps.clone();
        self.reserved_area = config.reserved_area.clone();
        keymap_effects.update_keymaps(&config.keymaps);
    }
}

pub(crate) struct Binding {
    lua: mlua::Lua,
    function: mlua::Function,
}

impl Binding {
    #[tracing::instrument(skip_all)]
    pub(crate) fn call(&self, cx: &mut ActionContext) -> mlua::Result<()> {
        let cx = RefCell::new(cx);
        self.lua.scope(|scope| {
            let actions = build_actions(&self.lua, scope, &cx)?;
            self.function.call::<()>(actions)
        })
    }
}

fn log_initial_load_error(e: &anyhow::Error) {
    if e.downcast_ref::<std::io::Error>()
        .is_some_and(|io| io.kind() == std::io::ErrorKind::NotFound)
    {
        tracing::info!("Config file not found, using defaults");
    } else {
        tracing::warn!(error = %format!("{e:#}"), "Failed to load config, using defaults");
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::{KeymapEffects, LuaRuntime, PlatformEffects};
    use crate::config::ModalKeymaps;
    use crate::core::{
        Hub, PixelRect, PreferredLayouts, ReportedMonitor, WindowId, WindowMatcher, WindowMetadata,
        WindowRestrictions,
    };

    pub(crate) fn test_hub() -> Hub {
        let (runtime, _) = LuaRuntime::load(String::new(), &mut RecordingKeymap::default())
            .expect("the test Lua VM should build");
        test_hub_with(runtime)
    }

    pub(crate) struct TempFile(std::path::PathBuf);

    impl TempFile {
        pub(crate) fn rewrite(&self, src: &str) {
            std::fs::write(&self.0, src).expect("the temp config should rewrite");
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            std::fs::remove_file(&self.0).ok();
        }
    }

    fn write_temp(tag: &str, src: &str) -> TempFile {
        // Concurrent callers may pass the same tag, and macOS reports the time in
        // whole microseconds, so a counter names the file rather than a timestamp.
        static NEXT_FILE: AtomicUsize = AtomicUsize::new(0);
        let file = NEXT_FILE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("dome_rt_{tag}_{}_{file}.lua", std::process::id()));
        std::fs::write(&path, src).unwrap();
        TempFile(path)
    }

    /// A runtime loaded from `src` through a real file, so a test can rewrite the
    /// file and reload it. Dropping the `TempFile` deletes the file.
    pub(crate) fn loaded_runtime(tag: &str, src: &str) -> (LuaRuntime, TempFile) {
        let file = write_temp(tag, src);
        let path = file.0.to_str().expect("the temp path should be UTF-8");
        let (runtime, _) = LuaRuntime::load(path.to_string(), &mut RecordingKeymap::default())
            .expect("the test Lua VM should build");
        (runtime, file)
    }

    pub(crate) fn test_hub_with(runtime: LuaRuntime) -> Hub {
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
            runtime,
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
        pub unminimized: Vec<WindowId>,
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
        fn unminimize(&mut self, id: WindowId) {
            self.unminimized.push(id);
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
    use crate::core::Pixels;
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    use crate::platform::keymap::{KeymapPublisher, KeymapView};
    use test_support::{RecordingKeymap, loaded_runtime};

    fn meta_c() -> Keystroke {
        "meta+c".parse().unwrap()
    }

    fn insets(top: i32, bottom: i32, left: i32, right: i32) -> ReservedArea {
        ReservedArea {
            top: Pixels::new(top),
            bottom: Pixels::new(bottom),
            left: Pixels::new(left),
            right: Pixels::new(right),
        }
    }

    fn reserved_area_config(body: &str) -> String {
        format!("return {{ reserved_area = function(monitor) {body} end }}")
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn load_publishes_the_keymaps() {
        let (tx, rx) = std::sync::mpsc::channel();
        let mut publisher = KeymapPublisher::new(KeymapView::new(), tx);
        LuaRuntime::load(String::new(), &mut publisher).unwrap();

        let view = rx.try_recv().expect("load should publish the keymaps");
        let alt_0: Keystroke = "alt+0".parse().unwrap();
        assert_eq!(view.resolve(&alt_0), Some("main"));
    }

    #[test]
    fn reload_keeps_last_good_bindings_runnable_on_error() {
        let (mut runtime, good) = loaded_runtime(
            "good",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('ok') end } } }",
        );
        let mut keymap = RecordingKeymap::default();

        good.rewrite("this is not lua {{{");
        assert!(runtime.reload(&mut keymap).is_none());

        let mut hub = test_support::test_hub_with(runtime);
        let mut effects = test_support::RecordingEffects::default();
        hub.run_binding("main", &meta_c(), &mut effects, &mut keymap);
        assert_eq!(effects.executed, vec!["ok".to_string()]);
    }

    #[test]
    fn run_binding_drives_context() {
        let (runtime, _cfg) = loaded_runtime(
            "cb",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('wt') end } } }",
        );
        let mut keymap = RecordingKeymap::default();
        let mut hub = test_support::test_hub_with(runtime);
        let mut effects = test_support::RecordingEffects::default();
        hub.run_binding("main", &meta_c(), &mut effects, &mut keymap);
        assert_eq!(effects.executed, vec!["wt".to_string()]);
    }

    #[test]
    fn a_reload_before_an_in_flight_keypress_runs_the_new_binding() {
        let (mut runtime, cfg) = loaded_runtime(
            "inflight",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('old') end } } }",
        );
        let mut keymap = RecordingKeymap::default();

        cfg.rewrite(
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('new') end } } }",
        );
        assert!(runtime.reload(&mut keymap).is_some());

        let mut hub = test_support::test_hub_with(runtime);
        let mut effects = test_support::RecordingEffects::default();
        hub.run_binding("main", &meta_c(), &mut effects, &mut keymap);
        assert_eq!(effects.executed, vec!["new".to_string()]);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn a_reload_publishes_the_new_bound_keys_to_the_keyboard() {
        let (mut runtime, cfg) = loaded_runtime(
            "publish",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('old') end } } }",
        );
        let (tx, rx) = std::sync::mpsc::channel();
        let mut publisher = KeymapPublisher::new(KeymapView::new(), tx);

        cfg.rewrite(
            "return { keymaps = { main = { ['meta+x'] = function(a) a.execute('new') end } } }",
        );
        assert!(runtime.reload(&mut publisher).is_some());

        let view = rx
            .try_iter()
            .last()
            .expect("the reload publishes a keymap view");
        assert_eq!(view.resolve(&"meta+x".parse().unwrap()), Some(BASE_MODE));
        assert_eq!(view.resolve(&meta_c()), None);
    }

    #[test]
    fn a_reload_that_drops_the_binding_runs_nothing() {
        let (mut runtime, cfg) = loaded_runtime(
            "dropped",
            "return { keymaps = { main = { ['meta+c'] = function(a) a.execute('old') end } } }",
        );
        let mut keymap = RecordingKeymap::default();

        cfg.rewrite(
            "return { keymaps = { main = { ['meta+x'] = function(a) a.execute('other') end } } }",
        );
        assert!(runtime.reload(&mut keymap).is_some());

        let mut hub = test_support::test_hub_with(runtime);
        let mut effects = test_support::RecordingEffects::default();
        hub.run_binding("main", &meta_c(), &mut effects, &mut keymap);
        assert!(effects.executed.is_empty());
    }

    #[test]
    fn reserved_area_reads_the_insets_the_function_returns() {
        let (runtime, _cfg) = loaded_runtime(
            "insets",
            &reserved_area_config("return { top = 30, bottom = 1, left = 2, right = 3 }"),
        );
        assert_eq!(runtime.reserved_area("A"), insets(30, 1, 2, 3));
    }

    #[test]
    fn reserved_area_passes_only_the_monitor_name() {
        let (runtime, _cfg) = loaded_runtime(
            "name_only",
            &reserved_area_config(
                "local count = 0
                for _ in pairs(monitor) do count = count + 1 end
                return { top = count, left = monitor.name == 'A' and 5 or 0 }",
            ),
        );
        assert_eq!(runtime.reserved_area("A"), insets(1, 0, 5, 0));
        assert_eq!(runtime.reserved_area("B"), insets(1, 0, 0, 0));
    }

    #[test]
    fn reserved_area_calls_the_function_on_every_lookup() {
        let (runtime, _cfg) = loaded_runtime(
            "every_lookup",
            "local calls = 0
            return { reserved_area = function()
                calls = calls + 1
                return { top = calls }
            end }",
        );
        assert_eq!(runtime.reserved_area("A"), insets(1, 0, 0, 0));
        assert_eq!(runtime.reserved_area("A"), insets(2, 0, 0, 0));
        assert_eq!(runtime.reserved_area("B"), insets(3, 0, 0, 0));
    }

    #[test]
    fn a_failing_reserved_area_reserves_nothing() {
        let (runtime, _cfg) = loaded_runtime("failing", &reserved_area_config("error('boom')"));
        assert_eq!(runtime.reserved_area("A"), ReservedArea::ZERO);
    }

    #[test]
    fn a_reserved_area_that_returns_no_table_reserves_nothing() {
        for returned in ["5", "'top'", "true"] {
            let (runtime, _cfg) = loaded_runtime(
                "no_table",
                &reserved_area_config(&format!("return {returned}")),
            );
            assert_eq!(runtime.reserved_area("A"), ReservedArea::ZERO, "{returned}");
        }
    }

    #[test]
    fn a_reserved_area_that_returns_nil_reserves_nothing() {
        let (runtime, _cfg) = loaded_runtime(
            "nil",
            &reserved_area_config("if monitor.name == 'A' then return { top = 5 } end"),
        );
        assert_eq!(runtime.reserved_area("A"), insets(5, 0, 0, 0));
        assert_eq!(runtime.reserved_area("B"), ReservedArea::ZERO);
    }

    #[test]
    fn an_unusable_edge_reads_as_zero_and_keeps_the_rest() {
        let (runtime, _cfg) = loaded_runtime(
            "unusable_edge",
            &reserved_area_config("return { top = -1, bottom = 1.5, left = '3', right = 4 }"),
        );
        assert_eq!(runtime.reserved_area("A"), insets(0, 0, 0, 4));
    }

    #[test]
    fn an_absent_or_non_function_reserved_area_reserves_nothing() {
        for src in ["return {}", "return { reserved_area = 5 }"] {
            let (runtime, _cfg) = loaded_runtime("absent", src);
            assert_eq!(runtime.reserved_area("A"), ReservedArea::ZERO, "{src}");
        }
    }

    #[test]
    fn a_reload_calls_the_new_reserved_area() {
        let (mut runtime, cfg) =
            loaded_runtime("reload", &reserved_area_config("return { top = 10 }"));
        assert_eq!(runtime.reserved_area("A"), insets(10, 0, 0, 0));

        cfg.rewrite(&reserved_area_config("return { top = 20 }"));
        assert!(runtime.reload(&mut RecordingKeymap::default()).is_some());
        assert_eq!(runtime.reserved_area("A"), insets(20, 0, 0, 0));
    }

    #[test]
    fn a_failed_reload_keeps_the_reserved_area() {
        let (mut runtime, cfg) = loaded_runtime(
            "failed_reload",
            &reserved_area_config("return { top = 10 }"),
        );

        cfg.rewrite("this is not lua {{{");
        assert!(runtime.reload(&mut RecordingKeymap::default()).is_none());
        assert_eq!(runtime.reserved_area("A"), insets(10, 0, 0, 0));
    }
}
