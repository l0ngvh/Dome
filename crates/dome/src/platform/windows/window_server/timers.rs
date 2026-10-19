use super::{NativeEvent, WindowEvent};
use dome_core::WindowId;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

pub(super) fn kill_timer(window: HWND, id: usize) {
    unsafe { KillTimer(Some(window), id).ok() };
}

pub(super) struct NativeEventState {
    timer_window: HWND,
    next_timer_id: usize,
    timers: HashMap<usize, TimerKind>,
    dragging: HashSet<WindowId>,
    last_focus_sent: Option<Instant>,
    pending_focus: Option<WindowId>,
}

impl NativeEventState {
    pub(super) fn new(timer_window: HWND) -> Self {
        let mut events = Self {
            timer_window,
            next_timer_id: 1,
            timers: HashMap::new(),
            dragging: HashSet::new(),
            last_focus_sent: None,
            pending_focus: None,
        };
        events.schedule_timer(TimerKind::PeriodicTick, Duration::from_secs(2), set_timer);
        events
    }

    pub(super) fn cancel_timers(&mut self) {
        for id in self.timers.drain().map(|(id, _)| id) {
            unsafe { KillTimer(Some(self.timer_window), id).ok() };
        }
    }

    fn schedule_move_settle(&mut self, window_id: WindowId, observed_at: Instant, delay: Duration) {
        self.cancel_move_settle(window_id);
        self.schedule_timer(
            TimerKind::MoveSettle {
                window_id,
                observed_at,
            },
            delay,
            set_timer,
        );
    }

    fn cancel_move_settle(&mut self, target: WindowId) {
        let timer_id = self.timers.iter().find_map(|(&id, kind)| match kind {
            TimerKind::MoveSettle { window_id, .. } if *window_id == target => Some(id),
            _ => None,
        });
        if let Some(id) = timer_id {
            self.cancel_timer(id);
        }
    }

    fn schedule_timer(
        &mut self,
        kind: TimerKind,
        delay: Duration,
        set_timer: impl FnOnce(HWND, usize, u32) -> usize,
    ) -> bool {
        // KillTimer leaves queued WM_TIMER messages behind, so cancelled IDs cannot be reused.
        let id = self.next_timer_id;
        self.next_timer_id = id.checked_add(1).expect("timer IDs exhausted");
        if set_timer(self.timer_window, id, delay.as_millis() as u32) == 0 {
            tracing::warn!(?kind, "SetTimer failed");
            return false;
        }
        self.timers.insert(id, kind);
        true
    }

    fn cancel_timer(&mut self, id: usize) {
        if self.timers.remove(&id).is_some() {
            let window = self.timer_window;
            unsafe { KillTimer(Some(window), id).ok() };
        }
    }

    fn focus_event(
        &mut self,
        id: WindowId,
        now: Instant,
        set_timer: impl FnOnce(HWND, usize, u32) -> usize,
    ) -> Option<WindowEvent> {
        let remaining = self
            .last_focus_sent
            .map(|last| FOCUS_INTERVAL.saturating_sub(now.duration_since(last)))
            .unwrap_or(Duration::ZERO);
        if remaining.is_zero() {
            self.last_focus_sent = Some(now);
            self.pending_focus = None;
            Some(WindowEvent::FocusChanged(id))
        } else {
            self.pending_focus = Some(id);
            if !self
                .timers
                .values()
                .any(|kind| matches!(kind, TimerKind::Focus))
                && !self.schedule_timer(TimerKind::Focus, remaining, set_timer)
            {
                return self.settle_focus(now);
            }
            None
        }
    }

    fn settle_focus(&mut self, now: Instant) -> Option<WindowEvent> {
        self.pending_focus.take().map(|window_id| {
            self.last_focus_sent = Some(now);
            WindowEvent::FocusSettled(window_id)
        })
    }
}

