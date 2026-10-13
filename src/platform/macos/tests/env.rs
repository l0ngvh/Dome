use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, mpsc};
use std::time::{Duration, Instant};

use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;

use crate::action::{Action, Actions};
use crate::config::lua::test_support::{TempFile, loaded_runtime};
use crate::config::{Appearance, PreferredLayouts};
use crate::core::{Dimension, Hub, Length, Logical, MonitorId, PixelRect, TilingConfig, WindowId};
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::platform::macos::corner_radius::FALLBACK_CORNER_RADIUS;
use crate::platform::macos::dome::events::{
    DiscoveredWindow, FocusTarget, FullscreenKind, HubEvent, HubMessage, MacOsMetadata,
    MonitorScene, Scene,
};
use crate::platform::macos::dome::{Dome, SceneSender};
use crate::platform::macos::focus::SharedFocus;
use crate::platform::macos::runner::apply_report;
use crate::platform::macos::ui::internal_windows::{
    ContainerShow, FloatShow, InternalWindows, MonitorTilingData,
};
use crate::platform::macos::ui::{CaptureMessage, UiState, WindowLoopCallbacks, WindowLoopHandler};
use crate::platform::macos::window_server::{
    ApplicationLifetime, DebounceBurst, ExistingWindow, Observation, PositionReadOrigin,
    RecoveryEntry, WindowServer,
};
use crate::platform::macos::{MonitorInfo, Wake};

use super::{SCREEN_HEIGHT, SCREEN_WIDTH, baseline_tiling, default_monitor};

type MoveLog = Rc<RefCell<Vec<(CGWindowID, i32, i32, i32, i32)>>>;
type EffectLog = Rc<RefCell<Vec<String>>>;

struct WindowFacts {
    pid: i32,
    app: String,
    title: String,
    frame: Cell<PixelRect>,
    native_fullscreen: Cell<bool>,
    minimized: Cell<bool>,
    min_size: Cell<Option<(i32, i32)>>,
    max_size: Cell<Option<(i32, i32)>>,
    override_frame: Cell<Option<(i32, i32, i32, i32)>>,
    corner_radius: Cell<Length>,
}

struct MockWindowServer {
    facts: Rc<RefCell<HashMap<CGWindowID, Rc<WindowFacts>>>>,
    observations: Rc<RefCell<Vec<Observation>>>,
    moves: MoveLog,
    effects: EffectLog,
}

impl MockWindowServer {
    fn write(&mut self, key: CGWindowID, frame: PixelRect, resize: bool) {
        let facts = self.facts.borrow();
        let facts = &facts[&key];
        let frame = if let Some((x, y, width, height)) = facts.override_frame.get() {
            PixelRect::new(x, y, width, height)
        } else if resize {
            let mut width = frame.width().value();
            let mut height = frame.height().value();
            if let Some((w, h)) = facts.min_size.get() {
                width = width.max(w);
                height = height.max(h);
            }
            if let Some((w, h)) = facts.max_size.get() {
                width = width.min(w);
                height = height.min(h);
            }
            PixelRect::new(frame.x().value(), frame.y().value(), width, height)
        } else {
            frame
        };
        facts.frame.set(frame);
        self.moves.borrow_mut().push((
            key,
            frame.x().value(),
            frame.y().value(),
            frame.width().value(),
            frame.height().value(),
        ));
    }
}

