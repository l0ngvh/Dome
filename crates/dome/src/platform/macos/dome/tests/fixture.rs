use super::super::{Dome, MonitorInfo};
use crate::action::WorkspaceInfo;
use crate::config::{LogLevel, LuaRuntime, PreferredLayouts};
use crate::logging::Logger;
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::platform::macos::display::MonitorObservation;
use crate::platform::macos::menu_bar::MenuBar;
use crate::platform::macos::ui::{FloatBorderPlacement, OverlayPlacements, Overlays};
use crate::platform::macos::window_server::{
    AppObservation, DebounceBurst, WindowMetadata, WindowNotification, WindowObservation,
    WindowServer,
};
use crate::platform::messages::HubMessage;
use anyhow::{Context, Result};
use dome_core::{ContainerId, FractionalCoordinate, Logical, MonitorId, PixelRect, WindowId};
use futures::{FutureExt, channel::oneshot};
use std::cell::{Cell, Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

pub(super) const PARTITION_CONFIG: &str = r#"
return {
    layout = "partition_tree",
    border_size = 1,
    minimum_width = 1,
    minimum_height = 1,
    partition_tree = { automatic_tiling = false, tab_bar_height = 2 },
    reserved_area = function(monitor) return nil end,
}
"#;

struct NoopLogger;
impl Logger for NoopLogger {
    fn set_level(&self, _: LogLevel) {}
}

struct NoopMenuBar;
impl MenuBar for NoopMenuBar {
    fn update_workspaces(&self, _: &[WorkspaceInfo]) {}
}

pub(super) struct TestDome {
    dome: Dome,
    server: Rc<MockWindowServer>,
    overlays: Rc<RefCell<OverlayState>>,
    next_window_id: usize,
}

#[derive(Default)]
pub(super) struct OverlayState {
    pub requested: Option<OverlayPlacements>,
    pub floats: HashMap<WindowId, FloatBorderPlacement>,
}

struct MockOverlays(Rc<RefCell<OverlayState>>);
impl Overlays for MockOverlays {
    fn update(&mut self, placements: OverlayPlacements) {
        let mut state = self.0.borrow_mut();
        state.floats = placements.floats.clone();
        state.requested = Some(placements);
    }
}

#[derive(Default)]
struct MockWindowServer {
    windows: RefCell<HashMap<WindowId, WindowObservation>>,
    listed: RefCell<Vec<WindowId>>,
    observed_focus: Cell<Option<WindowId>>,
    maximum: RefCell<HashMap<WindowId, (u32, u32)>>,
    resisted_frames: RefCell<HashMap<WindowId, PixelRect>>,
    frame_requests: RefCell<HashMap<WindowId, Vec<PixelRect>>>,
}

impl WindowServer for MockWindowServer {
    fn focus_overlay(&self) {
        self.observed_focus.set(None);
    }
    fn set_frame(&self, id: WindowId, mut rect: PixelRect) -> Result<()> {
        self.frame_requests
            .borrow_mut()
            .entry(id)
            .or_default()
            .push(rect);
        if let Some(&resisted) = self.resisted_frames.borrow().get(&id) {
            rect = resisted;
        }
        if let Some(&(width, height)) = self.maximum.borrow().get(&id) {
            rect = PixelRect::new(
                rect.x().value(),
                rect.y().value(),
                rect.width().value().min(width),
                rect.height().value().min(height),
            );
        }
        self.windows
            .borrow_mut()
            .get_mut(&id)
            .context("window no longer exists")?
            .rect = Some(rect);
        Ok(())
    }
    fn hide_at(
        &self,
        id: WindowId,
        x: FractionalCoordinate<Logical>,
        y: FractionalCoordinate<Logical>,
    ) -> Result<()> {
        let mut windows = self.windows.borrow_mut();
        let window = windows.get_mut(&id).context("window no longer exists")?;
        let mut frame = window.rect.expect("window frame").to_dimension();
        frame.x = x;
        frame.y = y;
        window.rect = Some(PixelRect::from_dimension(frame));
        Ok(())
    }
    fn focus(&self, id: WindowId) -> Result<()> {
        self.observed_focus.set(Some(id));
        Ok(())
    }
    fn minimize(&self, id: WindowId) -> Result<()> {
        self.windows
            .borrow_mut()
            .get_mut(&id)
            .context("window no longer exists")?
            .minimized = true;
        Ok(())
    }
    fn unminimize(&self, id: WindowId) -> Result<()> {
        self.windows
            .borrow_mut()
            .get_mut(&id)
            .context("window no longer exists")?
            .minimized = false;
        Ok(())
    }
    fn close(&self, id: WindowId) -> Result<()> {
        {
            let windows = self.windows.borrow();
            let window = windows.get(&id).context("window no longer exists")?;
            anyhow::ensure!(window.valid, "window no longer accepts messages");
        }
        self.windows.borrow_mut().remove(&id);
        self.listed.borrow_mut().retain(|listed| *listed != id);
        if self.observed_focus.get() == Some(id) {
            self.observed_focus.set(None);
        }
        Ok(())
    }
}

fn observe(window: &WindowObservation) -> WindowObservation {
    WindowObservation {
        id: window.id,
        pid: window.pid,
        metadata: window.metadata.clone(),
        valid: window.valid,
        minimized: window.minimized,
        native_fullscreen: window.native_fullscreen,
        role: window.role.clone(),
        subrole: window.subrole.clone(),
        is_root: window.is_root,
        position_settable: window.position_settable,
        size_settable: window.size_settable,
        main_settable: window.main_settable,
        rect: window.rect,
        placement: window.placement,
        corner_radius: window.corner_radius,
    }
}

pub(super) fn monitor(id: u32, x: f64) -> MonitorObservation {
    MonitorObservation {
        display_id: id,
        name: format!("Display {id}"),
        x,
        y: 0.0,
        width: 100.0,
        height: 20.0,
        top_inset: 0.0,
        bottom_inset: 0.0,
        is_primary: id == 1,
        backing_scale: 1.0,
    }
}

impl TestDome {
    pub fn new(lua_source: &str) -> Self {
        let (sender, _) = std::sync::mpsc::channel();
        let mut keymap_publisher = KeymapPublisher::new(KeymapView::new(), sender);
        let (runtime, config) = LuaRuntime::load(
            "test-config.lua".into(),
            Ok(lua_source.to_owned()),
            &mut keymap_publisher,
        )
        .expect("test Lua VM");
        let server = Rc::new(MockWindowServer::default());
        let overlays = Rc::new(RefCell::new(OverlayState::default()));
        let dome = Dome::new(
            &[MonitorInfo::from(monitor(1, 0.0))],
            config.tiling,
            PreferredLayouts::default(),
            runtime,
            keymap_publisher,
            config.env,
            server.clone(),
            Box::new(MockOverlays(overlays.clone())),
            Rc::new(NoopMenuBar),
            config.appearance,
            Box::new(NoopLogger),
            None,
        );
        Self {
            dome,
            server,
            overlays,
            next_window_id: 1,
        }
    }

    pub fn reload_config(&mut self, lua_source: &str) {
        let config = self
            .dome
            .hub
            .load_config(Ok(lua_source.to_owned()), &mut self.dome.keymap_publisher)
            .expect("test config reload succeeds");
        self.dome.config_changed(config);
    }

    pub fn open(&mut self, title: &str) -> WindowId {
        self.open_with(title, |_| {})
    }

    pub fn open_with(
        &mut self,
        title: &str,
        attributes: impl FnOnce(&mut WindowObservation),
    ) -> WindowId {
        let id = self.pending_window(title, attributes);
        self.report_app_windows();
        id
    }

    pub fn pending_window(
        &mut self,
        title: &str,
        attributes: impl FnOnce(&mut WindowObservation),
    ) -> WindowId {
        let id = WindowId::new(self.next_window_id);
        self.next_window_id += 1;
        let mut window = WindowObservation {
            id,
            pid: std::process::id() as i32 + 1,
            metadata: WindowMetadata {
                title: Some(title.into()),
                app_name: Some("Test".into()),
                bundle_id: None,
            },
            valid: true,
            minimized: false,
            native_fullscreen: Some(false),
            role: Some("AXWindow".into()),
            subrole: Some("AXStandardWindow".into()),
            is_root: true,
            position_settable: true,
            size_settable: true,
            main_settable: true,
            rect: Some(PixelRect::new(5, 5, 40, 10)),
            placement: None,
            corner_radius: None,
        };
        attributes(&mut window);
        self.server.windows.borrow_mut().insert(id, window);
        self.server.listed.borrow_mut().push(id);
        id
    }

    pub fn update_window(&mut self, id: WindowId, change: impl FnOnce(&mut WindowObservation)) {
        self.change(id, change);
        self.report_app_windows();
    }

    fn change(&self, id: WindowId, change: impl FnOnce(&mut WindowObservation)) {
        change(
            self.server
                .windows
                .borrow_mut()
                .get_mut(&id)
                .expect("observed window"),
        );
    }

    pub fn pending_removals(&self, ids: &[WindowId]) {
        for &id in ids {
            self.change(id, |window| window.valid = false);
        }
    }

    pub fn app_observation(&self, pid: i32) -> AppObservation {
        let windows = self.server.windows.borrow();
        let listed = self.server.listed.borrow();
        let requested = windows
            .iter()
            .filter(|(_, window)| window.pid == pid)
            .map(|(id, window)| (*id, observe(window)))
            .collect::<HashMap<_, _>>();
        let requested_ids = requested.keys().copied().collect();
        AppObservation::new(
            pid,
            listed
                .iter()
                .filter_map(|id| windows.get(id))
                .filter(|window| window.pid == pid && window.valid)
                .map(observe)
                .collect(),
            requested,
            requested_ids,
        )
    }

    fn report_app_windows(&mut self) {
        let mut pids: Vec<_> = self
            .server
            .windows
            .borrow()
            .values()
            .map(|window| window.pid)
            .collect();
        pids.sort_unstable();
        pids.dedup();
        for pid in pids {
            self.deliver_notification(WindowNotification::AppWindows(self.app_observation(pid)));
        }
    }

    pub fn pid(&self, id: WindowId) -> i32 {
        self.server.windows.borrow()[&id].pid
    }

    pub fn resist_frame(&self, id: WindowId, rect: PixelRect) {
        self.server.resisted_frames.borrow_mut().insert(id, rect);
    }

    pub fn frame_requests(&self, id: WindowId) -> Vec<PixelRect> {
        self.server
            .frame_requests
            .borrow()
            .get(&id)
            .cloned()
            .unwrap_or_default()
    }

    pub fn maximum_size(&self, id: WindowId, width: u32, height: u32) {
        self.server.maximum.borrow_mut().insert(id, (width, height));
    }

    pub fn hub_request<T>(&mut self, message: impl FnOnce(oneshot::Sender<T>) -> HubMessage) -> T {
        let (reply, result) = oneshot::channel();
        self.dome.handle_hub_message(message(reply));
        result
            .now_or_never()
            .expect("synchronous domain reply")
            .expect("domain sent reply")
    }

    pub fn tab_container(&self, titles: &[&str], monitor: Option<MonitorId>) -> ContainerId {
        let overlays = self.overlays();
        let matches: Vec<_> = overlays
            .requested
            .as_ref()
            .expect("presented overlays")
            .tabs
            .iter()
            .filter(|(_, (id, tab))| {
                monitor.is_none_or(|expected| expected == *id)
                    && tab
                        .placement
                        .titles
                        .iter()
                        .map(String::as_str)
                        .eq(titles.iter().copied())
            })
            .map(|(id, _)| *id)
            .collect();
        assert_eq!(
            matches.len(),
            1,
            "tab titles must identify exactly one container"
        );
        matches[0]
    }

    pub fn click_tab(&mut self, titles: &[&str], monitor: Option<MonitorId>, index: usize) {
        let container = self.tab_container(titles, monitor);
        self.dome.tab_clicked(container, index);
    }

    pub fn deliver_notification(&mut self, notification: WindowNotification) {
        self.dome.handle_window_notification(notification);
    }

    pub fn run(&mut self, command: &str) {
        let action = command.parse().expect("test action");
        self.dome
            .handle_action(&action)
            .expect("test action succeeds");
    }

    pub fn mirror_click(&mut self, id: WindowId) {
        self.dome.mirror_clicked(id);
    }

    pub fn focus(&mut self, id: WindowId) {
        self.server.observed_focus.set(Some(id));
        self.deliver_notification(WindowNotification::Focus(id));
    }

    pub fn remove(&mut self, id: WindowId) {
        self.server.windows.borrow_mut().remove(&id);
        self.server
            .listed
            .borrow_mut()
            .retain(|listed| *listed != id);
        self.deliver_notification(WindowNotification::WindowRemoved(id));
    }

    pub fn moved(&mut self, id: WindowId, rect: PixelRect) {
        self.change(id, |window| {
            window.rect = Some(rect);
            window.minimized = false;
        });
        let pid = self.server.windows.borrow()[&id].pid;
        let now = Instant::now();
        self.deliver_notification(WindowNotification::MovementEnded { pid });
        self.deliver_notification(WindowNotification::Movement {
            app: self.app_observation(pid),
            observed_at: DebounceBurst {
                first: now,
                last: now,
            },
        });
    }

    pub fn monitors_changed(&mut self, observations: Vec<MonitorObservation>) {
        self.dome
            .monitors_changed(observations.into_iter().map(Into::into).collect());
        self.dome.finish_monitor_settle();
    }

    pub fn frame(&self, id: WindowId) -> PixelRect {
        self.server.windows.borrow()[&id]
            .rect
            .expect("window frame")
    }

    pub fn focused(&self) -> Option<WindowId> {
        self.server.observed_focus.get()
    }

    pub fn minimized(&self, id: WindowId) -> bool {
        self.server.windows.borrow()[&id].minimized
    }

    pub fn overlays(&self) -> Ref<'_, OverlayState> {
        self.overlays.borrow()
    }
}
