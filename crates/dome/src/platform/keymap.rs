use std::collections::{HashMap, HashSet};
use std::sync::mpsc::Sender;

use crate::config::{BASE_MODE, KeymapEffects, Keystroke, ModalKeymaps};

/// Holds bound keystrokes without their functions, because this view has to
/// stay `Send` and an `mlua::Function` is not.
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
    use super::{KeymapPublisher, KeymapView};
    use crate::config::{BASE_MODE, KeymapEffects, Keystroke, ModalKeymaps, Modifiers};
    use std::collections::HashMap;
    use std::sync::mpsc::Receiver;
    use std::time::Duration;
    fn keystroke(key: &str, modifiers: Modifiers) -> Keystroke {
        Keystroke {
            key: key.to_string(),
            modifiers,
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

    struct PublishedKeymap {
        publisher: KeymapPublisher,
        updates: Receiver<KeymapView>,
        view: KeymapView,
    }

    impl PublishedKeymap {
        fn switch_mode(&mut self, name: &str) {
            self.publisher.switch_mode(name);
            self.view = self.updates.recv_timeout(Duration::from_secs(2)).unwrap();
        }

        fn update_keymaps(&mut self, keymaps: &ModalKeymaps) {
            self.publisher.update_keymaps(keymaps);
            self.view = self.updates.recv_timeout(Duration::from_secs(2)).unwrap();
        }

        fn resolve(&self, stroke: &Keystroke) -> Option<&str> {
            self.view.resolve(stroke)
        }
    }

    fn view_with(keymaps: &ModalKeymaps) -> PublishedKeymap {
        let (sender, updates) = std::sync::mpsc::channel();
        let publisher = KeymapPublisher::new(KeymapView::new(), sender);
        let mut published = PublishedKeymap {
            publisher,
            updates,
            view: KeymapView::new(),
        };
        published.update_keymaps(keymaps);
        published
    }

    #[test]
    fn resolve_in_custom_mode_ignores_base_bindings() {
        let lua = mlua::Lua::new();
        let cmd_h = keystroke("h", Modifiers::META);
        let h = keystroke("h", Modifiers::empty());
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![h.clone()])]);
        let mut view = view_with(&keymaps);
        assert_eq!(view.resolve(&cmd_h), Some(BASE_MODE));
        assert_eq!(view.resolve(&h), None);
        view.switch_mode("resize");

        assert_eq!(view.resolve(&h), Some("resize"));
        assert!(view.resolve(&cmd_h).is_none());
    }

    #[test]
    fn switch_to_unknown_mode_stays_in_current_mode() {
        let lua = mlua::Lua::new();
        let cmd_h = keystroke("h", Modifiers::META);
        let h = keystroke("h", Modifiers::empty());
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![h.clone()])]);
        let mut view = view_with(&keymaps);
        view.switch_mode("resize");
        view.switch_mode("nonexistent");

        assert_eq!(view.resolve(&h), Some("resize"));
        assert!(view.resolve(&cmd_h).is_none());
    }

    #[test]
    fn update_keymaps_when_active_mode_still_present_preserves_it() {
        let lua = mlua::Lua::new();
        let h = keystroke("h", Modifiers::empty());
        let j = keystroke("j", Modifiers::empty());
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
    fn update_keymaps_when_active_mode_removed_publishes_base_fallback() {
        let lua = mlua::Lua::new();
        let h = keystroke("h", Modifiers::empty());
        let cmd_h = keystroke("h", Modifiers::META);
        let keymaps = make_keymaps(&lua, vec![cmd_h.clone()], vec![("resize", vec![h.clone()])]);
        let (tx, rx) = std::sync::mpsc::channel();
        let mut publisher = KeymapPublisher::new(KeymapView::new(), tx);

        publisher.update_keymaps(&keymaps);
        rx.recv_timeout(std::time::Duration::from_secs(2)).unwrap();
        publisher.switch_mode("resize");
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .resolve(&h),
            Some("resize")
        );

        publisher.update_keymaps(&make_keymaps(&lua, vec![cmd_h.clone()], vec![]));
        assert_eq!(
            rx.recv_timeout(std::time::Duration::from_secs(2))
                .unwrap()
                .resolve(&cmd_h),
            Some(BASE_MODE)
        );
    }
}
