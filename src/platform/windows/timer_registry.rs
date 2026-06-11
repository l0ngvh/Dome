use std::collections::HashMap;
use std::time::Duration;
use std::time::Instant;

use crate::platform::windows::external::HwndId;

#[derive(Clone, Copy, Debug)]
pub(super) enum TimerKind {
    Focus,
    MoveSettle { hwnd: HwndId, observed_at: Instant },
    Prune,
}

pub(super) trait OsTimer {
    fn set_timer(&self, hint: usize, period_ms: u32) -> usize;
    fn kill_timer(&self, id: usize);
}

pub(super) struct Win32Timer;

impl OsTimer for Win32Timer {
    fn set_timer(&self, hint: usize, period_ms: u32) -> usize {
        unsafe { windows::Win32::UI::WindowsAndMessaging::SetTimer(None, hint, period_ms, None) }
    }

    fn kill_timer(&self, id: usize) {
        unsafe { windows::Win32::UI::WindowsAndMessaging::KillTimer(None, id).ok() };
    }
}

pub(super) struct TimerRegistry {
    os: Box<dyn OsTimer>,
    by_id: HashMap<usize, TimerKind>,
}

impl TimerRegistry {
    pub(super) fn new(os: Box<dyn OsTimer>) -> Self {
        Self {
            os,
            by_id: HashMap::new(),
        }
    }

    /// With hWnd=NULL, SetTimer ignores nIDEvent when it doesn't match an
    /// existing timer and returns a new system-generated ID. Pass the
    /// previous ID to replace an existing timer, or 0 to create a new one.
    pub(super) fn schedule_focus(&mut self, delay: Duration) {
        let hint = self.find_focus_id().unwrap_or(0);
        self.schedule(TimerKind::Focus, hint, delay);
    }

    pub(super) fn schedule_move_settle(
        &mut self,
        hwnd: HwndId,
        observed_at: Instant,
        delay: Duration,
    ) {
        self.cancel_move_settle(hwnd);
        self.schedule(TimerKind::MoveSettle { hwnd, observed_at }, 0, delay);
    }

    pub(super) fn cancel_move_settle(&mut self, hwnd: HwndId) {
        if let Some(id) = self.find_move_settle_id(hwnd) {
            self.os.kill_timer(id);
            self.by_id.remove(&id);
        }
    }

    pub(super) fn schedule_prune(&mut self, period: Duration) {
        self.schedule(TimerKind::Prune, 0, period);
    }

    pub(super) fn dispatch(&mut self, timer_id: usize) -> Option<TimerKind> {
        let kind = self.by_id.get(&timer_id).copied()?;
        match kind {
            TimerKind::Focus | TimerKind::MoveSettle { .. } => {
                self.by_id.remove(&timer_id);
                self.os.kill_timer(timer_id);
            }
            TimerKind::Prune => {}
        }
        Some(kind)
    }

    fn schedule(&mut self, kind: TimerKind, hint: usize, delay: Duration) {
        let id = self.os.set_timer(hint, delay.as_millis() as u32);
        if id == 0 {
            tracing::warn!(?kind, "SetTimer failed");
            return;
        }
        if hint != 0 && hint != id {
            // Hint was a stale id (the previous timer already fired or was
            // killed). The OS allocated a fresh id, so drop the old entry.
            self.by_id.remove(&hint);
        }
        self.by_id.insert(id, kind);
    }

    fn find_focus_id(&self) -> Option<usize> {
        self.by_id
            .iter()
            .find_map(|(&id, k)| matches!(k, TimerKind::Focus).then_some(id))
    }

    fn find_move_settle_id(&self, target: HwndId) -> Option<usize> {
        self.by_id.iter().find_map(|(&id, k)| match k {
            TimerKind::MoveSettle { hwnd, .. } if *hwnd == target => Some(id),
            _ => None,
        })
    }
}

