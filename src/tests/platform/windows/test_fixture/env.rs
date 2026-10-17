use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use super::mock::{
    MockDisplay, MockExternalHwnd, MockSceneSender, MockWiring, MoveLog, NoopTaskbar,
    OverlayReport, ZOrderStack,
};
use super::{MonitorInfo, SPAWN_DIM, dim};
use crate::action::{Action, Actions, MinimizedWindow};
use crate::config::{Appearance, LuaRuntime, PreferredLayouts};
use crate::core::{
    ContainerPlacement, Dimension, FloatWindowPlacement, Length, Logical, MonitorId, Physical,
    PixelRect, Pixels, TilingConfig, TilingWindowPlacement, WindowId,
};
use crate::platform::keymap::KeymapPublisher;
use crate::platform::windows::HwndId;
use crate::platform::windows::{
    Dome, MonitorInfo as PlatformMonitorInfo, NewWindow, WindowsMetadata,
};
use crate::platform::windows::{FloatOverlayAction, RenderScene};

/// Last focus directive Dome issued.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FocusTarget {
    /// Before any focus directive has fired.
    Initial,
    Overlay,
    Window(HwndId),
}

/// A painted `ThumbnailShow` without its `window_id`, because a test names windows by `HwndId`.
#[derive(Debug, PartialEq)]
pub(crate) struct PaintedThumbnail {
    pub(crate) source: HwndId,
    pub(crate) frame: PixelRect,
    pub(crate) source_rect: PixelRect,
}

pub(crate) struct WindowsFixture {
    dome: Dome,
    moves: MoveLog,
    monitors: Arc<Mutex<Vec<PlatformMonitorInfo>>>,
    tiling: TilingConfig,
    z_stack: ZOrderStack,
    focus_target: Arc<Mutex<FocusTarget>>,
    mocks: HashMap<HwndId, Arc<MockExternalHwnd>>,
    exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
    scene: Rc<RefCell<MockSceneSender>>,
    next_window_id: isize,
}

pub(crate) struct WindowBuilder<'env> {
    env: &'env mut WindowsFixture,
    title: String,
    process: String,
    class: Option<String>,
    app_name: Option<String>,
    spawned_at: Dimension<Physical>,
    min_size: Option<(f32, f32)>,
    manageable: bool,
}

impl WindowBuilder<'_> {
    pub(crate) fn process(mut self, process: &str) -> Self {
        self.process = process.to_string();
        self
    }

    pub(crate) fn class(mut self, class: &str) -> Self {
        self.class = Some(class.to_string());
        self
    }

    pub(crate) fn spawned_at(mut self, spawned_at: Dimension<Physical>) -> Self {
        self.spawned_at = spawned_at;
        self
    }

    pub(crate) fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some((width, height));
        self
    }

    pub(crate) fn manageable(mut self, manageable: bool) -> Self {
        self.manageable = manageable;
        self
    }

    pub(crate) fn open(self) -> HwndId {
        let id = self.env.next_window_id;
        self.env.next_window_id += 1;
        let mut ext = MockExternalHwnd::new(id, &self.title, &self.process, self.env.wiring())
            .with_dimension(self.spawned_at)
            .with_manageable(self.manageable);
        if let Some(class) = &self.class {
            ext = ext.with_class(class);
        }
        if let Some(app_name) = &self.app_name {
            ext = ext.with_app_name(app_name);
        }
        if let Some((width, height)) = self.min_size {
            ext = ext.with_min_size(width, height);
        }
        self.env.open_with(Arc::new(ext))
    }

    fn title(mut self, title: &str) -> Self {
        self.title = title.to_string();
        self
    }

    fn app_name(mut self, app_name: &str) -> Self {
        self.app_name = Some(app_name.to_string());
        self
    }
}

