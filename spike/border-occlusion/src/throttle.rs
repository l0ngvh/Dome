//! The leading and trailing throttle on the z-order CGWindowList call. It is a
//! pure state machine. The caller passes the time in and owns the timers.

use std::time::{Duration, Instant};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Submit {
    CallNow,
    /// Arm a one-shot timer for this delay, then pass its firing to `fire`.
    Arm(Duration),
    /// A trailing call is already armed, and it covers this event.
    Pending,
}

pub struct Throttle {
    interval: Duration,
    /// A call right after an AX notification can return the old z-order, so
    /// each call can get one more call this long after it.
    second: Option<Duration>,
    last_call: Option<Instant>,
    armed: bool,
    second_armed: bool,
}

impl Throttle {
    pub fn new(interval: Duration) -> Throttle {
        Throttle {
            interval,
            second: None,
            last_call: None,
            armed: false,
            second_armed: false,
        }
    }

    pub fn with_second_call(interval: Duration, second: Duration) -> Throttle {
        Throttle {
            second: Some(second),
            ..Throttle::new(interval)
        }
    }

    pub fn submit(&mut self, now: Instant) -> Submit {
        if self.armed {
            return Submit::Pending;
        }
        match self.last_call {
            Some(last) if now.saturating_duration_since(last) < self.interval => {
                self.armed = true;
                Submit::Arm(self.interval - now.saturating_duration_since(last))
            }
            _ => {
                self.called(now);
                Submit::CallNow
            }
        }
    }

    /// The timer that `Submit::Arm` asked for fired. True means call now.
    pub fn fire(&mut self, now: Instant) -> bool {
        if !self.armed {
            return false;
        }
        self.armed = false;
        self.called(now);
        true
    }

    fn called(&mut self, now: Instant) {
        self.last_call = Some(now);
        self.second_armed = self.second.is_some();
    }

    /// After a call that `submit` or `fire` asked for, the delay of the
    /// one-shot timer to arm for the second call. Its firing goes to
    /// `fire_second`.
    pub fn second_delay(&self) -> Option<Duration> {
        self.second.filter(|_| self.second_armed)
    }

    /// The timer that `second_delay` asked for fired. True means call now.
    /// The second call starts no interval of its own, and it leaves an armed
    /// trailing call armed. So focus spam makes one throttled call per interval
    /// plus the second call of each.
    pub fn fire_second(&mut self) -> bool {
        std::mem::take(&mut self.second_armed)
    }

