mod drift;
mod lifecycle;
mod picker;
mod placement;
mod transitions;
mod zorder;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use crate::action::{Action, Actions};
use crate::config::Config;
use crate::core::{Dimension, Length, Physical};
use crate::picker::PickerEntry;
use crate::platform::windows::ScreenInfo;
use crate::platform::windows::dome::ObservedPosition;
use crate::platform::windows::dome::overlay::{FloatOverlayApi, PickerApi, TilingOverlayApi};
use crate::platform::windows::dome::{CreateOverlay, Dome, KeyboardSinkApi, QueryDisplay};
use crate::platform::windows::external::{HwndId, ManageExternalHwnd, ShowCmd, ZOrder};
use crate::platform::windows::taskbar::ManageTaskbar;

const SCREEN_WIDTH: Length = Length::new(1920.0);
const SCREEN_HEIGHT: Length = Length::new(1080.0);
const OFFSCREEN_POS: Length = Length::new(-32000.0);

/// Test helper: construct a `Dimension<Physical>` from integer coords.
fn dim(x: i32, y: i32, w: i32, h: i32) -> Dimension<Physical> {
    Dimension::new(
        Length::new(x as f32),
        Length::new(y as f32),
        Length::new(w as f32),
        Length::new(h as f32),
    )
}

fn default_screen() -> ScreenInfo {
    ScreenInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT),
        is_primary: true,
        scale: 1.0,
    }
}

