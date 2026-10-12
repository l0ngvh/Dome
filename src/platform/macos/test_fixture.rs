use crate::platform::keymap::KeymapPublisher;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;

use crate::action::{Action, Actions};
use crate::config::{LuaRuntime, PreferredLayouts};
use crate::core::{Dimension, Length, Logical, MonitorId, PixelRect, TilingConfig, WindowId};
use crate::platform::macos::MonitorInfo as PlatformMonitorInfo;
use crate::platform::macos::accessibility::ExternalWindow;
use crate::platform::macos::corner_radius::FALLBACK_CORNER_RADIUS;
use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::dome::{
    DebounceBurst, Dome, ExitNativeFullscreen, HubMessage, MacOSMetadata, NewWindow, PendingAdd,
    SceneSender, WindowMove,
};

pub(crate) const SCREEN_WIDTH: Length = Length::new(1920.0);
pub(crate) const SCREEN_HEIGHT: Length = Length::new(1080.0);

pub(crate) fn default_monitor() -> MonitorInfo {
    MonitorInfo {
        display_id: 1,
        name: "Test".to_string(),
        work_area: PixelRect::from_dimension_inward(Dimension::new(
            Length::ZERO,
            Length::ZERO,
            SCREEN_WIDTH,
            SCREEN_HEIGHT,
        )),
        bounds: Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT),
        full_height: SCREEN_HEIGHT.value(),
        is_primary: true,
        backing_scale: 2.0,
    }
}

pub(crate) fn second_monitor() -> MonitorInfo {
    MonitorInfo {
        display_id: 2,
        name: "External".to_string(),
        work_area: PixelRect::new(1920, 0, 2560, 1440),
        bounds: Dimension::new(
            Length::new(1920.0),
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        full_height: 1440.0,
        is_primary: false,
        backing_scale: 2.0,
    }
}

type MoveLog = Rc<RefCell<Vec<(CGWindowID, i32, i32, i32, i32)>>>;

type OverrideFrame = Rc<Cell<Option<(i32, i32, i32, i32)>>>;

#[derive(Clone)]
pub(crate) struct MockAXWindow {
    cg_id: CGWindowID,
    pid: i32,
    app_name: String,
    title: String,
    position: Rc<Cell<(i32, i32)>>,
    size: Rc<Cell<(i32, i32)>>,
    native_fullscreen: Rc<Cell<bool>>,
    min_size: Rc<Cell<Option<(i32, i32)>>>,
    max_size: Rc<Cell<Option<(i32, i32)>>>,
    override_frame: OverrideFrame,
    /// Whether this window is currently in the OS-level minimized state
    /// (in the dock).
    is_minimized: Rc<Cell<bool>>,
    is_valid: Rc<Cell<bool>>,
    corner_radius: Rc<Cell<Length<Logical>>>,
    moves: MoveLog,
    placements: Rc<Cell<usize>>,
}

// Safety: tests are single-threaded
unsafe impl Send for MockAXWindow {}
unsafe impl Sync for MockAXWindow {}

impl std::fmt::Display for MockAXWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}:{}] {}", self.pid, self.cg_id, self.app_name)
    }
}

impl MockAXWindow {
    pub(crate) fn placement_count(&self) -> usize {
        self.placements.get()
    }

    fn new(cg_id: CGWindowID, pid: i32, app_name: &str, title: &str, moves: MoveLog) -> Self {
        Self {
            cg_id,
            pid,
            app_name: app_name.to_owned(),
            title: title.to_owned(),
            position: Rc::new(Cell::new((0, 0))),
            size: Rc::new(Cell::new((800, 600))),
            native_fullscreen: Rc::new(Cell::new(false)),
            min_size: Rc::new(Cell::new(None)),
            max_size: Rc::new(Cell::new(None)),
            override_frame: Rc::new(Cell::new(None)),
            is_minimized: Rc::new(Cell::new(false)),
            is_valid: Rc::new(Cell::new(true)),
            corner_radius: Rc::new(Cell::new(FALLBACK_CORNER_RADIUS)),
            moves,
            placements: Rc::new(Cell::new(0)),
        }
    }