    /// The time of the last call that `submit` or `fire` asked for.
    pub fn last_call(&self) -> Option<Instant> {
        self.last_call
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const INTERVAL: Duration = Duration::from_millis(100);
    const SECOND: Duration = Duration::from_millis(50);

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Call {
        First,
        Second,
    }

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    /// Submits at each offset in `events` and fires each armed timer on time,
    /// in time order. A call replaces an armed second-call timer. Returns the
    /// offsets and kinds of the calls.
    fn simulate(mut throttle: Throttle, events: &[Duration]) -> Vec<(Duration, Call)> {
        let t0 = Instant::now();
        let mut trailing: Option<Duration> = None;
        let mut second: Option<Duration> = None;
        let mut calls = Vec::new();
        let mut next = 0;
        loop {
            let event = events.get(next).copied();
            let timer = [trailing, second].into_iter().flatten().min();
            let at = match (event, timer) {
                (None, None) => break,
                (Some(e), Some(t)) => e.min(t),
                (Some(e), None) => e,
                (None, Some(t)) => t,
            };
            let called = if trailing == Some(at) {
                trailing = None;
                throttle.fire(t0 + at)
            } else if second == Some(at) {
                second = None;
                if throttle.fire_second() {
                    calls.push((at, Call::Second));
                }
                false
            } else {
                next += 1;
                match throttle.submit(t0 + at) {
                    Submit::CallNow => true,
                    Submit::Arm(delay) => {
                        trailing = Some(at + delay);
                        false
                    }
                    Submit::Pending => false,
                }
            };
            if called {
                calls.push((at, Call::First));
                if let Some(delay) = throttle.second_delay() {
                    second = Some(at + delay);
                }
            }
        }
        calls
    }

    fn first_calls(events: &[Duration]) -> Vec<Duration> {
        simulate(Throttle::new(INTERVAL), events)
            .into_iter()
            .map(|(at, call)| {
                assert_eq!(call, Call::First, "no second call without a delay");
                at
            })
            .collect()
    }

    fn with_second(events: &[Duration]) -> Vec<(Duration, Call)> {
        simulate(Throttle::with_second_call(INTERVAL, SECOND), events)
    }

    #[test]
    fn the_first_event_calls_at_once() {
        assert_eq!(first_calls(&[ms(0)]), vec![ms(0)]);
    }

    #[test]
    fn events_inside_the_interval_share_one_trailing_call() {
        assert_eq!(
            first_calls(&[ms(0), ms(10), ms(20), ms(30)]),
            vec![ms(0), ms(100)]
        );
    }

    #[test]
    fn an_event_after_a_quiet_interval_calls_at_once() {
        assert_eq!(first_calls(&[ms(0), ms(250)]), vec![ms(0), ms(250)]);
    }

    #[test]
    fn spam_makes_at_most_one_call_per_interval() {
        let events: Vec<Duration> = (0..1000).map(ms).collect();
        let calls = first_calls(&events);
        for pair in calls.windows(2) {
            assert!(pair[1] - pair[0] >= INTERVAL, "calls {pair:?} closer than the interval");
        }
        assert_eq!(calls.len(), 11, "one leading call and one per interval: {calls:?}");
        assert!(
            *calls.last().unwrap() >= ms(999),
            "the trailing call covers the last event: {calls:?}"
        );
    }

    #[test]
    fn a_single_event_gets_a_second_call() {
        assert_eq!(
            with_second(&[ms(0)]),
            vec![(ms(0), Call::First), (ms(50), Call::Second)]
        );
    }

    #[test]
    fn a_burst_gets_a_second_call_and_a_trailing_call_with_its_own_second_call() {
        assert_eq!(
            with_second(&[ms(0), ms(10), ms(20), ms(30)]),
            vec![
                (ms(0), Call::First),
                (ms(50), Call::Second),
                (ms(100), Call::First),
                (ms(150), Call::Second)
            ]
        );
    }

    #[test]
    fn an_event_after_the_second_call_still_waits_for_the_interval() {
        assert_eq!(
            with_second(&[ms(0), ms(60)]),
            vec![
                (ms(0), Call::First),
                (ms(50), Call::Second),
                (ms(100), Call::First),
                (ms(150), Call::Second)
            ]
        );
    }

    #[test]
    fn an_event_after_the_interval_calls_at_once_after_a_second_call() {
        assert_eq!(
            with_second(&[ms(0), ms(120)]),
            vec![
                (ms(0), Call::First),
                (ms(50), Call::Second),
                (ms(120), Call::First),
                (ms(170), Call::Second)
            ]
        );
    }

    #[test]
    fn spam_makes_one_throttled_call_per_interval_each_with_a_second_call() {
        let events: Vec<Duration> = (0..1000).map(ms).collect();
        let calls = with_second(&events);
        let firsts: Vec<Duration> = calls
            .iter()
            .filter(|(_, c)| *c == Call::First)
            .map(|(at, _)| *at)
            .collect();
        for pair in firsts.windows(2) {
            assert!(pair[1] - pair[0] >= INTERVAL, "calls {pair:?} closer than the interval");
        }
        assert_eq!(firsts.len(), 11, "{calls:?}");
        let seconds: Vec<Duration> = calls
            .iter()
            .filter(|(_, c)| *c == Call::Second)
            .map(|(at, _)| *at)
            .collect();
        let expected: Vec<Duration> = firsts.iter().map(|at| *at + SECOND).collect();
        assert_eq!(seconds, expected, "{calls:?}");
    }
}
