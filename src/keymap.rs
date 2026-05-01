use crate::action::{Action, Actions};
use crate::config::{Keymap, ModalKeymaps};

/// Runtime state for modal keybinding resolution. Both macOS and Windows
/// keyboard handlers share a single `KeymapState` via `Arc<RwLock<KeymapState>>`.
///
/// Mode state lives here (not in core/hub) because keyboard callbacks must
/// synchronously decide whether to suppress a keypress. Hub-owned mode state
/// would require a round-trip and has a race where fast keypresses resolve
/// against the stale mode before a hub push arrives.
#[derive(Debug, Clone)]
pub(crate) struct KeymapState {
    keymaps: ModalKeymaps,
    active_mode: String,
}

impl KeymapState {
    pub(crate) fn new(keymaps: ModalKeymaps) -> Self {
        Self {
            keymaps,
            active_mode: "default".to_string(),
        }
    }

    /// The single entry point for keymap resolution. Both platforms call this.
    ///
    /// 1. Looks up `keymap` in the active mode's bindings. If the active mode
    ///    has been removed (e.g. config reload dropped it), logs a warning and
    ///    falls back to the `default` table so the keyboard keeps working.
    /// 2. For any `Action::Mode` in the result, switches mode immediately.
    /// 3. Returns only the non-Mode actions (to send to the hub).
    /// 4. Returns `None` if no binding exists (after fallback) or all actions
    ///    were Mode switches.
    ///
    /// Multiple Mode actions in one binding are processed in order -- last one
    /// wins (each switch_mode call overwrites the previous). This matches how
    /// shells process trailing redirections.
    pub(crate) fn resolve(&mut self, keymap: &Keymap) -> Option<Actions> {
        let bindings = if self.active_mode == "default" {
            &self.keymaps.default
        } else {
            match self.keymaps.modes.get(&self.active_mode) {
                Some(m) => m,
                None => {
                    tracing::warn!(
                        mode = %self.active_mode,
                        "Active mode missing from keymaps, falling back to default table"
                    );
                    &self.keymaps.default
                }
            }
        };

        let actions = bindings.get(keymap)?;

        // Fast path: when no Mode actions present (the common case), return a
        // single clone without the per-action filter loop.
        let has_mode = actions
            .into_iter()
            .any(|a| matches!(a, Action::Mode { .. }));
        if !has_mode {
            return Some(actions.clone());
        }

        // Clone into an owned Vec to drop the borrow on self.keymaps before
        // calling self.switch_mode() (which needs &mut self).
        let owned: Vec<Action> = actions.into_iter().cloned().collect();

        let mut hub_actions = Vec::new();
        for action in &owned {
            if let Action::Mode { name } = action {
                self.switch_mode(name);
            } else {
                hub_actions.push(action.clone());
            }
        }

        if hub_actions.is_empty() {
            return None;
        }
        Some(Actions::new(hub_actions))
    }

    /// Switch to a named mode. Unknown mode names log a warning and leave
    /// `active_mode` unchanged so the user gets immediate log feedback rather
    /// than a silent "nothing happens when I press keys" failure.
    pub(crate) fn switch_mode(&mut self, name: &str) {
        if name == "default" || self.keymaps.modes.contains_key(name) {
            self.active_mode = name.to_string();
        } else {
            tracing::warn!(mode = name, "Unknown mode, staying in current mode");
        }
    }

    /// Update keymaps on config reload. `active_mode` is preserved: if the new
    /// config still defines it, the user stays in it; if not, `resolve` falls
    /// back to the default table on the next keypress.
    pub(crate) fn update_keymaps(&mut self, keymaps: ModalKeymaps) {
        self.keymaps = keymaps;
    }

    /// Reserved for planned `dome query mode` IPC command. Currently only
    /// exercised by unit tests.
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
    use crate::action::{Action, Actions, FocusTarget, HubAction};
    use crate::config::{Keymap, Modifiers};

    fn km(key: &str, mods: Modifiers) -> Keymap {
        Keymap {
            key: key.to_string(),
            modifiers: mods,
        }
    }

    fn focus_left_actions() -> Actions {
        Actions::new(vec![Action::Hub(HubAction::Focus {
            target: FocusTarget::Left,
        })])
    }

    fn mode_action(name: &str) -> Action {
        Action::Mode {
            name: name.to_string(),
        }
    }

    fn make_keymaps(
        default: Vec<(Keymap, Actions)>,
        modes: Vec<(&str, Vec<(Keymap, Actions)>)>,
    ) -> ModalKeymaps {
        ModalKeymaps {
            default: default.into_iter().collect(),
            modes: modes
                .into_iter()
                .map(|(name, bindings)| (name.to_string(), bindings.into_iter().collect()))
                .collect(),
        }
    }

