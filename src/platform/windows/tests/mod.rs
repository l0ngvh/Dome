mod config_live_reload;
mod lifecycle;
mod picker;
mod placement;
mod transitions;
mod uncooperative;
mod zorder;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use crate::action::{Action, Actions};
use crate::config::{Config, LayoutConfig, LayoutWorkspaceConfig};
use crate::core::GlobalLayoutConfig;
use crate::core::PickerEntry;
use crate::core::{
    ContainerId, ContainerPlacement, Dimension, Length, Logical, Physical, TilingWindowPlacement,
    WindowId,
};
use crate::font::FontConfig;
use crate::platform::windows::dome::MonitorInfo;
use crate::platform::windows::dome::overlay::{
    FloatOverlayApi, PickerApi, TabBarOverlayApi, TilingOverlayApi,
};
use crate::platform::windows::dome::{
    CreateOverlay, Dome, NewWindow, QueryDisplay, WindowsMetadata,
};
use crate::platform::windows::external::{HwndId, ManageExternalWindow, ShowCmd, ZOrder};
use crate::platform::windows::taskbar::ManageTaskbar;
use crate::theme::Flavor;

/// Mirrors what the real tiling overlay shows on screen. The mock writes
/// this from `update`/`clear` so tests assert on state, not call counts.
#[derive(Clone, Debug)]
enum TilingOverlayState {
    Hidden,
    Visible { windows: Vec<TilingWindowPlacement> },
}

/// Mirrors what the real float overlay shows on screen. `update` writes
/// `Visible{..}` with the placement it received; `hide` writes `Hidden`.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FloatOverlayState {
    Hidden,
    Visible {
        window_id: WindowId,
        visible_frame: Dimension,
        z_order: ZOrder,
    },
}

impl FloatOverlayState {
    fn is_visible(&self) -> bool {
        matches!(self, FloatOverlayState::Visible { .. })
    }
}

/// Last focus directive Dome issued. The Hub either parks focus on the
/// sink (no window) or pushes it to a specific window's HWND. The mock
/// records whichever fires.
#[derive(Clone, Copy, Debug, PartialEq)]
enum FocusTarget {
    /// Before any focus directive has fired.
    Initial,
    Overlay,
    Window(HwndId),
}

/// Mirrors the real float overlay's shared observable state. `Rc` between
/// the mock map entry and every `MockFloatOverlay` clone Dome holds.
/// `Drop` on the last `MockFloatOverlay` sets `stale`; the snapshot skips
/// stale entries.
#[derive(Clone, Debug)]
struct FloatOverlayShared {
    overlay_id: HwndId,
    stale: Cell<bool>,
    state: Cell<FloatOverlayState>,
    flavor: Cell<Flavor>,
    font: RefCell<FontConfig>,
}

// ── Snapshot types ──

#[derive(Clone, Debug)]
struct TilingOverlaySnapshot {
    overlay_id: HwndId,
    state: TilingOverlayState,
    flavor: Flavor,
    font: FontConfig,
}

#[derive(Clone, Debug)]
struct FloatOverlaySnapshot {
    overlay_id: HwndId,
    state: FloatOverlayState,
    flavor: Flavor,
    font: FontConfig,
}

const SCREEN_WIDTH: Length = Length::new(1920.0);
const SCREEN_HEIGHT: Length = Length::new(1080.0);
const OFFSCREEN_POS: Length = Length::new(-32000.0);

/// Initial rect a freshly-spawned mock reports until layout overwrites it.
/// Tests that don't care about the pre-layout dimension pass `SPAWN_DIM`;
/// tests that exercise the registration-time dim (fullscreen detection,
/// min-size constraints) pass an explicit value.
const SPAWN_DIM: Dimension<Physical> = Dimension::new(
    Length::ZERO,
    Length::ZERO,
    Length::new(800.0),
    Length::new(600.0),
);

/// Test helper: construct a `Dimension<Physical>` from integer coords.
fn dim(x: i32, y: i32, w: i32, h: i32) -> Dimension<Physical> {
    Dimension::new(
        Length::new(x as f32),
        Length::new(y as f32),
        Length::new(w as f32),
        Length::new(h as f32),
    )
}

fn default_monitor() -> MonitorInfo {
    MonitorInfo {
        handle: 1,
        name: "Test".to_string(),
        dimension: Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT),
        is_primary: true,
        scale: 1.0,
    }
}

