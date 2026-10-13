use crate::core::MonitorId;
use crate::platform::windows::dome::events::FocusTarget;
use crate::platform::windows::external::HwndId;
use crate::platform::windows::window_server::WindowServer;
use std::time::{Duration, Instant};

#[derive(Default)]
pub(super) struct FocusState {
    last_request: Option<FocusTarget>,
    pending: Option<HwndId>,
    last_external_submission_at: Option<Instant>,
}

impl FocusState {
    pub(super) fn on_scene_focus(&mut self, request: Option<FocusTarget>) -> Option<MonitorId> {
        if request == self.last_request && request.is_some() {
            return None;
        }
        self.last_request = request;
        self.pending = match request {
            Some(FocusTarget::Window(key)) => Some(key),
            _ => None,
        };
        match request {
            Some(FocusTarget::Monitor(monitor)) => Some(monitor),
            _ => None,
        }
    }
    pub(super) fn request_changed(&self, request: Option<FocusTarget>) -> bool {
        request != self.last_request || request.is_none()
    }
    pub(super) fn due(&self, now: Instant) -> bool {
        self.pending.is_some()
            && self
                .last_external_submission_at
                .is_none_or(|at| now >= at + Duration::from_millis(100))
    }
    pub(super) fn pending(&self) -> Option<HwndId> {
        self.pending
    }
    pub(super) fn clear(&mut self) {
        self.pending = None;
    }
    pub(super) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        self.pending.map(|_| {
            self.last_external_submission_at
                .map_or(now, |at| at + Duration::from_millis(100))
        })
    }
    pub(super) fn submit_pending_focus(&mut self, now: Instant, windows: &mut dyn WindowServer) {
        if self
            .last_external_submission_at
            .is_some_and(|at| now < at + Duration::from_millis(100))
        {
            return;
        }
        if let Some(key) = self.pending.take() {
            self.last_external_submission_at = Some(now);
            windows.focus(key);
        }
    }
}
