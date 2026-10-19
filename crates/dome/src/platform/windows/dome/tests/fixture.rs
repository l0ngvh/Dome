use super::super::Dome;
use crate::action::WorkspaceInfo;
use crate::config::{LuaRuntime, PreferredLayouts};
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::platform::windows::display::MonitorInfo;
use crate::platform::windows::external::ShowCmd;
use crate::platform::windows::handle::OFFSCREEN_POS;
use crate::platform::windows::tray::Tray;
use crate::platform::windows::ui::{
    FloatBorderRequest, FloatBorderState, OverlayPlacements, Overlays,
};
use crate::platform::windows::window_server::{
    NativeWindowState, PlacementObservation, WindowMetadata, WindowNotification, WindowObservation,
    WindowOrder, WindowServer,
};
use dome_core::{
    FractionalLength, LimitObservation, LimitUpdate, Logical, MonitorId, PixelRect, WindowId,
};
use std::cell::{Cell, Ref, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::Instant;
use windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPEDWINDOW;

struct NoopTray;
impl Tray for NoopTray {
    fn update_workspaces(&self, _: &[WorkspaceInfo]) {}
}

pub(super) struct TestDome {
    pub dome: Dome,
    pub server: Rc<MockWindowServer>,
    overlays: Rc<RefCell<OverlayState>>,
    next_window: usize,
}

#[derive(Default)]
pub(super) struct OverlayState {
    pub requested: Option<OverlayPlacements>,
    pub floats: HashMap<WindowId, FloatBorderState>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum StackWindow {
    Application(WindowId),
    TilingOverlay(MonitorId),
    FloatBorder(WindowId),
}

struct MockOverlays {
    state: Rc<RefCell<OverlayState>>,
    server: Rc<MockWindowServer>,
}

impl Overlays for MockOverlays {
    fn update(&mut self, placements: OverlayPlacements) {
        let mut state = self.state.borrow_mut();
        for band in self.server.stack.borrow_mut().iter_mut() {
            band.retain(|window| match window {
                StackWindow::Application(_) => true,
                StackWindow::TilingOverlay(id) => placements.tiling.contains_key(id),
                StackWindow::FloatBorder(id) => placements.floats.contains_key(id),
            });
        }
        for (id, placement) in &placements.tiling {
            let window = StackWindow::TilingOverlay(*id);
            self.server.register_stack_window(window);
            self.server.apply_order(window, placement.order);
        }
        state
            .floats
            .retain(|id, _| placements.floats.contains_key(id));
        for (id, request) in &placements.floats {
            if let FloatBorderRequest::Present(border) = request {
                state.floats.insert(*id, *border);
                let window = StackWindow::FloatBorder(*id);
                self.server.register_stack_window(window);
                let (FloatBorderState::Visible(placement) | FloatBorderState::Hidden(placement)) =
                    border;
                self.server.apply_order(window, placement.order);
            }
        }
        if let Some(monitor) = placements.focus_tiling {
            self.server
                .raise_within_band(StackWindow::TilingOverlay(monitor));
        }
        state.requested = Some(placements);
    }
}

struct NoopLogger;
impl crate::logging::Logger for NoopLogger {
    fn set_level(&self, _: crate::config::LogLevel) {}
}

pub(super) fn monitor(handle: isize) -> MonitorInfo {
    MonitorInfo {
        handle,
        name: format!("Display {handle}"),
        gdi_device: format!("device{handle}"),
        work_area: PixelRect::new(if handle == 1 { 0 } else { 100 }, 0, 100, 20),
        is_primary: handle == 1,
        scale: 1.0,
    }
}

impl TestDome {
    pub fn new(lua_source: &str) -> Self {
        let (sender, _receiver) = std::sync::mpsc::channel();
        let mut keymap = KeymapPublisher::new(KeymapView::new(), sender);
        let (runtime, config) = LuaRuntime::load(
            "test-config.lua".into(),
            Ok(lua_source.to_owned()),
            &mut keymap,
        )
        .expect("test Lua VM");
        let server = Rc::new(MockWindowServer::default());
        let overlays = Rc::new(RefCell::new(OverlayState::default()));
        let mut dome = Dome::new(
            vec![monitor(1)],
            config.tiling,
            PreferredLayouts::default(),
            server.clone(),
            config.appearance,
            runtime,
            keymap,
            config.env,
            Box::new(MockOverlays {
                state: overlays.clone(),
                server: server.clone(),
            }),
            Rc::new(NoopTray),
            Box::new(NoopLogger),
        )
        .expect("test Dome");
        dome.apply_layout();
        Self {
            dome,
            server,
            overlays,
            next_window: 1,
        }
    }

    pub fn open(&mut self, title: &str) -> WindowId {
        let id = self.begin_window_observation(title);
        self.complete_window_observation(id);
        assert!(self.dome.is_managed(id));
        id
    }

    pub fn begin_window_observation(&mut self, title: &str) -> WindowId {
        let id = WindowId::new(self.next_window);
        self.next_window += 1;
        self.server
            .register_stack_window(StackWindow::Application(id));
        self.server.windows.borrow_mut().insert(
            id,
            WindowObservation {
                id,
                pid: if std::process::id() == 1 { 2 } else { 1 },
                metadata: WindowMetadata {
                    title: Some(title.into()),
                    process: "test.exe".into(),
                    process_path: None,
                    class: None,
                    aumid: None,
                    app_name: Some("Test".into()),
                },
                constraints: LimitObservation {
                    min_width: LimitUpdate::Cleared,
                    min_height: LimitUpdate::Cleared,
                    max_width: LimitUpdate::Cleared,
                    max_height: LimitUpdate::Cleared,
                },
                state: NativeWindowState {
                    visible: true,
                    minimized: false,
                    cloaked: false,
                    is_root: true,
                    has_owner: false,
                    style: WS_OVERLAPPEDWINDOW.0,
                    extended_style: 0,
                    elevated: false,
                    window_rect: PixelRect::new(5, 5, 40, 10),
                    monitor_work_area: Some(monitor(1).work_area),
                },
            },
        );
        id
    }

    pub fn complete_window_observation(&mut self, id: WindowId) {
        let window = self.server.observe(id);
        let rect = window.state.window_rect;
        self.deliver_notification(WindowNotification::WindowObserved {
            window,
            rect,
            monitor: 1,
            dragged_at: None,
        });
    }

    pub fn deliver_notification(&mut self, notification: WindowNotification) {
        self.dome.handle_window_notification(notification);
    }

    pub fn run(&mut self, command: &str) {
        self.dome
            .handle_action(&command.parse().expect("test action"))
            .expect("action succeeds");
    }

    pub fn focus(&mut self, window: WindowId) {
        self.server.set_foreground_window(window);
        self.deliver_notification(WindowNotification::FocusChanged(window));
    }

    pub fn remove(&mut self, window: WindowId) {
        self.server.windows.borrow_mut().remove(&window);
        for band in self.server.stack.borrow_mut().iter_mut() {
            band.retain(|entry| *entry != StackWindow::Application(window));
        }
        self.server.orders.borrow_mut().remove(&window);
        self.server.pending_placements.borrow_mut().remove(&window);
        self.server.corner_radii.borrow_mut().remove(&window);
        if self.server.observed_focus.get() == Some(window) {
            self.server.observed_focus.set(None);
        }
        self.deliver_notification(WindowNotification::WindowRemoved(window));
    }

    pub fn change(&self, window: WindowId, change: impl FnOnce(&mut WindowObservation)) {
        change(
            self.server
                .windows
                .borrow_mut()
                .get_mut(&window)
                .expect("observed window"),
        );
    }

    pub fn moved(&mut self, window_id: WindowId, rect: PixelRect, monitor: isize) {
        self.change(window_id, |window| window.state.window_rect = rect);
        let constraints = self.server.windows.borrow()[&window_id].constraints;
        self.deliver_notification(WindowNotification::PlacementChanged {
            window_id,
            observed_at: Instant::now(),
            placement: Some(PlacementObservation::Visible { rect, monitor }),
            constraints: Some(constraints),
        });
    }

    pub fn complete_placement_observations(&mut self, monitor: isize) {
        let windows = std::mem::take(&mut *self.server.pending_placements.borrow_mut());
        for window_id in windows {
            let window = self.server.observe(window_id);
            let placement = if window.state.minimized {
                PlacementObservation::Minimized
            } else {
                PlacementObservation::Visible {
                    rect: window.state.window_rect,
                    monitor,
                }
            };
            self.deliver_notification(WindowNotification::PlacementChanged {
                window_id,
                observed_at: Instant::now(),
                placement: Some(placement),
                constraints: Some(window.constraints),
            });
        }
    }

    pub fn set_corner_radius(&self, window: WindowId, radius: f64) {
        self.server
            .corner_radii
            .borrow_mut()
            .insert(window, FractionalLength::new(radius));
    }

    pub fn display_changed(&mut self, monitors: Vec<MonitorInfo>, exclusive: Option<WindowId>) {
        self.server.exclusive.set(exclusive);
        let windows = self.dome.apply_display_change(monitors, exclusive);
        self.server.request_constraints(windows);
    }

    pub fn frame(&self, window: WindowId) -> PixelRect {
        self.server.windows.borrow()[&window].state.window_rect
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.server.observed_focus.get()
    }

    pub fn minimized(&self, window: WindowId) -> bool {
        self.server.windows.borrow()[&window].state.minimized
    }

    pub fn order(&self, window: WindowId) -> Option<WindowOrder> {
        self.server.orders.borrow().get(&window).copied()
    }

    pub fn in_taskbar(&self, window: WindowId) -> bool {
        self.server.in_taskbar(window)
    }

    pub fn overlays(&self) -> Ref<'_, OverlayState> {
        self.overlays.borrow()
    }
}

#[derive(Default)]
pub(super) struct MockWindowServer {
    windows: RefCell<HashMap<WindowId, WindowObservation>>,
    observed_focus: Cell<Option<WindowId>>,
    exclusive: Cell<Option<WindowId>>,
    orders: RefCell<HashMap<WindowId, WindowOrder>>,
    // Normal then topmost, each front to back. This models successful supplied effects,
    // not native writes, owned windows, tabs, thumbnails or asynchronous failures.
    stack: RefCell<[Vec<StackWindow>; 2]>,
    taskbar: RefCell<HashSet<WindowId>>,
    pending_placements: RefCell<HashSet<WindowId>>,
    corner_radii: RefCell<HashMap<WindowId, FractionalLength<Logical>>>,
    pub resisted: RefCell<HashSet<WindowId>>,
    pub close_failures: RefCell<HashMap<WindowId, String>>,
}

impl MockWindowServer {
    pub fn stacking_order(&self) -> Vec<StackWindow> {
        let stack = self.stack.borrow();
        stack[1].iter().chain(&stack[0]).copied().collect()
    }

    pub fn in_taskbar(&self, window: WindowId) -> bool {
        self.taskbar.borrow().contains(&window)
    }

    pub fn observe(&self, id: WindowId) -> WindowObservation {
        let windows = self.windows.borrow();
        let window = &windows[&id];
        WindowObservation {
            id,
            pid: window.pid,
            metadata: window.metadata.clone(),
            constraints: window.constraints,
            state: NativeWindowState {
                visible: window.state.visible,
                minimized: window.state.minimized,
                cloaked: window.state.cloaked,
                is_root: window.state.is_root,
                has_owner: window.state.has_owner,
                style: window.state.style,
                extended_style: window.state.extended_style,
                elevated: window.state.elevated,
                window_rect: window.state.window_rect,
                monitor_work_area: window.state.monitor_work_area,
            },
        }
    }

    fn register_stack_window(&self, window: StackWindow) {
        let mut stack = self.stack.borrow_mut();
        if !stack.iter().any(|band| band.contains(&window)) {
            stack[0].insert(0, window);
        }
    }

    fn apply_order(&self, window: StackWindow, order: WindowOrder) {
        let mut stack = self.stack.borrow_mut();
        let current_band = stack
            .iter()
            .position(|band| band.contains(&window))
            .expect("registered stack window");
        let (target_band, after) = match order {
            WindowOrder::Unchanged => return,
            WindowOrder::Topmost => (1, None),
            WindowOrder::NotTopmost if current_band == 0 => return,
            WindowOrder::NotTopmost => (0, None),
            WindowOrder::After(id) => {
                let reference = StackWindow::Application(id);
                if reference == window {
                    return;
                }
                let Some(reference_band) = stack.iter().position(|band| band.contains(&reference))
                else {
                    return;
                };
                (reference_band, Some(reference))
            }
        };
        stack[current_band].retain(|entry| *entry != window);
        let band = &mut stack[target_band];
        let position = after.map_or(0, |reference| {
            band.iter()
                .position(|entry| *entry == reference)
                .expect("reference remains in target band")
                + 1
        });
        band.insert(position, window);
    }

    fn raise_within_band(&self, window: StackWindow) {
        let mut stack = self.stack.borrow_mut();
        let band = stack
            .iter_mut()
            .find(|band| band.contains(&window))
            .expect("registered foreground window");
        band.retain(|entry| *entry != window);
        band.insert(0, window);
    }
}

#[async_trait::async_trait(?Send)]
impl WindowServer for MockWindowServer {
    fn request_constraints(&self, _: Vec<WindowId>) {}

    async fn get_exclusive_fullscreen_window(&self) -> anyhow::Result<Option<WindowId>> {
        Ok(self.exclusive.get())
    }

    fn set_position(&self, id: WindowId, order: WindowOrder, rect: PixelRect) {
        if !self.resisted.borrow().contains(&id)
            && let Some(window) = self.windows.borrow_mut().get_mut(&id)
            && window.state.window_rect != rect
        {
            window.state.window_rect = rect;
            self.pending_placements.borrow_mut().insert(id);
        }
        self.set_z_order(id, order);
    }

    fn set_z_order(&self, id: WindowId, order: WindowOrder) {
        self.apply_order(StackWindow::Application(id), order);
        if order != WindowOrder::Unchanged {
            self.orders.borrow_mut().insert(id, order);
        }
    }

    fn move_offscreen(&self, id: WindowId) {
        let window = StackWindow::Application(id);
        {
            let mut stack = self.stack.borrow_mut();
            for band in stack.iter_mut() {
                band.retain(|entry| *entry != window);
            }
            stack[0].push(window);
        }
        if !self.resisted.borrow().contains(&id)
            && let Some(window) = self.windows.borrow_mut().get_mut(&id)
        {
            let rect = window.state.window_rect;
            let offscreen =
                PixelRect::from_pixels(OFFSCREEN_POS, OFFSCREEN_POS, rect.width(), rect.height());
            if window.state.window_rect != offscreen {
                window.state.window_rect = offscreen;
                self.pending_placements.borrow_mut().insert(id);
            }
        }
    }

    fn show_cmd(&self, id: WindowId, command: ShowCmd) -> anyhow::Result<()> {
        if let Some(window) = self.windows.borrow_mut().get_mut(&id) {
            window.state.minimized = matches!(command, ShowCmd::Minimize);
        }
        Ok(())
    }

    fn close(&self, id: WindowId) -> anyhow::Result<()> {
        if let Some(message) = self.close_failures.borrow().get(&id) {
            return Err(anyhow::anyhow!(message.clone()).context("close native window"));
        }
        Ok(())
    }

    fn add_tab(&self, id: WindowId) {
        self.taskbar.borrow_mut().insert(id);
    }

    fn delete_tab(&self, id: WindowId) {
        self.taskbar.borrow_mut().remove(&id);
    }

    fn set_foreground_window(&self, id: WindowId) {
        self.raise_within_band(StackWindow::Application(id));
        self.observed_focus.set(Some(id));
    }

    fn corner_radius(&self, id: WindowId) -> FractionalLength<Logical> {
        self.corner_radii
            .borrow()
            .get(&id)
            .copied()
            .unwrap_or(FractionalLength::ZERO)
    }
}
