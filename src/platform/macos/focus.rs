use super::window_server::WindowServer;
use objc2_core_graphics::CGWindowID;
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

const EXTERNAL_FOCUS_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct SharedFocus {
    state: Mutex<FocusState>,
    wake: OnceLock<Box<dyn Fn() + Send + Sync>>,
}

#[derive(Default)]
struct FocusState {
    pending: Option<CGWindowID>,
    submitted: Option<CGWindowID>,
    last_external_submission_at: Option<Instant>,
}

impl SharedFocus {
    pub(super) fn new() -> Self {
        Self {
            state: Mutex::new(FocusState::default()),
            wake: OnceLock::new(),
        }
    }

    /// Bind once after app construction and before enabling focus producers.
    pub(super) fn bind_waker(&self, wake: impl Fn() + Send + Sync + 'static) {
        assert!(
            self.wake.set(Box::new(wake)).is_ok(),
            "focus waker already bound"
        );
    }

    pub(super) fn request(&self, window: CGWindowID) {
        {
            self.lock_state().pending = Some(window);
        }
        self.wake.get().expect("focus waker bound before requests")();
    }

    pub(super) fn next_deadline(&self, now: Instant) -> Option<Instant> {
        let state = self.lock_state();
        state.pending?;
        Some(
            state
                .last_external_submission_at
                .map_or(now, |at| (at + EXTERNAL_FOCUS_INTERVAL).max(now)),
        )
    }

    pub(super) fn clear_pending(&self) {
        self.lock_state().pending = None;
    }

    pub(super) fn clear_external(&self) {
        let mut state = self.lock_state();
        state.pending = None;
        state.submitted = None;
    }

    #[cfg(test)]
    pub(super) fn assert_unlocked(&self) {
        assert!(
            self.state.try_lock().is_ok(),
            "focus mutex spans native call"
        );
    }

    pub(super) fn is_submitted(&self, window: CGWindowID) -> bool {
        self.lock_state().submitted == Some(window)
    }

    pub(super) fn submit_pending_focus(&self, now: Instant, windows: &mut dyn WindowServer) {
        let window = {
            let mut state = self.lock_state();
            if state
                .last_external_submission_at
                .is_some_and(|at| now < at + EXTERNAL_FOCUS_INTERVAL)
            {
                return;
            }
            let Some(window) = state.pending.take() else {
                return;
            };
            state.submitted = Some(window);
            state.last_external_submission_at = Some(now);
            window
        };
        windows.focus(window);
    }

