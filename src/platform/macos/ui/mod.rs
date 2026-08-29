mod compositor;
mod mirror;
mod overlay;
mod status_menu;

use std::cell::{Cell, OnceCell, RefCell};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::{Arc, mpsc};

use dispatch2::{DispatchQueue, DispatchRetained};
use dome_auxiliary_window::{AuxiliaryLoopHandler, EventLoop, LoopWaker};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_app_kit::NSApplication;
use objc2_core_graphics::CGWindowID;
use objc2_io_surface::IOSurface;

use super::dome::{HubEvent, HubMessage, SceneSender};
use super::listeners::EventListener;
use crate::config::Config;
use crate::core::{ContainerId, MonitorId, WindowId};
use crate::platform::render::WgpuContext;
use mirror::{WindowCapture, create_captures_async};
use overlay::{FloatOverlay, TabBarOverlay, TilingOverlay};
use status_menu::StatusMenu;

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
    event_loop: EventLoop,
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

        let state = Rc::new(UiState {
            scene_rx,
            capture_rx,
            capture_sender: OnceCell::new(),
            capture_queue: DispatchQueue::new("dome.capture", None),
            tiling_overlays: RefCell::new(HashMap::new()),
            tab_bar_overlays: RefCell::new(HashMap::new()),
            float_overlays: RefCell::new(HashMap::new()),
            captures: RefCell::new(HashMap::new()),
            event_listener,
            gpu,
            config: RefCell::new(config),
            last_focused: Cell::new(None),
            last_focused_monitor_id: Cell::new(None),
            status_menu: RefCell::new(None),
            hub_sender,
        });

        let event_loop = EventLoop::new(Box::new(WindowLoopHandler {
            state: state.clone(),
        }));
        let waker = event_loop.waker();
        let sender = MessageSender {
            tx: scene_tx,
            waker: waker.clone(),
        };
        if state
            .capture_sender
            .set(CaptureSender {
                tx: capture_tx,
                waker,
            })
            .is_err()
        {
            unreachable!("capture sender is set exactly once");
        }

        (Self { event_loop }, sender)
    }

    pub(super) fn run(self) {
        self.event_loop.run();
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
    // Set once in Ui::new, after the loop's waker exists.
    capture_sender: OnceCell<CaptureSender>,
    // Serial background queue for SCStream output handlers. Keeps IOSurface extraction
    // off the main thread while preserving scene ordering.
    capture_queue: DispatchRetained<DispatchQueue>,
    tiling_overlays: RefCell<HashMap<MonitorId, TilingOverlay>>,
    tab_bar_overlays: RefCell<HashMap<ContainerId, TabBarOverlay>>,
    float_overlays: RefCell<HashMap<CGWindowID, FloatOverlay>>,
    // Owns each live WindowCapture to keep its SCStream running.
    captures: RefCell<HashMap<CGWindowID, WindowCapture>>,
    event_listener: EventListener,
    gpu: Rc<WgpuContext>,
    config: RefCell<Config>,
    last_focused: Cell<Option<WindowId>>,
    last_focused_monitor_id: Cell<Option<MonitorId>>,
    status_menu: RefCell<Option<StatusMenu>>,
    hub_sender: calloop::channel::Sender<HubEvent>,
}

struct WindowLoopHandler {
    state: Rc<UiState>,
}

impl AuxiliaryLoopHandler for WindowLoopHandler {
    fn on_started(&mut self) {
        tracing::info!("Application did finish launching");
    }

    fn on_stopping(&mut self) {
        self.state.hub_sender.send(HubEvent::Shutdown).ok();
    }