    fn set_native_fullscreen(&self, value: bool) {
        self.native_fullscreen.set(value);
    }

    fn set_min_size(&self, w: i32, h: i32) {
        self.min_size.set(Some((w, h)));
    }

    fn set_max_size(&self, w: i32, h: i32) {
        self.max_size.set(Some((w, h)));
    }

    fn set_valid(&self, valid: bool) {
        self.is_valid.set(valid);
    }
}

impl ExternalWindow for MockAXWindow {
    fn cg_id(&self) -> CGWindowID {
        self.cg_id
    }
    fn pid(&self) -> i32 {
        self.pid
    }
    fn is_native_fullscreen(&self, _marker: &DispatcherMarker) -> bool {
        self.native_fullscreen.get()
    }
    fn get_position(
        &self,
        _marker: &DispatcherMarker,
    ) -> Result<(Length<Logical>, Length<Logical>)> {
        let (x, y) = self.position.get();
        Ok((Length::new(x as f32), Length::new(y as f32)))
    }
    fn get_size(&self, _marker: &DispatcherMarker) -> Result<(Length<Logical>, Length<Logical>)> {
        let (w, h) = self.size.get();
        Ok((Length::new(w as f32), Length::new(h as f32)))
    }
    fn set_frame(&self, rect: PixelRect<Logical>) -> Result<()> {
        self.placements.set(self.placements.get() + 1);
        let x = rect.x().value();
        let y = rect.y().value();
        let w = rect.width().value();
        let h = rect.height().value();
        let (x, y, w, h) = if let Some(ovr) = self.override_frame.get() {
            ovr
        } else {
            let mut w = w;
            let mut h = h;
            if let Some((min_w, min_h)) = self.min_size.get() {
                w = w.max(min_w);
                h = h.max(min_h);
            }
            if let Some((max_w, max_h)) = self.max_size.get() {
                w = w.min(max_w);
                h = h.min(max_h);
            }
            (x, y, w, h)
        };
        self.position.set((x, y));
        self.size.set((w, h));
        self.moves.borrow_mut().push((self.cg_id, x, y, w, h));
        Ok(())
    }
    fn focus(&self) -> Result<()> {
        Ok(())
    }
    fn hide_at(&self, x: Length<Logical>, y: Length<Logical>) -> Result<()> {
        self.placements.set(self.placements.get() + 1);
        let x = x.value() as i32;
        let y = y.value() as i32;
        let (x, y, w, h) = if let Some(ovr) = self.override_frame.get() {
            ovr
        } else {
            let (w, h) = self.size.get();
            (x, y, w, h)
        };
        self.position.set((x, y));
        self.size.set((w, h));
        self.moves.borrow_mut().push((self.cg_id, x, y, w, h));
        Ok(())
    }
    fn minimize(&self) -> Result<()> {
        self.is_minimized.set(true);
        Ok(())
    }
    fn unminimize(&self) -> Result<()> {
        self.is_minimized.set(false);
        Ok(())
    }
    fn close(&self) -> Result<()> {
        Ok(())
    }
    fn is_valid(&self, _marker: &DispatcherMarker) -> bool {
        self.is_valid.get()
    }
    fn is_minimized(&self, _marker: &DispatcherMarker) -> bool {
        self.is_minimized.get()
    }
    fn read_title(&self, _marker: &DispatcherMarker) -> Option<String> {
        Some(self.title.clone())
    }
    fn read_close_button_frame(&self, _marker: &DispatcherMarker) -> Option<Dimension<Logical>> {
        None
    }
    fn refresh_enhanced_ui(&self, _marker: &DispatcherMarker) {}
}

/// Simulated macOS environment. Owns all AX windows.
/// MockAXWindow uses Rc<Cell> so clones given to Dome share the same state.
pub(crate) struct MacOSFixture {
    windows: HashMap<CGWindowID, MockAXWindow>,
    moves: MoveLog,
    next_cg_id: u32,
    scene_state: Arc<Mutex<SceneState>>,
    tiling: TilingConfig,
}