fn second_monitor() -> MonitorInfo {
    MonitorInfo {
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
    monitors: Arc<Mutex<Vec<MonitorInfo>>>,
    exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
}

impl QueryDisplay for MockDisplay {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>> {
        Ok(self.monitors.lock().unwrap().clone())
    }

    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId> {
        *self.exclusive_fullscreen_hwnd.lock().unwrap()
    }
}

struct TestEnv {
    dome: Dome,
    moves: MoveLog,
    /// Per-HwndId handle to every mock window registered with the dome.
    mocks: HashMap<HwndId, Arc<MockExternalHwnd>>,
    /// Monitors shared with MockDisplay so add_monitor can mutate them.
    monitors: Arc<Mutex<Vec<MonitorInfo>>>,
    exclusive_fullscreen_hwnd: Arc<Mutex<Option<HwndId>>>,
    config: Config,
    overlays: Rc<RefCell<MockOverlays>>,
    picker: Rc<RefCell<MockPicker>>,
    tab_bars: Rc<RefCell<HashMap<ContainerId, MockTabBarOverlay>>>,
    z_stack: ZOrderStack,
    focus_target: Arc<Mutex<FocusTarget>>,
}

impl TestEnv {
    fn new() -> Self {
        Self::new_with_config(Config::default())
    }

    fn new_with_config(config: Config) -> Self {
        Self::new_with_monitors(config, LayoutConfig::default(), vec![default_monitor()])
    }

    fn new_with_layout_settings(
        config: Config,
        layout: GlobalLayoutConfig,
        workspace_overrides: Vec<LayoutWorkspaceConfig>,
    ) -> Self {
        let mut config = config;
        config.strategy = layout.strategy;
        config.partition_tree = layout.partition_tree;
        config.master = layout.master;
        config.size_constraints = layout.size_constraints;
        config.float = layout.float;
        config.fullscreen = layout.fullscreen;
        Self::new_with_monitors(
            config,
            LayoutConfig {
                workspace: workspace_overrides,
            },
            vec![default_monitor()],
        )
    }

    fn new_with_monitors(config: Config, layout: LayoutConfig, monitors: Vec<MonitorInfo>) -> Self {
        setup_logger();

        let exclusive_fullscreen_hwnd = Arc::new(Mutex::new(None));
        let shared_monitors = Arc::new(Mutex::new(monitors));
        let display = MockDisplay {
            monitors: shared_monitors.clone(),
            exclusive_fullscreen_hwnd: exclusive_fullscreen_hwnd.clone(),
        };
        let focus_target = Arc::new(Mutex::new(FocusTarget::Initial));
        let picker = Rc::new(RefCell::new(MockPicker {
            visible: Cell::new(false),
            entries: RefCell::new(Vec::new()),
            loaded_icons: RefCell::new(HashSet::new()),
            flavor: Cell::new(config.theme),
            font: RefCell::new(config.font.clone()),
            config: config.clone(),
        }));
        let z_stack = ZOrderStack::new();
        let next_float_overlay_id = Rc::new(Cell::new(9000_isize));
        let next_tiling_overlay_id = Rc::new(Cell::new(9900_isize));
        let tiling_focus_target = focus_target.clone();
        let tiling_overlay_ids: Rc<RefCell<HashSet<HwndId>>> =
            Rc::new(RefCell::new(HashSet::new()));
        let tab_bars = Rc::new(RefCell::new(HashMap::new()));
        let overlays = Rc::new(RefCell::new(MockOverlays {
            tiling_overlays: HashMap::new(),
            next_tiling_overlay_id: next_tiling_overlay_id.clone(),
            tiling_focus_target: tiling_focus_target.clone(),
            tiling_overlay_ids: tiling_overlay_ids.clone(),
            float_overlays: HashMap::new(),
            z_stack: z_stack.clone(),
            next_float_overlay_id: next_float_overlay_id.clone(),
            tab_bars: tab_bars.clone(),
        }));

        let dome = Dome::new(
            config.clone(),
            layout.workspace,
            Rc::new(NoopTaskbar),
            Box::new(overlays.clone()),
            Box::new(display),
            Box::new(picker.clone()),
        )
        .unwrap();
        Self {
            dome,
            moves: Arc::new(Mutex::new(Vec::new())),
            mocks: HashMap::new(),
            monitors: shared_monitors,
            exclusive_fullscreen_hwnd,
            config,
            overlays,
            picker,
            tab_bars,
            z_stack,
            focus_target,
        }
    }

    fn open(&mut self, id: isize, title: &str, process: &str, dim: Dimension<Physical>) -> HwndId {
        let ext = Arc::new(
            MockExternalHwnd::with_title(
                id,
                title,
                process,
                self.moves.clone(),
                self.z_stack.clone(),
                self.focus_target.clone(),
            )
            .with_dimension(dim),
        );
        self.open_with(ext)
    }

    /// Mirrors the runner's window-creation pipeline:
    /// the inspection step (worker thread) gates on `check_unmanageable`, and only
    /// manageable windows reach `Dome::add_window`. Unmanageable mocks are
    /// only registered for `env.dim` lookup so tests can inspect their
    /// untouched dimension.
    fn open_with(&mut self, ext: Arc<MockExternalHwnd>) -> HwndId {
        let hwnd_id = ext.hwnd_id;
        self.mocks.insert(hwnd_id, ext.clone());
        if !ext.manageable {
            return hwnd_id;
        }
        let new = NewWindow {
            ext: ext.clone(),
            metadata: WindowsMetadata {
                title: ext.title.clone(),
                process: ext.process.clone(),
                class: ext.class.clone(),
                aumid: None,
                app_name: ext.app_name.clone(),
            },
            constraints: (
                ext.min_size.0,
                ext.min_size.1,
                ext.max_size.0,
                ext.max_size.1,
            ),
        };
        let dim = ext.get_dim();
        self.dome.add_window(new, dim, 1);
        hwnd_id
    }

    fn settle(&mut self, limit: usize) {
        for _ in 0..limit {
            if !self.flush_moves() {
                return;
            }
        }
        let remaining = self.moves.lock().unwrap().len();
        if remaining > 0 {
            panic!("settle did not converge after {limit} iterations ({remaining} moves pending)");
        }
    }

    fn monitor_for_pos(&self, x: Length, y: Length) -> isize {
        let monitors = self.monitors.lock().unwrap();
        monitors
            .iter()
            .find(|m| {
                let d = m.dimension;
                x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height
            })
            .map(|m| m.handle)
            .unwrap_or(1)
    }

    fn flush_moves(&mut self) -> bool {
        let pending = std::mem::take(&mut *self.moves.lock().unwrap());
        if pending.is_empty() {
            return false;
        }
        let mut last_pos: HashMap<HwndId, Dimension> = HashMap::new();
        for (id, dim) in pending {
            last_pos.insert(id, dim);
        }
        for (hwnd_id, dim) in last_pos {
            self.dome.clear_move_state(hwnd_id);
            let minimized = self
                .mocks
                .get(&hwnd_id)
                .is_some_and(|m| m.minimized.load(Ordering::Relaxed));
            if minimized {
                // Mirrors production: the placement-read closure early-
                // returns when `IsIconic` reports true, so `window_moved`
                // never sees an iconic observation.
                continue;
            }
            let monitor = self.monitor_for_pos(dim.x, dim.y);
            self.dome
                .handle_window_moved(hwnd_id, dim, monitor, Instant::now());
        }
        self.dome.apply_layout();
        true
    }

    /// Configure a window to resist repositioning and report it at `pos`.
    fn simulate_resist(&self, hwnd: HwndId, pos: (i32, i32, i32, i32)) {
        let dim = Dimension::new(
            Length::new(pos.0 as f32),
            Length::new(pos.1 as f32),
            Length::new(pos.2 as f32),
            Length::new(pos.3 as f32),
        );
        let ext = self.mock(hwnd);
        ext.set_override_position(Some(pos));
        *ext.dimension.lock().unwrap() = dim;
        self.moves.lock().unwrap().push((hwnd, dim));
    }

    fn destroy_window(&mut self, hwnd: HwndId) {
        self.mocks.remove(&hwnd);
        self.dome.window_destroyed(hwnd);
        self.z_stack.remove(hwnd);
        self.dome.apply_layout();
    }

    fn minimize_window(&mut self, hwnd: HwndId) {
        self.mock(hwnd).minimized.store(true, Ordering::Relaxed);
        self.dome.window_minimized(hwnd);
        self.dome.apply_layout();
    }

    fn unminimize_window(&mut self, hwnd: HwndId) {
        // Production observes a restore as a move through the placement-read
        // pipeline, so we must drive a move event here (not just toggle a flag).
        //
        // Flag clear before move: `flush_moves`'s iconic guard drops moves for
        // minimized windows. Matches OS ordering (MINIMIZEEND before LOCATIONCHANGE).
        self.mock(hwnd).minimized.store(false, Ordering::Relaxed);
        let dim = self.mock(hwnd).get_dim();
        self.moves.lock().unwrap().push((hwnd, dim));
        self.flush_moves();
    }

    fn focus_window(&mut self, hwnd: HwndId) {
        self.dome.handle_focus(hwnd);
        self.dome.apply_layout();
    }

    fn mock(&self, hwnd: HwndId) -> &MockExternalHwnd {
        self.mocks.get(&hwnd).unwrap_or_else(|| {
            panic!("window {hwnd:?} is not registered (destroyed or never opened?)")
        })
    }

    fn dim(&self, hwnd: HwndId) -> Dimension {
        self.mock(hwnd).get_dim()
    }

    fn move_window_to(&self, hwnd: HwndId, dim: Dimension) {
        *self.mock(hwnd).dimension.lock().unwrap() = dim;
        self.moves.lock().unwrap().push((hwnd, dim));
    }

    fn is_minimized(&self, hwnd: HwndId) -> bool {
        self.mock(hwnd).minimized.load(Ordering::Relaxed)
    }

    fn is_offscreen(&self, hwnd: HwndId) -> bool {
        self.mock(hwnd).is_offscreen()
    }

    fn is_topmost(&self, hwnd: HwndId) -> bool {
        self.z_stack.is_topmost(hwnd)
    }

    /// A window is "at the bottom" iff it sits in the combined z-order below
    /// every other displayed (non-offscreen) managed mock AND below the tiling
    /// overlay. The overlay-above invariant matters because Win32's close-time
    /// focus walk descends the z-order. If a parked window from another
    /// workspace sat above the overlay, that workspace would activate on close
    /// (see docs/architecture.md, "Virtual workspaces"). Vacuously true on the
    /// displayed-peers leg when no displayed peer exists.
    fn is_bottom(&self, hwnd: HwndId) -> bool {
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
        let overlay_ids = self.overlays.borrow().tiling_overlay_ids.borrow().clone();
        overlay_ids.iter().all(
            |&overlay_id| match stack.iter().position(|&h| h == overlay_id) {
                Some(overlay_idx) => overlay_idx < idx,
                None => true,
            },
        )
    }

    fn clear_override_position(&self, hwnd: HwndId) {
        self.mock(hwnd).set_override_position(None);
    }

    fn run_actions(&mut self, s: &str) {
        let action: Action = s.parse().unwrap();
        match &action {
            Action::Focus(t) => self.dome.apply_focus(t),
            Action::Move(t) => self.dome.apply_move(t),
            Action::Toggle(t) => self.dome.apply_toggle(t),
            Action::Master(t) => self.dome.apply_master(t),
            Action::ToggleMinimized => self.dome.toggle_picker(),
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

    fn focus_target(&self) -> FocusTarget {
        *self.focus_target.lock().unwrap()
    }

    fn tiling_overlays(&self) -> Vec<TilingOverlaySnapshot> {
        let o = self.overlays.borrow();
        o.tiling_overlays
            .values()
            .map(|ov| TilingOverlaySnapshot {
                overlay_id: ov.overlay_id,
                state: ov.state(),
                flavor: ov.flavor(),
                font: ov.font(),
            })
            .collect()
    }

    fn float_overlays(&self) -> Vec<FloatOverlaySnapshot> {
        let o = self.overlays.borrow();
        o.float_overlays
            .iter()
            .filter(|(_, shared)| !shared.stale.get())
            .map(|(_, shared)| FloatOverlaySnapshot {
                overlay_id: shared.overlay_id,
                state: shared.state.get(),
                flavor: shared.flavor.get(),
                font: shared.font.borrow().clone(),
            })
            .collect()
    }

    fn picker_flavor(&self) -> Flavor {
        self.picker.borrow().flavor.get()
    }

    fn picker_font(&self) -> FontConfig {
        self.picker.borrow().font.borrow().clone()
    }

    fn picker_loaded_icons(&self) -> HashSet<String> {
        self.picker.borrow().loaded_icons.borrow().clone()
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

    fn add_monitor(&mut self, monitor: MonitorInfo) {
        self.monitors.lock().unwrap().push(monitor);
        self.dome.handle_display_change();
        self.dome.apply_layout();
    }

    fn z_order(&self) -> Vec<HwndId> {
        self.z_stack.stack()
    }

    fn tiling_z_order(&self) -> Vec<HwndId> {
        self.z_stack.normal_stack()
    }
}

fn fullscreen_dim() -> Dimension {
    Dimension::new(Length::ZERO, Length::ZERO, SCREEN_WIDTH, SCREEN_HEIGHT)
}

type MoveLog = Arc<Mutex<Vec<(HwndId, Dimension)>>>;

struct ZOrderBands {
    topmost: Vec<HwndId>,
    normal: Vec<HwndId>,
}

/// Emulates Win32's z-order stack for test assertions. Tracks the relative
/// ordering of windows as `set_position` and `move_offscreen` calls arrive.
#[derive(Clone)]
struct ZOrderStack {
    bands: Arc<Mutex<ZOrderBands>>,
}

impl ZOrderStack {
    fn new() -> Self {
        Self {
            bands: Arc::new(Mutex::new(ZOrderBands {
                topmost: Vec::new(),
                normal: Vec::new(),
            })),
        }
    }

    fn apply(&self, hwnd: HwndId, z: ZOrder) {
        let mut bands = self.bands.lock().unwrap();

        // Record original position for Unchanged
        let orig_topmost_pos = bands.topmost.iter().position(|&id| id == hwnd);
        let orig_normal_pos = bands.normal.iter().position(|&id| id == hwnd);

        match z {
            // Win32 self-reference (SetWindowPos(hwnd, hwnd, ...)) is a no-op.
            ZOrder::After(other) if other == hwnd => return,
            _ => {}
        }

        // Remove from both lists
        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);

        match z {
            ZOrder::After(other) => {
                if let Some(pos) = bands.topmost.iter().position(|&id| id == other) {
                    bands.topmost.insert(pos + 1, hwnd);
                } else if let Some(pos) = bands.normal.iter().position(|&id| id == other) {
                    bands.normal.insert(pos + 1, hwnd);
                } else {
                    bands.normal.push(hwnd);
                }
            }
            ZOrder::Topmost => {
                bands.topmost.insert(0, hwnd);
            }
            ZOrder::NotTopmost => {
                // HWND_NOTOPMOST: remove from topmost band, prepend to normal
                // band only if not already there (already removed above).
                let clamped = orig_normal_pos.unwrap_or(0).min(bands.normal.len());
                bands.normal.insert(clamped, hwnd);
            }
            ZOrder::Unchanged => {
                // Re-insert at original position (clamped to list length)
                if let Some(pos) = orig_topmost_pos {
                    let clamped = pos.min(bands.topmost.len());
                    bands.topmost.insert(clamped, hwnd);
                } else if let Some(pos) = orig_normal_pos {
                    let clamped = pos.min(bands.normal.len());
                    bands.normal.insert(clamped, hwnd);
                } else {
                    bands.normal.push(hwnd);
                }
            }
        }
    }

    fn move_to_bottom(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);
        bands.normal.push(hwnd);
    }

    /// Returns the full z-order stack from top to bottom: topmost band first, then normal.
    fn stack(&self) -> Vec<HwndId> {
        let bands = self.bands.lock().unwrap();
        let mut result = bands.topmost.clone();
        result.extend_from_slice(&bands.normal);
        result
    }

    fn normal_stack(&self) -> Vec<HwndId> {
        self.bands.lock().unwrap().normal.clone()
    }

    fn is_topmost(&self, hwnd: HwndId) -> bool {
        self.bands.lock().unwrap().topmost.contains(&hwnd)
    }

    /// Returns the HWND sitting directly above `hwnd` in the combined z-order
    /// stack (topmost band first, then normal). Mirrors `GetWindow(GW_HWNDPREV)`.
    fn window_above(&self, hwnd: HwndId) -> Option<HwndId> {
        let bands = self.bands.lock().unwrap();
        let combined: Vec<HwndId> = bands
            .topmost
            .iter()
            .chain(bands.normal.iter())
            .copied()
            .collect();
        let idx = combined.iter().position(|&h| h == hwnd)?;
        if idx == 0 {
            None
        } else {
            Some(combined[idx - 1])
        }
    }

    /// Removes a window from both z-order bands. Mirrors Win32 `DestroyWindow`.
    fn remove(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.topmost.retain(|&id| id != hwnd);
        bands.normal.retain(|&id| id != hwnd);
    }

    /// Simulate CreateWindowExW: place a freshly-created HWND at the top of
    /// the normal z-order band. Models the OS-side birth event; the tiling
    /// overlay's explicit drop-to-bottom park is applied separately by the
    /// caller via `move_to_bottom`.
    fn simulate_create(&self, hwnd: HwndId) {
        let mut bands = self.bands.lock().unwrap();
        bands.normal.retain(|&id| id != hwnd);
        bands.normal.insert(0, hwnd);
    }
}

struct MockExternalHwnd {
    hwnd_id: HwndId,
    manageable: bool,
    title: Option<String>,
    process: String,
    class: Option<String>,
    app_name: Option<String>,
    dimension: Mutex<Dimension>,
    override_position: Mutex<Option<(i32, i32, i32, i32)>>,
    minimized: AtomicBool,
    min_size: (f32, f32),
    max_size: (f32, f32),
    z_stack: ZOrderStack,
    moves: MoveLog,
    focus_target: Arc<Mutex<FocusTarget>>,
}

impl MockExternalHwnd {
    fn with_title(
        id: isize,
        title: &str,
        process: &str,
        moves: MoveLog,
        z_stack: ZOrderStack,
        focus_target: Arc<Mutex<FocusTarget>>,
    ) -> Self {
        let hwnd_id = HwndId::test(id);
        z_stack.simulate_create(hwnd_id);
        Self {
            hwnd_id,
            manageable: true,
            title: Some(title.to_string()),
            process: process.to_string(),
            class: None,
            app_name: None,
            dimension: Mutex::new(Dimension::new(
                Length::ZERO,
                Length::ZERO,
                Length::new(800.0),
                Length::new(600.0),
            )),
            override_position: Mutex::new(None),
            minimized: AtomicBool::new(false),
            min_size: (0.0, 0.0),
            max_size: (0.0, 0.0),
            z_stack,
            moves,
            focus_target,
        }
    }

    fn with_manageable(mut self, manageable: bool) -> Self {
        self.manageable = manageable;
        self
    }

    fn with_class(mut self, class: &str) -> Self {
        self.class = Some(class.to_string());
        self
    }

    fn with_dimension(self, dim: Dimension) -> Self {
        *self.dimension.lock().unwrap() = dim;
        self
    }

    fn set_override_position(&self, pos: Option<(i32, i32, i32, i32)>) {
        *self.override_position.lock().unwrap() = pos;
    }

    fn get_dim(&self) -> Dimension {
        *self.dimension.lock().unwrap()
    }

    fn is_offscreen(&self) -> bool {
        let dim = self.get_dim();
        dim.x <= OFFSCREEN_POS || dim.y <= OFFSCREEN_POS
    }
}

impl ManageExternalWindow for MockExternalHwnd {
    fn id(&self) -> HwndId {
        self.hwnd_id
    }

    fn pid(&self) -> u32 {
        // Tests do not exercise pid plumbing yet; return a deterministic
        // sentinel derived from the hwnd so log output stays stable.
        1
    }

    fn set_position(&self, z: ZOrder, dim: Dimension) {
        self.minimized.store(false, Ordering::Relaxed);
        let dim = self.override_position.lock().unwrap().map_or(dim, |pos| {
            Dimension::new(
                Length::new(pos.0 as f32),
                Length::new(pos.1 as f32),
                Length::new(pos.2 as f32),
                Length::new(pos.3 as f32),
            )
        });
        *self.dimension.lock().unwrap() = dim;
        self.z_stack.apply(self.hwnd_id, z);
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
        self.z_stack.move_to_bottom(self.hwnd_id);
        self.moves.lock().unwrap().push((self.hwnd_id, dim));
    }

    fn show_cmd(&self, cmd: ShowCmd) {
        match cmd {
            ShowCmd::Minimize => {
                // Production: SW_MINIMIZE flips IsIconic and parks the window
                // at the iconic-cache rect. Tests never observe the iconic
                // rect (the placement-read closure early-returns on IsIconic
                // before reading), so the mock skips the rect overwrite
                // entirely. The move-log push exists to drive the
                // LOCATIONCHANGE replay in flush_moves; the value is dropped
                // by the iconic guard before reaching window_moved.
                self.minimized.store(true, Ordering::Relaxed);
                let dim = *self.dimension.lock().unwrap();
                self.moves.lock().unwrap().push((self.hwnd_id, dim));
            }
            ShowCmd::Restore => {
                self.minimized.store(false, Ordering::Relaxed);
            }
        }
    }

    fn set_foreground_window(&self) {
        *self.focus_target.lock().unwrap() = FocusTarget::Window(self.hwnd_id);
    }

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
        self.z_stack.remove(self.hwnd_id);
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

#[derive(Clone)]
struct MockFloatOverlay {
    overlay_id: HwndId,
    z_stack: ZOrderStack,
    shared: Rc<FloatOverlayShared>,
    overlays: Rc<RefCell<MockOverlays>>,
    config: Rc<RefCell<Config>>,
}

impl MockFloatOverlay {
    fn new(
        overlay_id: HwndId,
        z_stack: ZOrderStack,
        config: Config,
        overlays: Rc<RefCell<MockOverlays>>,
    ) -> Self {
        Self {
            overlay_id,
            z_stack,
            shared: Rc::new(FloatOverlayShared {
                overlay_id,
                stale: Cell::new(false),
                state: Cell::new(FloatOverlayState::Hidden),
                flavor: Cell::new(config.theme),
                font: RefCell::new(config.font.clone()),
            }),
            overlays,
            config: Rc::new(RefCell::new(config)),
        }
    }
}

impl FloatOverlayApi for MockFloatOverlay {
    fn update(
        &mut self,
        wp: &crate::core::FloatWindowPlacement,
        _: &Config,
        z_order: ZOrder,
        _scale: f32,
    ) {
        self.shared.state.set(FloatOverlayState::Visible {
            window_id: wp.id,
            visible_frame: wp.visible_frame,
            z_order,
        });
        self.overlays
            .borrow_mut()
            .float_overlays
            .insert(wp.id, self.shared.clone());
        self.z_stack.apply(self.overlay_id, z_order);
    }
    fn hide(&mut self) {
        self.shared.state.set(FloatOverlayState::Hidden);
        self.z_stack.remove(self.overlay_id);
    }
    fn set_config(&mut self, config: &Config) {
        self.shared.flavor.set(config.theme);
        *self.shared.font.borrow_mut() = config.font.clone();
        *self.config.borrow_mut() = config.clone();
    }
}

impl Drop for MockFloatOverlay {
    fn drop(&mut self) {
        self.shared.stale.set(true);
    }
}

/// `monitor` is shared (not just `Cell<Dimension>`) so the struct stays
/// cheaply `Clone`: the factory hands clones to the Hub while `TestEnv`
/// retains one for inspection.
#[derive(Clone)]
struct MockTilingOverlay {
    overlay_id: HwndId,
    z_stack: ZOrderStack,
    state: Rc<RefCell<TilingOverlayState>>,
    flavor: Rc<Cell<Flavor>>,
    font: Rc<RefCell<FontConfig>>,
    monitor: Rc<Cell<Dimension>>,
    config: Rc<RefCell<Config>>,
    focus_target: Arc<Mutex<FocusTarget>>,
}

impl MockTilingOverlay {
    fn new(
        overlay_id: HwndId,
        z_stack: ZOrderStack,
        config: Config,
        focus_target: Arc<Mutex<FocusTarget>>,
    ) -> Self {
        Self {
            overlay_id,
            z_stack,
            state: Rc::new(RefCell::new(TilingOverlayState::Hidden)),
            flavor: Rc::new(Cell::new(config.theme)),
            font: Rc::new(RefCell::new(config.font.clone())),
            monitor: Rc::new(Cell::new(Dimension::default())),
            config: Rc::new(RefCell::new(config)),
            focus_target,
        }
    }

    fn state(&self) -> TilingOverlayState {
        self.state.borrow().clone()
    }

    fn flavor(&self) -> Flavor {
        self.flavor.get()
    }

    fn font(&self) -> FontConfig {
        self.font.borrow().clone()
    }
}

impl TilingOverlayApi for MockTilingOverlay {
    fn update(
        &mut self,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        _containers: &[(ContainerPlacement, Vec<String>)],
        _scale: f32,
    ) {
        if self.monitor.get() != monitor {
            self.monitor.set(monitor);
            // Monitor-change branch: mirror production's HWND_BOTTOM park.
            self.z_stack.move_to_bottom(self.overlay_id);
        }
        // Same-monitor path: no z-order call. Matches production behavior
        // where show_tiling's per-window lift maintains the invariant.
        *self.state.borrow_mut() = TilingOverlayState::Visible {
            windows: windows.to_vec(),
        };
    }
    fn clear(&mut self) {
        *self.state.borrow_mut() = TilingOverlayState::Hidden;
    }
    fn set_config(&mut self, config: &Config) {
        self.flavor.set(config.theme);
        *self.font.borrow_mut() = config.font.clone();
        *self.config.borrow_mut() = config.clone();
    }
    fn set_tab_bar_height(&mut self, _height: Length<Logical>) {}
    fn window_above(&self) -> Option<HwndId> {
        self.z_stack.window_above(self.overlay_id)
    }
    fn demote_below(&mut self, managed: HwndId) {
        self.z_stack.apply(self.overlay_id, ZOrder::After(managed));
    }
    fn focus(&self) {
        *self.focus_target.lock().unwrap() = FocusTarget::Overlay;
    }
}

struct MockPicker {
    visible: Cell<bool>,
    entries: RefCell<Vec<PickerEntry>>,
    loaded_icons: RefCell<HashSet<String>>,
    flavor: Cell<Flavor>,
    font: RefCell<FontConfig>,
    config: Config,
}

impl PickerApi for Rc<RefCell<MockPicker>> {
    fn show(&mut self, entries: Vec<PickerEntry>, _monitor_dim: Dimension, _scale: f32) {
        let this = self.borrow_mut();
        *this.entries.borrow_mut() = entries;
        this.visible.set(true);
    }

    fn hide(&mut self) {
        self.borrow().visible.set(false);
    }

    fn is_visible(&self) -> bool {
        self.borrow().visible.get()
    }

    fn icons_to_load(
        &mut self,
        lookup_hwnd: &dyn Fn(crate::core::WindowId) -> Option<HwndId>,
    ) -> Vec<(String, HwndId)> {
        let this = self.borrow_mut();
        let entries = this.entries.borrow();
        let mut loaded = this.loaded_icons.borrow_mut();
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
        self.borrow_mut().loaded_icons.borrow_mut().insert(app_id);
    }

    fn rerender(&mut self) {}

    fn set_config(&mut self, config: &Config) {
        let mut this = self.borrow_mut();
        this.flavor.set(config.theme);
        *this.font.borrow_mut() = config.font.clone();
        this.config = config.clone();
    }
}

#[derive(Clone, Debug)]
struct TabBarUpdate {
    titles: Vec<String>,
    active_index: usize,
}

/// Test mirror of a per-`ContainerId` tab bar. The Rcs are shared with the
/// `MockTabBarHandle` clone that production wraps in a `Box<dyn TabBarOverlayApi>`,
/// so updates from production are observable here.
#[derive(Clone)]
struct MockTabBarOverlay {
    container_id: ContainerId,
    last_update: Rc<RefCell<Option<TabBarUpdate>>>,
}

impl MockTabBarOverlay {
    fn new(container_id: ContainerId) -> Self {
        Self {
            container_id,
            last_update: Rc::new(RefCell::new(None)),
        }
    }

    fn last_update(&self) -> Option<TabBarUpdate> {
        self.last_update.borrow().clone()
    }
}

/// Wrapper boxed as `Box<dyn TabBarOverlayApi>` and handed to `Dome`. Holding
/// the wrapper rather than `MockTabBarOverlay` directly lets us mirror
/// production's `position_windows` retain pass: when `Dome` drops the box,
/// the entry vanishes from the test mirror map. Putting `Drop` on
/// `MockTabBarOverlay` would re-enter `borrow_mut` on the same RefMut held
/// open by `HashMap::remove` and panic.
struct MockTabBarHandle {
    inner: MockTabBarOverlay,
    map: Rc<RefCell<HashMap<ContainerId, MockTabBarOverlay>>>,
}

impl TabBarOverlayApi for MockTabBarHandle {
    fn update(
        &mut self,
        _rect: Dimension,
        titles: Vec<String>,
        active_index: usize,
        _is_highlighted: bool,
        _scale: f32,
    ) {
        *self.inner.last_update.borrow_mut() = Some(TabBarUpdate {
            titles,
            active_index,
        });
    }
    fn hide(&mut self) {}
    fn set_config(&mut self, _config: &Config) {}
}

impl Drop for MockTabBarHandle {
    fn drop(&mut self) {
        self.map.borrow_mut().remove(&self.inner.container_id);
    }
}

struct MockOverlays {
    tiling_overlays: HashMap<HwndId, MockTilingOverlay>,
    next_tiling_overlay_id: Rc<Cell<isize>>,
    tiling_focus_target: Arc<Mutex<FocusTarget>>,
    tiling_overlay_ids: Rc<RefCell<HashSet<HwndId>>>,
    float_overlays: HashMap<WindowId, Rc<FloatOverlayShared>>,
    z_stack: ZOrderStack,
    next_float_overlay_id: Rc<Cell<isize>>,
    tab_bars: Rc<RefCell<HashMap<ContainerId, MockTabBarOverlay>>>,
}

impl CreateOverlay for Rc<RefCell<MockOverlays>> {
    fn create_tiling_overlay(
        &self,
        config: Config,
        _tab_bar_height: Length<Logical>,
        monitor: Dimension,
        _scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        let this = self.borrow();
        let id_val = this.next_tiling_overlay_id.get();
        this.next_tiling_overlay_id.set(id_val + 1);
        let id = HwndId::test(id_val);

        let overlay = MockTilingOverlay::new(
            id,
            this.z_stack.clone(),
            config.clone(),
            this.tiling_focus_target.clone(),
        );
        // Record monitor dimension from create call (also updated on
        // subsequent `update` calls).
        overlay.monitor.set(monitor);
        // Mirror production: CreateWindowExW seeds at top of normal band,
        // then explicitly drop to HWND_BOTTOM.
        this.z_stack.simulate_create(id);
        this.z_stack.move_to_bottom(id);
        this.tiling_overlay_ids.borrow_mut().insert(id);
        drop(this);
        self.borrow_mut()
            .tiling_overlays
            .insert(id, overlay.clone());
        Ok(Box::new(overlay))
    }
    fn create_float_overlay(
        &self,
        config: Config,
        _scale: f32,
        _visible_frame: Dimension,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        let this = self.borrow();
        let id_val = this.next_float_overlay_id.get();
        drop(this);
        self.borrow_mut().next_float_overlay_id.set(id_val + 1);
        let id = HwndId::test(id_val);

        let z_stack = self.borrow().z_stack.clone();
        let overlays = self.clone();
        let overlay = MockFloatOverlay::new(id, z_stack, config, overlays);

        // Mirror CreateWindowExW: seed at top of normal band. The first
        // `update()` call will reposition (typically `ZOrder::After(float_window)`).
        self.borrow().z_stack.simulate_create(id);
        Ok(Box::new(overlay))
    }
    fn create_tab_bar(
        &self,
        _config: Config,
        container_id: ContainerId,
        _rect: Dimension,
        _scale: f32,
    ) -> anyhow::Result<Box<dyn TabBarOverlayApi>> {
        let this = self.borrow();
        let inner = MockTabBarOverlay::new(container_id);
        this.tab_bars
            .borrow_mut()
            .insert(container_id, inner.clone());
        Ok(Box::new(MockTabBarHandle {
            inner,
            map: this.tab_bars.clone(),
        }))
    }
}

fn setup_logger() {
    use tracing_subscriber::EnvFilter;
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let _ = tracing_subscriber::fmt().with_env_filter(filter).try_init();
}
