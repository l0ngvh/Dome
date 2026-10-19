use super::observations::{NativeAppObservation, NativeWindowObservation};
use super::{
    AppObservation, NativeState, NativeWindowServer, WindowNotification, WindowObservation,
};
use crate::platform::macos::accessibility::AccessibilityApplication;
use crate::platform::macos::running_application::RunningApp;
use anyhow::{Context, Result, anyhow};
use dome_core::WindowId;
use objc2_core_foundation::{CFDictionary, CFNumber, CFType};
use objc2_core_graphics::{
    CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption, kCGWindowNumber, kCGWindowOwnerPID,
};
use std::collections::HashMap;

impl NativeWindowServer {
    pub(super) fn register_current_windows(&self) -> Result<()> {
        let apps: HashMap<_, _> = RunningApp::all()
            .map(|app| (app.pid(), !app.is_hidden()))
            .collect();
        self.state
            .borrow_mut()
            .reconcile_current_windows(apps, current_native_windows())
    }

    pub(super) fn refresh_all_observers(&self) {
        let mut state = self.state.borrow_mut();
        state.observed_pids.clear();
        for application in state.applications.values() {
            application.refresh_observer();
        }
    }
}

impl NativeState {
    pub(super) fn application_for_window(
        &self,
        id: WindowId,
    ) -> Result<(CGWindowID, &AccessibilityApplication)> {
        let native = *self
            .native_ids
            .get(&id)
            .ok_or_else(|| anyhow!("Unknown window {id}"))?;
        let pid = self
            .window_pids
            .get(&native)
            .ok_or_else(|| anyhow!("Unknown owner for {id}"))?;
        let application = self
            .applications
            .get(pid)
            .ok_or_else(|| anyhow!("Unknown application {pid}"))?;
        Ok((native, application))
    }

    pub(super) fn requested_native_ids(&self, ids: &[WindowId]) -> Vec<CGWindowID> {
        ids.iter()
            .filter_map(|id| self.native_ids.get(id).copied())
            .collect()
    }

    pub(super) fn reconcile_current_windows(
        &mut self,
        apps: HashMap<i32, bool>,
        windows: Result<HashMap<CGWindowID, i32>>,
    ) -> Result<()> {
        let windows = windows?;
        let terminated = self
            .apps
            .keys()
            .filter(|pid| !apps.contains_key(pid))
            .copied()
            .collect::<Vec<_>>();
        for pid in terminated {
            self.retire_app(pid);
            if self.apps.remove(&pid) == Some(true) {
                self.emit(WindowNotification::AppRemoved { pid });
            }
        }
        for (pid, visible) in apps {
            self.ensure_application(pid);
            let previous = self.apps.insert(pid, visible);
            if !visible && previous != Some(false) {
                self.movement.remove(&pid);
                self.emit(WindowNotification::AppRemoved { pid });
            }
        }
        let absent = self
            .window_ids
            .keys()
            .filter(|id| !windows.contains_key(id))
            .copied()
            .collect::<Vec<_>>();
        for id in absent {
            self.retire_window(id);
        }
        for (id, pid) in windows {
            if self.apps.contains_key(&pid) {
                self.register_window(id, pid);
            }
        }
        Ok(())
    }

    pub(super) fn register_window(&mut self, native_id: CGWindowID, pid: i32) -> WindowId {
        self.window_pids.insert(native_id, pid);
        if let Some(&id) = self.window_ids.get(&native_id) {
            return id;
        }
        let id = WindowId::new(self.next_window_id);
        self.next_window_id = self
            .next_window_id
            .checked_add(1)
            .expect("window identities exhausted");
        self.window_ids.insert(native_id, id);
        self.native_ids.insert(id, native_id);
        id
    }

    pub(super) fn retire_window(&mut self, native_id: CGWindowID) {
        if let Some(pid) = self.window_pids.remove(&native_id)
            && let Some(application) = self.applications.get(&pid)
        {
            application.forget_window(native_id);
        }
        let mut focus = self.focus.lock().expect("focus lock");
        if focus.pending == Some(native_id) {
            focus.pending = None;
        }
        if focus.submitted == Some(native_id) {
            focus.submitted = None;
        }
        drop(focus);
        if let Some(id) = self.window_ids.remove(&native_id) {
            self.native_ids.remove(&id);
            self.emit(WindowNotification::WindowRemoved(id));
        }
    }

    pub(super) fn retire_app(&mut self, pid: i32) {
        self.applications.remove(&pid);
        self.observed_pids.remove(&pid);
        let ids: Vec<_> = self
            .window_pids
            .iter()
            .filter_map(|(&id, &owner)| (owner == pid).then_some(id))
            .collect();
        for id in ids {
            self.retire_window(id);
        }
    }