impl WindowServer for MockWindowServer {
    fn ingest_native_events(&mut self, _: Instant) {
        panic!("behavioral server must not poll native events")
    }
    fn ingest_read_results(&mut self, _: Instant) {
        panic!("behavioral server must not poll native results")
    }
    fn has_observations(&self) -> bool {
        !self.observations.borrow().is_empty()
    }
    fn take_observations(&mut self) -> Vec<Observation> {
        std::mem::take(&mut *self.observations.borrow_mut())
    }
    fn next_deadline(&self) -> Option<Instant> {
        panic!("behavioral server has no native deadline")
    }
    fn refresh_applications(&mut self, _: Instant) {
        panic!("behavioral server must not enumerate applications")
    }
    fn set_frame(&mut self, key: CGWindowID, frame: PixelRect) {
        self.effects.borrow_mut().push(format!("frame {key}"));
        self.write(key, frame, true);
    }
    fn move_offscreen(&mut self, key: CGWindowID, position: (Length, Length)) {
        self.effects.borrow_mut().push(format!("park {key}"));
        let frame = self.facts.borrow()[&key].frame.get();
        self.write(
            key,
            PixelRect::new(
                position.0.value() as i32,
                position.1.value() as i32,
                frame.width().value(),
                frame.height().value(),
            ),
            false,
        );
    }
    fn minimize(&mut self, key: CGWindowID) {
        self.effects.borrow_mut().push(format!("minimize {key}"));
        self.facts.borrow()[&key].minimized.set(true);
    }
    fn unminimize(&mut self, key: CGWindowID) {
        self.effects.borrow_mut().push(format!("unminimize {key}"));
        self.facts.borrow()[&key].minimized.set(false);
    }
    fn close(&mut self, key: CGWindowID) {
        self.effects.borrow_mut().push(format!("close {key}"));
    }
    fn focus(&mut self, key: CGWindowID) {
        self.effects.borrow_mut().push(format!("focus {key}"));
    }
    fn recover(&mut self, _: CGWindowID, _: PixelRect) {
        panic!("recovery must use the shutdown handoff")
    }
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        self.effects
            .borrow_mut()
            .push(format!("shutdown {}", recovery.len()));
    }
}

struct MockInternalWindows {
    painted: Rc<RefCell<SceneState>>,
    effects: EffectLog,
}

impl InternalWindows for MockInternalWindows {
    fn set_monitors(&mut self, _: &[MonitorScene]) {
        let mut painted = self.painted.borrow_mut();
        painted.monitor_dims.clear();
        painted.tiling_corner_radii.clear();
        self.effects.borrow_mut().push("monitors".into());
    }
    fn show_tiling(&mut self, tiling: &MonitorTilingData) {
        let mut painted = self.painted.borrow_mut();
        painted.monitor_dims.push(tiling.monitor_dim);
        painted.tiling_corner_radii.extend(
            tiling
                .windows
                .iter()
                .map(|show| (show.placement.id, show.corner_radius)),
        );
        self.effects.borrow_mut().push("tiling".into());
    }
    fn show_floats(&mut self, floats: &[FloatShow], _: Option<CGWindowID>) {
        self.painted.borrow_mut().floats = floats
            .iter()
            .map(|show| {
                (
                    show.cg_id,
                    FloatSnapshot {
                        outer_frame: show.placement.border_box.to_dimension(),
                        content_dim: show.content_dim,
                        corner_radius: show.corner_radius,
                        cocoa_frame: show.cocoa_frame,
                    },
                )
            })
            .collect();
        self.effects.borrow_mut().push("floats".into());
    }
    fn show_tab_bars(&mut self, _: &[ContainerShow]) {
        self.effects.borrow_mut().push("tabs".into());
    }
    fn apply_capture_messages(&mut self, _: Vec<CaptureMessage>) {
        self.effects.borrow_mut().push("capture".into());
    }
    fn focus_tiling_overlay(&mut self, monitor: MonitorId) {
        self.painted.borrow_mut().focused_monitor_id = Some(monitor);
        self.effects.borrow_mut().push("overlay focus".into());
    }
    fn set_appearance(&mut self, _: &Appearance) {
        self.effects.borrow_mut().push("appearance".into());
    }
}

struct MockSceneSender(mpsc::Sender<HubMessage>);
impl SceneSender for MockSceneSender {
    fn send(&self, message: HubMessage) {
        self.0.send(message).unwrap();
    }
}

pub(super) struct TestEnv {
    dome: Option<Dome>,
    handler: Option<WindowLoopHandler>,
    windows: Rc<RefCell<HashMap<CGWindowID, Rc<WindowFacts>>>>,
    lifetimes: HashMap<i32, Arc<ApplicationLifetime>>,
    observations: Rc<RefCell<Vec<Observation>>>,
    moves: MoveLog,
    effects: EffectLog,
    next_key: CGWindowID,
    painted: Rc<RefCell<SceneState>>,
    domain_sender: mpsc::Sender<HubMessage>,
    domain_output: mpsc::Receiver<HubMessage>,
    ui_sender: Option<mpsc::Sender<HubMessage>>,
    reports: Option<calloop::channel::Channel<HubEvent>>,
    wake_sender: mpsc::Sender<Wake>,
    wakes: mpsc::Receiver<Wake>,
    deadlines: Rc<RefCell<HashMap<Wake, Instant>>>,
    pub(super) focus: Arc<SharedFocus>,
    last_scene: Option<Scene>,
    display_clock: Option<(Instant, Instant)>,
    held_scenes: Vec<Scene>,
    hold_scenes: bool,
    now: Instant,
    drag_starts: HashMap<i32, Instant>,
    tiling: TilingConfig,
    config_file: Option<TempFile>,
}