    #[test]
    fn keymap_state_resolve_default_mode() {
        let cmd_h = km("h", Modifiers::CMD);
        let keymaps = make_keymaps(vec![(cmd_h.clone(), focus_left_actions())], vec![]);
        let mut state = KeymapState::new(keymaps);
        let result = state.resolve(&cmd_h);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_string(), "[focus left]");
    }

    #[test]
    fn keymap_state_resolve_no_binding() {
        let cmd_h = km("h", Modifiers::CMD);
        let cmd_j = km("j", Modifiers::CMD);
        let keymaps = make_keymaps(vec![(cmd_h, focus_left_actions())], vec![]);
        let mut state = KeymapState::new(keymaps);
        assert!(state.resolve(&cmd_j).is_none());
    }

    #[test]
    fn keymap_state_resolve_custom_mode() {
        let cmd_h = km("h", Modifiers::CMD);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), focus_left_actions())],
            vec![("resize", vec![(h.clone(), focus_left_actions())])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // h resolves in resize mode
        let result = state.resolve(&h);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_string(), "[focus left]");

        // cmd+h does NOT resolve in resize mode (not bound there)
        assert!(state.resolve(&cmd_h).is_none());
    }

    #[test]
    fn keymap_state_resolve_filters_mode_actions() {
        let cmd_r = km("r", Modifiers::CMD);
        let keymaps = make_keymaps(
            vec![(cmd_r.clone(), Actions::new(vec![mode_action("resize")]))],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        // Mode action consumed internally, nothing returned to hub
        assert!(state.resolve(&cmd_r).is_none());
        assert_eq!(state.active_mode(), "resize");
    }

    #[test]
    fn keymap_state_resolve_mixed_actions() {
        let cmd_r = km("r", Modifiers::CMD);
        let keymaps = make_keymaps(
            vec![(
                cmd_r.clone(),
                Actions::new(vec![
                    Action::Hub(HubAction::Focus {
                        target: FocusTarget::Left,
                    }),
                    mode_action("resize"),
                ]),
            )],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        let result = state.resolve(&cmd_r);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_string(), "[focus left]");
        assert_eq!(state.active_mode(), "resize");
    }

    #[test]
    fn keymap_state_switch_to_unknown_mode_from_default() {
        let keymaps = make_keymaps(vec![], vec![]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("nonexistent");
        assert_eq!(state.active_mode(), "default");
    }

    #[test]
    fn keymap_state_switch_to_unknown_mode_from_custom_mode_preserves_mode() {
        let keymaps = make_keymaps(vec![], vec![("resize", vec![])]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");
        state.switch_mode("nonexistent");
        // Must stay in "resize", not fall back to "default"
        assert_eq!(state.active_mode(), "resize");
    }

    #[test]
    fn keymap_state_switch_to_default_while_default_is_noop() {
        let cmd_h = km("h", Modifiers::CMD);
        let keymaps = make_keymaps(vec![(cmd_h.clone(), focus_left_actions())], vec![]);
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("default");
        assert_eq!(state.active_mode(), "default");
        // Bindings still resolve after same-mode switch
        assert!(state.resolve(&cmd_h).is_some());
    }

    #[test]
    fn keymap_state_update_keymaps_preserves_active_mode_when_still_present() {
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(
            vec![],
            vec![("resize", vec![(h.clone(), focus_left_actions())])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload with new keymaps that still define "resize"
        let new_keymaps = make_keymaps(
            vec![],
            vec![("resize", vec![(h.clone(), focus_left_actions())])],
        );
        state.update_keymaps(new_keymaps);
        assert_eq!(state.active_mode(), "resize");
        assert!(state.resolve(&h).is_some());
    }

    #[test]
    fn keymap_state_resolve_falls_back_to_default_when_active_mode_missing() {
        let cmd_h = km("h", Modifiers::CMD);
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), focus_left_actions())],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload with keymaps where "resize" no longer exists
        let new_keymaps = make_keymaps(vec![(cmd_h.clone(), focus_left_actions())], vec![]);
        state.update_keymaps(new_keymaps);

        // active_mode is still "resize" (update_keymaps does not reset)
        assert_eq!(state.active_mode(), "resize");
        // But resolve falls back to default table
        let result = state.resolve(&cmd_h);
        assert!(result.is_some());
        assert_eq!(result.unwrap().to_string(), "[focus left]");
    }

    #[test]
    fn keymap_state_resolve_falls_back_when_key_unbound_in_default() {
        let cmd_h = km("h", Modifiers::CMD);
        let cmd_j = km("j", Modifiers::CMD);
        let keymaps = make_keymaps(
            vec![(cmd_h, focus_left_actions())],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // Reload to remove resize mode
        let new_keymaps = make_keymaps(vec![], vec![]);
        state.update_keymaps(new_keymaps);

        // Falls back to default, but cmd+j is not bound there either
        assert!(state.resolve(&cmd_j).is_none());
    }
}