impl WindowsFixture {
    pub(crate) fn move_size_started(&mut self, window: HwndId) {
        self.dome.move_size_started(window);
    }
    pub(crate) fn clear_move_state(&mut self, window: HwndId) {
        self.dome.clear_move_state(window);
    }
    pub(crate) fn location_changed(&mut self, window: HwndId) -> bool {
        self.dome.location_changed(window)
    }
    pub(crate) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        *self.monitors.lock().unwrap() = monitors
            .into_iter()
            .map(MonitorInfo::into_platform)
            .collect();
        self.dome.handle_display_change();
    }
    pub(crate) fn handle_dpi_change(&mut self) {
        self.dome.handle_dpi_change();
    }
    pub(crate) fn update_titles(&mut self, titles: Vec<(HwndId, Option<String>)>) {
        self.dome.update_titles(titles);
    }
    pub(crate) fn tab_clicked(&mut self, container: crate::core::ContainerId, index: usize) {
        self.dome.tab_clicked(container, index);
    }
    pub(crate) fn query_minimized_windows_json(&self) -> String {
        self.dome.query_minimized_windows_json()
    }
    pub(crate) fn query_workspaces_json(&self) -> String {
        self.dome.query_workspaces_json()
    }
    pub(crate) fn query_monitors_json(&self) -> String {
        self.dome.query_monitors_json()
    }
    pub(crate) fn set_constraints_for(
        &mut self,
        window: HwndId,
        constraints: crate::core::LimitObservation,
    ) {
        self.dome.set_constraints_for(window, constraints);
    }
    pub(crate) fn handle_window_moved(
        &mut self,
        window: HwndId,
        rect: PixelRect<Physical>,
        monitor: isize,
        observed_at: Instant,
    ) -> bool {
        self.dome
            .handle_window_moved(window, rect, monitor, observed_at)
    }
    pub(crate) fn retry_drifted_windows(&mut self) {
        self.dome.retry_drifted_windows();
    }

    pub(crate) fn new(
        tiling: TilingConfig,
        monitors: Vec<MonitorInfo>,
        runtime: LuaRuntime,
        keymap: KeymapPublisher,
    ) -> Self {
        setup_logger();
        let exclusive_fullscreen_hwnd = Arc::new(Mutex::new(None));
        let shared_monitors = Arc::new(Mutex::new(
            monitors
                .into_iter()
                .map(MonitorInfo::into_platform)
                .collect(),
        ));
        let display = MockDisplay {
            monitors: shared_monitors.clone(),
            exclusive_fullscreen_hwnd: exclusive_fullscreen_hwnd.clone(),
        };
        let wiring = MockWiring {
            moves: MoveLog::default(),
            z_stack: ZOrderStack::new(),
            focus_target: Arc::new(Mutex::new(FocusTarget::Initial)),
        };
        let scene = Rc::new(RefCell::new(MockSceneSender::new(wiring.clone())));
        let dome = Dome::new(
            tiling.clone(),
            PreferredLayouts::default(),
            Rc::new(NoopTaskbar),
            Box::new(display),
            Box::new(scene.clone()),
            runtime,
            keymap,
            HashMap::new(),
        )
        .unwrap();
        let mut env = WindowsFixture {
            dome,
            moves: wiring.moves,
            monitors: shared_monitors,
            tiling,
            z_stack: wiring.z_stack,
            focus_target: wiring.focus_target,
            mocks: HashMap::new(),
            exclusive_fullscreen_hwnd,
            scene,
            next_window_id: 1,
        };
        env.deliver_overlay_reports();
        env
    }

    pub(crate) fn layout(&mut self) {
        self.dome.apply_layout();
        self.deliver_overlay_reports();
    }

    pub(crate) fn window(&mut self) -> WindowBuilder<'_> {
        WindowBuilder {
            env: self,
            title: "App".to_string(),
            process: "app.exe".to_string(),
            class: None,
            app_name: None,
            spawned_at: SPAWN_DIM,
            min_size: None,
            manageable: true,
        }
    }

    pub(crate) fn open(&mut self) -> HwndId {
        self.window().open()
    }

    pub(crate) fn open_many(&mut self, count: usize) -> Vec<HwndId> {
        (0..count).map(|_| self.open()).collect()
    }

    pub(crate) fn open_bar(&mut self) -> HwndId {
        self.window()
            .title("Zebar - vanilla")
            .process("zebar.exe")
            .class("Tauri Window")
            .app_name("Zebar")
            .spawned_at(dim(0, 0, 1920, 30))
            .open()
    }

    pub(crate) fn settle(&mut self, limit: usize) {
        for _ in 0..limit {
            if !self.flush_moves() {
                return;
            }
        }
        let remaining = self.moves.len();
        if remaining > 0 {
            panic!("settle did not converge after {limit} iterations ({remaining} moves pending)");
        }
    }

    pub(crate) fn flush_moves(&mut self) -> bool {
        if self.moves.is_empty() {
            return false;
        }
        let mut last_pos: HashMap<HwndId, Dimension> = HashMap::new();
        for (id, dim) in self.moves.take() {
            last_pos.insert(id, dim);
        }
        for (hwnd_id, dim) in last_pos {
            self.dome.clear_move_state(hwnd_id);
            let minimized = self
                .mocks
                .get(&hwnd_id)
                .is_some_and(|m| m.minimized.load(Ordering::Relaxed));
            if minimized {
                // Win32 reports no move for an iconic window.
                continue;
            }
            let monitor = self.monitor_for_pos(dim.x, dim.y);
            self.dome.handle_window_moved(
                hwnd_id,
                PixelRect::from_dimension(dim),
                monitor,
                Instant::now(),
            );
        }
        self.layout();
        true
    }

    /// Configure a window to resist repositioning and report it at `pos`.
    pub(crate) fn simulate_resist(&self, hwnd: HwndId, pos: (i32, i32, i32, i32)) {
        let dim = Dimension::new(
            Length::new(pos.0 as f32),
            Length::new(pos.1 as f32),
            Length::new(pos.2 as f32),
            Length::new(pos.3 as f32),
        );
        let ext = self.mock(hwnd);
        ext.set_override_position(Some(pos));
        *ext.dimension.lock().unwrap() = dim;
        self.moves.record(hwnd, dim);
    }

    pub(crate) fn clear_moves(&self) {
        self.moves.clear();
    }

    pub(crate) fn assert_settled(&self, reason: &str) {
        assert!(self.moves.is_empty(), "{reason}");
    }

    pub(crate) fn assert_placement_pending(&self, reason: &str) {
        assert!(!self.moves.is_empty(), "{reason}");
    }

    pub(crate) fn moved(&self, hwnd: HwndId) -> bool {
        self.moves.contains(hwnd)
    }

    pub(crate) fn destroy_window(&mut self, hwnd: HwndId) {
        self.mocks.remove(&hwnd);
        self.dome.window_destroyed(hwnd);
        self.z_stack.remove(hwnd);
        self.layout();
    }

    pub(crate) fn minimize_window(&mut self, hwnd: HwndId) {
        self.mock(hwnd).minimized.store(true, Ordering::Relaxed);
        self.dome.window_minimized(hwnd);
        self.layout();
    }

    pub(crate) fn unminimize_window(&mut self, hwnd: HwndId) {
        // The flag clears before the move, which is the OS order of MINIMIZEEND
        // before LOCATIONCHANGE.
        self.mock(hwnd).minimized.store(false, Ordering::Relaxed);
        let dim = self.mock(hwnd).get_dim();
        self.moves.record(hwnd, dim);
        self.flush_moves();
    }

    pub(crate) fn focus_window(&mut self, hwnd: HwndId) {
        self.dome.handle_focus(hwnd);
        self.layout();
    }

    /// Replays `HubEvent::ThumbnailClicked`, including clicks on an older scene.
    pub(crate) fn click_thumbnail(&mut self, id: WindowId) {
        self.dome.thumbnail_clicked(id);
        self.deliver_overlay_reports();
    }

    pub(crate) fn thumbnail_window_id(&self, index: usize, source: HwndId) -> WindowId {
        let scene = self.painted_scene();
        let mut matching = scene.monitors[index]
            .thumbnails
            .iter()
            .filter(|show| show.source == source);
        let thumbnail = matching
            .next()
            .expect("the scene shows a thumbnail for this source");
        assert!(
            matching.next().is_none(),
            "one thumbnail per source on a monitor"
        );
        thumbnail.window_id
    }

    pub(crate) fn dim(&self, hwnd: HwndId) -> Dimension {
        self.mock(hwnd).get_dim()
    }

    pub(crate) fn move_window_to(&self, hwnd: HwndId, dim: Dimension) {
        *self.mock(hwnd).dimension.lock().unwrap() = dim;
        self.moves.record(hwnd, dim);
    }

    pub(crate) fn is_minimized(&self, hwnd: HwndId) -> bool {
        self.mock(hwnd).minimized.load(Ordering::Relaxed)
    }

    pub(crate) fn is_offscreen(&self, hwnd: HwndId) -> bool {
        self.mock(hwnd).is_offscreen()
    }

    pub(crate) fn is_topmost(&self, hwnd: HwndId) -> bool {
        self.z_stack.is_topmost(hwnd)
    }

    /// True when `hwnd` sits below every displayed peer and below the tiling
    /// overlay. Win32 walks the z-order downward to pick the next focus when a
    /// window closes, so a parked window above the overlay would bring its
    /// workspace back.
    pub(crate) fn is_bottom(&self, hwnd: HwndId) -> bool {
        let stack = self.z_stack.stack();
        let Some(idx) = stack.iter().position(|&h| h == hwnd) else {
            return false;
        };
        let displayed_above = self
            .mocks
            .values()
            .filter(|m| m.hwnd_id != hwnd && !m.is_offscreen())
            .all(|m| match stack.iter().position(|&h| h == m.hwnd_id) {
                Some(peer_idx) => peer_idx < idx,
                None => true,
            });
        if !displayed_above {
            return false;
        }
        let overlay_ids = self.tiling_overlay_ids();
        overlay_ids.iter().all(
            |&overlay_id| match stack.iter().position(|&h| h == overlay_id) {
                Some(overlay_idx) => overlay_idx < idx,
                None => true,
            },
        )
    }

    pub(crate) fn clear_override_position(&self, hwnd: HwndId) {
        self.mock(hwnd).set_override_position(None);
    }

    pub(crate) fn run_actions(&mut self, s: &str) {
        let action: Action = s.parse().unwrap();
        self.send_action(&action);
    }

    pub(crate) fn only_minimized_window_id(&self) -> WindowId {
        let windows: Vec<MinimizedWindow> =
            serde_json::from_str(&self.query_minimized_windows_json()).unwrap();
        assert_eq!(windows.len(), 1, "expected exactly one minimized window");
        windows[0].id
    }

    pub(crate) fn run_unminimize(&mut self, hwnd: HwndId) {
        let window_id = self.only_minimized_window_id();
        assert!(self.is_minimized(hwnd), "restore target must be minimized");
        self.send_action(&Action::UnminimizeWindow { id: window_id });
        assert!(
            !self.is_minimized(hwnd),
            "unminimize action must restore the requested window"
        );
        self.restore_reported(hwnd);
    }

    pub(crate) fn enter_exclusive_fullscreen(&mut self, hwnd: HwndId) {
        *self.exclusive_fullscreen_hwnd.lock().unwrap() = Some(hwnd);
        self.dome.handle_display_change();
        *self.exclusive_fullscreen_hwnd.lock().unwrap() = None;
        self.layout();
    }

    pub(crate) fn focus_target(&self) -> FocusTarget {
        *self.focus_target.lock().unwrap()
    }

    pub(crate) fn tiling_overlay_id(&self) -> HwndId {
        let tiling = self.tiling_overlay_ids();
        assert_eq!(tiling.len(), 1, "expected single tiling overlay");
        tiling[0]
    }

    pub(crate) fn tiling_overlay_id_for(&self, monitor: MonitorId) -> Option<HwndId> {
        self.scene.borrow().tiling_overlay_for(monitor)
    }

    pub(crate) fn float_overlay_id(&self, window: WindowId) -> Option<HwndId> {
        self.scene.borrow().float_overlay_for(window)
    }

    pub(crate) fn float_overlay_ids(&self) -> Vec<HwndId> {
        self.scene.borrow().float_overlay_ids()
    }

    pub(crate) fn painted_monitors(&self) -> usize {
        self.painted_scene().monitors.len()
    }

    pub(crate) fn painted_monitor_id(&self, index: usize) -> MonitorId {
        self.painted_scene().monitors[index].monitor_id
    }

    pub(crate) fn painted_windows(&self, index: usize) -> Vec<TilingWindowPlacement> {
        self.painted_scene().monitors[index]
            .tiling_windows
            .iter()
            .map(|show| show.placement)
            .collect()
    }

    pub(crate) fn painted_corner_radii(&self, index: usize) -> Vec<Length<Logical>> {
        self.painted_scene().monitors[index]
            .tiling_windows
            .iter()
            .map(|show| show.corner_radius)
            .collect()
    }

    pub(crate) fn thumbnail_corner_radius(
        &self,
        index: usize,
        source: HwndId,
    ) -> Option<Length<Logical>> {
        let id = self.thumbnail_window_id(index, source);
        self.painted_scene().monitors[index]
            .tiling_windows
            .iter()
            .find(|show| show.placement.id == id)
            .map(|show| show.corner_radius)
    }

    pub(crate) fn painted_thumbnails(&self, index: usize) -> Vec<PaintedThumbnail> {
        self.painted_scene().monitors[index]
            .thumbnails
            .iter()
            .map(|show| PaintedThumbnail {
                source: show.source,
                frame: show.frame,
                source_rect: show.source_rect,
            })
            .collect()
    }

    pub(crate) fn painted_containers(&self, index: usize) -> Vec<ContainerPlacement> {
        self.painted_scene().monitors[index].containers.clone()
    }

    pub(crate) fn painted_border_thickness(&self, index: usize) -> Pixels<Physical> {
        self.painted_scene().monitors[index].border_thickness
    }

    pub(crate) fn painted_work_area(&self, index: usize) -> PixelRect {
        self.painted_scene().monitors[index].work_area
    }

    /// The float placement the newest scene paints, for the one float these tests keep.
    pub(crate) fn painted_float(&self) -> Option<FloatWindowPlacement> {
        self.painted_scene()
            .float_overlays
            .iter()
            .find_map(|a| match a {
                FloatOverlayAction::Create { placement, .. }
                | FloatOverlayAction::Update { placement, .. } => Some(*placement),
                FloatOverlayAction::Hide(_) => None,
            })
    }

    pub(crate) fn only_painted_float(&self) -> FloatWindowPlacement {
        let scene = self.painted_scene();
        let mut windows = scene
            .monitors
            .iter()
            .flat_map(|monitor| &monitor.float_windows);
        let window = *windows.next().expect("the scene paints one float window");
        assert!(
            windows.next().is_none(),
            "expected exactly one painted float"
        );
        window
    }

    pub(crate) fn painted_float_corner_radius(&self) -> Option<Length<Logical>> {
        self.painted_scene()
            .float_overlays
            .iter()
            .find_map(|a| match a {
                FloatOverlayAction::Create { corner_radius, .. }
                | FloatOverlayAction::Update { corner_radius, .. } => Some(*corner_radius),
                FloatOverlayAction::Hide(_) => None,
            })
    }

    pub(crate) fn set_corner_radius(&self, hwnd: HwndId, radius: Length<Logical>) {
        self.mock(hwnd).set_corner_radius(radius);
    }

    pub(crate) fn only_painted_window(&self) -> TilingWindowPlacement {
        let windows = self.painted_windows(0);
        assert_eq!(windows.len(), 1, "these tests tile exactly one window");
        windows[0]
    }

    pub(crate) fn border(&self) -> Length {
        Length::from_pixels(self.border_at(1.0))
    }

    pub(crate) fn border_at(&self, scale: f32) -> Pixels<Physical> {
        Pixels::round(Length::from_pixels(self.tiling.border_size).to_unit(scale))
    }

    pub(crate) fn outset(&self, content_box: Dimension) -> Dimension {
        self.outset_at(content_box, 1.0)
    }

    pub(crate) fn outset_at(&self, content_box: Dimension, scale: f32) -> Dimension {
        PixelRect::from_dimension(content_box)
            .outset_by(self.border_at(scale))
            .to_dimension()
    }

    pub(crate) fn clipped_outset(&self, content_box: Dimension) -> Dimension {
        PixelRect::from_dimension(self.outset(content_box))
            .clip(self.primary_monitor().work_area)
            .expect("the border box still overlaps the work area")
            .to_dimension()
    }

    pub(crate) fn inset_at(&self, border_box: Dimension, scale: f32) -> Dimension {
        PixelRect::from_dimension(border_box)
            .inset_by(self.border_at(scale))
            .to_dimension()
    }

    pub(crate) fn assert_horizontally_tiled(&self, windows: &[Dimension]) {
        let primary = self.primary_monitor();
        let work_area = primary.work_area.to_dimension();
        let border = Length::from_pixels(self.border_at(primary.scale));
        assert!(!windows.is_empty());
        for (index, placed) in windows.iter().enumerate() {
            assert_eq!(placed.y, border, "window {index} y");
            assert_eq!(
                placed.height,
                work_area.height - border * 2.0,
                "window {index} height"
            );
            assert!(placed.width > Length::new(0.0), "window {index} width");
        }
        assert_eq!(windows[0].x, border, "first window x");
        let last = windows.last().unwrap();
        assert_eq!(
            last.x + last.width,
            work_area.width - border,
            "last window right edge"
        );
        for index in 1..windows.len() {
            let gap = windows[index].x - (windows[index - 1].x + windows[index - 1].width);
            assert_eq!(
                gap,
                border * 2.0,
                "gap between window {} and {}",
                index - 1,
                index
            );
        }
    }

    pub(crate) fn window_appearance(&self) -> Option<Appearance> {
        self.scene.borrow().appearance()
    }

    pub(crate) fn reload(&mut self) {
        self.tiling = self
            .dome
            .reload()
            .expect("the test config should load")
            .tiling;
        self.layout();
    }

    pub(crate) fn add_monitor(&mut self, monitor: MonitorInfo) {
        self.monitors.lock().unwrap().push(monitor.into_platform());
        self.dome.handle_display_change();
        self.deliver_overlay_reports();
        self.layout();
    }

    pub(crate) fn remove_monitor(&mut self, handle: isize) {
        self.monitors.lock().unwrap().retain(|m| m.handle != handle);
        self.dome.handle_display_change();
        self.deliver_overlay_reports();
        self.layout();
    }

    pub(crate) fn change_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        *self.monitors.lock().unwrap() = monitors
            .into_iter()
            .map(MonitorInfo::into_platform)
            .collect();
        self.dome.handle_display_change();
        self.deliver_overlay_reports();
        self.layout();
    }

    pub(crate) fn set_monitor_scale(&self, handle: isize, scale: f32) {
        let mut monitors = self.monitors.lock().unwrap();
        let monitor = monitors
            .iter_mut()
            .find(|m| m.handle == handle)
            .unwrap_or_else(|| panic!("monitor {handle} is not in the test monitor set"));
        monitor.scale = scale;
    }

    pub(crate) fn raise_without_notifying_dome(&self, hwnd: HwndId) {
        self.z_stack.move_to_top(hwnd);
    }

    pub(crate) fn z_order(&self) -> Vec<HwndId> {
        self.z_stack.stack()
    }

    pub(crate) fn tiling_z_order(&self) -> Vec<HwndId> {
        self.z_stack.normal_stack()
    }

    /// Stands in for the runner's `dispatch_window_created`, except that the
    /// monitor is fixed at 1 and the mock's `manageable` flag decides
    /// manageability. Keep everything else in step with the runner.
    fn open_with(&mut self, ext: Arc<MockExternalHwnd>) -> HwndId {
        let hwnd_id = ext.hwnd_id;
        self.mocks.insert(hwnd_id, ext.clone());
        let metadata = WindowsMetadata {
            title: ext.title.clone(),
            process: ext.process.clone(),
            process_path: None,
            class: ext.class.clone(),
            aumid: None,
            app_name: ext.app_name.clone(),
        };
        if !ext.manageable {
            return hwnd_id;
        }
        let new = NewWindow {
            ext: ext.clone(),
            metadata,
            constraints: ext.constraints,
        };
        let dim = ext.get_dim();
        self.dome.add_window(new, PixelRect::from_dimension(dim), 1);
        self.deliver_overlay_reports();
        hwnd_id
    }

    /// Drains the pending reports into the domain, standing in for
    /// `HubEvent::TilingOverlayReady` and `HubEvent::FloatOverlayReady`.
    fn deliver_overlay_reports(&mut self) {
        let reports = self.scene.borrow_mut().take_pending_reports();
        for report in reports {
            match report {
                OverlayReport::Tiling(monitor, overlay) => {
                    self.dome.tiling_overlay_ready(monitor, overlay);
                }
                OverlayReport::Float(window, overlay) => {
                    self.dome.float_overlay_ready(window, overlay);
                }
            }
        }
    }

    fn primary_monitor(&self) -> PlatformMonitorInfo {
        let monitors = self.monitors.lock().unwrap();
        monitors
            .iter()
            .find(|m| m.is_primary)
            .cloned()
            .expect("the test monitor set has a primary monitor")
    }

    fn monitor_for_pos(&self, x: Length, y: Length) -> isize {
        let monitors = self.monitors.lock().unwrap();
        monitors
            .iter()
            .find(|m| {
                let d = m.work_area.to_dimension();
                x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height
            })
            .map(|m| m.handle)
            .unwrap_or(1)
    }

    fn mock(&self, hwnd: HwndId) -> &MockExternalHwnd {
        self.mocks.get(&hwnd).unwrap_or_else(|| {
            panic!("window {hwnd:?} is not registered (destroyed or never opened?)")
        })
    }

    fn painted_scene(&self) -> Ref<'_, RenderScene> {
        Ref::map(self.scene.borrow(), |sender| sender.latest_scene())
    }

    fn wiring(&self) -> MockWiring {
        MockWiring {
            moves: self.moves.clone(),
            z_stack: self.z_stack.clone(),
            focus_target: self.focus_target.clone(),
        }
    }

    fn tiling_overlay_ids(&self) -> Vec<HwndId> {
        self.scene.borrow().tiling_overlay_ids()
    }

    fn send_action(&mut self, action: &Action) {
        // The harness runs no main thread, so an `exit` action has nothing to quit.
        let main_thread_id = 0;
        self.dome
            .handle_actions(&Actions::new(vec![action.clone()]), main_thread_id);
        self.deliver_overlay_reports();
    }

    /// Replay the restore event, which Win32 reports as a move at the window's
    /// current rect.
    fn restore_reported(&mut self, hwnd: HwndId) {
        let dim = self.mock(hwnd).get_dim();
        self.moves.record(hwnd, dim);
        self.flush_moves();
    }
}

fn setup_logger() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
