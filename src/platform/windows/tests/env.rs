use std::cell::{OnceCell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use super::fixtures::{SPAWN_DIM, baseline_tiling, default_monitor, dim};
use super::mock::{
    MockDisplay, MockExternalHwnd, MockInternalWindows, MockSceneSender, MockWindowServer,
    MockWiring, MoveLog, NoopTaskbar, ZOrderStack,
};
use crate::action::{Action, Actions};
use crate::config::lua::test_support::{TempFile, loaded_runtime};
use crate::config::{Appearance, PreferredLayouts};
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, FloatWindowPlacement, Length, LimitObservation,
    Logical, MonitorId, Physical, PixelRect, Pixels, TilingConfig, TilingWindowPlacement, WindowId,
};
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::platform::windows::dome::events::{HubMessage, Scene};
use crate::platform::windows::dome::{Dome, HubEvent, MonitorInfo, WindowsMetadata};
use crate::platform::windows::external::HwndId;
use crate::platform::windows::ui::UiState;
use crate::platform::windows::window_server::{
    DiscoveredWindow, NativeEvent, Observation, SettleRead,
};
use crate::platform::windows::{HubSender, Wake, WindowLoopCallbacks, WindowLoopHandler};
use std::sync::mpsc;

/// Last focus directive Dome issued.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum FocusTarget {
    /// Before any focus directive has fired.
    Initial,
    Overlay,
    Window(HwndId),
}

pub(super) struct TestEnv {
    dome: Dome,
    moves: MoveLog,
    display_failed: Arc<AtomicBool>,
    monitors: Arc<Mutex<Vec<MonitorInfo>>>,
    tiling: TilingConfig,
    config_file: TempFile,
    z_stack: ZOrderStack,
    pub(super) focus_target: Arc<Mutex<FocusTarget>>,
    mocks: HashMap<HwndId, Arc<MockExternalHwnd>>,
    handler: WindowLoopHandler,
    server: Rc<RefCell<MockWindowServer>>,
    internal: Rc<RefCell<MockInternalWindows>>,
    ui_intake: mpsc::Sender<HubMessage>,
    reports: mpsc::Receiver<HubEvent>,
    now: Instant,
    deadlines: Rc<RefCell<HashMap<Wake, Instant>>>,
    wakes: Rc<RefCell<VecDeque<Wake>>>,
    hold_reports: bool,
    scene: Rc<RefCell<MockSceneSender>>,
    next_window_id: isize,
}

pub(super) struct TestEnvBuilder {
    tiling: TilingConfig,
    config_source: String,
    monitors: Vec<MonitorInfo>,
}

pub(super) struct WindowBuilder<'env> {
    env: &'env mut TestEnv,
    title: String,
    process: String,
    class: Option<String>,
    app_name: Option<String>,
    spawned_at: Dimension<Physical>,
    min_size: Option<(f32, f32)>,
    manageable: bool,
}

impl TestEnvBuilder {
    pub(super) fn monitors(mut self, monitors: Vec<MonitorInfo>) -> Self {
        self.monitors = monitors;
        self
    }

    pub(super) fn tiling(mut self, adjust: impl FnOnce(&mut TilingConfig)) -> Self {
        adjust(&mut self.tiling);
        self
    }

    /// The Lua config the runtime loads before `Dome::new`. The tiling config
    /// still comes from `tiling`.
    pub(super) fn config_source(mut self, source: &str) -> Self {
        self.config_source = source.to_string();
        self
    }

