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
// that made the call.
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

/// The hub thread owns this persistent Luau VM and its registered callbacks,
/// none of which are `Send`. The VM outlives each load so a function-valued
/// binding stays callable after the load that created it.
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