    pub(super) fn emit(&self, update: WindowNotification) {
        let _ = self.notifications.send(update);
    }

    pub(super) fn eligible(&self, native_id: CGWindowID, pid: i32) -> bool {
        self.apps.get(&pid) == Some(&true) && self.window_pids.get(&native_id) == Some(&pid)
    }

    pub(super) fn accept_focus(&self, window: &super::NativeFocusedWindow) -> Option<WindowId> {
        self.translate_window_id(window.cg_id, window.pid)
    }

    pub(super) fn accept_app(&mut self, native: NativeAppObservation) -> Option<AppObservation> {
        let (observation, retired) =
            filter_completed_app(native, &self.apps, &self.window_ids, &self.window_pids)?;
        for id in retired {
            self.retire_window(id);
        }
        Some(observation)
    }

    fn ensure_application(&mut self, pid: i32) {
        if !self.applications.contains_key(&pid)
            && let Some(app) = RunningApp::new(pid)
        {
            self.applications.insert(
                pid,
                AccessibilityApplication::new(
                    &app,
                    self.focus.clone(),
                    self.sender.clone(),
                    self.suspended.clone(),
                ),
            );
        }
    }

    fn translate_window_id(&self, native_id: CGWindowID, pid: i32) -> Option<WindowId> {
        if !self.eligible(native_id, pid) {
            return None;
        }
        self.window_ids.get(&native_id).copied()
    }
}

fn current_native_windows() -> Result<HashMap<CGWindowID, i32>> {
    let windows = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0)
        .context("Core Graphics window list unavailable")?;
    let mut identities = HashMap::new();
    // Reject incomplete decoding rather than interpret a missing identity as destroyed.
    for window in unsafe { windows.cast_unchecked::<CFType>() } {
        let dictionary = window
            .downcast_ref::<CFDictionary>()
            .context("Core Graphics window entry is not a dictionary")?;
        let dictionary = unsafe { dictionary.cast_unchecked::<CFType, CFType>() };
        let number = |key: &CFType| {
            dictionary
                .get(key)
                .and_then(|value| value.downcast::<CFNumber>().ok())
                .and_then(|number| number.as_i64())
        };
        let id = number(unsafe { kCGWindowNumber }.as_ref())
            .context("Core Graphics window number unavailable")?;
        let pid = number(unsafe { kCGWindowOwnerPID }.as_ref())
            .context("Core Graphics window owner unavailable")?;
        identities.insert(CGWindowID::try_from(id)?, i32::try_from(pid)?);
    }
    Ok(identities)
}

fn filter_completed_app(
    native: NativeAppObservation,
    apps: &HashMap<i32, bool>,
    window_ids: &HashMap<CGWindowID, WindowId>,
    window_pids: &HashMap<CGWindowID, i32>,
) -> Option<(AppObservation, Vec<CGWindowID>)> {
    if apps.get(&native.pid) != Some(&true) {
        return None;
    }
    let eligible = |id, pid| apps.get(&pid) == Some(&true) && window_pids.get(&id) == Some(&pid);
    let retired = native
        .invalid
        .into_iter()
        .filter(|&id| {
            eligible(id, native.pid) && !native.listed.iter().any(|window| window.cg_id == id)
        })
        .collect::<Vec<_>>();
    let translate_id = |id| {
        if retired.contains(&id) {
            None
        } else {
            window_ids.get(&id).copied()
        }
    };
    let translate = |window: NativeWindowObservation| {
        if !eligible(window.cg_id, window.pid) {
            return None;
        }
        let id = translate_id(window.cg_id)?;
        Some(translate_observation(id, window))
    };
    let requested = native
        .requested
        .into_values()
        .filter_map(|window| {
            let window = translate(window)?;
            Some((window.id, window))
        })
        .collect();
    let listed = native.listed.into_iter().filter_map(translate).collect();
    let requested_ids = native
        .requested_ids
        .into_iter()
        .filter_map(translate_id)
        .collect();
    Some((
        AppObservation::new(native.pid, listed, requested, requested_ids),
        retired,
    ))
}

