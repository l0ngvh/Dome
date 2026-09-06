mod compositor;
mod mirror;
mod overlay;

use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use dispatch2::{DispatchQueue, DispatchRetained};
use dome_auxiliary_window::{App, AppHandler, Icon, LoopWaker, MenuEntry, Shell};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::NSApplication;
use objc2_core_graphics::CGWindowID;
use objc2_io_surface::IOSurface;

use super::dome::{HubEvent, HubMessage, SceneSender, get_all_monitors};
use super::listeners::EventListener;
use crate::action::{Actions, WorkspaceInfo};
use crate::config::Config;
use crate::core::{ContainerId, MonitorId, WindowId};
use crate::platform::render::WgpuContext;
use crate::platform::shell_menu::{build_menu, focused_tooltip, id_to_action};
use mirror::{WindowCapture, create_captures_async};
use overlay::{FloatOverlay, TabBarOverlay, TilingOverlay};

#[derive(Clone)]
pub(super) struct MessageSender {
    tx: mpsc::Sender<HubMessage>,
    waker: LoopWaker,
}

// The waker signals the main run loop from any thread, and the mpsc sender carries the
// scene to the main thread where on_wake drains it.
unsafe impl Send for MessageSender {}

impl MessageSender {
    pub(super) fn send(&self, msg: HubMessage) {
        if self.tx.send(msg).is_ok() {
            self.waker.wake();
        }
    }
}

impl SceneSender for MessageSender {
    fn send(&self, msg: HubMessage) {
        MessageSender::send(self, msg);
    }
}

/// Separate from `HubMessage` so ScreenCaptureKit types stay inside the ui module.
enum CaptureMessage {
    Ready {
        cg_id: CGWindowID,
        capture: WindowCapture,
    },
    Frame {
        cg_id: CGWindowID,
        surface: Retained<IOSurface>,
    },
}

#[derive(Clone)]
pub(super) struct CaptureSender {
    tx: mpsc::Sender<CaptureMessage>,
    waker: LoopWaker,
}

impl CaptureSender {
    fn send(&self, msg: CaptureMessage) {
        if self.tx.send(msg).is_ok() {
            self.waker.wake();
        }
    }
}

pub(super) struct Ui {
    app: App,
}

impl Ui {
    pub(super) fn new(
        _mtm: MainThreadMarker,
        hub_sender: calloop::channel::Sender<HubEvent>,
        event_listener: EventListener,
        config: Config,
    ) -> (Self, MessageSender) {
        let (scene_tx, scene_rx) = mpsc::channel();
        let (capture_tx, capture_rx) = mpsc::channel();
        let gpu = Rc::new(create_wgpu_context().expect("wgpu instance/adapter/device init"));

        let capture_sender: Rc<OnceCell<CaptureSender>> = Rc::new(OnceCell::new());
        let state = UiState {
            scene_rx,
            capture_rx,
            capture_sender: Rc::clone(&capture_sender),
            capture_queue: DispatchQueue::new("dome.capture", None),
            tiling_overlays: HashMap::new(),
            tab_bar_overlays: HashMap::new(),
            float_overlays: HashMap::new(),
            captures: HashMap::new(),
            event_listener,
            gpu,
            config,
            last_focused: None,
            last_focused_monitor_id: None,
            workspaces: Vec::new(),
            hub_sender,
        };

        let app = App::new(
            Icon::from_png(STATUS_BAR_ICON_PNG).expect("status bar icon decodes"),
            Box::new(WindowLoopHandler { state }),
        )
        .expect("app init");
        let waker = app.waker();
        let sender = MessageSender {
            tx: scene_tx,
            waker: waker.clone(),
        };
        if capture_sender
            .set(CaptureSender {
                tx: capture_tx,
                waker,
            })
            .is_err()
        {
            unreachable!("capture sender is set exactly once");
        }

        (Self { app }, sender)
    }

    pub(super) fn run(self) {
        self.app.run();
    }
}

fn create_wgpu_context() -> anyhow::Result<WgpuContext> {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::METAL;
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::LowPower,
        compatible_surface: None,
        force_fallback_adapter: false,
    }))?;
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default()))?;
    Ok(WgpuContext::new(
        instance,
        adapter,
        Arc::new(device),
        Arc::new(queue),
    ))
}

