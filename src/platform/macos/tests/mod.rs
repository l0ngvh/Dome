mod lifecycle;
mod placement;
mod transitions;
mod uncooperative;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use anyhow::Result;
use objc2_core_graphics::CGWindowID;

use crate::action::Action;
use crate::config::Config;
use crate::core::{Dimension, Length, Logical, MonitorId, PixelRect, WindowId};
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::ExternalWindow;
use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::dome::{
    BarGeometry, DebounceBurst, Dome, ExitNativeFullscreen, HubMessage, MacOSMetadata, NewWindow,
    PendingAdd, SceneSender, WindowMove,
};

const SCREEN_WIDTH: Length = Length::new(1920.0);
const SCREEN_HEIGHT: Length = Length::new(1080.0);

/// A work area whose every edge sits on the side of the half-point boundary where
/// rounding to nearest would grow the rectangle past what AppKit reported, so
/// containment fails under any policy other than inward. Its consumers name that
/// policy themselves, since `MonitorInfo` now holds an already-snapped rect.
const FRACTIONAL_WORK_AREA: Dimension = Dimension::new(
    Length::new(10.4),
    Length::new(20.4),
    Length::new(1899.2),
    Length::new(1039.2),
);

fn assert_inside_work_area(frame: (i32, i32, i32, i32), reported: Dimension) {
    let (x, y, width, height) = frame;
    assert!(
        x as f32 >= reported.x.value() && y as f32 >= reported.y.value(),
        "origin ({x}, {y}) escaped the reported ({}, {})",
        reported.x.value(),
        reported.y.value()
    );
    assert!(
        (x + width) as f32 <= (reported.x + reported.width).value()
            && (y + height) as f32 <= (reported.y + reported.height).value(),
        "far edge ({}, {}) escaped the reported ({}, {})",
        x + width,
        y + height,
        (reported.x + reported.width).value(),
        (reported.y + reported.height).value()
    );
}

fn default_monitor() -> MonitorInfo {
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
        scale: 2.0,
    }
}

fn second_monitor() -> MonitorInfo {
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
        scale: 2.0,
    }
}

type MoveLog = Rc<RefCell<Vec<(CGWindowID, i32, i32, i32, i32)>>>;

type OverrideFrame = Rc<Cell<Option<(i32, i32, i32, i32)>>>;

#[derive(Clone)]
struct MockAXWindow {
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
    moves: MoveLog,
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
            moves,
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
    fn refresh_enhanced_ui(&self, _marker: &DispatcherMarker) {}
}

/// Simulated macOS environment. Owns all AX windows — assertions go through here.
/// MockAXWindow uses Rc<Cell> so clones given to Dome share the same state.
struct MacOS {
    windows: HashMap<CGWindowID, MockAXWindow>,
    moves: MoveLog,
    next_cg_id: u32,
    scene_state: Arc<Mutex<SceneState>>,
}

