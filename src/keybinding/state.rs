//! The keyboard thread's active-mode state and the hub thread's publisher.

use std::sync::mpsc::Sender;

use super::{BASE_MODE, CallbackId, Keymap, ModalKeymaps};

#[derive(Debug, Clone)]
pub(crate) struct KeymapState {
    keymaps: ModalKeymaps,
    active_mode: String,
}

impl KeymapState {
    pub(crate) fn new(keymaps: ModalKeymaps) -> Self {
        Self {
            keymaps,
            active_mode: BASE_MODE.to_string(),
        }
    }

    /// Returns `Some` for any bound key, so the keyboard handler suppresses it.
    pub(crate) fn resolve(&self, keymap: &Keymap) -> Option<CallbackId> {
        let bindings = match self.keymaps.modes.get(&self.active_mode) {
            Some(m) => m,
            None => {
                tracing::warn!(
                    mode = %self.active_mode,
                    base = BASE_MODE,
                    "Active mode missing from keymaps, falling back to the base mode"
                );
                self.keymaps.modes.get(BASE_MODE)?
            }
        };
        bindings.get(keymap).copied()
    }

    fn switch_mode(&mut self, name: &str) {
        if self.keymaps.modes.contains_key(name) {
            self.active_mode = name.to_string();
        } else {
            tracing::warn!(mode = name, "Unknown mode, staying in current mode");
        }
    }

    /// Preserves `active_mode`. If the reload drops it, `resolve` falls back to
    /// the base mode.
    fn update_keymaps(&mut self, keymaps: ModalKeymaps) {
        self.keymaps = keymaps;
    }
}

/// Streaming a copy on each change keeps the keyboard thread's keypress path
/// lock-free.
pub(crate) struct KeymapPublisher {
    state: KeymapState,
    tx: Sender<KeymapState>,
}

impl KeymapPublisher {
    pub(crate) fn new(state: KeymapState, tx: Sender<KeymapState>) -> Self {
        Self { state, tx }
    }

    pub(crate) fn switch_mode(&mut self, name: &str) {
        self.state.switch_mode(name);
        let _ = self.tx.send(self.state.clone());
    }

    pub(crate) fn update_keymaps(&mut self, keymaps: ModalKeymaps) {
        self.state.update_keymaps(keymaps);
        let _ = self.tx.send(self.state.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keybinding::Modifiers;
    use std::collections::HashMap;

    fn km(key: &str, mods: Modifiers) -> Keymap {
        Keymap {
            key: key.to_string(),
            modifiers: mods,
        }
    }

    fn make_keymaps(
        base: Vec<(Keymap, CallbackId)>,
        modes: Vec<(&str, Vec<(Keymap, CallbackId)>)>,
    ) -> ModalKeymaps {
        let mut all: HashMap<String, HashMap<Keymap, CallbackId>> = HashMap::new();
        all.insert(BASE_MODE.to_string(), base.into_iter().collect());
        for (name, bindings) in modes {
            all.insert(name.to_string(), bindings.into_iter().collect());
        }
        ModalKeymaps { modes: all }
    }

    #[test]
    fn resolve_in_base_mode() {
        let cmd_h = km("h", Modifiers::META);
        let cmd_j = km("j", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h.clone(), CallbackId(1))], vec![]);
        let state = KeymapState::new(keymaps);
        assert_eq!(state.resolve(&cmd_h), Some(CallbackId(1)));
        assert!(state.resolve(&cmd_j).is_none());
    }

    #[test]
    fn resolve_in_custom_mode_ignores_base_bindings() {
        let cmd_h = km("h", Modifiers::META);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), CallbackId(1))],
            vec![("resize", vec![(h.clone(), CallbackId(2))])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        assert_eq!(state.resolve(&h), Some(CallbackId(2)));
        assert!(state.resolve(&cmd_h).is_none());
    }

    #[test]
    fn switch_to_unknown_mode_stays_in_current_mode() {
        let cmd_h = km("h", Modifiers::META);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), CallbackId(1))],
            vec![("resize", vec![(h.clone(), CallbackId(2))])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");
        state.switch_mode("nonexistent");

        // Still in resize: its binding resolves and the base binding does not.
        assert_eq!(state.resolve(&h), Some(CallbackId(2)));
        assert!(state.resolve(&cmd_h).is_none());
    }

    #[test]
    fn update_keymaps_preserves_active_mode_when_still_present() {
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(vec![], vec![("resize", vec![(h.clone(), CallbackId(1))])]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        let new_keymaps = make_keymaps(vec![], vec![("resize", vec![(h.clone(), CallbackId(2))])]);
        state.update_keymaps(new_keymaps);
        assert_eq!(state.resolve(&h), Some(CallbackId(2)));
    }

    #[test]
    fn resolve_falls_back_to_base_when_active_mode_missing() {
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), CallbackId(1))],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload drops "resize"; resolve falls back to the base mode.
        let new_keymaps = make_keymaps(vec![(cmd_h.clone(), CallbackId(1))], vec![]);
        state.update_keymaps(new_keymaps);
        assert_eq!(state.resolve(&cmd_h), Some(CallbackId(1)));
    }

    #[test]
    fn keymap_publisher_streams_a_copy_on_each_change() {
        let h = km("h", Modifiers::empty());
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), CallbackId(1))],
            vec![("resize", vec![(h.clone(), CallbackId(2))])],
        );
        let (tx, rx) = std::sync::mpsc::channel();
        let mut publisher = KeymapPublisher::new(KeymapState::new(keymaps), tx);

        publisher.switch_mode("resize");
        let after_switch = rx.recv().unwrap();
        assert_eq!(after_switch.resolve(&h), Some(CallbackId(2)));

        // active_mode stays "resize" but no longer exists, so resolve falls back to base.
        publisher.update_keymaps(make_keymaps(vec![(cmd_h.clone(), CallbackId(9))], vec![]));
        let after_update = rx.recv().unwrap();
        assert_eq!(after_update.resolve(&cmd_h), Some(CallbackId(9)));
    }
}
