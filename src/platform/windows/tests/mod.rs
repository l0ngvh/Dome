mod drift;
mod lifecycle;
mod placement;
mod transitions;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::action::{Action, Actions};
use crate::config::Config;
use crate::core::Dimension;
use crate::platform::windows::ScreenInfo;
use crate::platform::windows::dome::ObservedPosition;
use crate::platform::windows::dome::overlay::{FloatOverlayApi, TilingOverlayApi};
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

fn second_screen() -> ScreenInfo {
    ScreenInfo {
        handle: 2,
        name: "External".to_string(),
        dimension: Dimension {
            x: SCREEN_WIDTH,
            y: 0.0,
            width: 2560.0,
            height: 1440.0,
        },
        is_primary: false,
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
    moves: MoveLog,
    exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
    config: Config,
    overlay_focus_count: Rc<Cell<u32>>,
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
        let overlay_focus_count = Rc::new(Cell::new(0));
        let dome = Dome::new(
            config.clone(),
            Rc::new(NoopTaskbar),
            Box::new(NoopOverlays {
                focus_count: overlay_focus_count.clone(),
            }),
            Box::new(display),
        )
        .unwrap();
        Self {
            dome,
            moves: Arc::new(Mutex::new(Vec::new())),
            exclusive_fullscreen_hwnd,
            config,
            overlay_focus_count,
        }
    }

    fn spawn_window(&self, id: isize, title: &str, process: &str) -> Arc<MockExternalHwnd> {
        Arc::new(MockExternalHwnd::with_title(
            id,
            title,
            process,
            self.moves.clone(),
        ))
    }

    fn add_window(&mut self, ext: Arc<MockExternalHwnd>) {
        if !ext.manageable {
            return;
        }
        let dim = ext.get_dim();
        let observation = if dim.x <= 0.0
            && dim.y <= 0.0
            && dim.width >= SCREEN_WIDTH
            && dim.height >= SCREEN_HEIGHT
        {
            ObservedPosition::Fullscreen
        } else {
            ObservedPosition::Visible(
                dim.x as i32,
                dim.y as i32,
                dim.width as i32,
                dim.height as i32,
            )
        };
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
            observation,
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

    fn settle(&mut self, limit: usize) {
        for i in 0..limit {
            let pending = std::mem::take(&mut *self.moves.lock().unwrap());
            if pending.is_empty() {
                return;
            }
            let mut last_pos: HashMap<HwndId, (i32, i32, i32, i32)> = HashMap::new();
            for (id, x, y, w, h) in pending {
                last_pos.insert(id, (x, y, w, h));
            }
            for (hwnd_id, (x, y, w, h)) in last_pos {
                self.dome.placement_timeout(hwnd_id);
                self.dome
                    .window_moved(hwnd_id, ObservedPosition::Visible(x, y, w, h));
            }
            self.dome.apply_layout();
            if i == limit - 1 {
                let remaining = self.moves.lock().unwrap().len();
                if remaining > 0 {
                    panic!(
                        "settle did not converge after {limit} iterations ({remaining} moves pending)"
                    );
                }
            }
        }
    }

    fn flush_moves(&mut self) {
        let pending = std::mem::take(&mut *self.moves.lock().unwrap());
        if pending.is_empty() {
            return;
        }
        let mut last_pos: HashMap<HwndId, (i32, i32, i32, i32)> = HashMap::new();
        for (id, x, y, w, h) in pending {
            last_pos.insert(id, (x, y, w, h));
        }
        for (hwnd_id, (x, y, w, h)) in last_pos {
            self.dome.placement_timeout(hwnd_id);
            self.dome
                .window_moved(hwnd_id, ObservedPosition::Visible(x, y, w, h));
        }
        self.dome.apply_layout();
    }

    /// Configure a window to resist repositioning and report it at `pos`.
    fn simulate_resist(&self, ext: &Arc<MockExternalHwnd>, pos: (i32, i32, i32, i32)) {
        ext.set_override_position(Some(pos));
        *ext.dimension.lock().unwrap() = Dimension {
            x: pos.0 as f32,
            y: pos.1 as f32,
            width: pos.2 as f32,
            height: pos.3 as f32,
        };
        ext.simulate_external_move();
    }

    fn destroy_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.window_destroyed(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn minimize_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.window_minimized(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn focus_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.handle_focus(ext.hwnd_id);
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

    fn overlay_focus_count(&self) -> u32 {
        self.overlay_focus_count.get()
    }

    fn reset_overlay_focus(&self) {
        self.overlay_focus_count.set(0);
    }

    fn add_screen(&mut self, screen: ScreenInfo) {
        let mut screens = vec![default_screen()];
        screens.push(screen);
        self.dome.screens_changed(screens);
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

type MoveLog = Arc<Mutex<Vec<(HwndId, i32, i32, i32, i32)>>>;

struct MockExternalHwnd {
    hwnd_id: HwndId,
    manageable: bool,
    title: Option<String>,
    process: String,
    dimension: Mutex<Dimension>,
    override_position: Mutex<Option<(i32, i32, i32, i32)>>,
    should_float: bool,
    iconic: AtomicBool,
    min_size: (f32, f32),
    max_size: (f32, f32),
    z_state: Mutex<ZOrderState>,
    moves: MoveLog,
}

impl MockExternalHwnd {
    fn with_title(id: isize, title: &str, process: &str, moves: MoveLog) -> Self {
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
            override_position: Mutex::new(None),
            should_float: false,
            iconic: AtomicBool::new(false),
            min_size: (0.0, 0.0),
            max_size: (0.0, 0.0),
            z_state: Mutex::new(ZOrderState::Normal),
            moves,
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

    fn set_override_position(&self, pos: Option<(i32, i32, i32, i32)>) {
        *self.override_position.lock().unwrap() = pos;
    }

    /// Simulate the app moving itself — push current dimension to the move log.
    fn simulate_external_move(&self) {
        let dim = self.get_dim();
        self.moves.lock().unwrap().push((
            self.hwnd_id,
            dim.x as i32,
            dim.y as i32,
            dim.width as i32,
            dim.height as i32,
        ));
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

    fn is_iconic(&self) -> bool {
        self.iconic.load(Ordering::Relaxed)
    }

    fn set_position(&self, z: ZOrder, x: i32, y: i32, cx: i32, cy: i32) {
        self.iconic.store(false, Ordering::Relaxed);
        let (x, y, cx, cy) = self
            .override_position
            .lock()
            .unwrap()
            .unwrap_or((x, y, cx, cy));
        *self.dimension.lock().unwrap() = Dimension {
            x: x as f32,
            y: y as f32,
            width: cx as f32,
            height: cy as f32,
        };
        let mut z_state = self.z_state.lock().unwrap();
        match z {
            ZOrder::Topmost => *z_state = ZOrderState::Topmost,
            ZOrder::After(_) => *z_state = ZOrderState::Normal,
            ZOrder::Unchanged => {}
        }
        self.moves
            .lock()
            .unwrap()
            .push((self.hwnd_id, x, y, cx, cy));
    }

    fn move_offscreen(&self) {
        let pos = if let Some((x, y, w, h)) = *self.override_position.lock().unwrap() {
            *self.dimension.lock().unwrap() = Dimension {
                x: x as f32,
                y: y as f32,
                width: w as f32,
                height: h as f32,
            };
            (x, y, w, h)
        } else {
            let mut dim = self.dimension.lock().unwrap();
            dim.x = OFFSCREEN_POS;
            dim.y = OFFSCREEN_POS;
            (
                OFFSCREEN_POS as i32,
                OFFSCREEN_POS as i32,
                dim.width as i32,
                dim.height as i32,
            )
        };
        *self.z_state.lock().unwrap() = ZOrderState::Bottom;
        self.moves
            .lock()
            .unwrap()
            .push((self.hwnd_id, pos.0, pos.1, pos.2, pos.3));
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

    fn recover(&self, _was_maximized: bool) {
        let mut dim = self.dimension.lock().unwrap();
        dim.x = 100.0;
        dim.y = 100.0;
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

struct NoopFloatOverlay;
impl FloatOverlayApi for NoopFloatOverlay {
    fn id(&self) -> HwndId {
        HwndId::test(0)
    }
    fn update(&mut self, _: &crate::core::FloatWindowPlacement, _: &Config, _: ZOrder) {}
    fn hide(&mut self) {}
}

struct NoopTilingOverlay {
    focus_count: Rc<Cell<u32>>,
}

impl TilingOverlayApi for NoopTilingOverlay {
    fn id(&self) -> HwndId {
        HwndId::test(0)
    }
    fn update(
        &mut self,
        _: Dimension,
        _: &[crate::core::TilingWindowPlacement],
        _: &[(crate::core::ContainerPlacement, Vec<String>)],
    ) {
    }
    fn clear(&mut self) {}
    fn focus(&self) {
        self.focus_count.set(self.focus_count.get() + 1);
    }
    fn set_config(&mut self, _: Config) {}
}

struct NoopOverlays {
    focus_count: Rc<Cell<u32>>,
}

impl CreateOverlay for NoopOverlays {
    fn create_tiling_overlay(&self, _: Config) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        Ok(Box::new(NoopTilingOverlay {
            focus_count: self.focus_count.clone(),
        }))
    }
    fn create_float_overlay(&self) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        Ok(Box::new(NoopFloatOverlay))
    }
}
