use std::time::{Duration, Instant};

use anyhow::Result;
use dome_core::WindowId;

use super::{NativeEvent, NativeState, send_native_event};
use crate::platform::macos::timer::Timer;

const FOCUS_INTERVAL: Duration = Duration::from_millis(100);

impl NativeState {
    pub(super) fn request_focus(&mut self, id: WindowId) -> Result<()> {
        let (native, _) = self.application_for_window(id)?;
        let delay = {
            let mut focus = self.focus.lock().expect("focus lock");
            focus.pending = Some(native);
            focus
                .last_external_submission_at
                .map(|last| (last + FOCUS_INTERVAL).saturating_duration_since(Instant::now()))
                .unwrap_or_default()
        };
        if delay.is_zero() {
            self.submit_focus();
        } else if self.focus_timer.is_none() {
            let sender = self.sender.clone();
            self.focus_timer = Some(Timer::once(delay, move || {
                send_native_event(&sender, NativeEvent::SubmitFocus);
            }));
        }
        Ok(())
    }

    pub(super) fn submit_focus(&mut self) {
        self.focus_timer.take();
        let mut focus = self.focus.lock().expect("focus lock");
        if focus.pending.is_none() {
            return;
        }
        if let Some(last) = focus.last_external_submission_at {
            let remaining = (last + FOCUS_INTERVAL).saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                let sender = self.sender.clone();
                self.focus_timer = Some(Timer::once(remaining, move || {
                    send_native_event(&sender, NativeEvent::SubmitFocus);
                }));
                return;
            }
        }
        let native = focus.pending.take().expect("pending focus");
        let Some(&pid) = self.window_pids.get(&native) else {
            return;
        };
        if !self.eligible(native, pid) {
            return;
        }
        let Some(application) = self.applications.get(&pid) else {
            return;
        };
        focus.submitted = Some(native);
        focus.last_external_submission_at = Some(Instant::now());
        drop(focus);
        application.focus(native);
    }

    pub(super) fn focus_overlay(&mut self) {
        self.focus_timer.take();
        let mut focus = self.focus.lock().expect("focus lock");
        focus.pending = None;
        focus.submitted = None;
    }
}