pub(super) struct TestEnvBuilder<'env> {
    env: &'env mut TestEnv,
    tiling: TilingConfig,
    config_source: String,
}
impl TestEnvBuilder<'_> {
    pub(super) fn tiling(mut self, adjust: impl FnOnce(&mut TilingConfig)) -> Self {
        adjust(&mut self.tiling);
        self
    }
    pub(super) fn config_source(mut self, source: &str) -> Self {
        self.config_source = source.to_string();
        self
    }
    pub(super) fn build(self) {
        let Self {
            env,
            tiling,
            config_source,
        } = self;
        assert!(env.dome.is_none());
        let (keymap_sender, _keymap_receiver) = mpsc::channel();
        let keymap = KeymapPublisher::new(KeymapView::new(), keymap_sender);
        let (runtime, file) = loaded_runtime("macos_env", &config_source);
        env.dome = Some(Dome::new(
            &[default_monitor()],
            tiling.clone(),
            PreferredLayouts::default(),
            Box::new(MockSceneSender(env.domain_sender.clone())),
            runtime,
            keymap,
            HashMap::new(),
        ));
        env.tiling = tiling;
        env.config_file = Some(file);
        let (sender, intake) = mpsc::channel();
        let (_capture_sender, capture_intake) = mpsc::channel();
        let (hub, reports) = calloop::channel::channel();
        let ui = UiState::new(
            intake,
            hub,
            Box::new(MockInternalWindows {
                painted: env.painted.clone(),
                effects: env.effects.clone(),
            }),
            env.focus.clone(),
            capture_intake,
        );
        let server = MockWindowServer {
            facts: env.windows.clone(),
            observations: env.observations.clone(),
            moves: env.moves.clone(),
            effects: env.effects.clone(),
        };
        let callbacks = Rc::new(OnceCell::new());
        let wakes = env.wake_sender.clone();
        let deadlines = env.deadlines.clone();
        let stop = env.effects.clone();
        let terminate = env.effects.clone();
        assert!(
            callbacks
                .set(WindowLoopCallbacks {
                    wake: Box::new(move |wake| {
                        wakes.send(wake).unwrap();
                    }),
                    wake_at: Box::new(move |wake, at| {
                        if let Some(at) = at {
                            deadlines.borrow_mut().insert(wake, at);
                        } else {
                            deadlines.borrow_mut().remove(&wake);
                        }
                    }),
                    stop_wakes: Box::new(move || stop.borrow_mut().push("stop wakes".into())),
                    terminate: Box::new(move || terminate.borrow_mut().push("terminate".into())),
                })
                .is_ok()
        );
        env.handler = Some(WindowLoopHandler::new(ui, Box::new(server), callbacks));
        env.ui_sender = Some(sender);
        env.reports = Some(reports);
        env.layout();
    }
}