impl MacOSFixture {
    pub(crate) fn moves(&self) -> Vec<(CGWindowID, i32, i32, i32, i32)> {
        self.moves.borrow().clone()
    }

    pub(crate) fn clear_moves(&self) {
        self.moves.borrow_mut().clear();
    }

    pub(crate) fn set_window_frame(&self, id: CGWindowID, frame: (i32, i32, i32, i32)) {
        self.window(id).position.set((frame.0, frame.1));
        self.window(id).size.set((frame.2, frame.3));
    }

    pub(crate) fn set_corner_radius(&self, id: CGWindowID, radius: Length<Logical>) {
        self.window(id).corner_radius.set(radius);
    }

    pub(crate) fn set_valid(&self, id: CGWindowID, valid: bool) {
        self.window(id).set_valid(valid);
    }

    pub(crate) fn placement_count(&self, id: CGWindowID) -> usize {
        self.window(id).placement_count()
    }

    pub(crate) fn refresh_window(&mut self, dome: &mut MacOSDome, id: CGWindowID) -> MockAXWindow {
        let old = self.window(id).clone();
        old.set_valid(false);
        let fresh = MockAXWindow::new(id, old.pid, &old.app_name, &old.title, self.moves.clone());
        fresh.position.set(old.position.get());
        fresh.size.set(old.size.get());
        fresh.corner_radius.set(old.corner_radius.get());
        dome.0.reconcile_windows(
            &[crate::platform::macos::dome::ExtRefresh {
                cg_id: id,
                ext: Arc::new(fresh.clone()),
                corner_radius: fresh.corner_radius.get(),
            }],
            &[],
            &[],
            vec![],
            &[],
            &[],
        );
        self.windows.insert(id, fresh);
        old
    }

    pub(crate) fn new(tiling: TilingConfig) -> Self {
        Self {
            windows: HashMap::new(),
            moves: Rc::new(RefCell::new(Vec::new())),
            next_cg_id: 1,
            scene_state: Arc::new(Mutex::new(SceneState {
                focused_window: None,
                focused_monitor_id: None,
                floats: HashMap::new(),
                mirrors: HashMap::new(),
                tiling_corner_radii: HashMap::new(),
                monitor_dims: Vec::new(),
            })),
            tiling,
        }
    }

