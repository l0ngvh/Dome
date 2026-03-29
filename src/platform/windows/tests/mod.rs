use std::sync::{Arc, Mutex};

use crate::action::Actions;
use crate::config::Config;
use crate::core::Dimension;
use crate::platform::windows::OFFSCREEN_POS;
use crate::platform::windows::ScreenInfo;
use crate::platform::windows::dome::{Dome, FrameSender, LayoutFrame, TitleUpdate};
use crate::platform::windows::external::{HwndId, ManageExternalHwnd, ShowCmd, ZOrder};
use crate::platform::windows::handle::WindowMode;
use crate::platform::windows::taskbar::Taskbar;
use crate::platform::windows::wm::Wm;

const SCREEN_WIDTH: f32 = 1920.0;
const SCREEN_HEIGHT: f32 = 1080.0;

fn default_screen() -> ScreenInfo {
    ScreenInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension {
            x: 0.0,
            y: 0.0,
            width: SCREEN_WIDTH,
            height: SCREEN_HEIGHT,
        },
        is_primary: true,
    }
}

/// Test double for `FrameSender` — captures layout frames into a shared Vec
/// so `TestEnv::settle` can drain and feed them to Wm.
struct TestSender {
    frames: Arc<Mutex<Vec<LayoutFrame>>>,
}

impl FrameSender for TestSender {
    fn send_frame(&self, frame: LayoutFrame) {
        self.frames.lock().unwrap().push(frame);
    }
    fn send_titles(&self, _update: TitleUpdate) {}
    fn send_config(&self, _config: Config) {}
}

/// Test double for external windows. Positioning methods update an internal
/// `Mutex<Dimension>` that tests can inspect to verify final placement.
struct MockExternalHwnd {
    hwnd_id: HwndId,
    title: Option<String>,
    process: String,
    dimension: Mutex<Dimension>,
    monitor_handle: isize,
}

impl MockExternalHwnd {
    fn with_title(id: isize, title: &str, process: &str) -> Self {
        Self {
            hwnd_id: HwndId::test(id),
            title: Some(title.to_string()),
            process: process.to_string(),
            dimension: Mutex::new(Dimension {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }),
            monitor_handle: 1,
        }
    }

    fn get_dim(&self) -> Dimension {
        *self.dimension.lock().unwrap()
    }

    fn is_offscreen(&self) -> bool {
        let dim = self.get_dim();
        dim.x <= OFFSCREEN_POS || dim.y <= OFFSCREEN_POS
    }
}

impl ManageExternalHwnd for MockExternalHwnd {
    fn id(&self) -> HwndId {
        self.hwnd_id
    }

    fn is_manageable(&self) -> bool {
        true
    }

    fn get_window_title(&self) -> Option<String> {
        self.title.clone()
    }

    fn get_process_name(&self) -> anyhow::Result<String> {
        Ok(self.process.clone())
    }

    fn initial_window_mode(&self, _monitor: Option<&Dimension>) -> WindowMode {
        WindowMode::Tiling
    }

    fn get_dimension(&self) -> Dimension {
        self.get_dim()
    }

    fn get_size_constraints(&self) -> (f32, f32, f32, f32) {
        (0.0, 0.0, 0.0, 0.0)
    }

    fn get_monitor_handle(&self) -> Option<isize> {
        Some(self.monitor_handle)
    }

    fn is_iconic(&self) -> bool {
        false
    }

    fn set_position(&self, _z: ZOrder, x: i32, y: i32, cx: i32, cy: i32) {
        *self.dimension.lock().unwrap() = Dimension {
            x: x as f32,
            y: y as f32,
            width: cx as f32,
            height: cy as f32,
        };
    }

    fn move_offscreen(&self) {
        let mut dim = self.dimension.lock().unwrap();
        dim.x = OFFSCREEN_POS;
        dim.y = OFFSCREEN_POS;
    }

    fn show_cmd(&self, _cmd: ShowCmd) {}
    fn set_foreground_window(&self) {}
    fn add_to_taskbar(&self, _taskbar: &Taskbar) {}
    fn remove_from_taskbar(&self, _taskbar: &Taskbar) {}

    fn is_maximized(&self) -> bool {
        false
    }

    fn recover(&self, dim: Dimension, _was_maximized: bool) {
        *self.dimension.lock().unwrap() = dim;
    }
}

/// Integration test environment that wires Dome and Wm together via a
/// `TestSender`. Call Dome methods, then `settle()` to flush captured
/// frames through Wm — which positions `MockExternalHwnd` windows that
/// tests can inspect.
struct TestEnv {
    dome: Dome,
    wm: Wm,
    frames: Arc<Mutex<Vec<LayoutFrame>>>,
}

impl TestEnv {
    fn new() -> Self {
        let config = Config::default();
        let screen = default_screen();
        let frames: Arc<Mutex<Vec<LayoutFrame>>> = Arc::new(Mutex::new(Vec::new()));
        let sender = Box::new(TestSender {
            frames: frames.clone(),
        });
        let dome = Dome::new(config.clone(), vec![screen], Some(sender));
        let wm = Wm::new_for_test(config);
        Self { dome, wm, frames }
    }

    /// Drain captured frames from the test sender and feed them to Wm,
    /// which positions the mock windows.
    fn settle(&mut self) {
        for frame in self.frames.lock().unwrap().drain(..) {
            self.wm.apply_layout_frame(frame);
        }
    }

    fn add_window(&mut self, ext: Arc<dyn ManageExternalHwnd>) {
        self.dome.window_created(ext);
        self.settle();
    }

    fn run_actions(&mut self, s: &str) {
        let actions = Actions::new(vec![s.parse().unwrap()]);
        self.dome.run_hub_actions(&actions);
        self.settle();
    }
}

/// Assert that windows tile horizontally across the screen. Checks that the
/// first window starts at the left border, the last ends at the right border,
/// gaps between adjacent windows equal `2 * border`, and all share the same
/// vertical extent.
fn assert_h_tiled(dims: &[Dimension], screen: Dimension, border: f32) {
    assert!(!dims.is_empty());
    for (i, d) in dims.iter().enumerate() {
        assert_eq!(d.y, border, "window {i} y");
        assert_eq!(d.height, screen.height - 2.0 * border, "window {i} height");
        assert!(d.width > 0.0, "window {i} width");
    }
    assert_eq!(dims[0].x, border, "first window x");
    let last = dims.last().unwrap();
    assert!(
        (last.x + last.width - (screen.width - border)).abs() < 1.0,
        "last window right edge"
    );
    for i in 1..dims.len() {
        let gap = dims[i].x - (dims[i - 1].x + dims[i - 1].width);
        assert!(
            (gap - 2.0 * border).abs() < 1.0,
            "gap between window {} and {}",
            i - 1,
            i
        );
    }
}

#[test]
fn single_window_fills_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());
    assert_h_tiled(
        &[w1.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn two_windows_split_screen() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    let w2 = Arc::new(MockExternalHwnd::with_title(2, "App2", "app2.exe"));
    env.add_window(w1.clone());
    env.add_window(w2.clone());
    assert_h_tiled(
        &[w1.get_dim(), w2.get_dim()],
        default_screen().dimension,
        env.dome.config().border_size,
    );
}

#[test]
fn workspace_switch_hides_window() {
    let mut env = TestEnv::new();
    let w1 = Arc::new(MockExternalHwnd::with_title(1, "App1", "app1.exe"));
    env.add_window(w1.clone());
    assert!(!w1.is_offscreen());
    env.run_actions("focus workspace 1");
    assert!(w1.is_offscreen());
}
