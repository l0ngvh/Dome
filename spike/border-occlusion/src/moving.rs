//! The moving set of `moving-window-list`, the managed windows whose bounds the
//! overlay reads from CGWindowList every frame. It is a pure state machine. The
//! caller passes the time in and owns the timer.

use std::collections::HashMap;
use std::time::{Duration, Instant};

pub struct MovingSet {
    /// How long a window stays in the set after its last moved notification.
    tail: Duration,
    last_moved: HashMap<u32, Instant>,
}

impl MovingSet {
    pub fn new(tail: Duration) -> MovingSet {
        MovingSet {
            tail,
            last_moved: HashMap::new(),
        }
    }

    pub fn moved(&mut self, id: u32, now: Instant) {
        self.last_moved.insert(id, now);
    }

    pub fn remove(&mut self, id: u32) {
        self.last_moved.remove(&id);
    }

    /// Drops every window whose last notification is `tail` or more before
    /// `now`, then returns the rest in id order.
    pub fn current(&mut self, now: Instant) -> Vec<u32> {
        self.last_moved
            .retain(|_, at| now.saturating_duration_since(*at) < self.tail);
        let mut ids: Vec<u32> = self.last_moved.keys().copied().collect();
        ids.sort_unstable();
        ids
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAIL: Duration = Duration::from_millis(250);

    fn ms(n: u64) -> Duration {
        Duration::from_millis(n)
    }

    #[test]
    fn a_window_joins_on_a_notification_and_leaves_the_tail_after_its_last_one() {
        let t0 = Instant::now();
        let mut set = MovingSet::new(TAIL);
        assert!(set.current(t0).is_empty());
        set.moved(7, t0);
        assert_eq!(set.current(t0), vec![7]);
        set.moved(7, t0 + ms(100));
        assert_eq!(
            set.current(t0 + ms(300)),
            vec![7],
            "the second notification restarts the tail"
        );
        assert_eq!(set.current(t0 + ms(349)), vec![7]);
        assert!(set.current(t0 + ms(350)).is_empty());
    }

    #[test]
    fn each_window_keeps_its_own_tail() {
        let t0 = Instant::now();
        let mut set = MovingSet::new(TAIL);
        set.moved(9, t0);
        set.moved(3, t0 + ms(200));
        assert_eq!(set.current(t0 + ms(200)), vec![3, 9]);
        assert_eq!(set.current(t0 + ms(250)), vec![3]);
        assert!(set.current(t0 + ms(450)).is_empty());
    }

    #[test]
    fn a_removed_window_leaves_at_once() {
        let t0 = Instant::now();
        let mut set = MovingSet::new(TAIL);
        set.moved(5, t0);
        set.remove(5);
        assert!(set.current(t0).is_empty());
    }
}
