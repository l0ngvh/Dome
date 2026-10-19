use super::timers::kill_timer;
use super::{NativeWindowRegistry, NativeWindowServer, WindowNotification};
use crate::platform::windows::event_loop::RunLoop;
use crate::platform::windows::external::HwndId;
use dome_core::WindowId;
use std::cell::RefCell;
use std::time::Instant;

pub(in crate::platform::windows) enum WindowEvent {
    FocusChanged(WindowId),
    FocusSettled(WindowId),
    MovementStarted(WindowId),
    MovementSettled {
        window_id: WindowId,
        observed_at: Instant,
    },
    DragEnded {
        window_id: WindowId,
        observed_at: Instant,
    },
    WindowDestroyed(WindowId),
    WindowHidden(WindowId),
    WindowCloaked(WindowId),
    WindowCreated(WindowId),
    WindowShown(WindowId),
    WindowUncloaked(WindowId),
    WindowMinimized(WindowId),
    WindowRestored {
        window_id: WindowId,
        observed_at: Instant,
    },
    PeriodicTick,
}

pub(in crate::platform::windows) enum NativeEvent {
    WindowCreated(HwndId),
    WindowShown(HwndId),
    WindowUncloaked(HwndId),
    WindowHidden(HwndId),
    WindowCloaked(HwndId),
    WindowMinimized(HwndId),
    WindowRestored {
        hwnd_id: HwndId,
        observed_at: Instant,
    },
    WindowTitleChanged(HwndId),
    WindowFocused(HwndId),
    MoveSizeStart(HwndId),
    MoveSizeEnd {
        hwnd_id: HwndId,
        observed_at: Instant,
    },
    LocationChanged {
        hwnd_id: HwndId,
        observed_at: Instant,
    },
    WindowDestroyed(HwndId),
}

impl NativeWindowServer {
    pub(in crate::platform::windows) fn timer_elapsed(&self, timer: usize) {
        let event = self.events.borrow_mut().timer_event(timer, kill_timer);
        if let Some(event) = event {
            self.observe(event);
        }
    }

    pub(super) fn consume_event(&self, event: NativeEvent) {
        let native = event.native();
        if matches!(
            event,
            NativeEvent::WindowCreated(_)
                | NativeEvent::WindowShown(_)
                | NativeEvent::WindowUncloaked(_)
        ) && self.discover(native).is_none()
        {
            return;
        }
        let Some(id) = self.registry.borrow_mut().consume(&event) else {
            return;
        };
        if matches!(event, NativeEvent::WindowTitleChanged(_)) {
            let owner = self.owner.clone();
            RunLoop::current().spawn(async move {
                let Some(server) = owner.upgrade() else {
                    return;
                };
                if let Ok(title) = server.read_title(id).await
                    && server.registry.borrow().eligible(id)
                {
                    server.emit(WindowNotification::TitleChanged {
                        window_id: id,
                        title,
                    });
                }
            });
            return;
        }
        let event = self.events.borrow_mut().process_event(event, id);
        if let Some(event) = event {
            self.observe(event);
        }
    }

    pub(super) fn emit(&self, notification: WindowNotification) {
        let _ = self.notifications.unbounded_send(notification);
    }

    pub(super) fn observe(&self, event: WindowEvent) {
        if let Some(notification) = immediate_notification(&event) {
            self.emit(notification);
            return;
        }
        let owner = self.owner.clone();
        RunLoop::current().spawn(async move {
            let Some(server) = owner.upgrade() else {
                return;
            };
            if let Some(notification) = observe_event(&server, &server.registry, event).await {
                server.emit(notification);
            }
        });
    }
}

impl NativeEvent {
    pub(super) fn native(&self) -> HwndId {
        match self {
            Self::WindowCreated(id)
            | Self::WindowShown(id)
            | Self::WindowUncloaked(id)
            | Self::WindowHidden(id)
            | Self::WindowCloaked(id)
            | Self::WindowMinimized(id)
            | Self::WindowTitleChanged(id)
            | Self::WindowFocused(id)
            | Self::MoveSizeStart(id)
            | Self::WindowDestroyed(id) => *id,
            Self::WindowRestored { hwnd_id, .. }
            | Self::MoveSizeEnd { hwnd_id, .. }
            | Self::LocationChanged { hwnd_id, .. } => *hwnd_id,
        }
    }
}

fn immediate_notification(event: &WindowEvent) -> Option<WindowNotification> {
    Some(match *event {
        WindowEvent::WindowDestroyed(id)
        | WindowEvent::WindowHidden(id)
        | WindowEvent::WindowCloaked(id) => WindowNotification::WindowRemoved(id),
        WindowEvent::WindowMinimized(id) => WindowNotification::WindowMinimized(id),
        WindowEvent::FocusChanged(id) => WindowNotification::FocusChanged(id),
        WindowEvent::FocusSettled(id) => WindowNotification::FocusSettled(id),
        WindowEvent::MovementStarted(id) => WindowNotification::MovementStarted(id),
        WindowEvent::PeriodicTick => WindowNotification::PeriodicTick,
        _ => return None,
    })
}

async fn observe_event(
    server: &NativeWindowServer,
    registry: &RefCell<NativeWindowRegistry>,
    event: WindowEvent,
) -> Option<WindowNotification> {
    match event {
        WindowEvent::WindowCreated(id)
        | WindowEvent::WindowShown(id)
        | WindowEvent::WindowUncloaked(id) => {
            let (window, rect, monitor) = server.read_window(id).await.ok()?;
            registry.borrow().complete(window, rect, monitor, None)
        }
        WindowEvent::DragEnded {
            window_id,
            observed_at,
        } => {
            let (window, rect, monitor) = server.read_window(window_id).await.ok()?;
            registry
                .borrow()
                .complete(window, rect, monitor, Some(observed_at))
        }
        WindowEvent::MovementSettled {
            window_id,
            observed_at,
        }
        | WindowEvent::WindowRestored {
            window_id,
            observed_at,
        } => {
            let result = server.read_placement(window_id).await.ok();
            registry
                .borrow()
                .complete_placement(window_id, observed_at, result)
        }
        _ => immediate_notification(&event),
    }
}
