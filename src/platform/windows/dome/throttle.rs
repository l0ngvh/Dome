use std::time::{Duration, Instant};

pub(in crate::platform::windows) enum ThrottleResult<T> {
    Send(T),
    Pending,
    ScheduleFlush(Duration),
}

pub(in crate::platform::windows) struct Throttle<T> {
    interval: Duration,
    last_sent: Option<Instant>,
    pending: Option<T>,
    has_pending_timer: bool,
}

impl<T> Throttle<T> {
    pub(in crate::platform::windows) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_sent: None,
            pending: None,
            has_pending_timer: false,
        }
    }

    pub(in crate::platform::windows) fn submit(&mut self, value: T) -> ThrottleResult<T> {
        let now = Instant::now();
        let remaining = self
            .last_sent
            .map(|last| self.interval.saturating_sub(now.duration_since(last)))
            .unwrap_or(Duration::ZERO);

        if remaining.is_zero() {
            self.last_sent = Some(now);
            self.pending = None;
            ThrottleResult::Send(value)
        } else {
            self.pending = Some(value);
            if self.has_pending_timer {
                ThrottleResult::Pending
            } else {
                ThrottleResult::ScheduleFlush(remaining)
            }
        }
    }

    pub(in crate::platform::windows) fn flush(&mut self) -> Option<T> {
        self.has_pending_timer = false;
        if let Some(value) = self.pending.take() {
            self.last_sent = Some(Instant::now());
            Some(value)
        } else {
            None
        }
    }

    pub(in crate::platform::windows) fn mark_timer_scheduled(&mut self) {
        self.has_pending_timer = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    /// Simulates the calloop event loop side: tracks scheduled timer deadlines
    /// and fires them like calloop's Timer would.
    struct ThrottleHarness {
        throttle: Throttle<i32>,
        processed: Vec<i32>,
        timer_deadline: Option<Instant>,
    }

    impl ThrottleHarness {
        fn new(interval: Duration) -> Self {
            Self {
                throttle: Throttle::new(interval),
                processed: Vec::new(),
                timer_deadline: None,
            }
        }

        fn submit(&mut self, value: i32) {
            match self.throttle.submit(value) {
                ThrottleResult::Send(v) => self.processed.push(v),
                ThrottleResult::ScheduleFlush(delay) => {
                    self.timer_deadline = Some(Instant::now() + delay);
                    self.throttle.mark_timer_scheduled();
                }
                ThrottleResult::Pending => {}
            }
        }

        fn has_pending_timer(&self) -> bool {
            self.timer_deadline.is_some()
        }

        /// Sleep until the scheduled timer deadline, then flush — just like
        /// calloop would fire the Timer source after the delay.
        fn wait_for_timer(&mut self) {
            let deadline = self.timer_deadline.take().expect("no timer scheduled");
            let remaining = deadline.saturating_duration_since(Instant::now());
            if !remaining.is_zero() {
                thread::sleep(remaining);
            }
            if let Some(v) = self.throttle.flush() {
                self.processed.push(v);
            }
        }
    }

    #[test]
    fn single_focus_event_goes_through() {
        let mut h = ThrottleHarness::new(Duration::from_millis(20));
        h.submit(1);
        assert_eq!(h.processed, vec![1]);
        assert!(!h.has_pending_timer());
    }

    #[test]
    fn rapid_burst_throttles_to_first_and_last() {
        let mut h = ThrottleHarness::new(Duration::from_millis(20));
        h.submit(1);
        h.submit(2);
        h.submit(3);
        h.submit(4);
        h.submit(5);
        assert_eq!(h.processed, vec![1]);

        h.wait_for_timer();
        assert_eq!(h.processed, vec![1, 5]);
    }

    #[test]
    fn spaced_events_all_go_through_without_timer() {
        let mut h = ThrottleHarness::new(Duration::from_millis(10));
        h.submit(1);
        thread::sleep(Duration::from_millis(15));
        h.submit(2);
        thread::sleep(Duration::from_millis(15));
        h.submit(3);
        assert_eq!(h.processed, vec![1, 2, 3]);
        assert!(!h.has_pending_timer());
    }

    #[test]
    fn new_send_before_timer_fires_supersedes_pending() {
        let mut h = ThrottleHarness::new(Duration::from_millis(10));
        h.submit(1);
        h.submit(2);
        assert!(h.has_pending_timer());

        // Wait past the interval — next submit goes through directly
        thread::sleep(Duration::from_millis(15));
        h.submit(3);
        assert_eq!(h.processed, vec![1, 3]);

        // Timer fires but pending was cleared by the Send
        h.wait_for_timer();
        assert_eq!(h.processed, vec![1, 3]);
    }

    #[test]
    fn two_bursts_separated_by_timer() {
        let mut h = ThrottleHarness::new(Duration::from_millis(10));

        // First burst
        h.submit(1);
        h.submit(2);
        h.submit(3);
        h.wait_for_timer();
        assert_eq!(h.processed, vec![1, 3]);

        // Second burst — timer flag was reset by flush, so new timer can be scheduled
        thread::sleep(Duration::from_millis(15));
        h.submit(4);
        h.submit(5);
        h.submit(6);
        h.wait_for_timer();
        assert_eq!(h.processed, vec![1, 3, 4, 6]);
    }

    #[test]
    fn schedule_flush_delay_is_never_zero() {
        for _ in 0..1000 {
            let mut t = Throttle::new(Duration::from_millis(50));
            t.submit(1);
            match t.submit(2) {
                ThrottleResult::ScheduleFlush(d) => assert!(d > Duration::ZERO),
                _ => panic!("expected ScheduleFlush"),
            }
        }
    }
}