fn translate_observation(id: WindowId, native: NativeWindowObservation) -> WindowObservation {
    WindowObservation {
        id,
        pid: native.pid,
        metadata: native.metadata,
        valid: native.valid,
        minimized: native.minimized,
        native_fullscreen: native.native_fullscreen,
        role: native.role,
        subrole: native.subrole,
        is_root: native.is_root,
        position_settable: native.position_settable,
        size_settable: native.size_settable,
        main_settable: native.main_settable,
        rect: native.rect,
        placement: native.placement,
        corner_radius: native.corner_radius,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::macos::window_server::WindowMetadata;
    use dome_core::PixelRect;
    use std::collections::HashSet;

    fn observation(native: CGWindowID) -> NativeWindowObservation {
        NativeWindowObservation {
            cg_id: native,
            pid: 7,
            metadata: WindowMetadata {
                title: Some(format!("Window {native}")),
                app_name: Some("App".into()),
                bundle_id: None,
            },
            valid: true,
            minimized: false,
            native_fullscreen: Some(false),
            role: Some("AXWindow".into()),
            subrole: Some("AXStandardWindow".into()),
            is_root: true,
            position_settable: true,
            size_settable: true,
            main_settable: true,
            rect: Some(PixelRect::new(1, 2, 300, 200)),
            placement: None,
            corner_radius: None,
        }
    }

    fn completed() -> NativeAppObservation {
        NativeAppObservation {
            pid: 7,
            listed: vec![observation(91), observation(92)],
            requested: HashMap::from([(91, observation(91)), (92, observation(92))]),
            requested_ids: vec![91, 92],
            invalid: Vec::new(),
        }
    }

    #[test]
    fn complete_app_read_while_hidden_or_absent_rejects_results_until_visible() {
        let window_ids = HashMap::from([(91, WindowId::new(1)), (92, WindowId::new(2))]);
        let window_pids = HashMap::from([(91, 7), (92, 7)]);
        for apps in [HashMap::new(), HashMap::from([(7, false)])] {
            assert!(filter_completed_app(completed(), &apps, &window_ids, &window_pids).is_none());
        }
        let (accepted, retired) = filter_completed_app(
            completed(),
            &HashMap::from([(7, true)]),
            &window_ids,
            &window_pids,
        )
        .unwrap();
        assert!(retired.is_empty());
        assert_eq!(
            accepted
                .listed
                .iter()
                .map(|window| window.id)
                .collect::<Vec<_>>(),
            [WindowId::new(1), WindowId::new(2)]
        );
        assert_eq!(
            accepted.requested_ids,
            HashSet::from([WindowId::new(1), WindowId::new(2)])
        );
    }

    #[test]
    fn complete_app_read_with_invalid_unlisted_window_filters_all_results() {
        let mut native = completed();
        native.listed.retain(|window| window.cg_id != 91);
        native.invalid = vec![91, 91];
        let (accepted, retired) = filter_completed_app(
            native,
            &HashMap::from([(7, true)]),
            &HashMap::from([(91, WindowId::new(1)), (92, WindowId::new(2))]),
            &HashMap::from([(91, 7), (92, 7)]),
        )
        .unwrap();
        assert!(retired.contains(&91));
        assert!(!retired.contains(&92));
        assert_eq!(accepted.listed.len(), 1);
        assert_eq!(accepted.listed[0].id, WindowId::new(2));
        assert_eq!(
            accepted.requested.keys().copied().collect::<HashSet<_>>(),
            HashSet::from([WindowId::new(2)])
        );
        assert_eq!(accepted.requested_ids, HashSet::from([WindowId::new(2)]));
    }

    #[test]
    fn complete_app_read_with_invalid_window_still_listed_preserves_identity() {
        let mut native = completed();
        native.invalid = vec![91];
        let (accepted, retired) = filter_completed_app(
            native,
            &HashMap::from([(7, true)]),
            &HashMap::from([(91, WindowId::new(1)), (92, WindowId::new(2))]),
            &HashMap::from([(91, 7), (92, 7)]),
        )
        .unwrap();
        assert!(retired.is_empty());
        assert_eq!(accepted.listed[0].id, WindowId::new(1));
        assert_eq!(
            accepted.requested[&WindowId::new(1)]
                .metadata
                .title
                .as_deref(),
            Some("Window 91")
        );
        assert!(accepted.requested_ids.contains(&WindowId::new(1)));
    }

    #[test]
    fn complete_app_read_without_current_identity_filters_stale_list_and_requests() {
        let (accepted, retired) = filter_completed_app(
            completed(),
            &HashMap::from([(7, true)]),
            &HashMap::from([(92, WindowId::new(2))]),
            &HashMap::from([(92, 7)]),
        )
        .unwrap();
        assert!(retired.is_empty());
        assert_eq!(accepted.listed.len(), 1);
        assert_eq!(accepted.listed[0].id, WindowId::new(2));
        assert_eq!(
            accepted.requested.keys().copied().collect::<HashSet<_>>(),
            HashSet::from([WindowId::new(2)])
        );
        assert_eq!(accepted.requested_ids, HashSet::from([WindowId::new(2)]));
    }
}
