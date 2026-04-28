mod drift;
mod lifecycle;
mod placement;
mod transitions;
mod zorder;

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::action::{Action, Actions};
use crate::config::Config;
use crate::core::{Dimension, WindowId};
use crate::platform::windows::ScreenInfo;
use crate::platform::windows::dome::ObservedPosition;
use crate::platform::windows::dome::overlay::{FloatOverlayApi, PickerApi, TilingOverlayApi};
use crate::platform::windows::dome::{CreateOverlay, Dome, KeyboardSinkApi, QueryDisplay};
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
    sink_focus_count: Rc<Cell<u32>>,
    overlay_update_count: Rc<Cell<u32>>,
    picker_entries: Rc<RefCell<Vec<(WindowId, String)>>>,
    z_model: ZOrderModel,
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
        let sink_focus_count = Rc::new(Cell::new(0));
        let overlay_update_count = Rc::new(Cell::new(0));
        let picker_entries = Rc::new(RefCell::new(Vec::new()));
        let z_model = ZOrderModel::new();
        let dome = Dome::new(
            config.clone(),
            Rc::new(NoopTaskbar),
            Box::new(NoopOverlays {
                overlay_update_count: overlay_update_count.clone(),
                picker_entries: picker_entries.clone(),
                z_model: z_model.clone(),
            }),
            Box::new(display),
            Box::new(NoopKeyboardSink {
                focus_count: sink_focus_count.clone(),
            }),
        )
        .unwrap();
        Self {
            dome,
            moves: Arc::new(Mutex::new(Vec::new())),
            exclusive_fullscreen_hwnd,
            config,
            sink_focus_count,
            overlay_update_count,
            picker_entries,
            z_model,
        }
    }

    fn spawn_window(&self, id: isize, title: &str, process: &str) -> Arc<MockExternalHwnd> {
        Arc::new(MockExternalHwnd::with_title(
            id,
            title,
            process,
            self.moves.clone(),
            self.z_model.clone(),
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
        self.z_model.remove(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn minimize_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.window_minimized(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn restore_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.window_restored(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn focus_window(&mut self, ext: &Arc<MockExternalHwnd>) {
        self.dome.handle_focus(ext.hwnd_id);
        self.dome.apply_layout();
    }

    fn run_actions(&mut self, s: &str) {
        let action: Action = s.parse().unwrap();
        match action {
            Action::Hub(hub_action) => self.dome.execute_hub_action(&hub_action),
            Action::ToggleMinimizePicker => self.dome.toggle_picker(),
            _ => {}
        }
        self.dome.apply_layout();
    }

    fn enter_exclusive_fullscreen(&mut self, hwnd: HwndId) {
        *self.exclusive_fullscreen_hwnd.lock().unwrap() = Some(hwnd);
        self.dome.handle_display_change();
        *self.exclusive_fullscreen_hwnd.lock().unwrap() = None;
        self.dome.apply_layout();
    }

    fn sink_focus_count(&self) -> u32 {
        self.sink_focus_count.get()
    }

    fn reset_sink_focus(&self) {
        self.sink_focus_count.set(0);
    }

    fn overlay_update_count(&self) -> u32 {
        self.overlay_update_count.get()
    }

    fn add_screen(&mut self, screen: ScreenInfo) {
        let mut screens = vec![default_screen()];
        screens.push(screen);
        self.dome.screens_changed(screens);
        self.dome.apply_layout();
    }

    fn z_order(&self) -> Vec<HwndId> {
        self.z_model.stack()
    }

    fn tiling_z_order(&self) -> Vec<HwndId> {
        self.z_model.normal_stack()
    }

    fn overlay_id(&self) -> HwndId {
        HwndId::test(9999)
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

struct ZOrderStack {
    topmost: Vec<HwndId>,
    normal: Vec<HwndId>,
}

/// Emulates Win32's z-order stack for test assertions. Tracks the relative
/// ordering of windows as `set_position` and `move_offscreen` calls arrive.
#[derive(Clone)]
struct ZOrderModel {
    inner: Arc<Mutex<ZOrderStack>>,
}

impl ZOrderModel {
    fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(ZOrderStack {
                topmost: Vec::new(),
                normal: Vec::new(),
            })),
        }
    }

    fn apply(&self, hwnd: HwndId, z: ZOrder) {
        let mut stack = self.inner.lock().unwrap();

        // Record original position for Unchanged
        let orig_topmost_pos = stack.topmost.iter().position(|&id| id == hwnd);
        let orig_normal_pos = stack.normal.iter().position(|&id| id == hwnd);

        // Remove from both lists
        stack.topmost.retain(|&id| id != hwnd);
        stack.normal.retain(|&id| id != hwnd);

        match z {
            ZOrder::Top => {
                stack.normal.insert(0, hwnd);
            }
            ZOrder::After(other) => {
                if let Some(pos) = stack.normal.iter().position(|&id| id == other) {
                    stack.normal.insert(pos + 1, hwnd);
                } else {
                    stack.normal.push(hwnd);
                }
            }
            ZOrder::Topmost => {
                stack.topmost.insert(0, hwnd);
            }
            ZOrder::Unchanged => {
                // Re-insert at original position (clamped to list length)
                if let Some(pos) = orig_topmost_pos {
                    let clamped = pos.min(stack.topmost.len());
                    stack.topmost.insert(clamped, hwnd);
                } else if let Some(pos) = orig_normal_pos {
                    let clamped = pos.min(stack.normal.len());
                    stack.normal.insert(clamped, hwnd);
                } else {
                    stack.normal.push(hwnd);
                }
            }
        }
    }

    fn move_to_bottom(&self, hwnd: HwndId) {
        let mut stack = self.inner.lock().unwrap();
        stack.topmost.retain(|&id| id != hwnd);
        stack.normal.retain(|&id| id != hwnd);
        stack.normal.push(hwnd);
    }

    /// Returns the full z-order stack from top to bottom: topmost band first, then normal.
    fn stack(&self) -> Vec<HwndId> {
        let stack = self.inner.lock().unwrap();
        let mut result = stack.topmost.clone();
        result.extend_from_slice(&stack.normal);
        result
    }

    fn normal_stack(&self) -> Vec<HwndId> {
        self.inner.lock().unwrap().normal.clone()
    }

    /// Removes a window from both z-order bands. Mirrors Win32 `DestroyWindow`.
    fn remove(&self, hwnd: HwndId) {
        let mut stack = self.inner.lock().unwrap();
        stack.topmost.retain(|&id| id != hwnd);
        stack.normal.retain(|&id| id != hwnd);
    }
}

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
    z_model: ZOrderModel,
    moves: MoveLog,
}

impl MockExternalHwnd {
    fn with_title(
        id: isize,
        title: &str,
        process: &str,
        moves: MoveLog,
        z_model: ZOrderModel,
    ) -> Self {
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
            z_model,
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
            ZOrder::Top => *z_state = ZOrderState::Normal,
            ZOrder::After(_) => *z_state = ZOrderState::Normal,
            ZOrder::Unchanged => {}
        }
        self.z_model.apply(self.hwnd_id, z);
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
        self.z_model.move_to_bottom(self.hwnd_id);
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

impl Drop for MockExternalHwnd {
    fn drop(&mut self) {
        self.z_model.remove(self.hwnd_id);
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

struct NoopKeyboardSink {
    focus_count: Rc<Cell<u32>>,
}

impl KeyboardSinkApi for NoopKeyboardSink {
    fn focus(&self) {
        self.focus_count.set(self.focus_count.get() + 1);
    }
}

struct NoopFloatOverlay {
    overlay_update_count: Rc<Cell<u32>>,
}
impl FloatOverlayApi for NoopFloatOverlay {
    fn update(&mut self, _: &crate::core::FloatWindowPlacement, _: &Config, _: ZOrder) {
        self.overlay_update_count
            .set(self.overlay_update_count.get() + 1);
    }
    fn hide(&mut self) {}
}

struct NoopTilingOverlay;

impl TilingOverlayApi for NoopTilingOverlay {
    fn update(
        &mut self,
        _: Dimension,
        _: &[crate::core::TilingWindowPlacement],
        _: &[(crate::core::ContainerPlacement, Vec<String>)],
    ) {
    }
    fn clear(&mut self) {}
    fn set_config(&mut self, _: Config) {}
}

struct NoopPicker {
    visible: bool,
    entries: Rc<RefCell<Vec<(WindowId, String)>>>,
}

impl PickerApi for NoopPicker {
    fn show(&mut self, entries: Vec<(WindowId, String)>, _monitor_dim: Dimension) {
        self.entries.borrow_mut().clone_from(&entries);
        self.visible = true;
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

struct NoopOverlays {
    overlay_update_count: Rc<Cell<u32>>,
    picker_entries: Rc<RefCell<Vec<(WindowId, String)>>>,
    z_model: ZOrderModel,
}

impl CreateOverlay for NoopOverlays {
    fn create_tiling_overlay(&self, _: Config) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        // Seed the overlay at the top of the normal band, mirroring Win32
        // CreateWindowExW. Subsequent tiling windows placed with ZOrder::Top
        // push it down.
        self.z_model.apply(HwndId::test(9999), ZOrder::Top);
        Ok(Box::new(NoopTilingOverlay))
    }
    fn create_float_overlay(&self) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        Ok(Box::new(NoopFloatOverlay {
            overlay_update_count: self.overlay_update_count.clone(),
        }))
    }
    fn create_picker(
        &self,
        entries: Vec<(WindowId, String)>,
        monitor_dim: Dimension,
    ) -> anyhow::Result<Box<dyn PickerApi>> {
        let mut picker = NoopPicker {
            visible: false,
            entries: self.picker_entries.clone(),
        };
        picker.show(entries, monitor_dim);
        Ok(Box::new(picker))
    }
}
