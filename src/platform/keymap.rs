use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;

use crate::config::{BASE_MODE, KeymapEffects, Keystroke, ModalKeymaps};

/// Holds bound keystrokes without their functions, because this view has to
/// stay `Send` and an `mlua::Function` is not. The keymaps it projects live in
/// `LuaRuntime`.
#[derive(Debug, Clone)]
pub(crate) struct KeymapView {
    bound: HashMap<String, HashSet<Keystroke>>,
    active_mode: String,
}

impl KeymapView {
    pub(crate) fn new() -> Self {
        Self {
            bound: HashMap::new(),
            active_mode: BASE_MODE.to_string(),
        }
    }

    pub(crate) fn resolve(&self, keystroke: &Keystroke) -> Option<&str> {
        let (name, keystrokes) = match self.bound.get_key_value(&self.active_mode) {
            Some(entry) => entry,
            None => {
                crate::logging::warn_once!(
                    key: &self.active_mode,
                    mode = %self.active_mode,
                    base = BASE_MODE,
                    "Active mode missing from keymaps, falling back to the base mode"
                );
                self.bound.get_key_value(BASE_MODE)?
            }
        };
        keystrokes.contains(keystroke).then_some(name.as_str())
    }

    fn switch_mode(&mut self, name: &str) {
        if self.bound.contains_key(name) {
            self.active_mode = name.to_string();
        } else {
            tracing::warn!(mode = name, "Unknown mode, staying in current mode");
        }
    }

    /// Preserves `active_mode`. If the reload drops it, `resolve` falls back to
    /// the base mode.
    fn update_keymaps(&mut self, keymaps: &ModalKeymaps) {
        self.bound = keymaps
            .modes
            .iter()
            .map(|(name, bindings)| (name.clone(), bindings.keys().cloned().collect()))
            .collect();
    }
}

/// Streaming a copy on each change keeps the keyboard thread's keypress path
/// lock-free.
pub(crate) struct KeymapPublisher {
    view: KeymapView,
    tx: Sender<KeymapView>,
}

impl KeymapPublisher {
    pub(crate) fn new(view: KeymapView, tx: Sender<KeymapView>) -> Self {
        Self { view, tx }
    }
}

impl KeymapEffects for KeymapPublisher {
    fn switch_mode(&mut self, name: &str) {
        self.view.switch_mode(name);
        let _ = self.tx.send(self.view.clone());
    }

    fn update_keymaps(&mut self, keymaps: &ModalKeymaps) {
        self.view.update_keymaps(keymaps);
        let _ = self.tx.send(self.view.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Modifiers;
    fn km(key: &str, mods: Modifiers) -> Keystroke {
        Keystroke {
            key: key.to_string(),
            modifiers: mods,
        }
    }

    fn make_keymaps(
        lua: &mlua::Lua,
        base: Vec<Keystroke>,
        modes: Vec<(&str, Vec<Keystroke>)>,
    ) -> ModalKeymaps {
        let noop = lua.create_function(|_, ()| Ok(())).unwrap();
        let bind = |keys: Vec<Keystroke>| {
            keys.into_iter()
                .map(|k| (k, noop.clone()))
                .collect::<HashMap<_, _>>()
        };
        let mut all = HashMap::new();
        all.insert(BASE_MODE.to_string(), bind(base));
        for (name, keys) in modes {
            all.insert(name.to_string(), bind(keys));
        }
        ModalKeymaps { modes: all }
    }

    fn view_with(keymaps: &ModalKeymaps) -> KeymapView {
        let mut view = KeymapView::new();
        view.update_keymaps(keymaps);
        view
    }

    #[test]
    fn resolve_in_base_mode() {
        let lua = mlua::Lua::new();
        let cmd_h = km("h", Modifiers::META);
        let cmd_j = km("j", Modifiers::META);
        let view = view_with(&make_keymaps(&lua, vec![cmd_h.clone()], vec![]));
        assert_eq!(view.resolve(&cmd_h), Some(BASE_MODE));
        assert!(view.resolve(&cmd_j).is_none());
    }

    #[test]
    fn resolve_in_custom_mode_ignores_base_bindings() {
        let lua = mlua::Lua::new();
        let cmd_h = km("h", Modifiers::META);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![h.clone()])]);
        let mut view = view_with(&keymaps);
        view.switch_mode("resize");

        assert_eq!(view.resolve(&h), Some("resize"));
        assert!(view.resolve(&cmd_h).is_none());
    }

    #[test]
    fn switch_to_unknown_mode_stays_in_current_mode() {
        let lua = mlua::Lua::new();
        let cmd_h = km("h", Modifiers::META);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![h.clone()])]);
        let mut view = view_with(&keymaps);
        view.switch_mode("resize");
        view.switch_mode("nonexistent");

        assert_eq!(view.resolve(&h), Some("resize"));
        assert!(view.resolve(&cmd_h).is_none());
    }

    #[test]
    fn update_keymaps_preserves_active_mode_when_still_present() {
        let lua = mlua::Lua::new();
        let h = km("h", Modifiers::empty());
        let j = km("j", Modifiers::empty());
        let keymaps = make_keymaps(&lua, vec![], vec![("resize", vec![h.clone()])]);
        let mut view = view_with(&keymaps);
        view.switch_mode("resize");

        view.update_keymaps(&make_keymaps(
            &lua,
            vec![],
            vec![("resize", vec![j.clone()])],
        ));
        assert_eq!(view.resolve(&j), Some("resize"));
        assert!(view.resolve(&h).is_none());
    }

    #[test]
    fn resolve_falls_back_to_base_when_active_mode_missing() {
        let lua = mlua::Lua::new();
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![])]);
        let mut view = view_with(&keymaps);
        view.switch_mode("resize");

        view.update_keymaps(&make_keymaps(&lua, vec![cmd_h.clone()], vec![]));
        assert_eq!(view.resolve(&cmd_h), Some(BASE_MODE));
    }

    #[test]
    fn keymap_publisher_streams_a_copy_on_each_change() {
        let lua = mlua::Lua::new();
        let h = km("h", Modifiers::empty());
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![h.clone()])]);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut publisher = KeymapPublisher::new(KeymapView::new(), tx);

        publisher.update_keymaps(&keymaps);
        rx.recv().unwrap();
        publisher.switch_mode("resize");
        assert_eq!(rx.recv().unwrap().resolve(&h), Some("resize"));

        // active_mode stays "resize" but no longer exists, so resolve falls back to base.
        publisher.update_keymaps(&make_keymaps(&lua, vec![cmd_h.clone()], vec![]));
        assert_eq!(rx.recv().unwrap().resolve(&cmd_h), Some(BASE_MODE));
    }
}
