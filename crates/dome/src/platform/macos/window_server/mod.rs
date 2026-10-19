use dome_core::FractionalCoordinate;
mod events;
mod focus;
mod listeners;
mod observations;
mod operations;
mod registry;

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use dome_core::{Logical, PixelRect, WindowId};
use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;
use objc2_core_graphics::CGWindowID;

use crate::platform::macos::accessibility::{AccessibilityApplication, FocusState};
use crate::platform::macos::event_loop::{EventSender, RunLoop};
use events::WindowEvent;
pub(super) use events::{DebounceBurst, WindowNotification};
pub(in crate::platform::macos) use events::{NativeEvent, send_native_event};
use listeners::EventListener;
pub(super) use observations::{
    AppObservation, NativeFullscreenObservation, SystemObservation, WindowMetadata,
    WindowObservation,
};
pub(super) use observations::{NativeAppObservation, NativeFocusedWindow, NativeWindowObservation};

pub(super) trait WindowServer {
    fn set_frame(&self, id: WindowId, rect: PixelRect) -> Result<()>;
    fn focus(&self, id: WindowId) -> Result<()>;
    fn focus_overlay(&self);
    fn hide_at(
        &self,
        id: WindowId,
        x: FractionalCoordinate<Logical>,
        y: FractionalCoordinate<Logical>,
    ) -> Result<()>;
    fn minimize(&self, id: WindowId) -> Result<()>;
    fn unminimize(&self, id: WindowId) -> Result<()>;
    fn close(&self, id: WindowId) -> Result<()>;
}

pub(in crate::platform::macos) struct NativeWindowServer {
    listener: EventListener,
    state: RefCell<NativeState>,
}

impl NativeWindowServer {
    pub(in crate::platform::macos) fn new(
        is_suspended: Arc<AtomicBool>,
    ) -> Result<(Rc<Self>, UnboundedReceiver<WindowNotification>)> {
        let (sender, mut events) = RunLoop::channel();
        let (notifications, receiver) = RunLoop::channel();
        let server = Rc::new(Self {
            listener: EventListener::new(sender.clone(), is_suspended.clone()),
            state: RefCell::new(NativeState {
                applications: HashMap::new(),
                focus: Arc::new(Mutex::new(FocusState::default())),
                focus_timer: None,
                suspended: is_suspended,
                window_ids: HashMap::new(),
                native_ids: HashMap::new(),
                next_window_id: 1,
                sender,
                notifications,
                apps: HashMap::new(),
                window_pids: HashMap::new(),
                observed_pids: HashSet::new(),
                movement: HashMap::new(),
                timer_generation: 0,
            }),
        });
        server.register_current_windows()?;
        server.refresh_all_observers();
        let owner = Rc::downgrade(&server);
        RunLoop::current().spawn(async move {
            while let Some(event) = events.next().await {
                let Some(server) = owner.upgrade() else { break };
                server.receive(event);
            }
        });
        server.schedule(WindowEvent::RefreshTimerElapsed);
        Ok((server, receiver))
    }
}

struct NativeState {
    applications: HashMap<i32, AccessibilityApplication>,
    focus: Arc<Mutex<FocusState>>,
    focus_timer: Option<crate::platform::macos::timer::Timer>,
    suspended: Arc<AtomicBool>,
    window_ids: HashMap<CGWindowID, WindowId>,
    native_ids: HashMap<WindowId, CGWindowID>,
    next_window_id: usize,
    sender: EventSender<NativeEvent>,
    notifications: EventSender<WindowNotification>,
    apps: HashMap<i32, bool>,
    window_pids: HashMap<CGWindowID, i32>,
    observed_pids: HashSet<i32>,
    movement: HashMap<i32, (u64, DebounceBurst, crate::platform::macos::timer::Timer)>,
    timer_generation: u64,
}