    pub(super) fn build(self) -> TestEnv {
        setup_logger();

        let Self {
            tiling,
            config_source,
            monitors,
        } = self;
        let shared_monitors = Arc::new(Mutex::new(monitors));
        let display_failed = Arc::new(AtomicBool::new(false));
        let display = MockDisplay {
            failed: display_failed.clone(),
            monitors: shared_monitors.clone(),
        };
        let wiring = MockWiring {
            moves: MoveLog::default(),
            z_stack: ZOrderStack::new(),
            focus_target: Arc::new(Mutex::new(FocusTarget::Initial)),
        };
        let scene = Rc::new(RefCell::new(MockSceneSender::new()));
        let (keymap_tx, _keymap_rx) = std::sync::mpsc::channel();
        let keymap = KeymapPublisher::new(KeymapView::new(), keymap_tx);
        let (runtime, config_file) = loaded_runtime("windows_env", &config_source);
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
        let server = Rc::new(RefCell::new(MockWindowServer::default()));
        let internal = Rc::new(RefCell::new(MockInternalWindows::new(wiring.clone())));
        internal.borrow_mut().trace = server.borrow().trace.clone();
        internal.borrow_mut().stacking = server.borrow().stacking.clone();
        let (ui_intake, messages) = mpsc::channel();
        let (events, reports) = mpsc::channel();
        let hub = HubSender {
            thread_id: 0,
            events,
        };
        let callbacks = Rc::new(OnceCell::new());
        let deadlines = Rc::new(RefCell::new(HashMap::new()));
        let timed = deadlines.clone();
        let stopped = deadlines.clone();
        let wakes = Rc::new(RefCell::new(VecDeque::new()));
        server.borrow_mut().wakes = wakes.clone();
        let queued = wakes.clone();
        let stopped_wakes = wakes.clone();
        let _ = callbacks.set(WindowLoopCallbacks {
            wake: Box::new(move |wake| queued.borrow_mut().push_back(wake)),
            wake_at: Box::new(move |wake, deadline| {
                if let Some(deadline) = deadline {
                    timed.borrow_mut().insert(wake, deadline);
                } else {
                    timed.borrow_mut().remove(&wake);
                }
            }),
            stop_wakes: Box::new(move || {
                stopped.borrow_mut().clear();
                stopped_wakes.borrow_mut().clear();
            }),
            terminate: Box::new(|| {}),
        });
        let handler = WindowLoopHandler {
            ui: UiState::new(messages, hub.clone(), Box::new(internal.clone())),
            windows: Box::new(server.clone()),
            callbacks,
            hub_sender: hub,
            observe_foreground: Box::new(|| {}),
            stopping: false,
        };
        let mut env = TestEnv {
            dome,
            moves: wiring.moves,
            display_failed,
            monitors: shared_monitors,
            tiling,
            config_file,
            z_stack: wiring.z_stack,
            focus_target: wiring.focus_target,
            mocks: HashMap::new(),
            handler,
            server,
            internal,
            ui_intake,
            reports,
            now: Instant::now(),
            deadlines,
            wakes,
            hold_reports: false,
            scene,
            next_window_id: 1,
        };
        env.layout();
        env
    }
}