impl Drop for TimerRegistry {
    fn drop(&mut self) {
        for &id in self.by_id.keys() {
            self.os.kill_timer(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    struct MockOs {
        next_id: Cell<usize>,
        set_calls: Rc<RefCell<Vec<(usize, u32)>>>,
        kill_calls: Rc<RefCell<Vec<usize>>>,
        fail_set: Rc<Cell<bool>>,
    }

    impl MockOs {
        fn new(start_id: usize) -> Self {
            Self {
                next_id: Cell::new(start_id),
                set_calls: Rc::new(RefCell::new(Vec::new())),
                kill_calls: Rc::new(RefCell::new(Vec::new())),
                fail_set: Rc::new(Cell::new(false)),
            }
        }
    }

    impl OsTimer for MockOs {
        fn set_timer(&self, hint: usize, period_ms: u32) -> usize {
            self.set_calls.borrow_mut().push((hint, period_ms));
            if self.fail_set.get() {
                return 0;
            }
            if hint != 0 {
                return hint;
            }
            let id = self.next_id.get();
            self.next_id.set(id + 1);
            id
        }

        fn kill_timer(&self, id: usize) {
            self.kill_calls.borrow_mut().push(id);
        }
    }

    #[test]
    fn schedule_then_dispatch_returns_kind_and_clears_one_shot() {
        let mock = MockOs::new(100);
        let kill_calls = mock.kill_calls.clone();
        let mut reg = TimerRegistry::new(Box::new(mock));

        reg.schedule_focus(Duration::from_millis(500));
        reg.schedule_move_settle(HwndId::test(1), Instant::now(), Duration::from_millis(100));

        let focus_kind = reg.dispatch(100);
        assert!(matches!(focus_kind, Some(TimerKind::Focus)));
        assert!(reg.dispatch(100).is_none());

        let settle_kind = reg.dispatch(101);
        assert!(matches!(settle_kind, Some(TimerKind::MoveSettle { .. })));
        assert!(reg.dispatch(101).is_none());

        let kills = kill_calls.borrow();
        assert!(kills.contains(&100));
        assert!(kills.contains(&101));
    }

    #[test]
    fn dispatch_prune_keeps_entry_live() {
        let mock = MockOs::new(50);
        let kill_calls = mock.kill_calls.clone();
        let mut reg = TimerRegistry::new(Box::new(mock));

        reg.schedule_prune(Duration::from_secs(300));

        let kind = reg.dispatch(50);
        assert!(matches!(kind, Some(TimerKind::Prune)));

        let kind2 = reg.dispatch(50);
        assert!(matches!(kind2, Some(TimerKind::Prune)));

        assert!(kill_calls.borrow().is_empty());
    }

    #[test]
    fn cancel_move_settle_removes_only_target_hwnd() {
        let mock = MockOs::new(10);
        let kill_calls = mock.kill_calls.clone();
        let mut reg = TimerRegistry::new(Box::new(mock));

        reg.schedule_move_settle(HwndId::test(1), Instant::now(), Duration::from_millis(100));
        reg.schedule_move_settle(HwndId::test(2), Instant::now(), Duration::from_millis(100));

        reg.cancel_move_settle(HwndId::test(1));

        assert_eq!(kill_calls.borrow().len(), 1);
        assert_eq!(kill_calls.borrow()[0], 10);

        assert!(reg.dispatch(11).is_some());
        assert!(reg.dispatch(10).is_none());
    }

    #[test]
    fn drop_kills_every_live_timer() {
        let mock = MockOs::new(200);
        let kill_calls = mock.kill_calls.clone();
        let mut registry = TimerRegistry::new(Box::new(mock));

        registry.schedule_focus(Duration::from_millis(500));
        registry.schedule_move_settle(HwndId::test(1), Instant::now(), Duration::from_millis(100));
        registry.schedule_prune(Duration::from_secs(300));
        drop(registry);

        let kills = kill_calls.borrow();
        assert_eq!(kills.len(), 3);
        assert!(kills.contains(&200));
        assert!(kills.contains(&201));
        assert!(kills.contains(&202));
    }

    #[test]
    fn set_timer_zero_does_not_record() {
        let mock = MockOs::new(1);
        let kill_calls = mock.kill_calls.clone();
        let fail_set = mock.fail_set.clone();
        let mut reg = TimerRegistry::new(Box::new(mock));

        fail_set.set(true);
        reg.schedule_focus(Duration::from_millis(500));

        assert!(reg.by_id.is_empty());
        assert!(kill_calls.borrow().is_empty());
    }

    #[test]
    fn schedule_focus_uses_previous_id_as_hint_when_live() {
        let mock = MockOs::new(100);
        let set_calls = mock.set_calls.clone();
        let mut reg = TimerRegistry::new(Box::new(mock));

        reg.schedule_focus(Duration::from_millis(500));
        assert_eq!(set_calls.borrow()[0], (0, 500));

        reg.schedule_focus(Duration::from_millis(500));
        assert_eq!(set_calls.borrow()[1], (100, 500));

        reg.dispatch(100);

        reg.schedule_focus(Duration::from_millis(500));
        assert_eq!(set_calls.borrow()[2], (0, 500));
    }
}