    pub(crate) fn spawn_window(&mut self, pid: i32, app: &str, title: &str) -> CGWindowID {
        let cg_id = self.next_cg_id;
        self.next_cg_id += 1;
        self.windows.insert(
            cg_id,
            MockAXWindow::new(cg_id, pid, app, title, self.moves.clone()),
        );
        cg_id
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "test helper passing geometry directly"
    )]
    pub(crate) fn spawn_window_at(
        &mut self,
        pid: i32,
        app: &str,
        title: &str,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) -> CGWindowID {
        let cg_id = self.spawn_window(pid, app, title);
        let ax = self.window(cg_id);
        ax.position.set((x, y));
        ax.size.set((w, h));
        cg_id
    }

    fn window(&self, cg_id: CGWindowID) -> &MockAXWindow {
        &self.windows[&cg_id]
    }

    pub(crate) fn window_frame(&self, cg_id: CGWindowID) -> (i32, i32, i32, i32) {
        let ax = self.window(cg_id);
        let pos = ax.position.get();
        let size = ax.size.get();
        (pos.0, pos.1, size.0, size.1)
    }

    pub(crate) fn is_offscreen(&self, cg_id: CGWindowID) -> bool {
        let (x, y, _, _) = self.window_frame(cg_id);
        x >= SCREEN_WIDTH.value() as i32 - 1 || y >= SCREEN_HEIGHT.value() as i32 - 1
    }

    pub(crate) fn enter_native_fullscreen(&self, dome: &mut MacOSDome, cg_id: CGWindowID) {
        self.window(cg_id).set_native_fullscreen(true);
        dome.0
            .reconcile_windows(&[], &[], &[], vec![], &[cg_id], &[]);
    }

    pub(crate) fn exit_native_fullscreen(
        &self,
        dome: &mut MacOSDome,
        cg_id: CGWindowID,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let ax = self.window(cg_id);
        ax.position.set((x, y));
        ax.size.set((w, h));
        ax.set_native_fullscreen(false);
        dome.0.reconcile_windows(
            &[],
            &[],
            &[],
            vec![],
            &[],
            &[ExitNativeFullscreen {
                cg_id,
                rect: PixelRect::new(x, y, w, h),
                corner_radius: ax.corner_radius.get(),
            }],
        );
    }

    pub(crate) fn set_min_size(&self, cg_id: CGWindowID, w: i32, h: i32) {
        self.window(cg_id).set_min_size(w, h);
    }

    pub(crate) fn set_max_size(&self, cg_id: CGWindowID, w: i32, h: i32) {
        self.window(cg_id).set_max_size(w, h);
    }

    /// Configure a window to resist `set_frame` and `hide_at` by snapping to a
    /// fixed position. Pass `None` to stop resisting.
    pub(crate) fn set_override_frame(
        &self,
        cg_id: CGWindowID,
        frame: Option<(i32, i32, i32, i32)>,
    ) {
        self.window(cg_id).override_frame.set(frame);
    }

    /// Simulate the user minimizing the window at OS level (yellow button,
    /// dock click, app's own minimize).
    pub(crate) fn user_minimize(&self, dome: &mut MacOSDome, cg_id: CGWindowID) {
        self.window(cg_id).is_minimized.set(true);
        dome.0
            .reconcile_windows(&[], &[], &[cg_id], vec![], &[], &[]);
    }

    pub(crate) fn is_minimized(&self, cg_id: CGWindowID) -> bool {
        self.window(cg_id).is_minimized.get()
    }

    /// Replay the deminiaturize notification, which macOS routes as a move at
    /// the window's current frame.
    pub(crate) fn deminiaturize(&self, dome: &mut MacOSDome, cg_id: CGWindowID) {
        let (x, y, w, h) = self.window_frame(cg_id);
        self.simulate_external_move(dome, cg_id, x, y, w, h);
    }

    /// Simulate the OS reporting a window focused. Dome answers a minimized one
    /// with a restore, which macOS follows with a deminiaturize, so replay that.
    pub(crate) fn focus_window(&self, dome: &mut MacOSDome, cg_id: CGWindowID) {
        let was_minimized = self.is_minimized(cg_id);
        dome.0.focus_window_by_cg(cg_id);
        if was_minimized {
            self.deminiaturize(dome, cg_id);
        }
    }

    pub(crate) fn run_unminimize(&self, dome: &mut MacOSDome, cg_id: CGWindowID) {
        let id = dome
            .0
            .tracked_window(cg_id)
            .expect("window is tracked by the dome")
            .window_id;
        send_action(dome, &Action::UnminimizeWindow { id });
        // macOS posts the deminiaturize notification only for a window that
        // actually left the Dock.
        if !self.is_minimized(cg_id) {
            self.deminiaturize(dome, cg_id);
        }
    }

    // These helpers require observed_at.last >= placed_at to avoid stale events.
    // Tests exercising constraint/drift handling also assume
    // observed_at.first <= placed_at + 1s. The helpers use Instant::now() rather
    // than a controlled clock, so scheduling delays can violate that assumption.

    /// Simulate an external move (app/macOS moved the window) and feed it to Dome.
    ///
    /// Clears `is_minimized`. A window emitting a move event is, by definition,
    /// out of the dock.
    pub(crate) fn simulate_external_move(
        &self,
        dome: &mut MacOSDome,
        cg_id: CGWindowID,
        x: i32,
        y: i32,
        w: i32,
        h: i32,
    ) {
        let observed_at = Instant::now();
        let ax = self.window(cg_id);
        ax.position.set((x, y));
        ax.size.set((w, h));
        ax.is_minimized.set(false);
        dome.0.update_corner_radius(cg_id, ax.corner_radius.get());
        dome.0.windows_moved(vec![WindowMove {
            cg_id,
            rect: PixelRect::new(x, y, w, h),
            observed_at: DebounceBurst {
                first: observed_at,
                last: observed_at,
            },
        }]);
    }

    /// Drain pending AX move events and feed them back to Dome,
    /// simulating macOS window moved/resized notifications.
    pub(crate) fn flush_moves(&self, dome: &mut MacOSDome) {
        let pending = std::mem::take(&mut *self.moves.borrow_mut());
        if pending.is_empty() {
            return;
        }
        let observed_at = Instant::now();
        let moves: Vec<_> = pending
            .into_iter()
            .map(|(cg_id, x, y, w, h)| WindowMove {
                cg_id,
                rect: PixelRect::new(x, y, w, h),
                observed_at: DebounceBurst {
                    first: observed_at,
                    last: observed_at,
                },
            })
            .collect();
        dome.0.windows_moved(moves);
    }

    /// Run flush_moves in a loop until no new events are generated, or panic
    /// if it doesn't converge within `limit` iterations.
    pub(crate) fn settle(&self, dome: &mut MacOSDome, limit: usize) {
        for i in 0..limit {
            let pending = std::mem::take(&mut *self.moves.borrow_mut());
            if pending.is_empty() {
                return;
            }
            let observed_at = Instant::now();
            let moves: Vec<_> = pending
                .into_iter()
                .map(|(cg_id, x, y, w, h)| WindowMove {
                    cg_id,
                    rect: PixelRect::new(x, y, w, h),
                    observed_at: DebounceBurst {
                        first: observed_at,
                        last: observed_at,
                    },
                })
                .collect();
            dome.0.windows_moved(moves);
            if i == limit - 1 {
                let remaining = self.moves.borrow().len();
                if remaining > 0 {
                    panic!(
                        "settle did not converge after {limit} iterations ({remaining} moves pending)"
                    );
                }
            }
        }
    }

    pub(crate) fn build_dome(
        &mut self,
        tiling: TilingConfig,
        runtime: LuaRuntime,
        keymap: KeymapPublisher,
    ) -> MacOSDome {
        let dome = Dome::new(
            &[default_monitor().into_platform()],
            tiling.clone(),
            PreferredLayouts::default(),
            Box::new(TestSender {
                scene_state: self.scene_state.clone(),
            }),
            runtime,
            keymap,
            HashMap::new(),
        );
        self.tiling = tiling;
        MacOSDome(dome)
    }

    pub(crate) fn tiling(&self) -> TilingConfig {
        self.tiling.clone()
    }

    pub(crate) fn border(&self) -> f32 {
        Length::from_pixels(self.tiling.border_size).logical()
    }

    pub(crate) fn reload(&mut self, dome: &mut MacOSDome) {
        self.tiling = dome.0.reload().expect("the test config should load").tiling;
    }

    pub(crate) fn last_scene_state(&self) -> SceneState {
        self.scene_state.lock().unwrap().clone()
    }

    /// The tiling overlay rect of the monitor at `index` in the newest scene.
    pub(crate) fn painted_work_area(&self, index: usize) -> Dimension {
        self.scene_state.lock().unwrap().monitor_dims[index]
    }

    pub(crate) fn last_float_snapshot(&self, cg_id: CGWindowID) -> Option<FloatSnapshot> {
        self.scene_state.lock().unwrap().floats.get(&cg_id).copied()
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct FloatSnapshot {
    pub(crate) outer_frame: Dimension,
    pub(crate) content_dim: Dimension,
    pub(crate) corner_radius: Length<Logical>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct MirrorSnapshot {
    pub(crate) cocoa_frame: NSRect,
    /// The captured part of the window, relative to the window's top-left corner.
    pub(crate) source: Dimension,
    pub(crate) scale: f64,
}

#[derive(Clone)]
pub(crate) struct SceneState {
    pub(crate) focused_window: Option<WindowId>,
    pub(crate) focused_monitor_id: Option<MonitorId>,
    pub(crate) floats: HashMap<CGWindowID, FloatSnapshot>,
    pub(crate) mirrors: HashMap<CGWindowID, MirrorSnapshot>,
    pub(crate) tiling_corner_radii: HashMap<WindowId, Length<Logical>>,
    /// Each tiling overlay's `monitor_dim`, in scene order.
    pub(crate) monitor_dims: Vec<Dimension>,
}

struct TestSender {
    scene_state: Arc<Mutex<SceneState>>,
}

impl SceneSender for TestSender {
    fn send(&self, msg: HubMessage) {
        if let HubMessage::Scene(scene) = &msg {
            let mut state = self.scene_state.lock().unwrap();
            state.focused_window = scene.focused_window;
            state.focused_monitor_id = Some(scene.focused_monitor_id);
            state.tiling_corner_radii = scene
                .tiling
                .iter()
                .flat_map(|monitor| &monitor.windows)
                .map(|show| (show.placement.id, show.corner_radius))
                .collect();
            state.floats = scene
                .float_shows
                .iter()
                .map(|show| {
                    (
                        show.cg_id,
                        FloatSnapshot {
                            outer_frame: show.placement.border_box.to_dimension(),
                            content_dim: show.content_dim,
                            corner_radius: show.corner_radius,
                        },
                    )
                })
                .collect();
            state.mirrors = scene
                .mirror_shows
                .iter()
                .map(|show| {
                    (
                        show.cg_id,
                        MirrorSnapshot {
                            cocoa_frame: show.cocoa_frame,
                            source: show.source,
                            scale: show.scale,
                        },
                    )
                })
                .collect();
            state.monitor_dims = scene.tiling.iter().map(|t| t.monitor_dim).collect();
        }
    }
}

fn new_window(macos: &MacOSFixture, cg_id: CGWindowID, bundle_id: Option<&str>) -> PendingAdd {
    let ax = macos.window(cg_id);
    let pos = ax.position.get();
    let size = ax.size.get();
    PendingAdd::Positioned {
        new: NewWindow {
            ax: Arc::new(ax.clone()),
            metadata: MacOSMetadata {
                title: Some(ax.title.clone()),
                app_name: Some(ax.app_name.clone()),
                bundle_id: bundle_id.map(str::to_owned),
            },
        },
        rect: PixelRect::new(pos.0, pos.1, size.0, size.1),
        corner_radius: ax.corner_radius.get(),
    }
}

pub(crate) fn start_drag(dome: &mut MacOSDome, pid: i32) {
    dome.0.set_pid_moving(pid, true);
}

#[expect(
    clippy::too_many_arguments,
    reason = "test helper passing geometry directly"
)]
pub(crate) fn end_drag(
    dome: &mut MacOSDome,
    macos: &MacOSFixture,
    pid: i32,
    cg_id: CGWindowID,
    x: i32,
    y: i32,
    w: i32,
    h: i32,
) {
    let ax = macos.window(cg_id);
    ax.position.set((x, y));
    ax.size.set((w, h));
    dome.0.set_pid_moving(pid, false);
    let observed_at = Instant::now();
    dome.0.windows_moved(vec![WindowMove {
        cg_id,
        rect: PixelRect::new(x, y, w, h),
        observed_at: DebounceBurst {
            first: observed_at,
            last: observed_at,
        },
    }]);
}