impl WindowBuilder<'_> {
    pub(super) fn process(mut self, process: &str) -> Self {
        self.process = process.to_string();
        self
    }

    pub(super) fn class(mut self, class: &str) -> Self {
        self.class = Some(class.to_string());
        self
    }

    pub(super) fn spawned_at(mut self, spawned_at: Dimension<Physical>) -> Self {
        self.spawned_at = spawned_at;
        self
    }

    pub(super) fn min_size(mut self, width: f32, height: f32) -> Self {
        self.min_size = Some((width, height));
        self
    }

    pub(super) fn manageable(mut self, manageable: bool) -> Self {
        self.manageable = manageable;
        self
    }

    pub(super) fn open(self) -> HwndId {
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

impl TestEnv {
    pub(super) fn new() -> Self {
        Self::builder().build()
    }

    pub(super) fn builder() -> TestEnvBuilder {
        TestEnvBuilder {
            tiling: baseline_tiling(),
            config_source: "return {}".to_string(),
            monitors: vec![default_monitor()],
        }
    }

    pub(super) fn layout(&mut self) {
        self.dome.apply_layout();
        self.pump_ui();
    }

    pub(super) fn report_move(
        &mut self,
        window: HwndId,
        frame: PixelRect<Physical>,
        monitor: isize,
        observed_at: Instant,
    ) -> bool {
        let before = self.server.borrow().limits.len();
        self.observe(Observation::Settled {
            window,
            read: SettleRead {
                observed_at,
                minimized: false,
                is_maximized: false,
                placement: Some((frame, monitor)),
            },
        });
        self.server.borrow().limits.len() > before
    }

    pub(super) fn report_size_limits(&mut self, window: HwndId, limits: LimitObservation) {
        self.observe(Observation::SizeLimits { window, limits });
    }
    pub(super) fn start_drag(&mut self, window: HwndId) {
        self.server
            .borrow_mut()
            .native
            .push(NativeEvent::MoveStarted {
                window,
                observed_at: self.now,
            });
    }
    pub(super) fn finish_drag(&mut self, window: HwndId) {
        self.server
            .borrow_mut()
            .native
            .push(NativeEvent::MoveEnded {
                window,
                observed_at: self.now,
            });
    }
    pub(super) fn location_changed(&mut self, window: HwndId) -> bool {
        self.server.borrow_mut().native.push(NativeEvent::Moved {
            window,
            observed_at: self.now,
        });
        self.mocks.contains_key(&window)
    }
    pub(super) fn report_monitors(&mut self, monitors: Vec<MonitorInfo>) -> Vec<HwndId> {
        let before = self.server.borrow().limits.len();
        self.dome.monitors_changed(monitors);
        self.layout();
        self.server.borrow().limits[before..].to_vec()
    }
    pub(super) fn dpi_changed(&mut self) -> Vec<HwndId> {
        let monitors = self.monitors.lock().unwrap().clone();
        self.report_monitors(monitors)
    }
    pub(super) fn report_titles(&mut self, titles: Vec<(HwndId, Option<String>)>) {
        for (window, title) in titles {
            self.observe(Observation::Title { window, title });
        }
    }

    pub(super) fn tab_clicked(&mut self, container: ContainerId, index: usize) {
        self.dome.tab_clicked(container, index);
    }

    pub(super) fn window_id(&self, window: HwndId) -> Option<WindowId> {
        self.dome.window_id_for(window)
    }

    pub(super) fn is_managed(&self, window: HwndId) -> bool {
        self.dome.window_id_for(window).is_some()
    }

    pub(super) fn query_monitors_json(&self) -> String {
        self.dome.query_monitors_json()
    }

    pub(super) fn query_workspaces_json(&self) -> String {
        self.dome.query_workspaces_json()
    }

    pub(super) fn query_minimized_windows_json(&self) -> String {
        self.dome.query_minimized_windows_json()
    }

    pub(super) fn window(&mut self) -> WindowBuilder<'_> {
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

    pub(super) fn open(&mut self) -> HwndId {
        self.window().open()
    }

    pub(super) fn open_many(&mut self, count: usize) -> Vec<HwndId> {
        (0..count).map(|_| self.open()).collect()
    }

    pub(super) fn open_bar(&mut self) -> HwndId {
        self.window()
            .title("Zebar - vanilla")
            .process("zebar.exe")
            .class("Tauri Window")
            .app_name("Zebar")
            .spawned_at(dim(0, 0, 1920, 30))
            .open()
    }

    pub(super) fn settle(&mut self, limit: usize) {
        for _ in 0..limit {
            self.pump_ui();
            if !self.flush_moves() {
                return;
            }
        }
        let remaining = self.moves.len();
        if remaining > 0 {
            panic!("settle did not converge after {limit} iterations ({remaining} moves pending)");
        }
    }

    pub(super) fn flush_moves(&mut self) -> bool {
        if self.moves.is_empty() {
            return false;
        }
        let mut last_pos: HashMap<HwndId, Dimension> = HashMap::new();
        for (id, dim) in self.moves.take() {
            last_pos.insert(id, dim);
        }
        for (hwnd_id, dim) in last_pos {
            let minimized = self
                .mocks
                .get(&hwnd_id)
                .is_some_and(|m| m.minimized.load(Ordering::Relaxed));
            if minimized {
                // Win32 reports no move for an iconic window.
                continue;
            }
            let monitor = self.monitor_for_pos(dim.x, dim.y);
            self.observe(Observation::Settled {
                window: hwnd_id,
                read: SettleRead {
                    observed_at: self.now,
                    minimized: false,
                    is_maximized: false,
                    placement: Some((PixelRect::from_dimension(dim), monitor)),
                },
            });
        }
        self.layout();
        true
    }

    /// Configure a window to resist repositioning and report it at `pos`.
    pub(super) fn simulate_resist(&self, hwnd: HwndId, pos: (i32, i32, i32, i32)) {
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

    pub(super) fn clear_moves(&self) {
        self.moves.clear();
    }

    pub(super) fn assert_settled(&self, reason: &str) {
        assert!(self.moves.is_empty(), "{reason}");
    }

    pub(super) fn assert_placement_pending(&self, reason: &str) {
        assert!(!self.moves.is_empty(), "{reason}");
    }

    pub(super) fn moved(&self, hwnd: HwndId) -> bool {
        self.moves.contains(hwnd)
    }

    pub(super) fn destroy_window(&mut self, hwnd: HwndId) {
        self.mocks.remove(&hwnd);
        self.server.borrow_mut().windows.remove(&hwnd);
        self.observe(Observation::Gone(hwnd));
        self.z_stack.remove(hwnd);
        self.layout();
    }

    pub(super) fn minimize_window(&mut self, hwnd: HwndId) {
        self.mock(hwnd).minimized.store(true, Ordering::Relaxed);
        self.observe(Observation::Minimized(hwnd));
        self.layout();
    }

    pub(super) fn unminimize_window(&mut self, hwnd: HwndId) {
        // The flag clears before the move, which is the OS order of MINIMIZEEND
        // before LOCATIONCHANGE.
        self.mock(hwnd).minimized.store(false, Ordering::Relaxed);
        let dim = self.mock(hwnd).get_dim();
        self.moves.record(hwnd, dim);
        self.flush_moves();
    }

    pub(super) fn focus_window(&mut self, hwnd: HwndId) {
        self.observe(Observation::Foreground { window: hwnd });
        self.layout();
    }

    pub(super) fn dim(&self, hwnd: HwndId) -> Dimension {
        self.mock(hwnd).get_dim()
    }

    pub(super) fn move_window_to(&self, hwnd: HwndId, dim: Dimension) {
        *self.mock(hwnd).dimension.lock().unwrap() = dim;
        self.moves.record(hwnd, dim);
    }

    pub(super) fn is_minimized(&self, hwnd: HwndId) -> bool {
        self.mock(hwnd).minimized.load(Ordering::Relaxed)
    }

    pub(super) fn is_offscreen(&self, hwnd: HwndId) -> bool {
        self.mock(hwnd).is_offscreen()
    }

    pub(super) fn is_topmost(&self, hwnd: HwndId) -> bool {
        self.z_stack.is_topmost(hwnd)
    }

    /// True when `hwnd` sits below every displayed peer and below the tiling
    /// overlay. Win32 walks the z-order downward to pick the next focus when a
    /// window closes, so a parked window above the overlay would bring its
    /// workspace back.
    pub(super) fn is_bottom(&self, hwnd: HwndId) -> bool {
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

    pub(super) fn clear_override_position(&self, hwnd: HwndId) {
        self.mock(hwnd).set_override_position(None);
    }

    pub(super) fn run_actions(&mut self, s: &str) {
        let action: Action = s.parse().unwrap();
        self.send_action(&action);
    }

    pub(super) fn run_unminimize(&mut self, hwnd: HwndId) {
        let window_id = self
            .dome
            .window_id_for(hwnd)
            .expect("window is registered with the dome");
        self.send_action(&Action::UnminimizeWindow { id: window_id });
        self.restore_reported(hwnd);
    }

    pub(super) fn enter_exclusive_fullscreen(&mut self, hwnd: HwndId) {
        self.observe(Observation::ExclusiveFullscreen(Some(hwnd)));
        self.layout();
    }

    pub(super) fn focus_target(&self) -> FocusTarget {
        *self.focus_target.lock().unwrap()
    }

    pub(super) fn tiling_overlay_id(&self) -> HwndId {
        let tiling = self.tiling_overlay_ids();
        assert_eq!(tiling.len(), 1, "expected single tiling overlay");
        tiling[0]
    }

    pub(super) fn tiling_overlay_id_for(&self, monitor: MonitorId) -> Option<HwndId> {
        self.internal.borrow().tiling_overlay_for(monitor)
    }

    pub(super) fn float_overlay_id(&self, window: WindowId) -> Option<HwndId> {
        self.internal.borrow().float_overlay_for(window)
    }

    pub(super) fn float_overlay_ids(&self) -> Vec<HwndId> {
        self.internal.borrow().float_overlay_ids()
    }

    pub(super) fn painted_monitors(&self) -> usize {
        self.internal.borrow().monitors.len()
    }

    pub(super) fn painted_monitor_id(&self, index: usize) -> MonitorId {
        self.internal.borrow().monitors[index].monitor.id
    }

    pub(super) fn painted_windows(&self, index: usize) -> Vec<TilingWindowPlacement> {
        self.internal.borrow().monitors[index]
            .windows
            .iter()
            .map(|show| show.placement)
            .collect()
    }

    pub(super) fn painted_corner_radii(&self, index: usize) -> Vec<Length<Logical>> {
        self.internal.borrow().monitors[index]
            .windows
            .iter()
            .map(|show| show.corner_radius)
            .collect()
    }

    pub(super) fn painted_containers(&self, index: usize) -> Vec<ContainerPlacement> {
        self.internal.borrow().monitors[index]
            .monitor
            .containers
            .clone()
    }

    pub(super) fn painted_border_thickness(&self, index: usize) -> Pixels<Physical> {
        self.internal.borrow().monitors[index]
            .monitor
            .border_thickness
    }

    pub(super) fn painted_work_area(&self, index: usize) -> PixelRect {
        self.internal.borrow().monitors[index].monitor.work_area
    }

    /// The float placement the newest scene paints, for the one float these tests keep.
    pub(super) fn painted_float(&self) -> Option<FloatWindowPlacement> {
        self.internal.borrow().floats.first().map(|f| f.placement)
    }

    pub(super) fn painted_float_scale_and_thickness(&self) -> Option<(f32, Pixels<Physical>)> {
        self.internal
            .borrow()
            .floats
            .first()
            .map(|float| (float.scale, float.border_thickness))
    }

    pub(super) fn painted_float_corner_radius(&self) -> Option<Length<Logical>> {
        self.internal
            .borrow()
            .floats
            .first()
            .map(|f| f.corner_radius)
    }

    pub(super) fn set_corner_radius(&self, hwnd: HwndId, radius: Length<Logical>) {
        self.mock(hwnd).set_corner_radius(radius);
    }

    pub(super) fn only_painted_window(&self) -> TilingWindowPlacement {
        let windows = self.painted_windows(0);
        assert_eq!(windows.len(), 1, "these tests tile exactly one window");
        windows[0]
    }

    pub(super) fn border(&self) -> Length {
        Length::from_pixels(self.border_at(1.0))
    }

    pub(super) fn border_at(&self, scale: f32) -> Pixels<Physical> {
        Pixels::round(Length::from_pixels(self.tiling.border_size).to_unit(scale))
    }

    pub(super) fn outset(&self, content_box: Dimension) -> Dimension {
        self.outset_at(content_box, 1.0)
    }

    pub(super) fn outset_at(&self, content_box: Dimension, scale: f32) -> Dimension {
        PixelRect::from_dimension(content_box)
            .outset_by(self.border_at(scale))
            .to_dimension()
    }

    pub(super) fn clipped_outset(&self, content_box: Dimension) -> Dimension {
        PixelRect::from_dimension(self.outset(content_box))
            .clip(self.primary_monitor().work_area)
            .expect("the border box still overlaps the work area")
            .to_dimension()
    }

    pub(super) fn inset_at(&self, border_box: Dimension, scale: f32) -> Dimension {
        PixelRect::from_dimension(border_box)
            .inset_by(self.border_at(scale))
            .to_dimension()
    }

    pub(super) fn assert_horizontally_tiled(&self, windows: &[Dimension]) {
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

    pub(super) fn window_appearance(&self) -> Option<Appearance> {
        self.internal.borrow().appearance.clone()
    }

    pub(super) fn change_config(&mut self, source: &str) {
        self.config_file.rewrite(source);
        self.tiling = self
            .dome
            .reload()
            .expect("the test config should load")
            .tiling;
        self.layout();
    }

    pub(super) fn add_monitor(&mut self, monitor: MonitorInfo) {
        self.monitors.lock().unwrap().push(monitor);
        self.dome.handle_display_change();
        self.pump_ui();
        self.layout();
    }

    pub(super) fn remove_monitor(&mut self, handle: isize) {
        self.monitors.lock().unwrap().retain(|m| m.handle != handle);
        self.dome.handle_display_change();
        self.pump_ui();
        self.layout();
    }

    pub(super) fn fail_display_refresh(&mut self) {
        self.display_failed.store(true, Ordering::Relaxed);
        self.dome.handle_display_change();
        self.pump_ui();
        self.display_failed.store(false, Ordering::Relaxed);
    }

    pub(super) fn change_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        *self.monitors.lock().unwrap() = monitors;
        self.dome.handle_display_change();
        self.pump_ui();
        self.layout();
    }

    pub(super) fn set_monitor_scale(&self, handle: isize, scale: f32) {
        let mut monitors = self.monitors.lock().unwrap();
        let monitor = monitors
            .iter_mut()
            .find(|m| m.handle == handle)
            .unwrap_or_else(|| panic!("monitor {handle} is not in the test monitor set"));
        monitor.scale = scale;
        monitor.dots_per_inch = (scale * 96.0).round() as u32;
    }

    pub(super) fn raise_without_notifying_dome(&self, hwnd: HwndId) {
        self.z_stack.move_to_top(hwnd);
    }

    pub(super) fn z_order(&self) -> Vec<HwndId> {
        self.z_stack.stack()
    }

    pub(super) fn tiling_z_order(&self) -> Vec<HwndId> {
        self.z_stack.normal_stack()
    }

    fn open_with(&mut self, ext: Arc<MockExternalHwnd>) -> HwndId {
        let key = ext.hwnd_id;
        self.mocks.insert(key, ext.clone());
        self.server.borrow_mut().windows.insert(key, ext.clone());
        if ext.manageable {
            let frame = PixelRect::from_dimension(ext.get_dim());
            let monitor = self.monitor_for_pos(ext.get_dim().x, ext.get_dim().y);
            self.observe(Observation::Discovered(DiscoveredWindow {
                key,
                process_id: 1,
                is_maximized: false,
                metadata: WindowsMetadata {
                    title: ext.title.clone(),
                    process: ext.process.clone(),
                    process_path: None,
                    class: ext.class.clone(),
                    aumid: None,
                    app_name: ext.app_name.clone(),
                },
                frame,
                monitor,
                limits: Some(ext.constraints),
                fullscreen: None,
            }));
        }
        key
    }

    fn observe(&mut self, observation: Observation) {
        self.queue_observation(observation);
        self.pump_ui();
    }

    pub(super) fn queue_observation(&mut self, observation: Observation) {
        self.server.borrow_mut().observations.push(observation);
        self.wakes.borrow_mut().push_back(Wake::WindowServer);
    }

    fn pump_ui(&mut self) {
        for _ in 0..256 {
            let mut delivered = false;
            if !self.hold_reports {
                while let Ok(event) = self.reports.try_recv() {
                    assert!(self.dome.handle_report(event).is_ok());
                    delivered = true;
                }
            }
            let count = self.scene.borrow_mut().deliver(&self.ui_intake);
            if count > 0 {
                self.wakes.borrow_mut().push_back(Wake::Ui);
            }
            self.queue_due();
            let wake = self.wakes.borrow_mut().pop_front();
            if let Some(wake) = wake {
                let _ = self.handler.process_wake_at(wake, self.now);
            } else if !delivered {
                return;
            }
        }
        panic!("ready wake/report delivery did not converge");
    }

    fn queue_due(&mut self) {
        let due: Vec<_> = self
            .deadlines
            .borrow()
            .iter()
            .filter(|(_, at)| **at <= self.now)
            .map(|(&wake, _)| wake)
            .collect();
        for wake in due {
            self.deadlines.borrow_mut().remove(&wake);
            self.wakes.borrow_mut().push_back(wake);
        }
    }

    pub(super) fn deliver_ready_wakes(&mut self) {
        for _ in 0..256 {
            let wake = self.wakes.borrow_mut().pop_front();
            let Some(wake) = wake else { return };
            let _ = self.handler.process_wake_at(wake, self.now);
        }
        panic!("ready wakes did not converge");
    }

    pub(super) fn advance_time(&mut self, duration: Duration) {
        self.now += duration;
    }
    pub(super) fn now(&self) -> Instant {
        self.now
    }
    pub(super) fn deliver_due(&mut self) {
        self.queue_due();
        self.deliver_ready_wakes();
    }
    pub(super) fn deadline(&self, wake: Wake) -> Option<Instant> {
        self.deadlines.borrow().get(&wake).copied()
    }
    pub(super) fn ui_wake(&mut self) {
        self.wakes.borrow_mut().push_back(Wake::Ui);
        self.deliver_ready_wakes();
    }
    pub(super) fn latest_scene(&self) -> Scene {
        self.scene.borrow().latest.clone().unwrap()
    }
    pub(super) fn send_ui(&mut self, message: HubMessage) {
        self.ui_intake.send(message).unwrap();
        self.wakes.borrow_mut().push_back(Wake::Ui);
    }
    pub(super) fn hold_reports(&mut self, hold: bool) {
        self.hold_reports = hold;
    }
    pub(super) fn hold_scenes(&mut self, hold: bool) {
        self.scene.borrow_mut().hold_scenes = hold;
    }
    pub(super) fn release(&mut self) {
        self.pump_ui();
    }
    pub(super) fn take_reports(&mut self) -> Vec<HubEvent> {
        self.reports.try_iter().collect()
    }
    pub(super) fn focus_submissions(&self) -> Vec<HwndId> {
        self.server.borrow().focus.clone()
    }
    pub(super) fn close_submissions(&self) -> Vec<HwndId> {
        self.server.borrow().closed.clone()
    }
    pub(super) fn limits_demand(&self) -> Vec<HwndId> {
        self.server.borrow().limits.clone()
    }
    pub(super) fn displayed_monitors(&self) -> Vec<MonitorInfo> {
        self.server.borrow().displays.clone()
    }
    pub(super) fn recovery_records(&self) -> Vec<(HwndId, bool)> {
        self.server
            .borrow()
            .shutdown
            .iter()
            .map(|r| (r.window, r.is_maximized))
            .collect()
    }
    pub(super) fn recovery_calls(&self) -> usize {
        self.server.borrow().shutdown_calls
    }
    pub(super) fn stop(&mut self) {
        self.handler.stop();
    }
    pub(super) fn inject_observation(&mut self, observation: Observation) {
        self.observe(observation);
    }

    pub(super) fn trace(&self) -> Vec<&'static str> {
        self.server.borrow().trace.borrow().clone()
    }
    pub(super) fn clear_trace(&self) {
        self.server.borrow().trace.borrow_mut().clear();
    }
    pub(super) fn frame_submissions(&self) -> Vec<(HwndId, PixelRect<Physical>)> {
        self.server.borrow().frames.clone()
    }
    pub(super) fn park_submissions(&self) -> Vec<HwndId> {
        self.server.borrow().parked.clone()
    }
    pub(super) fn minimize_submissions(&self) -> Vec<HwndId> {
        self.server.borrow().minimized.clone()
    }
    pub(super) fn order_submissions(
        &self,
    ) -> Vec<(HwndId, crate::platform::windows::external::ZOrder)> {
        self.server.borrow().orders.clone()
    }
    pub(super) fn stacking_submissions(
        &self,
    ) -> Vec<(HwndId, crate::platform::windows::external::ZOrder)> {
        self.server.borrow().stacking.borrow().clone()
    }
    pub(super) fn tab_containers(&self, index: usize) -> Vec<ContainerPlacement> {
        self.internal.borrow().tab_monitors[index]
            .containers
            .clone()
    }
    pub(super) fn output_counts(&self) -> (usize, usize) {
        let internal = self.internal.borrow();
        (internal.paint_calls, internal.stack_calls)
    }
    pub(super) fn display_refreshes(&self) -> usize {
        self.server.borrow().display_refreshes
    }
    pub(super) fn deliver_report(&mut self, report: HubEvent) {
        assert!(self.dome.handle_report(report).is_ok());
        self.pump_ui();
    }
    fn primary_monitor(&self) -> MonitorInfo {
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

    fn wiring(&self) -> MockWiring {
        MockWiring {
            moves: self.moves.clone(),
            z_stack: self.z_stack.clone(),
            focus_target: self.focus_target.clone(),
        }
    }

    fn tiling_overlay_ids(&self) -> Vec<HwndId> {
        self.internal.borrow().tiling_overlay_ids()
    }

    fn send_action(&mut self, action: &Action) {
        // The harness runs no main thread, so an `exit` action has nothing to quit.
        let main_thread_id = 0;
        self.dome
            .handle_actions(&Actions::new(vec![action.clone()]), main_thread_id);
        self.pump_ui();
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
