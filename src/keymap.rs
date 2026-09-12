use crate::config::{BASE_MODE, CallbackId, Keymap, ModalKeymaps};

/// Shared by both platforms' keyboard handlers via `Arc<RwLock<KeymapState>>`.
///
/// The mode lives here because `resolve` reads it on the keyboard thread to
/// decide synchronously whether to suppress a keypress. Only the hub thread
/// writes it, through the `Action::Mode` handler, so a switch can land one hub
/// round-trip after a following keypress has already resolved against the old
/// mode.
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

    pub(crate) fn switch_mode(&mut self, name: &str) {
        if self.keymaps.modes.contains_key(name) {
            self.active_mode = name.to_string();
        } else {
            tracing::warn!(mode = name, "Unknown mode, staying in current mode");
        }
    }

    /// Preserves `active_mode`. If the reload drops it, `resolve` falls back to
    /// the base mode.
    pub(crate) fn update_keymaps(&mut self, keymaps: ModalKeymaps) {
        self.keymaps = keymaps;
    }

    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "reserved for planned `dome query mode` IPC command"
        )
    )]
    pub(crate) fn active_mode(&self) -> &str {
        &self.active_mode
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{CallbackId, Keymap, Modifiers};
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
    fn keymap_state_resolve_base_mode() {
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h.clone(), CallbackId(1))], vec![]);
        let state = KeymapState::new(keymaps);
        assert_eq!(state.resolve(&cmd_h), Some(CallbackId(1)));
    }

    #[test]
    fn keymap_state_resolve_no_binding() {
        let cmd_h = km("h", Modifiers::META);
        let cmd_j = km("j", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h, CallbackId(1))], vec![]);
        let state = KeymapState::new(keymaps);
        assert!(state.resolve(&cmd_j).is_none());
    }

    #[test]
    fn keymap_state_resolve_custom_mode() {
        let cmd_h = km("h", Modifiers::META);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), CallbackId(1))],
            vec![("resize", vec![(h.clone(), CallbackId(2))])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // h resolves in resize mode
        assert_eq!(state.resolve(&h), Some(CallbackId(2)));

        // cmd+h does NOT resolve in resize mode (not bound there)
        assert!(state.resolve(&cmd_h).is_none());
    }

    #[test]
    fn keymap_state_switch_to_unknown_mode_from_base() {
        let keymaps = make_keymaps(vec![], vec![]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("nonexistent");
        assert_eq!(state.active_mode(), "main");
    }

    #[test]
    fn keymap_state_switch_to_unknown_mode_from_custom_mode_preserves_mode() {
        let keymaps = make_keymaps(vec![], vec![("resize", vec![])]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");
        state.switch_mode("nonexistent");
        assert_eq!(state.active_mode(), "resize");
    }

    #[test]
    fn keymap_state_switch_to_base_while_base_is_noop() {
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h.clone(), CallbackId(1))], vec![]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("main");
        assert_eq!(state.active_mode(), "main");
        // Bindings still resolve after same-mode switch
        assert!(state.resolve(&cmd_h).is_some());
    }

    #[test]
    fn keymap_state_update_keymaps_preserves_active_mode_when_still_present() {
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(vec![], vec![("resize", vec![(h.clone(), CallbackId(1))])]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload with new keymaps that still define "resize"
        let new_keymaps = make_keymaps(vec![], vec![("resize", vec![(h.clone(), CallbackId(1))])]);
        state.update_keymaps(new_keymaps);
        assert_eq!(state.active_mode(), "resize");
        assert!(state.resolve(&h).is_some());
    }

    #[test]
    fn keymap_state_resolve_falls_back_to_base_when_active_mode_missing() {
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), CallbackId(1))],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload with keymaps where "resize" no longer exists
        let new_keymaps = make_keymaps(vec![(cmd_h.clone(), CallbackId(1))], vec![]);
        state.update_keymaps(new_keymaps);

        // active_mode is still "resize" (update_keymaps does not reset)
        assert_eq!(state.active_mode(), "resize");
        assert_eq!(state.resolve(&cmd_h), Some(CallbackId(1)));
    }

    #[test]
    fn keymap_state_resolve_falls_back_when_key_unbound_in_base() {
        let cmd_h = km("h", Modifiers::META);
        let cmd_j = km("j", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h, CallbackId(1))], vec![("resize", vec![])]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload to remove resize mode
        let new_keymaps = make_keymaps(vec![], vec![]);
        state.update_keymaps(new_keymaps);

        assert!(state.resolve(&cmd_j).is_none());
    }
}