pub(crate) fn send_action(dome: &mut MacOSDome, action: &Action) {
    let event_loop = calloop::EventLoop::<()>::try_new().expect("the test event loop should build");
    dome.0.handle_actions(
        &Actions::new(vec![action.clone()]),
        &event_loop.get_signal(),
    );
}

pub(crate) fn send(dome: &mut MacOSDome, s: &str) {
    let action: Action = s.parse().unwrap();
    send_action(dome, &action);
}

#[derive(Clone)]
pub(crate) struct MonitorInfo {
    pub(crate) display_id: u32,
    pub(crate) name: String,
    pub(crate) work_area: PixelRect,
    pub(crate) bounds: Dimension,
    pub(crate) full_height: f32,
    pub(crate) is_primary: bool,
    pub(crate) backing_scale: f64,
}

impl MonitorInfo {
    pub(in crate::platform::macos) fn into_platform(self) -> PlatformMonitorInfo {
        PlatformMonitorInfo {
            display_id: self.display_id,
            name: self.name,
            work_area: self.work_area,
            bounds: self.bounds,
            full_height: self.full_height,
            is_primary: self.is_primary,
            backing_scale: self.backing_scale,
        }
    }
}

pub(crate) struct MacOSDome(Dome);

impl MacOSDome {
    pub(crate) fn minimized(&mut self, windows: &[CGWindowID]) {
        self.0
            .reconcile_windows(&[], &[], windows, vec![], &[], &[]);
    }

