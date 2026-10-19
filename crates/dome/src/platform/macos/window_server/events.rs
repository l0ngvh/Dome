use super::{
    AppObservation, NativeFullscreenObservation, NativeState, NativeWindowServer, SystemObservation,
};
use crate::platform::macos::event_loop::{EventSender, RunLoop};
use crate::platform::macos::running_application::RunningApp;
use dome_core::WindowId;
use objc2_core_graphics::CGWindowID;
use std::collections::HashSet;
use std::rc::Rc;
use std::time::{Duration, Instant};

pub(in crate::platform::macos) enum NativeEvent {
    FocusObserved { pid: i32 },
    TitleObserved(CGWindowID),
    SubmitFocus,
    WindowCreated { id: CGWindowID, pid: i32 },
    WindowDestroyed { id: CGWindowID },
    AppHidden { pid: i32 },
    AppShown { pid: i32 },
    SpaceChanged,
    FocusChanged { pid: i32 },
    AppWindowsChanged { pid: i32 },
    RefreshTimerElapsed,
    ScreenUnlocked,
    TitleChanged(CGWindowID),
    MovementStarted { pid: i32 },
    MovementSettled { pid: i32, generation: u64 },
    WindowMovedOrResized { pid: i32, observed_at: Instant },
    PidObserved { pid: i32 },
    AppTerminated { pid: i32 },
}

#[derive(Debug, Copy, Clone)]
pub(in crate::platform::macos) struct DebounceBurst {
    pub(in crate::platform::macos) first: Instant,
    pub(in crate::platform::macos) last: Instant,
}

pub(super) enum WindowEvent {
    SpaceChanged,
    FocusChanged {
        pid: i32,
    },
    AppWindowsChanged {
        pid: i32,
    },
    RefreshTimerElapsed,
    ScreenUnlocked,
    TitleChanged(WindowId),
    MovementStarted {
        pid: i32,
    },
    MovementSettled {
        pid: i32,
        observed_at: DebounceBurst,
    },
    AppRemoved {
        pid: i32,
    },
}

pub(in crate::platform::macos) enum WindowNotification {
    AppWindows(AppObservation),
    AllWindows(SystemObservation),
    Movement {
        app: AppObservation,
        observed_at: DebounceBurst,
    },
    Focus(WindowId),
    NativeFullscreen(NativeFullscreenObservation),
    Title {
        id: WindowId,
        title: Option<String>,
    },
    MovementStarted {
        pid: i32,
    },
    MovementEnded {
        pid: i32,
    },
    AppRemoved {
        pid: i32,
    },
    WindowRemoved(WindowId),
}

const MOVE_DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);

impl NativeWindowServer {
    pub(super) fn refresh_system_observers(&self, observation: &SystemObservation) {
        let events = self.state.borrow_mut().refresh_observers(observation);
        self.refresh_all_observers();
        let state = self.state.borrow();
        for event in events {
            if let WindowEvent::MovementStarted { pid } = event {
                let _ = state.sender.send(NativeEvent::MovementStarted { pid });
            }
        }
    }

    pub(super) fn receive(self: &Rc<Self>, event: NativeEvent) {
        match event {
            NativeEvent::FocusObserved { pid } => {
                self.listener.focus_observed(pid);
                return;
            }
            NativeEvent::TitleObserved(id) => {
                self.listener.title_observed(id);
                return;
            }
            NativeEvent::SubmitFocus => {
                self.state.borrow_mut().submit_focus();
                return;
            }
            _ => {}
        }
        if let NativeEvent::WindowCreated { pid, .. } = &event {
            let Some(app) = RunningApp::new(*pid) else {
                return;
            };
            self.state
                .borrow_mut()
                .apps
                .entry(*pid)
                .or_insert(!app.is_hidden());
        }
        if matches!(
            event,
            NativeEvent::RefreshTimerElapsed
                | NativeEvent::ScreenUnlocked
                | NativeEvent::SpaceChanged
                | NativeEvent::AppWindowsChanged { .. }
                | NativeEvent::AppShown { .. }
                | NativeEvent::FocusChanged { .. }
        ) && let Err(error) = self.register_current_windows()
        {
            tracing::warn!(%error, "Failed to enumerate native window identities");
        }
        let request = self.state.borrow_mut().process_event(event);
        if let Some(request) = request {
            self.schedule(request);
        }
    }

    pub(super) fn schedule(self: &Rc<Self>, request: WindowEvent) {
        match request {
            WindowEvent::MovementStarted { pid } => {
                self.emit(WindowNotification::MovementStarted { pid })
            }

            WindowEvent::AppRemoved { pid } => self.emit(WindowNotification::AppRemoved { pid }),
            request => {
                if let WindowEvent::MovementSettled { pid, .. } = &request {
                    self.emit(WindowNotification::MovementEnded { pid: *pid });
                }
                let ids = self
                    .state
                    .borrow()
                    .native_ids
                    .keys()
                    .copied()
                    .collect::<Vec<_>>();
                let owner = Rc::downgrade(self);
                RunLoop::current().spawn(async move {
                    let Some(server) = owner.upgrade() else {
                        return;
                    };
                    if let Some(update) = read_update(&server, request, &ids).await {
                        server.emit(update);
                    }
                });
            }
        }
    }

    fn emit(&self, update: WindowNotification) {
        self.state.borrow().emit(update);
    }
}