fn second_screen() -> ScreenInfo {
    ScreenInfo {
        handle: 2,
        name: "External".to_string(),
        dimension: Dimension::new(
            SCREEN_WIDTH,
            Length::ZERO,
            Length::new(2560.0),
            Length::new(1440.0),
        ),
        is_primary: false,
        scale: 1.0,
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
    tiling_overlay_update_count: Rc<Cell<u32>>,
    tiling_overlay_clear_count: Rc<Cell<u32>>,
    float_overlay_apply_theme_count: Rc<Cell<u32>>,
    tiling_overlay_apply_theme_count: Rc<Cell<u32>>,
    float_overlay_apply_font_count: Rc<Cell<u32>>,
    tiling_overlay_apply_font_count: Rc<Cell<u32>>,
    picker_entries: Rc<RefCell<Vec<PickerEntry>>>,
    picker_loaded_icons: Rc<RefCell<HashSet<String>>>,
    z_model: ZOrderModel,
}

impl TestEnv {
    fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    fn new_with_config(config: Config) -> Self {
        Self::new_with_screens(config, vec![default_screen()])
    }

    fn new_with_screens(config: Config, screens: Vec<ScreenInfo>) -> Self {
        let exclusive_fullscreen_hwnd = Arc::new(Mutex::new(None));
        let display = MockDisplay {
            screens,
            exclusive_fullscreen_hwnd: exclusive_fullscreen_hwnd.clone(),
        };
        let sink_focus_count = Rc::new(Cell::new(0));
        let overlay_update_count = Rc::new(Cell::new(0));
        let tiling_overlay_update_count = Rc::new(Cell::new(0));
        let tiling_overlay_clear_count = Rc::new(Cell::new(0));
        let float_overlay_apply_theme_count = Rc::new(Cell::new(0));
        let tiling_overlay_apply_theme_count = Rc::new(Cell::new(0));
        let float_overlay_apply_font_count = Rc::new(Cell::new(0));
        let tiling_overlay_apply_font_count = Rc::new(Cell::new(0));
        let picker_entries = Rc::new(RefCell::new(Vec::new()));
        let picker_loaded_icons = Rc::new(RefCell::new(HashSet::new()));
        let z_model = ZOrderModel::new();

        let dome = Dome::new(
            config.clone(),
            Rc::new(NoopTaskbar),
            Box::new(NoopOverlays {
                overlay_update_count: overlay_update_count.clone(),
                tiling_overlay_update_count: tiling_overlay_update_count.clone(),
                tiling_overlay_clear_count: tiling_overlay_clear_count.clone(),
                float_overlay_apply_theme_count: float_overlay_apply_theme_count.clone(),
                tiling_overlay_apply_theme_count: tiling_overlay_apply_theme_count.clone(),
                float_overlay_apply_font_count: float_overlay_apply_font_count.clone(),
                tiling_overlay_apply_font_count: tiling_overlay_apply_font_count.clone(),
                picker_entries: picker_entries.clone(),
                picker_loaded_icons: picker_loaded_icons.clone(),
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
            tiling_overlay_update_count,
            tiling_overlay_clear_count,
            float_overlay_apply_theme_count,
            tiling_overlay_apply_theme_count,
            float_overlay_apply_font_count,
            tiling_overlay_apply_font_count,
            picker_entries,
            picker_loaded_icons,
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
        let observation = if dim.x <= Length::ZERO
            && dim.y <= Length::ZERO
            && dim.width >= SCREEN_WIDTH
            && dim.height >= SCREEN_HEIGHT
        {
            ObservedPosition::Fullscreen
        } else {
            ObservedPosition::Visible {
                rect: dim,
                monitor: 1,
            }
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
            ext.app_name.clone(),
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
            let mut last_pos: HashMap<HwndId, Dimension> = HashMap::new();
            for (id, dim) in pending {
                last_pos.insert(id, dim);
            }
            for (hwnd_id, dim) in last_pos {
                self.dome.placement_timeout(hwnd_id);
                self.dome.window_moved(
                    hwnd_id,
                    ObservedPosition::Visible {
                        rect: dim,
                        monitor: 1,
                    },
                );
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
        let mut last_pos: HashMap<HwndId, Dimension> = HashMap::new();
        for (id, dim) in pending {
            last_pos.insert(id, dim);
        }
        for (hwnd_id, dim) in last_pos {
            self.dome.placement_timeout(hwnd_id);
            self.dome.window_moved(
                hwnd_id,
                ObservedPosition::Visible {
                    rect: dim,
                    monitor: 1,
                },
            );
        }
        self.dome.apply_layout();
    }

    /// Configure a window to resist repositioning and report it at `pos`.
    fn simulate_resist(&self, ext: &Arc<MockExternalHwnd>, pos: (i32, i32, i32, i32)) {
        ext.set_override_position(Some(pos));
        *ext.dimension.lock().unwrap() = Dimension::new(
            Length::new(pos.0 as f32),
            Length::new(pos.1 as f32),
            Length::new(pos.2 as f32),
            Length::new(pos.3 as f32),
        );
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

    fn tiling_overlay_update_count(&self) -> u32 {
        self.tiling_overlay_update_count.get()
    }

    fn tiling_overlay_clear_count(&self) -> u32 {
        self.tiling_overlay_clear_count.get()
    }

    fn float_overlay_apply_theme_count(&self) -> u32 {
        self.float_overlay_apply_theme_count.get()
    }

    fn tiling_overlay_apply_theme_count(&self) -> u32 {
        self.tiling_overlay_apply_theme_count.get()
    }

    fn float_overlay_apply_font_count(&self) -> u32 {
        self.float_overlay_apply_font_count.get()
    }

    fn tiling_overlay_apply_font_count(&self) -> u32 {
        self.tiling_overlay_apply_font_count.get()
    }

    fn picker_loaded_icons(&self) -> HashSet<String> {
        self.picker_loaded_icons.borrow().clone()
    }

    fn picker_icons_to_load(&mut self) -> Vec<(String, HwndId)> {
        self.dome.picker_icons_to_load()
    }

    fn picker_receive_icon(&mut self, app_id: String) {
        // Use a 1x1 dummy image; the noop picker ignores the pixel data.
        let image = egui::ColorImage::new([1, 1], vec![egui::Color32::WHITE]);
        self.dome.picker_receive_icon(app_id, image);
    }

    fn picker_scale(&self) -> Option<f32> {
        self.dome.picker_scale()
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
    Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ZOrderState {
    Bottom,
    Normal,
    Topmost,
}

type MoveLog = Arc<Mutex<Vec<(HwndId, Dimension)>>>;

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
    app_name: Option<String>,
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
            app_name: None,
            dimension: Mutex::new(Dimension::new(
                Length::ZERO,
                Length::ZERO,
                Length::new(800.0),
                Length::new(600.0),
            )),
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

    fn with_min_size(mut self, w: f32, h: f32) -> Self {
        self.min_size = (w, h);
        self
    }

    fn with_dimension(self, dim: Dimension) -> Self {
        *self.dimension.lock().unwrap() = dim;
        self
    }

    fn set_override_position(&self, pos: Option<(i32, i32, i32, i32)>) {
        *self.override_position.lock().unwrap() = pos;
    }

    /// Simulate the app moving itself -- push current dimension to the move log.
    fn simulate_external_move(&self) {
        let dim = self.get_dim();
        self.moves.lock().unwrap().push((self.hwnd_id, dim));
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

    fn set_position(&self, z: ZOrder, dim: Dimension) {
        self.iconic.store(false, Ordering::Relaxed);
        let dim = self.override_position.lock().unwrap().map_or(dim, |pos| {
            Dimension::new(
                Length::new(pos.0 as f32),
                Length::new(pos.1 as f32),
                Length::new(pos.2 as f32),
                Length::new(pos.3 as f32),
            )
        });
        *self.dimension.lock().unwrap() = dim;
        let mut z_state = self.z_state.lock().unwrap();
        match z {
            ZOrder::Topmost => *z_state = ZOrderState::Topmost,
            ZOrder::Top => *z_state = ZOrderState::Normal,
            ZOrder::After(_) => *z_state = ZOrderState::Normal,
            ZOrder::Unchanged => {}
        }
        self.z_model.apply(self.hwnd_id, z);
        self.moves.lock().unwrap().push((self.hwnd_id, dim));
    }

    fn move_offscreen(&self) {
        let dim = if let Some((x, y, w, h)) = *self.override_position.lock().unwrap() {
            let d = Dimension::new(
                Length::new(x as f32),
                Length::new(y as f32),
                Length::new(w as f32),
                Length::new(h as f32),
            );
            *self.dimension.lock().unwrap() = d;
            d
        } else {
            let mut d = self.dimension.lock().unwrap();
            d.x = OFFSCREEN_POS;
            d.y = OFFSCREEN_POS;
            *d
        };
        *self.z_state.lock().unwrap() = ZOrderState::Bottom;
        self.z_model.move_to_bottom(self.hwnd_id);
        self.moves.lock().unwrap().push((self.hwnd_id, dim));
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
        dim.x = Length::new(100.0);
        dim.y = Length::new(100.0);
    }
}

impl Drop for MockExternalHwnd {
    fn drop(&mut self) {
        self.z_model.remove(self.hwnd_id);
    }
}

/// Assert that windows tile horizontally across the screen.
fn assert_h_tiled(dims: &[Dimension], screen: Dimension, border: f32) {
    let border_len = Length::new(border);
    assert!(!dims.is_empty());
    for (i, d) in dims.iter().enumerate() {
        assert_eq!(d.y, border_len, "window {i} y");
        assert_eq!(
            d.height,
            screen.height - Length::new(2.0 * border),
            "window {i} height"
        );
        assert!(d.width > Length::new(0.0), "window {i} width");
    }
    assert_eq!(dims[0].x, border_len, "first window x");
    let last = dims.last().unwrap();
    assert!(
        (last.x + last.width - (screen.width - border_len)).abs() < Length::new(1.0),
        "last window right edge"
    );
    for i in 1..dims.len() {
        let gap = dims[i].x - (dims[i - 1].x + dims[i - 1].width);
        assert!(
            (gap - Length::new(2.0 * border)).abs() < Length::new(2.0),
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
    apply_theme_count: Rc<Cell<u32>>,
    apply_font_count: Rc<Cell<u32>>,
}
impl FloatOverlayApi for NoopFloatOverlay {
    fn update(
        &mut self,
        _: &crate::core::FloatWindowPlacement,
        _: &Config,
        _: ZOrder,
        _scale: f32,
    ) {
        self.overlay_update_count
            .set(self.overlay_update_count.get() + 1);
    }
    fn hide(&mut self) {}
    fn apply_theme(&mut self, _flavor: crate::theme::Flavor) {
        self.apply_theme_count.set(self.apply_theme_count.get() + 1);
    }
    fn apply_font(&mut self, _font: &crate::font::FontConfig) {
        self.apply_font_count.set(self.apply_font_count.get() + 1);
    }
}

struct NoopTilingOverlay {
    update_count: Rc<Cell<u32>>,
    clear_count: Rc<Cell<u32>>,
    apply_theme_count: Rc<Cell<u32>>,
    apply_font_count: Rc<Cell<u32>>,
}

impl TilingOverlayApi for NoopTilingOverlay {
    fn update(
        &mut self,
        _: Dimension,
        _: &[crate::core::TilingWindowPlacement],
        _: &[(crate::core::ContainerPlacement, Vec<String>)],
        _scale: f32,
    ) {
        self.update_count.set(self.update_count.get() + 1);
    }
    fn clear(&mut self) {
        self.clear_count.set(self.clear_count.get() + 1);
    }
    fn set_config(&mut self, _: Config) {}
    fn apply_theme(&mut self, _flavor: crate::theme::Flavor) {
        self.apply_theme_count.set(self.apply_theme_count.get() + 1);
    }
    fn apply_font(&mut self, _font: &crate::font::FontConfig) {
        self.apply_font_count.set(self.apply_font_count.get() + 1);
    }
}

struct NoopPicker {
    visible: bool,
    entries: Rc<RefCell<Vec<PickerEntry>>>,
    loaded_icons: Rc<RefCell<HashSet<String>>>,
}

impl PickerApi for NoopPicker {
    fn show(&mut self, entries: Vec<PickerEntry>, _monitor_dim: Dimension, _scale: f32) {
        *self.entries.borrow_mut() = entries;
        self.visible = true;
    }

    fn hide(&mut self) {
        self.visible = false;
    }

    fn is_visible(&self) -> bool {
        self.visible
    }

    fn icons_to_load(
        &mut self,
        lookup_hwnd: &dyn Fn(crate::core::WindowId) -> Option<HwndId>,
    ) -> Vec<(String, HwndId)> {
        let entries = self.entries.borrow();
        let mut loaded = self.loaded_icons.borrow_mut();
        let mut result = Vec::new();
        for entry in entries.iter() {
            let Some(app_id) = entry.app_id.as_ref() else {
                continue;
            };
            if loaded.contains(app_id) {
                continue;
            }
            let Some(hwnd_id) = lookup_hwnd(entry.id) else {
                continue;
            };
            loaded.insert(app_id.clone());
            result.push((app_id.clone(), hwnd_id));
        }
        result
    }

    fn receive_icon(&mut self, app_id: String, _image: egui::ColorImage) {
        self.loaded_icons.borrow_mut().insert(app_id);
    }

    fn rerender(&mut self) {}
}

struct NoopOverlays {
    overlay_update_count: Rc<Cell<u32>>,
    tiling_overlay_update_count: Rc<Cell<u32>>,
    tiling_overlay_clear_count: Rc<Cell<u32>>,
    float_overlay_apply_theme_count: Rc<Cell<u32>>,
    tiling_overlay_apply_theme_count: Rc<Cell<u32>>,
    float_overlay_apply_font_count: Rc<Cell<u32>>,
    tiling_overlay_apply_font_count: Rc<Cell<u32>>,
    picker_entries: Rc<RefCell<Vec<PickerEntry>>>,
    picker_loaded_icons: Rc<RefCell<HashSet<String>>>,
    z_model: ZOrderModel,
}

impl CreateOverlay for NoopOverlays {
    fn create_tiling_overlay(
        &self,
        _: Config,
        _monitor: Dimension,
        _scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        // Seed the overlay at the top of the normal band, mirroring Win32
        // CreateWindowExW. Subsequent tiling windows placed with ZOrder::Top
        // push it down.
        self.z_model.apply(HwndId::test(9999), ZOrder::Top);
        Ok(Box::new(NoopTilingOverlay {
            update_count: self.tiling_overlay_update_count.clone(),
            clear_count: self.tiling_overlay_clear_count.clone(),
            apply_theme_count: self.tiling_overlay_apply_theme_count.clone(),
            apply_font_count: self.tiling_overlay_apply_font_count.clone(),
        }))
    }
    fn create_float_overlay(
        &self,
        _flavor: crate::theme::Flavor,
        _font: &crate::font::FontConfig,
        _scale: f32,
        _visible_frame: Dimension,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        Ok(Box::new(NoopFloatOverlay {
            overlay_update_count: self.overlay_update_count.clone(),
            apply_theme_count: self.float_overlay_apply_theme_count.clone(),
            apply_font_count: self.float_overlay_apply_font_count.clone(),
        }))
    }
    fn create_picker(
        &self,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        _flavor: crate::theme::Flavor,
        _font: &crate::font::FontConfig,
        scale: f32,
    ) -> anyhow::Result<Box<dyn PickerApi>> {
        let mut picker = NoopPicker {
            visible: false,
            entries: self.picker_entries.clone(),
            loaded_icons: self.picker_loaded_icons.clone(),
        };
        picker.show(entries, monitor_dim, scale);
        Ok(Box::new(picker))
    }
}