    pub(crate) fn add_windows(&mut self, macos: &MacOSFixture, windows: &[CGWindowID]) {
        let added = windows
            .iter()
            .map(|&id| new_window(macos, id, None))
            .collect();
        self.0.reconcile_windows(&[], &[], &[], added, &[], &[]);
    }

    pub(crate) fn add_windows_with_bundle_id(
        &mut self,
        macos: &MacOSFixture,
        windows: &[CGWindowID],
        bundle_id: &str,
    ) {
        let added = windows
            .iter()
            .map(|&id| new_window(macos, id, Some(bundle_id)))
            .collect();
        self.0.reconcile_windows(&[], &[], &[], added, &[], &[]);
    }

    pub(crate) fn add_native_fullscreen(&mut self, macos: &MacOSFixture, id: CGWindowID) {
        macos.window(id).set_native_fullscreen(true);
        let PendingAdd::Positioned { new, .. } = new_window(macos, id, None) else {
            unreachable!()
        };
        self.0.reconcile_windows(
            &[],
            &[],
            &[],
            vec![PendingAdd::NativeFullscreen { new }],
            &[],
            &[],
        );
    }

    pub(crate) fn remove_windows(&mut self, windows: &[CGWindowID]) {
        self.0
            .reconcile_windows(&[], windows, &[], vec![], &[], &[]);
    }

