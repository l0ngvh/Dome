mod lifecycle;
mod placement;
mod transitions;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::action::{Action, Actions};
use crate::config::Config;
use crate::core::Dimension;
use crate::platform::windows::OFFSCREEN_POS;
use crate::platform::windows::ScreenInfo;
use crate::platform::windows::dome::{Dome, NoopOverlays, NoopTaskbar};
use crate::platform::windows::external::{HwndId, ManageExternalHwnd, ShowCmd, ZOrder};
use crate::platform::windows::handle::WindowMode;

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

struct TestEnv {
    dome: Dome,
}

impl TestEnv {
    fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    fn new_with_config(config: Config) -> Self {
        let dome = Dome::new(
            config,
            vec![default_screen()],
            Box::new(NoopTaskbar),
            Box::new(NoopOverlays),
        );
        Self { dome }
    }

    fn add_window(&mut self, ext: Arc<dyn ManageExternalHwnd>) {
        let on_open = self.dome.window_created(ext);
        if let Some(actions) = on_open {
            for action in &actions {
                if let Action::Hub(hub_action) = action {
                    self.dome.execute_hub_action(hub_action);
                }
            }
        }
        self.dome.apply_layout();
    }

    fn destroy_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome
            .window_destroyed(ext.clone() as Arc<dyn ManageExternalHwnd>);
        self.dome.apply_layout();
    }

    fn minimize_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome
            .window_minimized(ext.clone() as Arc<dyn ManageExternalHwnd>);
        self.dome.apply_layout();
    }

    fn run_actions(&mut self, s: &str) {
        let action: Action = s.parse().unwrap();
        if let Action::Hub(hub_action) = action {
            self.dome.execute_hub_action(&hub_action);
        }
        self.dome.apply_layout();
    }
}

struct MockExternalHwnd {
    hwnd_id: HwndId,
    manageable: bool,
    title: Option<String>,
    process: String,
    dimension: Mutex<Dimension>,
    monitor_handle: isize,
    mode: WindowMode,
    iconic: AtomicBool,
    min_size: (f32, f32),
    max_size: (f32, f32),
    last_z: Mutex<Option<ZOrder>>,
}

impl MockExternalHwnd {
    fn with_title(id: isize, title: &str, process: &str) -> Self {
        Self {
            hwnd_id: HwndId::test(id),
            manageable: true,
            title: Some(title.to_string()),
            process: process.to_string(),
            dimension: Mutex::new(Dimension {
                x: 0.0,
                y: 0.0,
                width: 800.0,
                height: 600.0,
            }),
            monitor_handle: 1,
            mode: WindowMode::Tiling,
            iconic: AtomicBool::new(false),
            min_size: (0.0, 0.0),
            max_size: (0.0, 0.0),
            last_z: Mutex::new(None),
        }
    }

    fn with_mode(mut self, mode: WindowMode) -> Self {
        self.mode = mode;
        self
    }

    fn with_manageable(mut self, manageable: bool) -> Self {
        self.manageable = manageable;
        self
    }

    fn with_dimension(self, dim: Dimension) -> Self {
        *self.dimension.lock().unwrap() = dim;
        self
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
        self.manageable
    }

    fn get_window_title(&self) -> Option<String> {
        self.title.clone()
    }

    fn get_process_name(&self) -> anyhow::Result<String> {
        Ok(self.process.clone())
    }

    fn initial_window_mode(&self, _monitor: Option<&Dimension>) -> WindowMode {
        self.mode
    }

    fn get_dimension(&self) -> Dimension {
        self.get_dim()
    }

    fn get_size_constraints(&self) -> (f32, f32, f32, f32) {
        (
            self.min_size.0,
            self.min_size.1,
            self.max_size.0,
            self.max_size.1,
        )
    }

    fn get_monitor_handle(&self) -> Option<isize> {
        Some(self.monitor_handle)
    }

    fn is_iconic(&self) -> bool {
        self.iconic.load(Ordering::Relaxed)
    }

    fn set_position(&self, z: ZOrder, x: i32, y: i32, cx: i32, cy: i32) {
        self.iconic.store(false, Ordering::Relaxed);
        *self.dimension.lock().unwrap() = Dimension {
            x: x as f32,
            y: y as f32,
            width: cx as f32,
            height: cy as f32,
        };
        *self.last_z.lock().unwrap() = Some(z);
    }

    fn move_offscreen(&self) {
        let mut dim = self.dimension.lock().unwrap();
        dim.x = OFFSCREEN_POS;
        dim.y = OFFSCREEN_POS;
    }

    fn show_cmd(&self, cmd: ShowCmd) {
        match cmd {
            ShowCmd::Minimize => self.iconic.store(true, Ordering::Relaxed),
            ShowCmd::Restore => self.iconic.store(false, Ordering::Relaxed),
        }
    }

    fn set_foreground_window(&self) {}

    fn is_maximized(&self) -> bool {
        false
    }

    fn recover(&self, dim: Dimension, _was_maximized: bool) {
        *self.dimension.lock().unwrap() = dim;
    }
}

/// Assert that windows tile horizontally across the screen.
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