/// Main-thread UI state, owned by `WindowLoopHandler`.
struct UiState {
    scene_rx: mpsc::Receiver<HubMessage>,
    capture_rx: mpsc::Receiver<CaptureMessage>,
    // Shared with Ui::new so it can be set once the loop's waker exists, after the
    // handler that owns this state has moved into the event loop.
    capture_sender: Rc<OnceCell<CaptureSender>>,
    // Serial background queue for SCStream output handlers. Keeps IOSurface extraction
    // off the main thread while preserving scene ordering.
    capture_queue: DispatchRetained<DispatchQueue>,
    tiling_overlays: HashMap<MonitorId, TilingOverlay>,
    tab_bar_overlays: HashMap<ContainerId, TabBarOverlay>,
    float_overlays: HashMap<CGWindowID, FloatOverlay>,
    // Owns each live WindowCapture to keep its SCStream running.
    captures: HashMap<CGWindowID, WindowCapture>,
    event_listener: EventListener,
    gpu: Rc<WgpuContext>,
    config: Config,
    last_focused: Option<WindowId>,
    last_focused_monitor_id: Option<MonitorId>,
    workspaces: Vec<WorkspaceInfo>,
    hub_sender: calloop::channel::Sender<HubEvent>,
}

/// Template PNG embedded at compile time. macOS auto-tints alpha-defined shapes to match
/// dark or light mode when setTemplate is true. Embedding avoids the bundle-path search
/// NSImage::imageNamed uses, so cargo run and cargo make bundle both work with no fork.
const STATUS_BAR_ICON_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/macos/status_bar_icon.png"
));

struct WindowLoopHandler {
    state: UiState,
}

impl AppHandler for WindowLoopHandler {
    fn on_started(&mut self, _shell: &Shell) {
        tracing::info!("Application did finish launching");
    }

    fn on_stopping(&mut self) {
        self.state.hub_sender.send(HubEvent::Shutdown).ok();
    }

    fn menu(&mut self) -> Vec<MenuEntry> {
        build_menu(&self.state.workspaces, true)
    }