    pub(crate) fn window_moved(
        &mut self,
        id: CGWindowID,
        rect: PixelRect,
        first: Instant,
        last: Instant,
    ) {
        self.0.windows_moved(vec![WindowMove {
            cg_id: id,
            rect,
            observed_at: DebounceBurst { first, last },
        }]);
    }

    pub(crate) fn window_id_for(&self, id: CGWindowID) -> WindowId {
        self.0
            .tracked_window(id)
            .expect("the window has been admitted")
            .window_id
    }

    pub(crate) fn monitors_changed(&mut self, monitors: Vec<MonitorInfo>) {
        self.0.monitors_changed(
            monitors
                .into_iter()
                .map(MonitorInfo::into_platform)
                .collect(),
        );
    }

    pub(crate) fn app_terminated(&mut self, pid: i32) {
        self.0.app_terminated(pid);
    }
    pub(crate) fn finish_monitor_settle(&mut self) {
        self.0.finish_monitor_settle();
    }
    pub(crate) fn flush_layout(&mut self) {
        self.0.flush_layout();
    }
    pub(crate) fn mirror_clicked(&mut self, id: CGWindowID) {
        self.0.mirror_clicked(id);
    }
    pub(crate) fn apply_layout_file(&mut self, path: &str) {
        self.0.apply_layout_file(path);
    }
    pub(crate) fn query_monitors_json(&self) -> String {
        self.0.query_monitors_json()
    }
    pub(crate) fn query_workspaces_json(&self) -> String {
        self.0.query_workspaces_json()
    }
}