    fn lock_state(&self) -> MutexGuard<'_, FocusState> {
        match self.state.lock() {
            Ok(state) => state,
            Err(poisoned) => {
                let mut state = poisoned.into_inner();
                *state = FocusState {
                    last_external_submission_at: state.last_external_submission_at,
                    ..FocusState::default()
                };
                self.state.clear_poison();
                state
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{Length, PixelRect};
    use crate::platform::macos::window_server::{Observation, RecoveryEntry};
    use std::sync::{Arc, mpsc};

    struct FocusServer {
        focus: Arc<SharedFocus>,
        submissions: Vec<CGWindowID>,
    }

    impl WindowServer for FocusServer {
        fn focus(&mut self, window: CGWindowID) {
            assert!(
                self.focus.state.try_lock().is_ok(),
                "lock spans server call"
            );
            assert!(self.focus.is_submitted(window));
            self.submissions.push(window);
        }
        fn ingest_native_events(&mut self, _: Instant) {
            panic!("focus polled native events")
        }
        fn ingest_read_results(&mut self, _: Instant) {
            panic!("focus polled results")
        }
        fn has_observations(&self) -> bool {
            panic!("focus polled observations")
        }
        fn take_observations(&mut self) -> Vec<Observation> {
            panic!("focus drained observations")
        }
        fn next_deadline(&self) -> Option<Instant> {
            panic!("focus polled server deadline")
        }
        fn refresh_applications(&mut self, _: Instant) {
            panic!("focus refreshed applications")
        }
        fn set_frame(&mut self, _: CGWindowID, _: PixelRect) {
            panic!("focus placed window")
        }
        fn move_offscreen(&mut self, _: CGWindowID, _: (Length, Length)) {
            panic!("focus parked window")
        }
        fn minimize(&mut self, _: CGWindowID) {
            panic!("focus minimized window")
        }
        fn unminimize(&mut self, _: CGWindowID) {
            panic!("focus restored window")
        }
        fn close(&mut self, _: CGWindowID) {
            panic!("focus closed window")
        }
        fn recover(&mut self, _: CGWindowID, _: PixelRect) {
            panic!("focus recovered window")
        }
        fn shutdown(&mut self, _: Vec<RecoveryEntry>) {
            panic!("focus stopped server")
        }
    }

    fn fixture() -> (Arc<SharedFocus>, FocusServer, mpsc::Receiver<()>) {
        let focus = Arc::new(SharedFocus::new());
        let weak = Arc::downgrade(&focus);
        let (sender, wakes) = mpsc::channel();
        focus.bind_waker(move || {
            let focus = weak.upgrade().unwrap();
            assert!(focus.state.try_lock().is_ok(), "lock spans wake");
            sender.send(()).unwrap();
        });
        let server = FocusServer {
            focus: focus.clone(),
            submissions: Vec::new(),
        };
        (focus, server, wakes)
    }

    #[test]
    fn request_focus_releases_newest_at_fixed_boundary() {
        let (focus, mut server, wakes) = fixture();
        let now = Instant::now();
        focus.request(1);
        assert!(wakes.try_recv().is_ok());
        assert_eq!(focus.next_deadline(now), Some(now));
        focus.submit_pending_focus(now, &mut server);
        assert_eq!(server.submissions, [1]);
        assert_eq!(focus.next_deadline(now), None);
        focus.request(2);
        focus.request(3);
        assert_eq!(wakes.try_iter().count(), 2);
        let boundary = now + Duration::from_millis(100);
        assert_eq!(
            focus.next_deadline(now + Duration::from_millis(80)),
            Some(boundary)
        );
        focus.submit_pending_focus(boundary - Duration::from_nanos(1), &mut server);
        assert_eq!(server.submissions, [1]);
        focus.submit_pending_focus(boundary, &mut server);
        focus.submit_pending_focus(boundary, &mut server);
        assert_eq!(server.submissions, [1, 3]);
        focus.request(3);
        assert!(wakes.try_recv().is_ok());
        focus.submit_pending_focus(boundary + Duration::from_millis(100), &mut server);
        assert_eq!(server.submissions, [1, 3, 3]);
    }

    #[test]
    fn replace_and_clear_pending_preserves_submitted_work() {
        let (focus, mut server, _wakes) = fixture();
        let now = Instant::now();
        focus.request(1);
        focus.submit_pending_focus(now, &mut server);
        focus.request(2);
        assert!(focus.is_submitted(1));
        assert!(!focus.is_submitted(2));
        focus.clear_pending();
        assert!(focus.is_submitted(1));
        focus.submit_pending_focus(now + Duration::from_secs(1), &mut server);
        assert_eq!(server.submissions, [1]);
        focus.request(2);
        focus.submit_pending_focus(now + Duration::from_secs(1), &mut server);
        assert!(!focus.is_submitted(1));
        assert!(focus.is_submitted(2));
        assert_eq!(server.submissions, [1, 2]);
    }

    #[test]
    fn clear_external_preserves_external_spacing() {
        let (focus, mut server, _wakes) = fixture();
        let now = Instant::now();
        focus.request(1);
        focus.clear_external();
        focus.request(2);
        assert_eq!(focus.next_deadline(now), Some(now));
        focus.submit_pending_focus(now, &mut server);
        focus.request(3);
        focus.clear_external();
        assert!(!focus.is_submitted(2));
        assert_eq!(focus.next_deadline(now), None);
        focus.submit_pending_focus(now + Duration::from_secs(1), &mut server);
        assert_eq!(server.submissions, [2]);
        focus.request(4);
        let boundary = now + Duration::from_millis(100);
        assert_eq!(focus.next_deadline(now), Some(boundary));
        focus.submit_pending_focus(boundary - Duration::from_nanos(1), &mut server);
        assert_eq!(server.submissions, [2]);
        focus.submit_pending_focus(boundary, &mut server);
        assert_eq!(server.submissions, [2, 4]);
    }

    #[test]
    fn lock_poisoned_focus_discards_intent_without_resetting_spacing() {
        let (focus, mut server, _wakes) = fixture();
        let now = Instant::now();
        focus.request(1);
        focus.submit_pending_focus(now, &mut server);
        focus.request(2);
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _state = focus.state.lock().unwrap();
            panic!("interrupted focus state update");
        }));
        assert!(result.is_err());
        assert!(!focus.is_submitted(1));
        assert_eq!(focus.next_deadline(now), None);
        focus.request(3);
        assert_eq!(
            focus.next_deadline(now),
            Some(now + Duration::from_millis(100))
        );
        focus.submit_pending_focus(now + Duration::from_millis(100), &mut server);
        assert_eq!(server.submissions, [1, 3]);
    }
}