    fn on_wake(&mut self) {
        let state = self.state.clone();
        let mtm = MainThreadMarker::new().expect("on_wake runs on the main thread");
        while let Ok(msg) = state.scene_rx.try_recv() {
            match msg {
                HubMessage::Scene(scene) => {
                    let sender_clone = state.hub_sender.clone();
                    state
                        .status_menu
                        .borrow_mut()
                        .get_or_insert_with(|| StatusMenu::new(mtm, sender_clone))
                        .update(mtm, &scene.workspaces);

                    let mut tiling_overlays = state.tiling_overlays.borrow_mut();
                    let mut float_overlays = state.float_overlays.borrow_mut();
                    let mut captures = state.captures.borrow_mut();

                    let config = state.config.borrow().clone();
                    let gpu = state.gpu.clone();
                    let hub_sender = state.hub_sender.clone();

                    let active_monitors: Vec<_> =
                        scene.tiling.iter().map(|t| t.monitor_id).collect();
                    for data in &scene.tiling {
                        let overlay = tiling_overlays.entry(data.monitor_id).or_insert_with(|| {
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
                    tiling_overlays.retain(|id, _| active_monitors.contains(id));

                    let mut tab_bar_overlays = state.tab_bar_overlays.borrow_mut();
                    let mut active_tab_bars: HashSet<ContainerId> = HashSet::new();
                    for data in &scene.tiling {
                        for cs in &data.containers {
                            if !cs.placement.is_tabbed || cs.placement.titles.is_empty() {
                                continue;
                            }
                            let entry =
                                tab_bar_overlays.entry(cs.placement.id).or_insert_with(|| {
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
                    tab_bar_overlays.retain(|id, _| active_tab_bars.contains(id));
                    drop(tab_bar_overlays);

                    let mut capture_pairs = Vec::new();
                    for show in &scene.float_shows {
                        let is_new = !float_overlays.contains_key(&show.cg_id);
                        let overlay = float_overlays.entry(show.cg_id).or_insert_with(|| {
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

                        if let Some(capture) = captures.get_mut(&show.cg_id) {
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
                    float_overlays.retain(|cg_id, _| active_floats.contains(cg_id));
                    captures.retain(|cg_id, _| active_floats.contains(cg_id));

                    drop(tiling_overlays);
                    drop(float_overlays);
                    drop(captures);

                    {
                        let last = state.last_focused.get();
                        let last_monitor = state.last_focused_monitor_id.get();
                        let monitor_changed =
                            last_monitor.is_some_and(|m| m != scene.focused_monitor_id);
                        if last != scene.focused_window || monitor_changed {
                            state.last_focused.set(scene.focused_window);
                            if scene.focused_window.is_none() {
                                let overlays = state.tiling_overlays.borrow();
                                if let Some(overlay) = overlays.get(&scene.focused_monitor_id) {
                                    overlay.focus(mtm);
                                }
                            }
                        }
                        state
                            .last_focused_monitor_id
                            .set(Some(scene.focused_monitor_id));
                    }
                }
                HubMessage::RefreshObservers => {
                    state.event_listener.refresh_all_observers();
                }
                HubMessage::ConfigChanged(new_config) => {
                    let new_config = *new_config;
                    *state.config.borrow_mut() = new_config.clone();
                    for overlay in state.float_overlays.borrow_mut().values_mut() {
                        overlay.set_config(&new_config);
                    }
                    for overlay in state.tiling_overlays.borrow_mut().values_mut() {
                        overlay.set_config(&new_config);
                    }
                    for overlay in state.tab_bar_overlays.borrow().values() {
                        overlay.set_config(&new_config);
                    }
                }
                HubMessage::Shutdown => {
                    NSApplication::sharedApplication(mtm).terminate(None);
                    return;
                }
            }
        }

        while let Ok(msg) = state.capture_rx.try_recv() {
            match msg {
                CaptureMessage::Ready { cg_id, capture } => {
                    if state.float_overlays.borrow().contains_key(&cg_id) {
                        state.captures.borrow_mut().insert(cg_id, capture);
                    }
                }
                CaptureMessage::Frame { cg_id, surface } => {
                    if let Some(overlay) = state.float_overlays.borrow_mut().get_mut(&cg_id) {
                        overlay.apply_frame(&surface);
                    }
                }
            }
        }
    }
}