impl NativeState {
    pub(super) fn refresh_observers(&mut self, system: &SystemObservation) -> Vec<WindowEvent> {
        for pid in system.terminated_pids.iter().chain(&system.hidden_pids) {
            self.movement.remove(pid);
        }
        for pid in &system.terminated_pids {
            self.retire_app(*pid);
        }
        let visible_pids: HashSet<_> = system
            .apps
            .iter()
            .filter(|app| !app.listed.is_empty())
            .map(|app| app.pid)
            .collect();
        let pids: Vec<_> = self
            .observed_pids
            .union(&visible_pids)
            .copied()
            .filter(|pid| {
                !system.terminated_pids.contains(pid)
                    && !system.hidden_pids.contains(pid)
                    && !self.movement.contains_key(pid)
            })
            .collect();
        pids.into_iter()
            .map(|pid| self.record_movement(pid, Instant::now()))
            .collect()
    }

    fn next_timer_generation(&mut self) -> u64 {
        self.timer_generation = self
            .timer_generation
            .checked_add(1)
            .expect("timer generations exhausted");
        self.timer_generation
    }

    fn record_movement(&mut self, pid: i32, observed_at: Instant) -> WindowEvent {
        let first = self
            .movement
            .get(&pid)
            .map_or(observed_at, |(_, burst, _)| burst.first);
        let generation = self.next_timer_generation();
        let sender = self.sender.clone();
        let timer = crate::platform::macos::timer::Timer::once(MOVE_DEBOUNCE_INTERVAL, move || {
            let _ = sender.send(NativeEvent::MovementSettled { pid, generation });
        });
        self.movement.insert(
            pid,
            (
                generation,
                DebounceBurst {
                    first,
                    last: observed_at,
                },
                timer,
            ),
        );
        WindowEvent::MovementStarted { pid }
    }
}

impl NativeState {
    pub(super) fn process_event(&mut self, event: NativeEvent) -> Option<WindowEvent> {
        match event {
            NativeEvent::WindowCreated { id, pid } => {
                if !self.apps.contains_key(&pid) {
                    return None;
                }
                self.register_window(id, pid);
                Some(WindowEvent::AppWindowsChanged { pid })
            }
            NativeEvent::WindowDestroyed { id } => {
                self.retire_window(id);
                None
            }
            NativeEvent::AppHidden { pid } => {
                let previous = self.apps.insert(pid, false);
                self.movement.remove(&pid);
                (previous != Some(false)).then_some(WindowEvent::AppRemoved { pid })
            }
            NativeEvent::AppShown { pid } => {
                self.apps.insert(pid, true);
                Some(WindowEvent::AppWindowsChanged { pid })
            }
            NativeEvent::SpaceChanged => Some(WindowEvent::SpaceChanged),
            NativeEvent::FocusChanged { pid } => Some(WindowEvent::FocusChanged { pid }),
            NativeEvent::AppWindowsChanged { pid } => Some(WindowEvent::AppWindowsChanged { pid }),
            NativeEvent::RefreshTimerElapsed => Some(WindowEvent::RefreshTimerElapsed),
            NativeEvent::ScreenUnlocked => Some(WindowEvent::ScreenUnlocked),
            NativeEvent::MovementStarted { pid } => self
                .movement
                .contains_key(&pid)
                .then_some(WindowEvent::MovementStarted { pid }),
            NativeEvent::TitleChanged(id) => self
                .window_ids
                .get(&id)
                .copied()
                .map(WindowEvent::TitleChanged),
            NativeEvent::MovementSettled { pid, generation } => {
                if self
                    .movement
                    .get(&pid)
                    .is_some_and(|(current, _, _)| *current == generation)
                {
                    let (_, observed_at, _) = self.movement.remove(&pid).expect("current movement");
                    Some(WindowEvent::MovementSettled { pid, observed_at })
                } else {
                    None
                }
            }
            NativeEvent::WindowMovedOrResized { pid, observed_at } => {
                Some(self.record_movement(pid, observed_at))
            }
            NativeEvent::PidObserved { pid } => {
                if self.applications.contains_key(&pid) {
                    self.observed_pids.insert(pid);
                }
                None
            }
            NativeEvent::FocusObserved { .. }
            | NativeEvent::TitleObserved(_)
            | NativeEvent::SubmitFocus => {
                unreachable!("owner handles coordination events before native state")
            }
            NativeEvent::AppTerminated { pid } => {
                let previous = self.apps.remove(&pid);
                self.retire_app(pid);
                self.movement.remove(&pid);
                (previous == Some(true)).then_some(WindowEvent::AppRemoved { pid })
            }
        }
    }
}

pub(in crate::platform::macos) fn send_native_event(
    sender: &EventSender<NativeEvent>,
    event: NativeEvent,
) {
    if sender.send(event).is_err() {
        tracing::debug!("Owner loop closed, event dropped");
    }
}

async fn read_update(
    server: &NativeWindowServer,
    request: WindowEvent,
    ids: &[WindowId],
) -> Option<WindowNotification> {
    Some(match request {
        WindowEvent::AppWindowsChanged { pid } => {
            WindowNotification::AppWindows(server.read_app_windows(pid, ids).await?)
        }
        WindowEvent::MovementSettled { pid, observed_at } => WindowNotification::Movement {
            app: server.read_app_windows(pid, ids).await?,
            observed_at,
        },
        WindowEvent::RefreshTimerElapsed | WindowEvent::ScreenUnlocked => {
            WindowNotification::AllWindows(server.read_all_windows(ids).await?)
        }
        WindowEvent::FocusChanged { pid } => {
            WindowNotification::Focus(server.read_focus(pid).await?)
        }
        WindowEvent::SpaceChanged => {
            WindowNotification::NativeFullscreen(server.read_native_fullscreen().await?)
        }
        WindowEvent::TitleChanged(id) => WindowNotification::Title {
            id,
            title: server.read_title(id).await?,
        },
        WindowEvent::MovementStarted { pid } => WindowNotification::MovementStarted { pid },
        WindowEvent::AppRemoved { pid } => WindowNotification::AppRemoved { pid },
    })
}
