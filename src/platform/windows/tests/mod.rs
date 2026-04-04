mod lifecycle;
mod placement;
mod transitions;

use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::action::{Action, Actions};
use crate::config::Config;
use crate::core::Dimension;
use crate::platform::windows::ScreenInfo;
use crate::platform::windows::dome::overlay::{
    ContainerOverlayApi, NoopContainerOverlay, NoopWindowOverlay, WindowOverlayApi,
};
use crate::platform::windows::dome::{CreateOverlay, Dome, QueryDisplay};
use crate::platform::windows::external::{HwndId, ManageExternalHwnd, ShowCmd, ZOrder};
use crate::platform::windows::taskbar::ManageTaskbar;

const SCREEN_WIDTH: f32 = 1920.0;
const SCREEN_HEIGHT: f32 = 1080.0;
const OFFSCREEN_POS: f32 = -32000.0;

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

struct MockDisplay {
    screens: Vec<ScreenInfo>,
    exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
}

impl QueryDisplay for MockDisplay {
    fn get_all_screens(&self) -> anyhow::Result<Vec<ScreenInfo>> {
        Ok(self.screens.clone())
    }

    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId> {
        *self.exclusive_fullscreen_hwnd.lock().unwrap()
    }
}

struct TestEnv {
    dome: Dome,
    exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
    config: Config,
}

impl TestEnv {
    fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    fn new_with_config(config: Config) -> Self {
        let screens = vec![default_screen()];
        let exclusive_fullscreen_hwnd = Arc::new(Mutex::new(None));
        let display = MockDisplay {
            screens,
            exclusive_fullscreen_hwnd: exclusive_fullscreen_hwnd.clone(),
        };
        let dome = Dome::new(
            config.clone(),
            Rc::new(NoopTaskbar),
            Box::new(NoopOverlays),
            Box::new(display),
        )
        .unwrap();
        Self {
            dome,
            exclusive_fullscreen_hwnd,
            config,
        }
    }

    fn add_window(&mut self, ext: Arc<MockExternalHwnd>) {
        if !ext.manageable {
            return;
        }
        let on_open = self.dome.try_manage_window(
            ext.clone(),
            ext.title.clone(),
            ext.process.clone(),
            (
                ext.min_size.0,
                ext.min_size.1,
                ext.max_size.0,
                ext.max_size.1,
            ),
        );
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
        self.dome.window_destroyed(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn minimize_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.window_minimized(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn run_actions(&mut self, s: &str) {
        let action: Action = s.parse().unwrap();
        if let Action::Hub(hub_action) = action {
            self.dome.execute_hub_action(&hub_action);
        }
        self.dome.apply_layout();
    }

    fn enter_exclusive_fullscreen(&mut self, hwnd: HwndId) {
        *self.exclusive_fullscreen_hwnd.lock().unwrap() = Some(hwnd);
        self.dome.handle_display_change();
        *self.exclusive_fullscreen_hwnd.lock().unwrap() = None;
        self.dome.apply_layout();
    }
}

fn fullscreen_dim() -> Dimension {
    Dimension {
        x: 0.0,
        y: 0.0,
        width: SCREEN_WIDTH,
        height: SCREEN_HEIGHT,
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ZOrderState {
    Bottom,
    Normal,
    Topmost,
}

struct MockExternalHwnd {
    hwnd_id: HwndId,
    manageable: bool,
    title: Option<String>,
    process: String,
    dimension: Mutex<Dimension>,
    monitor_handle: isize,
    should_float: bool,
    iconic: AtomicBool,
    min_size: (f32, f32),
    max_size: (f32, f32),
    z_state: Mutex<ZOrderState>,
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
            should_float: false,
            iconic: AtomicBool::new(false),
            min_size: (0.0, 0.0),
            max_size: (0.0, 0.0),
            z_state: Mutex::new(ZOrderState::Normal),
        }
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

    fn is_topmost(&self) -> bool {
        *self.z_state.lock().unwrap() == ZOrderState::Topmost
    }

    fn is_bottom(&self) -> bool {
        *self.z_state.lock().unwrap() == ZOrderState::Bottom
    }
}

impl ManageExternalHwnd for MockExternalHwnd {
    fn id(&self) -> HwndId {
        self.hwnd_id
    }

    fn should_float(&self) -> bool {
        self.should_float
    }

    fn get_dimension(&self) -> Dimension {
        self.get_dim()
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
        let mut z_state = self.z_state.lock().unwrap();
        match z {
            ZOrder::Topmost => *z_state = ZOrderState::Topmost,
            ZOrder::NotTopmost | ZOrder::After(_) => *z_state = ZOrderState::Normal,
            ZOrder::Unchanged => {}
        }
    }

    fn move_offscreen(&self) {
        let mut dim = self.dimension.lock().unwrap();
        dim.x = OFFSCREEN_POS;
        dim.y = OFFSCREEN_POS;
        drop(dim);
        *self.z_state.lock().unwrap() = ZOrderState::Bottom;
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
            (gap - 2.0 * border).abs() < 2.0,
            "gap between window {} and {}",
            i - 1,
            i
        );
    }
}

struct NoopTaskbar;
impl ManageTaskbar for NoopTaskbar {
    fn add_tab(&self, _: HwndId) {}
    fn delete_tab(&self, _: HwndId) {}
}

struct NoopOverlays;
impl CreateOverlay for NoopOverlays {
    fn create_window_overlay(&self) -> anyhow::Result<Box<dyn WindowOverlayApi>> {
        Ok(Box::new(NoopWindowOverlay))
    }
    fn create_container_overlay(&self, _: Config) -> anyhow::Result<Box<dyn ContainerOverlayApi>> {
        Ok(Box::new(NoopContainerOverlay))
    }
}