impl NativeEventState {
    pub(super) fn process_event(
        &mut self,
        event: NativeEvent,
        window_id: WindowId,
    ) -> Option<WindowEvent> {
        match event {
            NativeEvent::WindowCreated(_) => Some(WindowEvent::WindowCreated(window_id)),
            NativeEvent::WindowShown(_) => Some(WindowEvent::WindowShown(window_id)),
            NativeEvent::WindowUncloaked(_) => Some(WindowEvent::WindowUncloaked(window_id)),
            NativeEvent::WindowHidden(_) => {
                self.dragging.remove(&window_id);
                self.cancel_move_settle(window_id);
                Some(WindowEvent::WindowHidden(window_id))
            }
            NativeEvent::WindowCloaked(_) => {
                self.dragging.remove(&window_id);
                self.cancel_move_settle(window_id);
                Some(WindowEvent::WindowCloaked(window_id))
            }
            NativeEvent::WindowMinimized(_) => Some(WindowEvent::WindowMinimized(window_id)),
            NativeEvent::WindowRestored {
                hwnd_id: _,
                observed_at,
            } => Some(WindowEvent::WindowRestored {
                window_id,
                observed_at,
            }),
            NativeEvent::WindowTitleChanged(_) => {
                unreachable!("title reads use the async callback")
            }
            NativeEvent::WindowFocused(_) => self.focus_event(window_id, Instant::now(), set_timer),
            NativeEvent::MoveSizeStart(_) => {
                self.dragging.insert(window_id);
                self.schedule_move_settle(window_id, Instant::now(), DRAG_SAFETY_TIMEOUT);
                Some(WindowEvent::MovementStarted(window_id))
            }
            NativeEvent::MoveSizeEnd {
                hwnd_id: _,
                observed_at,
            } => {
                self.cancel_move_settle(window_id);
                self.dragging.remove(&window_id);
                Some(WindowEvent::DragEnded {
                    window_id,
                    observed_at,
                })
            }
            NativeEvent::LocationChanged {
                hwnd_id: _,
                observed_at,
            } => {
                if self.dragging.contains(&window_id) {
                    return None;
                }
                self.schedule_move_settle(window_id, observed_at, MOVE_DEBOUNCE_INTERVAL);
                Some(WindowEvent::MovementStarted(window_id))
            }
            NativeEvent::WindowDestroyed(_) => {
                self.dragging.remove(&window_id);
                self.cancel_move_settle(window_id);
                Some(WindowEvent::WindowDestroyed(window_id))
            }
        }
    }

    pub(super) fn timer_event(
        &mut self,
        timer_id: usize,
        kill_timer: impl FnOnce(HWND, usize),
    ) -> Option<WindowEvent> {
        let kind = self.timers.get(&timer_id).copied()?;
        if !matches!(kind, TimerKind::PeriodicTick) {
            self.timers.remove(&timer_id);
            kill_timer(self.timer_window, timer_id);
        }
        match kind {
            TimerKind::Focus => self.settle_focus(Instant::now()),
            TimerKind::MoveSettle {
                window_id,
                observed_at,
            } => {
                self.dragging.remove(&window_id);
                Some(WindowEvent::MovementSettled {
                    window_id,
                    observed_at,
                })
            }
            TimerKind::PeriodicTick => Some(WindowEvent::PeriodicTick),
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum TimerKind {
    Focus,
    MoveSettle {
        window_id: WindowId,
        observed_at: Instant,
    },
    PeriodicTick,
}

const FOCUS_INTERVAL: Duration = Duration::from_millis(500);
const MOVE_DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_TIMEOUT: Duration = Duration::from_secs(60);

fn set_timer(window: HWND, id: usize, milliseconds: u32) -> usize {
    unsafe { SetTimer(Some(window), id, milliseconds, None) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn focus_in_burst_delivers_leading_and_latest_without_superseded_focus() {
        for overdue in [false, true] {
            let mut events = NativeEventState {
                timer_window: HWND::default(),
                next_timer_id: 1,
                timers: HashMap::new(),
                dragging: HashSet::new(),
                last_focus_sent: None,
                pending_focus: None,
            };
            let start = Instant::now();
            let first = WindowId::new(1);
            let second = WindowId::new(2);
            let latest = WindowId::new(3);
            assert!(matches!(
                events.focus_event(first, start, |_, id, _| id),
                Some(WindowEvent::FocusChanged(id)) if id == first
            ));
            let mut scheduled = None;
            assert!(
                events
                    .focus_event(second, start + Duration::from_millis(100), |_, id, _| {
                        scheduled = Some(id);
                        id
                    })
                    .is_none()
            );
            let time = if overdue {
                start + FOCUS_INTERVAL
            } else {
                start + Duration::from_millis(250)
            };
            let result = events.focus_event(latest, time, |_, id, _| id);
            if overdue {
                assert!(matches!(result, Some(WindowEvent::FocusChanged(id)) if id == latest));
            } else {
                assert!(result.is_none());
            }
            let timer = scheduled.expect("trailing focus scheduled");
            let result = events.timer_event(timer, |_, _| {});
            if overdue {
                assert!(result.is_none());
            } else {
                assert!(matches!(result, Some(WindowEvent::FocusSettled(id)) if id == latest));
            }
            assert!(events.timer_event(timer, |_, _| {}).is_none());
        }
    }
}
