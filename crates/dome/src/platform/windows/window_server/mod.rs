mod events;
mod observations;
mod operations;
mod registry;
mod timers;

use super::external::ShowCmd;
use crate::platform::windows::event_loop::{RunLoop, Shutdown};
use dome_core::{FractionalLength, Logical, Physical, PixelRect, WindowId};
pub(super) use events::{NativeEvent, WindowEvent};
use futures::{
    StreamExt,
    channel::mpsc::{UnboundedReceiver, UnboundedSender},
};
pub(super) use observations::{NativeWindowState, WindowMetadata, WindowObservation};
pub(super) use observations::{PlacementObservation, WindowNotification};
use registry::NativeWindowRegistry;
use std::cell::RefCell;
use std::rc::{Rc, Weak};
use timers::NativeEventState;
use windows::Win32::Foundation::HWND;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum WindowOrder {
    Topmost,
    NotTopmost,
    After(WindowId),
    Unchanged,
}

#[async_trait::async_trait(?Send)]
pub(super) trait WindowServer {
    fn request_constraints(&self, windows: Vec<WindowId>);
    async fn get_exclusive_fullscreen_window(&self) -> anyhow::Result<Option<WindowId>>;
    fn set_position(&self, id: WindowId, z: WindowOrder, rect: PixelRect<Physical>);
    fn set_z_order(&self, id: WindowId, z: WindowOrder);
    fn move_offscreen(&self, id: WindowId);
    fn show_cmd(&self, id: WindowId, command: ShowCmd) -> anyhow::Result<()>;
    fn close(&self, id: WindowId) -> anyhow::Result<()>;
    fn add_tab(&self, id: WindowId);
    fn delete_tab(&self, id: WindowId);
    fn set_foreground_window(&self, id: WindowId);
    fn corner_radius(&self, id: WindowId) -> FractionalLength<Logical>;
}

pub(super) struct NativeWindowServer {
    events: RefCell<NativeEventState>,
    registry: RefCell<NativeWindowRegistry>,
    taskbar: super::taskbar::Taskbar,
    notifications: UnboundedSender<WindowNotification>,
    owner: Weak<Self>,
    _event_hooks: super::event_listener::EventHookHandle,
}

impl NativeWindowServer {
    pub(super) fn new(
        mut timers: UnboundedReceiver<usize>,
        timer_window: HWND,
        shutdown: Shutdown,
    ) -> anyhow::Result<(Rc<Self>, UnboundedReceiver<WindowNotification>)> {
        let taskbar = super::taskbar::Taskbar::new()?;
        let (native_sender, mut native_events) = RunLoop::channel();
        let event_hooks = super::event_listener::start_event_hooks(native_sender, shutdown)?;
        let (notifications, receiver) = futures::channel::mpsc::unbounded();
        let events = NativeEventState::new(timer_window);
        let server = Rc::new_cyclic(|owner| Self {
            taskbar,
            registry: RefCell::new(NativeWindowRegistry::new()),
            events: RefCell::new(events),
            notifications,
            owner: owner.clone(),
            _event_hooks: event_hooks,
        });
        let owner = Rc::downgrade(&server);
        RunLoop::current().spawn(async move {
            while let Some(event) = native_events.next().await {
                let Some(server) = owner.upgrade() else { break };
                server.consume_event(event);
            }
        });
        let owner = Rc::downgrade(&server);
        RunLoop::current().spawn(async move {
            while let Some(timer) = timers.next().await {
                let Some(server) = owner.upgrade() else { break };
                server.timer_elapsed(timer);
            }
        });
        for id in server.enumerate_windows() {
            server.observe(WindowEvent::WindowCreated(id));
        }
        Ok((server, receiver))
    }
}

impl Drop for NativeWindowServer {
    fn drop(&mut self) {
        self.events.get_mut().cancel_timers();
    }
}
