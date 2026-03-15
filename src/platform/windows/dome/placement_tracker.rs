use std::collections::HashMap;
use std::time::Duration;

use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};

use crate::platform::windows::handle::ManagedHwnd;

use super::Dome;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_TIMEOUT: Duration = Duration::from_secs(60);

enum MoveKind {
    UserDrag,
    Programmatic,
}

pub(super) struct PlacementTracker {
    windows: HashMap<ManagedHwnd, (MoveKind, RegistrationToken)>,
    handle: LoopHandle<'static, Dome>,
}

impl PlacementTracker {
    pub(super) fn new(handle: LoopHandle<'static, Dome>) -> Self {
        Self {
            windows: HashMap::new(),
            handle,
        }
    }

    pub(super) fn drag_started(&mut self, hwnd: ManagedHwnd) {
        self.cancel(hwnd);
        let token = self
            .handle
            .insert_source(
                Timer::from_duration(DRAG_SAFETY_TIMEOUT),
                move |_, _, dome: &mut Dome| {
                    dome.placement_tracker.windows.remove(&hwnd);
                    dome.handle_resize(hwnd);
                    TimeoutAction::Drop
                },
            )
            .expect("Failed to insert timer");
        self.windows.insert(hwnd, (MoveKind::UserDrag, token));
    }

    pub(super) fn drag_ended(&mut self, hwnd: ManagedHwnd) {
        self.cancel(hwnd);
    }

    pub(super) fn location_changed(&mut self, hwnd: ManagedHwnd) {
        if matches!(self.windows.get(&hwnd), Some((MoveKind::UserDrag, _))) {
            return;
        }
        self.cancel(hwnd);
        let token = self
            .handle
            .insert_source(
                Timer::from_duration(DEBOUNCE_INTERVAL),
                move |_, _, dome: &mut Dome| {
                    dome.placement_tracker.windows.remove(&hwnd);
                    dome.handle_resize(hwnd);
                    TimeoutAction::Drop
                },
            )
            .expect("Failed to insert timer");
        self.windows.insert(hwnd, (MoveKind::Programmatic, token));
    }

    pub(super) fn clear(&mut self, hwnd: ManagedHwnd) {
        self.cancel(hwnd);
    }

    pub(super) fn is_moving(&self, hwnd: ManagedHwnd) -> bool {
        self.windows.contains_key(&hwnd)
    }

    fn cancel(&mut self, hwnd: ManagedHwnd) {
        if let Some((_, token)) = self.windows.remove(&hwnd) {
            self.handle.remove(token);
        }
    }
}
