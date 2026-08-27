use crate::action::{Action, Actions};
use crate::config::{Binding, CallbackId, Keymap, ModalKeymaps};

/// The outcome of resolving a keypress. A static action list goes to the hub.
/// A callback is dispatched to the `dome-lua` thread by id.
#[derive(Debug)]
pub(crate) enum Resolved {
    Actions(Actions),
    Callback(CallbackId),
}

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
    ///    was removed (e.g. a reload dropped it), logs a warning and falls back
    ///    to the `default` table so the keyboard keeps working.
    /// 2. A callback binding resolves directly to `Resolved::Callback(id)` for
    ///    the caller to dispatch to the `dome-lua` thread. A callback is never
    ///    a mode switch.
    /// 3. A static binding switches mode for any `Action::Mode` immediately and
    ///    returns the remaining actions as `Resolved::Actions`.
    /// 4. Returns `None` if no binding exists (after fallback) or a static
    ///    binding held only mode switches.
    ///
    /// Returning `Some` for any bound key, callback included, is what makes a
    /// callback binding suppress the keypress even though it emits no actions
    /// on the event-tap thread.
    ///
    /// Multiple Mode actions in one binding are processed in order, last one
    /// wins.
    pub(crate) fn resolve(&mut self, keymap: &Keymap) -> Option<Resolved> {
        // Clone the matched binding to drop the borrow on self.keymaps before
        // calling self.switch_mode() (which needs &mut self).
        let binding = {
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
            bindings.get(keymap)?.clone()
        };

        let actions = match binding {
            Binding::Callback(id) => return Some(Resolved::Callback(id)),
            Binding::Static(actions) => actions,
        };

        // Fast path: when no Mode actions present (the common case), return
        // without the per-action filter loop.
        let has_mode = (&actions)
            .into_iter()
            .any(|a| matches!(a, Action::Mode { .. }));
        if !has_mode {
            return Some(Resolved::Actions(actions));
        }

        let mut hub_actions = Vec::new();
        for action in &actions {
            if let Action::Mode { name } = action {
                self.switch_mode(name);
            } else {
                hub_actions.push(action.clone());
            }
        }

        if hub_actions.is_empty() {
            return None;
        }
        Some(Resolved::Actions(Actions::new(hub_actions)))
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
    use crate::action::{Action, Actions, FocusTarget};
    use crate::config::{Binding, CallbackId, Keymap, Modifiers};
    use std::collections::HashMap;

    fn km(key: &str, mods: Modifiers) -> Keymap {
        Keymap {
            key: key.to_string(),
            modifiers: mods,
        }
    }

    fn focus_left_actions() -> Actions {
        Actions::new(vec![Action::Focus(FocusTarget::Left)])
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
            default: default
                .into_iter()
                .map(|(k, a)| (k, Binding::Static(a)))
                .collect(),
            modes: modes
                .into_iter()
                .map(|(name, bindings)| {
                    (
                        name.to_string(),
                        bindings
                            .into_iter()
                            .map(|(k, a)| (k, Binding::Static(a)))
                            .collect(),
                    )
                })
                .collect(),
        }
    }

    fn expect_focus_left(resolved: Option<Resolved>) {
        match resolved {
            Some(Resolved::Actions(a)) => assert_eq!(a.to_string(), "[focus left]"),
            other => panic!("expected [focus left], got {other:?}"),
        }
    }

    #[test]
    fn keymap_state_resolve_default_mode() {
        let cmd_h = km("h", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h.clone(), focus_left_actions())], vec![]);
        let mut state = KeymapState::new(keymaps);
        expect_focus_left(state.resolve(&cmd_h));
    }

    #[test]
    fn keymap_state_resolve_callback_binding_suppresses() {
        let cmd_c = km("c", Modifiers::META);
        let mut default = HashMap::new();
        default.insert(cmd_c.clone(), Binding::Callback(CallbackId(3)));
        let keymaps = ModalKeymaps {
            default,
            modes: HashMap::new(),
        };
        let mut state = KeymapState::new(keymaps);
        match state.resolve(&cmd_c) {
            Some(Resolved::Callback(id)) => assert_eq!(id, CallbackId(3)),
            other => panic!("expected callback, got {other:?}"),
        }
    }

    #[test]
    fn keymap_state_resolve_no_binding() {
        let cmd_h = km("h", Modifiers::META);
        let cmd_j = km("j", Modifiers::META);
        let keymaps = make_keymaps(vec![(cmd_h, focus_left_actions())], vec![]);
        let mut state = KeymapState::new(keymaps);
        assert!(state.resolve(&cmd_j).is_none());
    }

    #[test]
    fn keymap_state_resolve_custom_mode() {
        let cmd_h = km("h", Modifiers::META);
        let h = km("h", Modifiers::empty());
        let keymaps = make_keymaps(
            vec![(cmd_h.clone(), focus_left_actions())],
            vec![("resize", vec![(h.clone(), focus_left_actions())])],
        );
        let mut state = KeymapState::new(keymaps);
        state.switch_mode("resize");

        // h resolves in resize mode
        expect_focus_left(state.resolve(&h));

        // cmd+h does NOT resolve in resize mode (not bound there)
        assert!(state.resolve(&cmd_h).is_none());
    }

    #[test]
    fn keymap_state_resolve_filters_mode_actions() {
        let cmd_r = km("r", Modifiers::META);
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
        let cmd_r = km("r", Modifiers::META);
        let keymaps = make_keymaps(
            vec![(
                cmd_r.clone(),
                Actions::new(vec![
                    Action::Focus(FocusTarget::Left),
                    mode_action("resize"),
                ]),
            )],
            vec![("resize", vec![])],
        );
        let mut state = KeymapState::new(keymaps);
        expect_focus_left(state.resolve(&cmd_r));
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
        let cmd_h = km("h", Modifiers::META);
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
        let cmd_h = km("h", Modifiers::META);
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
        expect_focus_left(state.resolve(&cmd_h));
    }

    #[test]
    fn keymap_state_resolve_falls_back_when_key_unbound_in_default() {
        let cmd_h = km("h", Modifiers::META);
        let cmd_j = km("j", Modifiers::META);
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