impl TestEnv {
    pub(super) fn new() -> Self {
        let (domain_sender, domain_output) = mpsc::channel();
        let (wake_sender, wakes) = mpsc::channel();
        let focus = Arc::new(SharedFocus::new());
        let focus_wakes = wake_sender.clone();
        focus.bind_waker(move || {
            focus_wakes.send(Wake::FocusRequest).unwrap();
        });
        Self {
            dome: None,
            handler: None,
            windows: Rc::new(RefCell::new(HashMap::new())),
            lifetimes: HashMap::new(),
            observations: Rc::new(RefCell::new(Vec::new())),
            moves: Rc::new(RefCell::new(Vec::new())),
            effects: Rc::new(RefCell::new(Vec::new())),
            next_key: 1,
            painted: Rc::new(RefCell::new(SceneState::default())),
            domain_sender,
            domain_output,
            ui_sender: None,
            reports: None,
            wake_sender,
            wakes,
            deadlines: Rc::new(RefCell::new(HashMap::new())),
            focus,
            last_scene: None,
            display_clock: None,
            held_scenes: Vec::new(),
            hold_scenes: false,
            now: Instant::now() + Duration::from_secs(20),
            drag_starts: HashMap::new(),
            tiling: baseline_tiling(),
            config_file: None,
        }
    }
    pub(super) fn now(&self) -> Instant {
        self.now
    }
    pub(super) fn advance(&mut self, duration: Duration) {
        self.now += duration;
    }
    pub(super) fn setup(&mut self) {
        self.builder().build();
    }
    pub(super) fn builder(&mut self) -> TestEnvBuilder<'_> {
        let tiling = self.tiling.clone();
        TestEnvBuilder {
            env: self,
            tiling,
            config_source: "return {}".into(),
        }
    }
    fn dome_mut(&mut self) -> &mut Dome {
        self.dome.as_mut().expect("build environment first")
    }
    pub(super) fn with_hub<R>(&mut self, change: impl FnOnce(&mut Hub) -> R) -> R {
        self.dome_mut().with_hub(change)
    }
    pub(super) fn spawn_window(&mut self, pid: i32, app: &str, title: &str) -> CGWindowID {
        let key = self.next_key;
        self.next_key += 1;
        self.lifetimes
            .entry(pid)
            .or_insert_with(|| Arc::new(ApplicationLifetime { pid }));
        self.windows.borrow_mut().insert(
            key,
            Rc::new(WindowFacts {
                pid,
                app: app.into(),
                title: title.into(),
                frame: Cell::new(PixelRect::new(0, 0, 800, 600)),
                native_fullscreen: Cell::new(false),
                minimized: Cell::new(false),
                min_size: Cell::new(None),
                max_size: Cell::new(None),
                override_frame: Cell::new(None),
                corner_radius: Cell::new(FALLBACK_CORNER_RADIUS),
            }),
        );
        key
    }
    #[expect(
        clippy::too_many_arguments,
        reason = "test helper passing geometry directly"
    )]
    pub(super) fn spawn_window_at(
        &mut self,
        pid: i32,
        app: &str,
        title: &str,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) -> CGWindowID {
        let key = self.spawn_window(pid, app, title);
        self.set_window_frame(key, PixelRect::new(x, y, width, height));
        key
    }
    fn window(&self, key: CGWindowID) -> Rc<WindowFacts> {
        self.windows.borrow()[&key].clone()
    }
    pub(super) fn window_frame(&self, key: CGWindowID) -> (i32, i32, i32, i32) {
        let frame = self.window(key).frame.get();
        (
            frame.x().value(),
            frame.y().value(),
            frame.width().value(),
            frame.height().value(),
        )
    }
    pub(super) fn is_offscreen(&self, key: CGWindowID) -> bool {
        let (x, y, _, _) = self.window_frame(key);
        x >= SCREEN_WIDTH.value() as i32 - 1 || y >= SCREEN_HEIGHT.value() as i32 - 1
    }
    pub(super) fn set_window_frame(&self, key: CGWindowID, frame: PixelRect) {
        self.window(key).frame.set(frame);
    }
    pub(super) fn set_min_size(&self, key: CGWindowID, width: i32, height: i32) {
        self.window(key).min_size.set(Some((width, height)));
    }
    pub(super) fn set_max_size(&self, key: CGWindowID, width: i32, height: i32) {
        self.window(key).max_size.set(Some((width, height)));
    }
    pub(super) fn set_override_frame(&self, key: CGWindowID, frame: Option<(i32, i32, i32, i32)>) {
        self.window(key).override_frame.set(frame);
    }
    pub(super) fn set_corner_radius(&self, key: CGWindowID, radius: Length) {
        self.window(key).corner_radius.set(radius);
    }
    pub(super) fn set_native_fullscreen(&self, key: CGWindowID, value: bool) {
        self.window(key).native_fullscreen.set(value);
    }
    pub(super) fn is_minimized(&self, key: CGWindowID) -> bool {
        self.window(key).minimized.get()
    }
    fn existing(&self, key: CGWindowID) -> ExistingWindow {
        let facts = self.window(key);
        ExistingWindow {
            key,
            frame: (!facts.minimized.get() && !facts.native_fullscreen.get())
                .then(|| facts.frame.get()),
            corner_radius: facts.corner_radius.get(),
            minimized: facts.minimized.get(),
            fullscreen: facts
                .native_fullscreen
                .get()
                .then_some(FullscreenKind::Native),
        }
    }
    pub(super) fn queue_discoveries(&mut self, keys: &[CGWindowID]) {
        for &key in keys {
            let facts = self.window(key);
            self.queue_observation(Observation::AppWindows {
                lifetime: self.lifetimes[&facts.pid].clone(),
                discovered: vec![DiscoveredWindow {
                    key,
                    process_id: facts.pid,
                    metadata: MacOsMetadata {
                        title: Some(facts.title.clone()),
                        app_name: Some(facts.app.clone()),
                        bundle_id: None,
                    },
                    frame: (!facts.native_fullscreen.get()).then(|| facts.frame.get()),
                    corner_radius: facts.corner_radius.get(),
                    minimized: facts.minimized.get(),
                    fullscreen: facts
                        .native_fullscreen
                        .get()
                        .then_some(FullscreenKind::Native),
                }],
                existing: Vec::new(),
                removed: Vec::new(),
            });
        }
    }
    pub(super) fn discover_windows(&mut self, keys: &[CGWindowID]) {
        self.queue_discoveries(keys);
        self.roundtrip();
    }
    pub(super) fn discover_native_fullscreen(&mut self, key: CGWindowID) {
        self.set_native_fullscreen(key, true);
        self.discover_windows(&[key]);
    }
    pub(super) fn queue_observation(&mut self, observation: Observation) {
        self.observations.borrow_mut().push(observation);
    }
    pub(super) fn queue_position(
        &mut self,
        key: CGWindowID,
        frame: PixelRect,
        origin: PositionReadOrigin,
    ) {
        let mut read = self.existing(key);
        read.frame = Some(frame);
        self.queue_observation(Observation::Settled {
            lifetime: self.lifetimes[&self.window(key).pid].clone(),
            origin,
            windows: vec![read],
        });
    }
    pub(super) fn set_minimized(&self, key: CGWindowID, minimized: bool) {
        self.window(key).minimized.set(minimized);
    }
    pub(super) fn queue_current_facts(&mut self, key: CGWindowID) {
        self.queue_observation(Observation::AppWindows {
            lifetime: self.lifetimes[&self.window(key).pid].clone(),
            discovered: Vec::new(),
            existing: vec![self.existing(key)],
            removed: Vec::new(),
        });
    }
    fn report_existing(&mut self, key: CGWindowID) {
        self.queue_current_facts(key);
        self.roundtrip();
    }
    pub(super) fn enter_native_fullscreen(&mut self, key: CGWindowID) {
        self.set_native_fullscreen(key, true);
        self.report_existing(key);
    }
    pub(super) fn exit_native_fullscreen(
        &mut self,
        key: CGWindowID,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        self.set_native_fullscreen(key, false);
        self.set_window_frame(key, PixelRect::new(x, y, width, height));
        self.report_existing(key);
    }
    pub(super) fn user_minimize(&mut self, key: CGWindowID) {
        self.window(key).minimized.set(true);
        self.report_existing(key);
    }
    pub(super) fn deminiaturize(&mut self, key: CGWindowID) {
        self.window(key).minimized.set(false);
        self.report_existing(key);
    }
    pub(super) fn focus_window(&mut self, key: CGWindowID) {
        let minimized = self.is_minimized(key);
        self.deliver_report(HubEvent::FocusReported(key));
        self.pump_scenes();
        self.ui_pass();
        if minimized {
            self.deminiaturize(key);
        }
    }
    pub(super) fn run_unminimize(&mut self, key: CGWindowID) {
        let id = self.window_id(key).unwrap();
        self.send_action(&Action::UnminimizeWindow { id });
        if !self.is_minimized(key) {
            self.deminiaturize(key);
        }
    }
    pub(super) fn simulate_external_move(
        &mut self,
        key: CGWindowID,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        self.set_window_frame(key, PixelRect::new(x, y, width, height));
        self.window(key).minimized.set(false);
        self.report_move(
            key,
            PixelRect::new(x, y, width, height),
            DebounceBurst {
                first: self.now,
                last: self.now,
            },
        );
    }
    pub(super) fn report_move(&mut self, key: CGWindowID, frame: PixelRect, burst: DebounceBurst) {
        self.queue_position(key, frame, PositionReadOrigin::Settle(burst));
        self.roundtrip();
    }
    pub(super) fn remove_windows(&mut self, keys: &[CGWindowID]) {
        for &key in keys {
            self.queue_observation(Observation::AppWindows {
                lifetime: self.lifetimes[&self.window(key).pid].clone(),
                discovered: Vec::new(),
                existing: Vec::new(),
                removed: vec![key],
            });
        }
        self.roundtrip();
    }
    pub(super) fn report_minimized(&mut self, keys: &[CGWindowID]) {
        for &key in keys {
            self.deliver_report(HubEvent::WindowMinimized(key));
        }
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn app_terminated(&mut self, pid: i32) {
        self.queue_observation(Observation::AppGone {
            lifetime: self.lifetimes[&pid].clone(),
        });
        self.roundtrip();
    }
    pub(super) fn change_monitors(&mut self, monitors: Vec<MonitorInfo>) {
        self.dome_mut().monitors_changed(monitors);
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn finish_monitor_settle(&mut self) {
        self.advance(Duration::from_secs(11));
    }
    pub(super) fn start_drag(&mut self, pid: i32) {
        self.drag_starts.insert(pid, self.now);
    }
    pub(super) fn end_drag(
        &mut self,
        pid: i32,
        key: CGWindowID,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
    ) {
        let first = self.drag_starts.remove(&pid).unwrap();
        let frame = PixelRect::new(x, y, width, height);
        self.set_window_frame(key, frame);
        self.report_move(
            key,
            frame,
            DebounceBurst {
                first,
                last: self.now,
            },
        );
    }
    pub(super) fn layout(&mut self) {
        self.dome_mut().flush_layout();
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn mirror_clicked(&mut self, key: CGWindowID) {
        self.dome_mut().mirror_clicked(key);
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn apply_layout_file(&mut self, path: &str) {
        self.dome_mut().apply_layout_file(path);
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn change_config(&mut self, source: &str) {
        self.config_file.as_ref().unwrap().rewrite(source);
        self.tiling = self.dome_mut().reload().expect("test config loads").tiling;
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn border(&self) -> f32 {
        Length::from_pixels(self.tiling.border_size).logical()
    }
    pub(super) fn last_scene_state(&self) -> SceneState {
        self.painted.borrow().clone()
    }
    pub(super) fn painted_work_area(&self, index: usize) -> Dimension {
        self.painted.borrow().monitor_dims[index]
    }
    pub(super) fn last_float_snapshot(&self, key: CGWindowID) -> Option<FloatSnapshot> {
        self.painted.borrow().floats.get(&key).copied()
    }
    pub(super) fn scene(&self) -> Scene {
        self.last_scene.clone().unwrap()
    }
    pub(super) fn window_id(&self, key: CGWindowID) -> Option<WindowId> {
        self.last_scene
            .as_ref()?
            .windows
            .get(&key)
            .map(|entry| entry.id)
    }
    pub(super) fn is_managed(&self, key: CGWindowID) -> bool {
        self.window_id(key).is_some()
    }
    pub(super) fn query_monitors_json(&self) -> String {
        self.dome.as_ref().unwrap().query_monitors_json()
    }
    pub(super) fn query_workspaces_json(&self) -> String {
        self.dome.as_ref().unwrap().query_workspaces_json()
    }
    pub(super) fn send_action(&mut self, action: &Action) {
        let event_loop = calloop::EventLoop::<()>::try_new().unwrap();
        self.dome_mut().handle_actions(
            &Actions::new(vec![action.clone()]),
            &event_loop.get_signal(),
        );
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn run_actions(&mut self, source: &str) {
        self.send_action(&source.parse().unwrap());
    }
    pub(super) fn deliver_report(&mut self, report: HubEvent) {
        assert!(apply_report(self.dome_mut(), report).is_ok());
    }
    pub(super) fn take_reports(&mut self) -> Vec<HubEvent> {
        let mut reports = Vec::new();
        while let Ok(report) = self.reports.as_ref().unwrap().try_recv() {
            reports.push(report);
        }
        reports
    }
    pub(super) fn pump_reports(&mut self) -> usize {
        let reports = self.take_reports();
        let count = reports.len();
        for report in reports {
            self.deliver_report(report);
        }
        count
    }
    pub(super) fn hold_scenes(&mut self, hold: bool) {
        self.hold_scenes = hold;
    }
    pub(super) fn take_held_scenes(&mut self) -> Vec<Scene> {
        std::mem::take(&mut self.held_scenes)
    }
    pub(super) fn send_message(&mut self, message: HubMessage) {
        self.ui_sender.as_ref().unwrap().send(message).unwrap();
    }
    pub(super) fn pump_scenes(&mut self) -> usize {
        let mut count = 0;
        while let Ok(mut message) = self.domain_output.try_recv() {
            count += 1;
            if let HubMessage::Scene(scene) = &mut message {
                let observed = scene.displays_changed_at;
                let mapped = match self.display_clock {
                    Some((previous, mapped)) if previous == observed => mapped,
                    Some(_) => self.now,
                    None => self.now - Duration::from_secs(20),
                };
                self.display_clock = Some((observed, mapped));
                scene.displays_changed_at = mapped;
                self.last_scene = Some(scene.clone());
                if self.hold_scenes {
                    self.held_scenes.push(scene.clone());
                    continue;
                }
                let mut painted = self.painted.borrow_mut();
                painted.focused_window = scene
                    .focused_window
                    .and_then(|key| scene.windows.get(&key).map(|entry| entry.id));
                if let Some(FocusTarget::Monitor(monitor)) = scene.focus_request {
                    painted.focused_monitor_id = Some(monitor);
                }
            }
            self.send_message(message);
        }
        count
    }
    pub(super) fn wake(&mut self, wake: Wake) {
        assert!(
            self.handler
                .as_mut()
                .unwrap()
                .process_wake_at(wake, self.now)
                .is_continue()
        );
    }
    pub(super) fn ui_pass(&mut self) {
        self.wake(Wake::Ui);
    }
    pub(super) fn release_ready_focus(&mut self) {
        while let Ok(wake) = self.wakes.try_recv() {
            self.wake(wake);
        }
        let due: Vec<_> = self
            .deadlines
            .borrow()
            .iter()
            .filter(|(_, at)| **at <= self.now)
            .map(|(wake, _)| *wake)
            .collect();
        for wake in due {
            self.deadlines.borrow_mut().remove(&wake);
            self.wake(wake);
        }
    }
    pub(super) fn deadline(&self, wake: Wake) -> Option<Instant> {
        self.deadlines.borrow().get(&wake).copied()
    }
    fn roundtrip(&mut self) {
        self.ui_pass();
        self.pump_reports();
        self.pump_scenes();
        self.ui_pass();
    }
    pub(super) fn clear_moves(&self) {
        self.moves.borrow_mut().clear();
    }
    pub(super) fn recorded_moves(&self) -> Vec<(CGWindowID, i32, i32, i32, i32)> {
        self.moves.borrow().clone()
    }
    pub(super) fn clear_effects(&self) {
        self.effects.borrow_mut().clear();
    }
    pub(super) fn effects(&self) -> Vec<String> {
        self.effects.borrow().clone()
    }
    pub(super) fn flush_moves(&mut self) {
        let pending = std::mem::take(&mut *self.moves.borrow_mut());
        for (key, x, y, width, height) in pending {
            self.queue_position(
                key,
                PixelRect::new(x, y, width, height),
                PositionReadOrigin::Settle(DebounceBurst {
                    first: self.now,
                    last: self.now,
                }),
            );
        }
        self.roundtrip();
    }
    pub(super) fn settle(&mut self, limit: usize) {
        for _ in 0..limit {
            self.flush_moves();
            self.release_ready_focus();
            if self.moves.borrow().is_empty() && self.observations.borrow().is_empty() {
                return;
            }
        }
        panic!("settle did not converge after {limit} iterations");
    }
    pub(super) fn refresh_window_and_assert_identity(&mut self, key: CGWindowID) {
        let id = self.window_id(key).unwrap();
        self.report_existing(key);
        assert_eq!(self.window_id(key), Some(id));
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct FloatSnapshot {
    pub(super) outer_frame: Dimension,
    pub(super) content_dim: Dimension,
    pub(super) corner_radius: Length<Logical>,
    pub(super) cocoa_frame: NSRect,
}

#[derive(Clone, Default)]
pub(super) struct SceneState {
    pub(super) focused_window: Option<WindowId>,
    pub(super) focused_monitor_id: Option<MonitorId>,
    pub(super) floats: HashMap<CGWindowID, FloatSnapshot>,
    pub(super) tiling_corner_radii: HashMap<WindowId, Length<Logical>>,
    pub(super) monitor_dims: Vec<Dimension>,
}
