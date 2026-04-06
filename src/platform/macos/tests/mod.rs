mod lifecycle;
mod placement;
mod transitions;
mod uncooperative;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use objc2_core_graphics::CGWindowID;

use crate::action::Actions;
use crate::config::Config;
use crate::core::Dimension;
use crate::platform::macos::MonitorInfo;
use crate::platform::macos::accessibility::AXWindowApi;
use crate::platform::macos::dispatcher::DispatcherMarker;
use crate::platform::macos::dome::{Dome, FrameSender, HubMessage, NewWindow, WindowMove};

const SCREEN_WIDTH: f32 = 1920.0;
const SCREEN_HEIGHT: f32 = 1080.0;

fn default_screen() -> MonitorInfo {
    MonitorInfo {
        display_id: 1,
        name: "Test".to_string(),
        dimension: Dimension {
            x: 0.0,
            y: 0.0,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        },
        full_height: SCREEN_HEIGHT,
        is_primary: true,
        scale: 2.0,
    }
}

type MoveLog = Rc<RefCell<Vec<(CGWindowID, i32, i32, i32, i32)>>>;

/// Mock AXWindow with shared state so clones given to Dome reflect
/// the same position/size when Dome calls set_frame.
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
    /// When set, `set_frame` and `hide_at` snap to this position/size instead
    /// of the requested one, simulating a window that resists placement.
    override_frame: OverrideFrame,
    /// Number of times `minimize()` was called on this window.
    minimize_count: Rc<Cell<u32>>,
    /// Number of times `unminimize()` was called on this window.
    unminimize_count: Rc<Cell<u32>>,
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
            minimize_count: Rc::new(Cell::new(0)),
            unminimize_count: Rc::new(Cell::new(0)),
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
}

// Marker params on read methods satisfy the trait contract. Tests never call
// these methods directly — they feed pre-built data to Dome instead.
impl AXWindowApi for MockAXWindow {
    fn cg_id(&self) -> CGWindowID {
        self.cg_id
    }
    fn pid(&self) -> i32 {
        self.pid
    }
    fn is_native_fullscreen(&self, _marker: &DispatcherMarker) -> bool {
        self.native_fullscreen.get()
    }
    fn get_position(&self, _marker: &DispatcherMarker) -> Result<(i32, i32)> {
        Ok(self.position.get())
    }
    fn get_size(&self, _marker: &DispatcherMarker) -> Result<(i32, i32)> {
        Ok(self.size.get())
    }
    fn set_frame(&self, x: i32, y: i32, w: i32, h: i32) -> Result<()> {
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
    fn hide_at(&self, x: i32, y: i32) -> Result<()> {
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
        self.minimize_count.set(self.minimize_count.get() + 1);
        Ok(())
    }
    fn unminimize(&self) -> Result<()> {
        self.unminimize_count.set(self.unminimize_count.get() + 1);
        Ok(())
    }
    fn is_valid(&self, _marker: &DispatcherMarker) -> bool {
        true
    }
    fn is_minimized(&self, _marker: &DispatcherMarker) -> bool {
        false
    }
    fn read_title(&self, _marker: &DispatcherMarker) -> Option<String> {
        Some(self.title.clone())
    }
}

/// Simulated macOS environment. Owns all AX windows — assertions go through here.
/// MockAXWindow uses Rc<Cell> so clones given to Dome share the same state.
struct MacOS {
    windows: HashMap<CGWindowID, MockAXWindow>,
    moves: MoveLog,
    next_cg_id: u32,
}

impl MacOS {
    fn new() -> Self {
        Self {
            windows: HashMap::new(),
            moves: Rc::new(RefCell::new(Vec::new())),
            next_cg_id: 1,
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
        x >= SCREEN_WIDTH as i32 - 1 || y >= SCREEN_HEIGHT as i32 - 1
    }

    /// Simulate macOS/app moving a window to a new position.
    fn move_window(&self, cg_id: CGWindowID, x: i32, y: i32, w: i32, h: i32) {
        let ax = self.window(cg_id);
        ax.position.set((x, y));
        ax.size.set((w, h));
    }

    fn enter_native_fullscreen(&self, cg_id: CGWindowID) {
        self.window(cg_id).set_native_fullscreen(true);
    }

    fn exit_native_fullscreen(&self, cg_id: CGWindowID) {
        self.window(cg_id).set_native_fullscreen(false);
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

    /// How many times `minimize()` was called on this window.
    fn minimize_count(&self, cg_id: CGWindowID) -> u32 {
        self.window(cg_id).minimize_count.get()
    }

    /// How many times `unminimize()` was called on this window.
    fn unminimize_count(&self, cg_id: CGWindowID) -> u32 {
        self.window(cg_id).unminimize_count.get()
    }

    /// Simulate an external move (app/macOS moved the window) and feed it to Dome.
    fn report_move(&self, dome: &mut Dome, cg_id: CGWindowID) {
        let observed_at = Instant::now() + std::time::Duration::from_secs(60);
        let ax = self.window(cg_id);
        let (x, y) = ax.position.get();
        let (w, h) = ax.size.get();
        dome.windows_moved(vec![WindowMove {
            cg_id,
            x,
            y,
            w,
            h,
            observed_at,
            is_native_fullscreen: ax.native_fullscreen.get(),
        }]);
    }

    /// Drain pending AX move events and feed them back to Dome,
    /// simulating macOS window moved/resized notifications.
    fn flush_moves(&self, dome: &mut Dome) {
        let pending = std::mem::take(&mut *self.moves.borrow_mut());
        if pending.is_empty() {
            return;
        }
        let observed_at = Instant::now() + std::time::Duration::from_secs(60);
        let moves: Vec<_> = pending
            .into_iter()
            .map(|(cg_id, x, y, w, h)| {
                let is_native_fullscreen = self
                    .windows
                    .get(&cg_id)
                    .map(|ax| ax.native_fullscreen.get())
                    .unwrap_or(false);
                WindowMove {
                    cg_id,
                    x,
                    y,
                    w,
                    h,
                    observed_at,
                    is_native_fullscreen,
                }
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
            // Use a timestamp far in the future so events are never considered stale.
            // In production, the debounce timer ensures observations are always recent.
            let observed_at = Instant::now() + std::time::Duration::from_secs(60);
            let moves: Vec<_> = pending
                .into_iter()
                .map(|(cg_id, x, y, w, h)| {
                    let is_native_fullscreen = self
                        .windows
                        .get(&cg_id)
                        .map(|ax| ax.native_fullscreen.get())
                        .unwrap_or(false);
                    WindowMove {
                        cg_id,
                        x,
                        y,
                        w,
                        h,
                        observed_at,
                        is_native_fullscreen,
                    }
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
        let sender = TestSender;
        Dome::new(&[default_screen()], config, Box::new(sender))
    }
}

struct TestSender;

impl FrameSender for TestSender {
    fn send(&self, _msg: HubMessage) {}
}

fn new_window(macos: &MacOS, cg_id: CGWindowID) -> NewWindow {
    let ax = macos.window(cg_id);
    let pos = ax.position.get();
    let size = ax.size.get();
    NewWindow {
        ax: Arc::new(ax.clone()),
        app_name: Some(ax.app_name.clone()),
        bundle_id: None,
        title: Some(ax.title.clone()),
        x: pos.0,
        y: pos.1,
        w: size.0,
        h: size.1,
        is_native_fullscreen: false,
    }
}

fn actions(s: &str) -> Actions {
    Actions::new(vec![s.parse().unwrap()])
}
