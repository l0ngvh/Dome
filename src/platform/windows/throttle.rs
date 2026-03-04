use std::time::{Duration, Instant};

use calloop::RegistrationToken;

pub(super) enum ThrottleResult<T> {
    Send(T),
    Pending,
}

pub(super) struct Throttle<T> {
    interval: Duration,
    last_sent: Option<Instant>,
    pending: Option<T>,
    timer_token: Option<RegistrationToken>,
}

impl<T> Throttle<T> {
    pub(super) fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_sent: None,
            pending: None,
            timer_token: None,
        }
    }

    pub(super) fn submit(&mut self, value: T) -> ThrottleResult<T> {
        let now = Instant::now();
        let can_send = self
            .last_sent
            .map(|last| now.duration_since(last) >= self.interval)
            .unwrap_or(true);

        if can_send {
            self.last_sent = Some(now);
            self.pending = None;
            ThrottleResult::Send(value)
        } else {
            self.pending = Some(value);
            ThrottleResult::Pending
        }
    }

    pub(super) fn flush(&mut self) -> Option<T> {
        self.timer_token = None;
        if let Some(value) = self.pending.take() {
            self.last_sent = Some(Instant::now());
            Some(value)
        } else {
            None
        }
    }

    pub(super) fn schedule_delay(&self) -> Option<Duration> {
        if self.pending.is_some() && self.timer_token.is_none() {
            let delay = self
                .last_sent
                .map(|last| self.interval.saturating_sub(last.elapsed()))
                .unwrap_or(Duration::ZERO);
            Some(delay)
        } else {
            None
        }
    }

    pub(super) fn set_timer_token(&mut self, token: RegistrationToken) {
        self.timer_token = Some(token);
    }
}
