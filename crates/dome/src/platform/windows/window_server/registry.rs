use super::{NativeEvent, PlacementObservation, WindowNotification, WindowObservation};
use crate::platform::windows::external::HwndId;
use dome_core::{LimitObservation, Physical, PixelRect, WindowId};
use std::collections::HashMap;
use std::time::Instant;

pub(super) struct NativeWindowRegistry {
    next_window_id: usize,
    by_native: HashMap<HwndId, WindowRecord>,
    by_id: HashMap<WindowId, HwndId>,
}

impl NativeWindowRegistry {
    pub(super) fn window_id(&self, native: HwndId) -> Option<WindowId> {
        self.by_native.get(&native).map(|record| record.id)
    }

    pub(super) fn native_id(&self, id: WindowId) -> Option<HwndId> {
        self.by_id.get(&id).copied()
    }

    pub(super) fn new() -> Self {
        Self {
            next_window_id: 1,
            by_native: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    pub(super) fn register(&mut self, native: HwndId) -> WindowId {
        if let Some(record) = self.by_native.get(&native) {
            return record.id;
        }
        let id = WindowId::new(self.next_window_id);
        self.next_window_id = self
            .next_window_id
            .checked_add(1)
            .expect("window IDs exhausted");
        self.by_native.insert(
            native,
            WindowRecord {
                id,
                visible: None,
                cloaked: None,
                minimized: None,
            },
        );
        self.by_id.insert(id, native);
        id
    }

    pub(super) fn consume(&mut self, event: &NativeEvent) -> Option<WindowId> {
        let native = event.native();
        if matches!(event, NativeEvent::WindowDestroyed(_)) {
            let record = self.by_native.remove(&native)?;
            self.by_id.remove(&record.id);
            return Some(record.id);
        }
        if matches!(
            event,
            NativeEvent::WindowCreated(_)
                | NativeEvent::WindowShown(_)
                | NativeEvent::WindowUncloaked(_)
        ) {
            self.register(native);
        }
        let record = self.by_native.get_mut(&native)?;
        match event {
            NativeEvent::WindowShown(_) => record.visible = Some(true),
            NativeEvent::WindowHidden(_) => record.visible = Some(false),
            NativeEvent::WindowCloaked(_) => record.cloaked = Some(true),
            NativeEvent::WindowUncloaked(_) => record.cloaked = Some(false),
            NativeEvent::WindowMinimized(_) => record.minimized = Some(true),
            NativeEvent::WindowRestored { .. } => record.minimized = Some(false),
            _ => {}
        }
        Some(record.id)
    }

    pub(super) fn eligible(&self, id: WindowId) -> bool {
        self.by_id
            .get(&id)
            .and_then(|native| self.by_native.get(native))
            .is_some_and(|record| record.visible != Some(false) && record.cloaked != Some(true))
    }

    pub(super) fn complete(
        &self,
        mut window: WindowObservation,
        rect: PixelRect<Physical>,
        monitor: isize,
        dragged_at: Option<Instant>,
    ) -> Option<WindowNotification> {
        self.accept(&mut window)
            .then_some(WindowNotification::WindowObserved {
                window,
                rect,
                monitor,
                dragged_at,
            })
    }

    pub(super) fn complete_placement(
        &self,
        window_id: WindowId,
        observed_at: Instant,
        result: Option<(PlacementObservation, LimitObservation)>,
    ) -> Option<WindowNotification> {
        if !self.eligible(window_id) {
            return None;
        }
        let (placement, constraints) = result.map_or((None, None), |(placement, constraints)| {
            let minimized = self
                .by_id
                .get(&window_id)
                .and_then(|native| self.by_native.get(native))
                .is_some_and(|record| record.minimized == Some(true));
            (
                Some(if minimized {
                    PlacementObservation::Minimized
                } else {
                    placement
                }),
                Some(constraints),
            )
        });
        Some(WindowNotification::PlacementChanged {
            window_id,
            observed_at,
            placement,
            constraints,
        })
    }

    fn accept(&self, window: &mut WindowObservation) -> bool {
        let Some(record) = self
            .by_id
            .get(&window.id)
            .and_then(|native| self.by_native.get(native))
        else {
            return false;
        };
        if let Some(visible) = record.visible {
            window.state.visible = visible;
        }
        if let Some(cloaked) = record.cloaked {
            window.state.cloaked = cloaked;
        }
        if let Some(minimized) = record.minimized {
            window.state.minimized = minimized;
        }
        window.state.visible && !window.state.cloaked
    }
}

struct WindowRecord {
    id: WindowId,
    visible: Option<bool>,
    cloaked: Option<bool>,
    minimized: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::windows::window_server::{NativeWindowState, WindowMetadata};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW;

    fn native() -> HwndId {
        HWND(123 as *mut _).into()
    }

    fn observation(id: WindowId) -> WindowObservation {
        WindowObservation {
            id,
            pid: 1,
            metadata: WindowMetadata {
                title: Some("Observed window".into()),
                process: "test.exe".into(),
                process_path: None,
                class: None,
                aumid: None,
                app_name: None,
            },
            constraints: LimitObservation::default(),
            state: NativeWindowState {
                visible: true,
                minimized: false,
                cloaked: false,
                is_root: true,
                has_owner: false,
                style: WS_OVERLAPPEDWINDOW.0,
                extended_style: 0,
                elevated: false,
                window_rect: PixelRect::new(10, 10, 80, 60),
                monitor_work_area: None,
            },
        }
    }

    fn complete(
        records: &NativeWindowRegistry,
        window: WindowObservation,
    ) -> Option<WindowNotification> {
        let rect = window.state.window_rect;
        records.complete(window, rect, 1, None)
    }

    #[test]
    fn complete_observations_after_lifecycle_changes_applies_current_restrictions() {
        let mut records = NativeWindowRegistry::new();
        let id = records
            .consume(&NativeEvent::WindowCreated(native()))
            .unwrap();
        for event in [
            NativeEvent::WindowHidden(native()),
            NativeEvent::WindowCloaked(native()),
            NativeEvent::WindowShown(native()),
        ] {
            records.consume(&event);
            assert!(complete(&records, observation(id)).is_none());
        }
        records.consume(&NativeEvent::WindowUncloaked(native()));
        records.consume(&NativeEvent::WindowMinimized(native()));
        let mut old = observation(id);
        old.state.visible = false;
        old.state.cloaked = true;
        let Some(WindowNotification::WindowObserved { window, .. }) = complete(&records, old)
        else {
            panic!("show and uncloak must admit the completed observation");
        };
        assert_eq!(window.id, id);
        assert!(window.state.visible && !window.state.cloaked && window.state.minimized);
    }

    #[test]
    fn complete_admission_after_destroy_rejects_old_result_even_when_handle_is_reused() {
        let mut records = NativeWindowRegistry::new();
        let id = records
            .consume(&NativeEvent::WindowCreated(native()))
            .unwrap();
        let pending = observation(id);
        records.consume(&NativeEvent::WindowDestroyed(native()));
        assert!(complete(&records, observation(id)).is_none());
        let replacement = records
            .consume(&NativeEvent::WindowCreated(native()))
            .unwrap();
        assert!(complete(&records, pending).is_none());
        assert!(
            matches!(complete(&records, observation(replacement)), Some(WindowNotification::WindowObserved { window, .. }) if window.id == replacement)
        );
    }

    #[test]
    fn complete_placement_after_minimize_prevents_stale_visible_reattachment() {
        let mut records = NativeWindowRegistry::new();
        let id = records
            .consume(&NativeEvent::WindowCreated(native()))
            .unwrap();
        let observed_at = Instant::now();
        let old = observation(id);
        let pending = (
            PlacementObservation::Visible {
                rect: old.state.window_rect,
                monitor: 1,
            },
            old.constraints,
        );
        records.consume(&NativeEvent::WindowMinimized(native()));
        assert!(matches!(
            records.complete_placement(id, observed_at, Some(pending)),
            Some(WindowNotification::PlacementChanged {
                window_id, placement: Some(PlacementObservation::Minimized), ..
            }) if window_id == id
        ));
        records.consume(&NativeEvent::WindowRestored {
            hwnd_id: native(),
            observed_at: Instant::now(),
        });
        let restored = observation(id);
        assert!(matches!(
            records.complete_placement(
                id,
                Instant::now(),
                Some((
                    PlacementObservation::Visible {
                        rect: restored.state.window_rect,
                        monitor: 1
                    },
                    restored.constraints
                ))
            ),
            Some(WindowNotification::PlacementChanged {
                placement: Some(PlacementObservation::Visible { .. }),
                ..
            })
        ));
    }
}