    fn on_menu_selected(&mut self, id: u32) {
        if let Some(action) = id_to_action(id, &self.state.workspaces) {
            self.state
                .hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])))
                .ok();
        }
    }

    fn on_display_changed(&mut self) {
        let mtm =
            MainThreadMarker::new().expect("screen-change notification runs on the main thread");
        match get_all_monitors(mtm) {
            Ok(monitors) => {
                self.state
                    .hub_sender
                    .send(HubEvent::MonitorsChanged(monitors))
                    .ok();
            }
            Err(e) => tracing::error!(%e, "Failed to enumerate monitors on screen change"),
        }
    }

    fn on_wake(&mut self, shell: &Shell) {
        let state = &mut self.state;
        let mtm = MainThreadMarker::new().expect("on_wake runs on the main thread");
        while let Ok(msg) = state.scene_rx.try_recv() {
            match msg {
                HubMessage::Scene(scene) => {
                    state.workspaces = scene.workspaces.clone();
                    shell.set_tooltip(&focused_tooltip(&scene.workspaces));

                    let config = state.config.clone();
                    let gpu = state.gpu.clone();
                    let hub_sender = state.hub_sender.clone();

                    let active_monitors: Vec<_> =
                        scene.tiling.iter().map(|t| t.monitor_id).collect();
                    for data in &scene.tiling {
                        let overlay =
                            state
                                .tiling_overlays
                                .entry(data.monitor_id)
                                .or_insert_with(|| {
                                    TilingOverlay::new(
                                        mtm,
                                        &gpu,
                                        config.clone(),
                                        data.cocoa_frame,
                                        data.scale,
                                    )
                                });
                        overlay.set_border_thickness(data.border_thickness);
                        if data.windows.is_empty() && data.containers.is_empty() {
                            overlay.clear();
                        } else {
                            overlay.render(
                                data.cocoa_frame,
                                data.scale,
                                data.monitor_dim,
                                &data.windows,
                                &data.containers,
                            );
                        }
                    }
                    state
                        .tiling_overlays
                        .retain(|id, _| active_monitors.contains(id));

                    let mut active_tab_bars: HashSet<ContainerId> = HashSet::new();
                    for data in &scene.tiling {
                        for cs in &data.containers {
                            if !cs.placement.is_tabbed || cs.placement.titles.is_empty() {
                                continue;
                            }
                            let entry = state
                                .tab_bar_overlays
                                .entry(cs.placement.id)
                                .or_insert_with(|| {
                                    TabBarOverlay::new(
                                        mtm,
                                        &gpu,
                                        config.clone(),
                                        cs.placement.id,
                                        cs.tab_bar_cocoa_frame,
                                        data.scale,
                                        hub_sender.clone(),
                                    )
                                });
                            entry.render(cs, data.scale, data.border_thickness);
                            active_tab_bars.insert(cs.placement.id);
                        }
                    }
                    state
                        .tab_bar_overlays
                        .retain(|id, _| active_tab_bars.contains(id));

                    let mut capture_pairs = Vec::new();
                    for show in &scene.float_shows {
                        let is_new = !state.float_overlays.contains_key(&show.cg_id);
                        let overlay = state.float_overlays.entry(show.cg_id).or_insert_with(|| {
                            FloatOverlay::new(
                                mtm,
                                show.cocoa_frame,
                                show.cg_id,
                                hub_sender.clone(),
                                &gpu,
                                config.theme,
                                &config.font,
                            )
                        });
                        overlay.render(
                            &show.placement,
                            show.cocoa_frame,
                            show.scale,
                            show.border_thickness,
                            scene.focused_window == Some(show.placement.id),
                        );

                        if is_new {
                            capture_pairs.push(show.cg_id);
                        }

                        if let Some(capture) = state.captures.get_mut(&show.cg_id) {
                            if scene.focused_window != Some(show.placement.id) {
                                capture.start(show.cg_id, show.content_dim, show.scale);
                            } else {
                                capture.stop();
                            }
                        }
                    }

                    if !capture_pairs.is_empty() {
                        let capture_sender = state
                            .capture_sender
                            .get()
                            .expect("capture sender set in Ui::new")
                            .clone();
                        create_captures_async(
                            capture_pairs,
                            state.capture_queue.clone(),
                            capture_sender,
                        );
                    }

                    // Float windows are rare, so we can afford recreating overlays
                    // and captures each time the workspace changes rather than
                    // tracking which windows transitioned from float to tiling.
                    let active_floats: HashSet<CGWindowID> =
                        scene.float_shows.iter().map(|s| s.cg_id).collect();
                    state
                        .float_overlays
                        .retain(|cg_id, _| active_floats.contains(cg_id));
                    state
                        .captures
                        .retain(|cg_id, _| active_floats.contains(cg_id));

                    {
                        let last = state.last_focused;
                        let last_monitor = state.last_focused_monitor_id;
                        let monitor_changed =
                            last_monitor.is_some_and(|m| m != scene.focused_monitor_id);
                        if last != scene.focused_window || monitor_changed {
                            state.last_focused = scene.focused_window;
                            if scene.focused_window.is_none()
                                && let Some(overlay) =
                                    state.tiling_overlays.get(&scene.focused_monitor_id)
                            {
                                overlay.focus(mtm);
                            }
                        }
                        state.last_focused_monitor_id = Some(scene.focused_monitor_id);
                    }
                }
                HubMessage::RefreshObservers => {
                    state.event_listener.refresh_all_observers();
                }
                HubMessage::ConfigChanged(new_config) => {
                    let new_config = *new_config;
                    state.config = new_config.clone();
                    for overlay in state.float_overlays.values_mut() {
                        overlay.set_config(&new_config);
                    }
                    for overlay in state.tiling_overlays.values_mut() {
                        overlay.set_config(&new_config);
                    }
                    for overlay in state.tab_bar_overlays.values() {
                        overlay.set_config(&new_config);
                    }
                }
                HubMessage::Shutdown => {
                    // This runs inside the wake-source callback, which holds a
                    // mutable borrow of the loop handler across on_wake. terminate
                    // fires applicationWillTerminate: synchronously, and that
                    // delegate method borrows the same handler again, so calling it
                    // here double-borrows and panics. Defer it to the next main
                    // run-loop turn, once the borrow is released.
                    DispatchQueue::main().exec_async(|| {
                        let mtm = MainThreadMarker::new()
                            .expect("main dispatch queue runs on the main thread");
                        NSApplication::sharedApplication(mtm).terminate(None);
                    });
                    return;
                }
            }
        }

        while let Ok(msg) = state.capture_rx.try_recv() {
            match msg {
                CaptureMessage::Ready { cg_id, capture } => {
                    if state.float_overlays.contains_key(&cg_id) {
                        state.captures.insert(cg_id, capture);
                    }
                }
                CaptureMessage::Frame { cg_id, surface } => {
                    if let Some(overlay) = state.float_overlays.get_mut(&cg_id) {
                        overlay.apply_frame(&surface);
                    }
                }
            }
        }
    }
}