impl MacOS {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            moves: Rc::new(RefCell::new(Vec::new())),
            next_cg_id: 1,
            scene_state: Arc::new(Mutex::new(SceneState {
                focused_window: None,
                focused_monitor_id: None,
                floats: HashMap::new(),
            })),
        }
    }

    fn spawn_window(&mut self, pid: i32, app: &str, title: &str) -> CGWindowID {
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
    fn spawn_window_at(
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

    fn window_frame(&self, cg_id: CGWindowID) -> (i32, i32, i32, i32) {
        let ax = self.window(cg_id);
        let pos = ax.position.get();
        let size = ax.size.get();
        (pos.0, pos.1, size.0, size.1)
    }

    fn is_offscreen(&self, cg_id: CGWindowID) -> bool {
        let (x, y, _, _) = self.window_frame(cg_id);
        x >= SCREEN_WIDTH.value() as i32 - 1 || y >= SCREEN_HEIGHT.value() as i32 - 1
    }

    fn enter_native_fullscreen(&self, dome: &mut Dome, cg_id: CGWindowID) {
        self.window(cg_id).set_native_fullscreen(true);
        dome.reconcile_windows(&[], &[], &[], vec![], &[cg_id], &[]);
    }

    fn exit_native_fullscreen(
        &self,
        dome: &mut Dome,
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
        dome.reconcile_windows(
            &[],
            &[],
            &[],
            vec![],
            &[],
            &[ExitNativeFullscreen {
                cg_id,
                rect: PixelRect::new(x, y, w, h),
            }],
        );
    }

    fn set_min_size(&self, cg_id: CGWindowID, w: i32, h: i32) {
        self.window(cg_id).set_min_size(w, h);
    }

    fn set_max_size(&self, cg_id: CGWindowID, w: i32, h: i32) {
        self.window(cg_id).set_max_size(w, h);
    }

    /// Configure a window to resist `set_frame` and `hide_at` by snapping to a
    /// fixed position. Pass `None` to stop resisting.
    fn set_override_frame(&self, cg_id: CGWindowID, frame: Option<(i32, i32, i32, i32)>) {
        self.window(cg_id).override_frame.set(frame);
    }

    /// Simulate the user minimizing the window at OS level (yellow button,
    /// dock click, app's own minimize).
    fn user_minimize(&self, dome: &mut Dome, cg_id: CGWindowID) {
        self.window(cg_id).is_minimized.set(true);
        dome.reconcile_windows(&[], &[], &[cg_id], vec![], &[], &[]);
    }

    fn is_minimized(&self, cg_id: CGWindowID) -> bool {
        self.window(cg_id).is_minimized.get()
    }

    // Why Instant::now() works for these helpers:
    // observed_at.last must be >= placed_at for the stale check, and
    // observed_at.first must be <= placed_at + 1s for the constraint/drift check.
    // Both hold because settle sets placed_at immediately before the helper
    // runs, so Instant::now() is microseconds after placed_at -- the stale
    // check sees observed_at.last > placed_at and the constraint/drift check
    // sees observed_at.first well within 1s of placed_at.

    /// Simulate an external move (app/macOS moved the window) and feed it to Dome.
    ///
    /// Clears `is_minimized`: a window emitting a move event is, by definition,
    /// out of the dock.
    fn simulate_external_move(
        &self,
        dome: &mut Dome,
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
        dome.windows_moved(vec![WindowMove {
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
    fn flush_moves(&self, dome: &mut Dome) {
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
        dome.windows_moved(moves);
    }

    /// Run flush_moves in a loop until no new events are generated, or panic
    /// if it doesn't converge within `limit` iterations.
    fn settle(&self, dome: &mut Dome, limit: usize) {
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
            dome.windows_moved(moves);
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
    fn setup_dome(&self) -> Dome {
        self.setup_dome_with_config(Config::default())
    }

    fn setup_dome_with_config(&self, config: Config) -> Dome {
        let sender = TestSender {
            scene_state: self.scene_state.clone(),
        };
        Dome::new(
            &[default_monitor()],
            config.clone(),
            Vec::new(),
            Box::new(sender),
        )
    }

    fn last_scene_state(&self) -> SceneState {
        self.scene_state.lock().unwrap().clone()
    }

    fn last_float_snapshot(&self, cg_id: CGWindowID) -> Option<FloatSnapshot> {
        self.scene_state.lock().unwrap().floats.get(&cg_id).copied()
    }
}

#[derive(Clone, Copy, Debug)]
struct FloatSnapshot {
    outer_frame: Dimension,
    content_dim: Dimension,
}

#[derive(Clone)]
struct SceneState {
    focused_window: Option<WindowId>,
    focused_monitor_id: Option<MonitorId>,
    floats: HashMap<CGWindowID, FloatSnapshot>,
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
            state.floats = scene
                .float_shows
                .iter()
                .map(|show| {
                    (
                        show.cg_id,
                        FloatSnapshot {
                            outer_frame: show.placement.border_box.to_dimension(),
                            content_dim: show.content_dim,
                        },
                    )
                })
                .collect();
        }
    }
}

fn new_window(macos: &MacOS, cg_id: CGWindowID) -> PendingAdd {
    let ax = macos.window(cg_id);
    let pos = ax.position.get();
    let size = ax.size.get();
    PendingAdd::Positioned {
        new: NewWindow {
            ax: Arc::new(ax.clone()),
            metadata: MacOSMetadata {
                title: Some(ax.title.clone()),
                app_name: Some(ax.app_name.clone()),
                bundle_id: None,
            },
        },
        rect: PixelRect::new(pos.0, pos.1, size.0, size.1),
    }
}

fn start_drag(dome: &mut Dome, pid: i32) {
    dome.set_pid_moving(pid, true);
}

#[expect(
    clippy::too_many_arguments,
    reason = "test helper passing geometry directly"
)]
fn end_drag(
    dome: &mut Dome,
    macos: &MacOS,
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
    dome.set_pid_moving(pid, false);
    let observed_at = Instant::now();
    dome.windows_moved(vec![WindowMove {
        cg_id,
        rect: PixelRect::new(x, y, w, h),
        observed_at: DebounceBurst {
            first: observed_at,
            last: observed_at,
        },
    }]);
}

fn send(dome: &mut Dome, s: &str) {
    let action: Action = s.parse().unwrap();
    match &action {
        Action::Focus(t) => {
            dome.apply_focus(t);
            dome.flush_layout();
        }
        Action::Move(t) => {
            dome.apply_move(t);
            dome.flush_layout();
        }
        Action::Toggle(t) => {
            dome.apply_toggle(t);
            dome.flush_layout();
        }
        Action::Master(t) => {
            dome.apply_master(t);
            dome.flush_layout();
        }
        _ => panic!("send() only handles tiling actions, got: {action}"),
    }
}
