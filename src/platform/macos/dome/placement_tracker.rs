use std::collections::HashMap;
use std::time::Duration;

use calloop::timer::{TimeoutAction, Timer};
use calloop::{LoopHandle, RegistrationToken};

use super::Dome;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct PlacementTracker {
    timers: HashMap<i32, RegistrationToken>,
    handle: LoopHandle<'static, Dome>,
}

impl PlacementTracker {
    pub(super) fn new(handle: LoopHandle<'static, Dome>) -> Self {
        Self {
            timers: HashMap::new(),
            handle,
        }
    }

    pub(super) fn window_moved(&mut self, pid: i32) {
        if let Some(token) = self.timers.remove(&pid) {
            self.handle.remove(token);
        }
        let token = self
            .handle
            .insert_source(
                Timer::from_duration(DEBOUNCE_INTERVAL),
                move |_, _, dome: &mut Dome| {
                    dome.placement_tracker.timers.remove(&pid);
                    dome.dispatch_refresh_windows(pid);
                    TimeoutAction::Drop
                },
            )
            .expect("Failed to insert timer");
        self.timers.insert(pid, token);
    }

    pub(super) fn cancel(&mut self, pid: i32) {
        if let Some(token) = self.timers.remove(&pid) {
            self.handle.remove(token);
        }
    }

    pub(super) fn is_moving(&self, pid: i32) -> bool {
        self.timers.contains_key(&pid)
    }
}
