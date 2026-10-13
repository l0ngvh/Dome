mod auxiliary_windows;
mod compositor;
pub(super) mod internal_windows;
mod mirror;
mod overlay;
mod recovery;
mod registry;
mod state;
mod window;

use std::cell::OnceCell;
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::{Arc, OnceLock, mpsc};
use std::time::Instant;

use dome_auxiliary_window::{App, AppHandler, Icon, LoopWaker, MenuEntry, Shell};
use objc2::{MainThreadMarker, rc::Retained};
use objc2_core_graphics::CGWindowID;
use objc2_io_surface::IOSurface;

use super::Wake;
use super::dome::events::{HubEvent, HubMessage};
use super::dome::{SceneSender, get_all_monitors};
use super::focus::SharedFocus;
use super::window_server::WindowServer;
use crate::action::Actions;
use crate::config::Appearance;
use crate::platform::render::WgpuContext;
use crate::platform::shell_menu::{build_menu, focused_tooltip, id_to_action};
use auxiliary_windows::AuxiliaryWindows;
use mirror::WindowCapture;
pub(super) use state::UiState;

#[derive(Clone)]
pub(super) struct UiSender {
    sender: mpsc::Sender<HubMessage>,
    waker: LoopWaker<Wake>,
}

impl UiSender {
    pub(super) fn send(&self, message: HubMessage) {
        if self.sender.send(message).is_ok() {
            self.waker.wake(Wake::Ui);
        }
    }
}

impl SceneSender for UiSender {
    fn send(&self, message: HubMessage) {
        UiSender::send(self, message);
    }
}

pub(super) enum CaptureMessage {
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
    sender: mpsc::Sender<CaptureMessage>,
    waker: LoopWaker<Wake>,
}

impl CaptureSender {
    fn send(&self, message: CaptureMessage) {
        if self.sender.send(message).is_ok() {
            self.waker.wake(Wake::Capture);
        }
    }
}

pub(super) struct Ui {
    app: App<Wake>,
}

impl Ui {
    pub(super) fn new(
        _mtm: MainThreadMarker,
        hub: calloop::channel::Sender<HubEvent>,
        windows: Box<dyn WindowServer>,
        focus: Arc<SharedFocus>,
        loop_waker: Arc<OnceLock<LoopWaker<Wake>>>,
        appearance: Appearance,
    ) -> (Self, UiSender) {
        let (sender, intake) = mpsc::channel();
        let (capture_sender, capture_intake) = mpsc::channel();
        let gpu = Rc::new(create_wgpu_context().expect("wgpu instance/adapter/device init"));
        let capture_binding = Rc::new(OnceCell::new());
        let internal = AuxiliaryWindows::new(gpu, appearance, hub.clone(), capture_binding.clone());
        let ui = UiState::new(
            intake,
            hub,
            Box::new(internal),
            focus.clone(),
            capture_intake,
        );
        let callbacks = Rc::new(OnceCell::new());
        let app = App::new(
            Icon::from_png(STATUS_BAR_ICON_PNG).expect("status bar icon decodes"),
            Box::new(WindowLoopHandler::new(ui, windows, callbacks.clone())),
        )
        .expect("app init");
        let waker = app.waker();
        let wake_handle = app.handle();
        let stop_handle = app.handle();
        let terminate_handle = app.handle();
        let callback_waker = waker.clone();
        assert!(
            callbacks
                .set(WindowLoopCallbacks {
                    wake: Box::new(move |wake| callback_waker.wake(wake)),
                    wake_at: Box::new(move |wake, at| wake_handle.wake_at(wake, at)),
                    stop_wakes: Box::new(move || stop_handle.stop_wakes()),
                    terminate: Box::new(move || terminate_handle.terminate()),
                })
                .is_ok(),
            "loop callbacks already bound"
        );
        assert!(
            loop_waker.set(waker.clone()).is_ok(),
            "loop waker already bound"
        );
        let focus_waker = waker.clone();
        focus.bind_waker(move || focus_waker.wake(Wake::FocusRequest));
        assert!(
            capture_binding
                .set(CaptureSender {
                    sender: capture_sender,
                    waker: waker.clone()
                })
                .is_ok(),
            "capture sender already bound"
        );
        (Self { app }, UiSender { sender, waker })
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

const STATUS_BAR_ICON_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/resources/macos/status_bar_icon.png"
));

pub(super) struct WindowLoopCallbacks {
    pub(super) wake: Box<dyn Fn(Wake)>,
    pub(super) wake_at: Box<dyn Fn(Wake, Option<Instant>)>,
    pub(super) stop_wakes: Box<dyn Fn()>,
    pub(super) terminate: Box<dyn Fn()>,
}

pub(super) struct WindowLoopHandler {
    ui: UiState,
    windows: Box<dyn WindowServer>,
    callbacks: Rc<OnceCell<WindowLoopCallbacks>>,
    stopping: bool,
}

impl WindowLoopHandler {
    pub(super) fn new(
        ui: UiState,
        windows: Box<dyn WindowServer>,
        callbacks: Rc<OnceCell<WindowLoopCallbacks>>,
    ) -> Self {
        Self {
            ui,
            windows,
            callbacks,
            stopping: false,
        }
    }

    fn callbacks(&self) -> &WindowLoopCallbacks {
        self.callbacks
            .get()
            .expect("loop callbacks bound before delivery")
    }

    pub(super) fn process_window_server_at(&mut self, now: Instant) {
        if self.stopping {
            return;
        }
        self.windows.ingest_native_events(now);
        self.windows.ingest_read_results(now);
        self.windows.refresh_applications(now);
        (self.callbacks().wake_at)(Wake::WindowServer, self.windows.next_deadline());
        if self.windows.has_observations() {
            (self.callbacks().wake)(Wake::Ui);
        }
    }

    pub(super) fn process_ui_at(&mut self, now: Instant) -> ControlFlow<()> {
        if self.stopping {
            return ControlFlow::Break(());
        }
        for observation in self.windows.take_observations() {
            self.ui.observe(self.windows.as_mut(), observation, now);
        }
        let ControlFlow::Continue((scene, commands)) = self.ui.drain_intake() else {
            self.on_stopping();
            (self.callbacks().terminate)();
            return ControlFlow::Break(());
        };
        if let Some(scene) = &scene {
            self.ui.apply_placements(self.windows.as_mut(), scene, now);
        }
        for command in commands {
            self.ui.apply_command(self.windows.as_mut(), command);
        }
        if let Some(scene) = &scene {
            self.ui.show_internal_windows(scene);
        }
        self.ui.apply_overlay_focus();
        (self.callbacks().wake_at)(Wake::FocusRequest, self.ui.focus.next_deadline(now));
        ControlFlow::Continue(())
    }

    pub(super) fn process_focus_request(&mut self, now: Instant) {
        if self.stopping {
            return;
        }
        self.ui
            .focus
            .submit_pending_focus(now, self.windows.as_mut());
        (self.callbacks().wake_at)(Wake::FocusRequest, self.ui.focus.next_deadline(now));
    }

    pub(super) fn process_capture(&mut self) {
        if !self.stopping {
            self.ui.process_capture();
        }
    }

    pub(super) fn process_wake_at(&mut self, wake: Wake, now: Instant) -> ControlFlow<()> {
        match wake {
            Wake::WindowServer => self.process_window_server_at(now),
            Wake::Ui => return self.process_ui_at(now),
            Wake::FocusRequest => self.process_focus_request(now),
            Wake::Capture => self.process_capture(),
        }
        ControlFlow::Continue(())
    }

    pub(super) fn on_stopping(&mut self) {
        if self.stopping {
            return;
        }
        self.stopping = true;
        (self.callbacks().stop_wakes)();
        self.ui.recover(self.windows.as_mut());
    }
}

impl AppHandler for WindowLoopHandler {
    type Wake = Wake;

    fn on_started(&mut self, _shell: &Shell) {
        (self.callbacks().wake)(Wake::WindowServer);
    }

    fn on_stopping(&mut self) {
        WindowLoopHandler::on_stopping(self);
    }

    fn on_wake(&mut self, shell: &Shell, wake: Wake) {
        if self.process_wake_at(wake, Instant::now()).is_continue() && wake == Wake::Ui {
            shell.set_tooltip(&focused_tooltip(&self.ui.workspaces));
        }
    }

    fn menu(&mut self) -> Vec<MenuEntry> {
        build_menu(&self.ui.workspaces, true)
    }

    fn on_menu_selected(&mut self, id: u32) {
        if let Some(action) = id_to_action(id, &self.ui.workspaces) {
            self.ui
                .hub
                .send(HubEvent::Action(Actions::new(vec![action])))
                .ok();
        }
    }

    fn on_display_changed(&mut self) {
        let mtm = MainThreadMarker::new().expect("screen-change notification runs on UI");
        match get_all_monitors(mtm) {
            Ok(monitors) => {
                self.ui.hub.send(HubEvent::MonitorsChanged(monitors)).ok();
            }
            Err(error) => tracing::error!(%error, "Failed to enumerate monitors on screen change"),
        }
    }
}

#[cfg(feature = "native-ui-test")]
mod native_loop_test;
#[cfg(feature = "native-ui-test")]
pub(crate) use native_loop_test::run as run_native_loop;
#[cfg(feature = "native-ui-test")]
pub(in crate::platform::macos) use native_loop_test::{run_bounded_native_test, validated};
#[cfg(any(test, feature = "native-ui-test"))]
mod test_support;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PixelRect;
    use crate::platform::macos::dome::events::{FocusTarget, WindowCommand, WindowPlacement};
    use std::time::Duration;

    use super::test_support::*;
    #[test]
    fn process_typed_wakes_keeps_passes_separate() {
        let mut fixture = Fixture::new();
        fixture.discover();
        fixture
            .sender
            .send(HubMessage::Scene(fixture.scene()))
            .unwrap();
        fixture
            .sender
            .send(HubMessage::Command(WindowCommand::Close(1)))
            .unwrap();
        assert_eq!(
            fixture.wake(Wake::WindowServer),
            [
                "native",
                "results",
                "refresh",
                "deadline WindowServer",
                "wake Ui"
            ]
        );
        assert_eq!(fixture.wake(Wake::Capture), ["capture 0"]);
        assert_eq!(
            fixture.wake(Wake::Ui),
            [
                "observations",
                "park 1",
                "close 1",
                "monitors",
                "tiling",
                "floats",
                "tabs",
                "overlay focus",
                "deadline FocusRequest"
            ]
        );
        assert!(matches!(
            fixture.reports.try_recv(),
            Ok(HubEvent::WindowsDiscovered(_))
        ));
        fixture.focus.request(1);
        fixture
            .capture_sender
            .send(CaptureMessage::Frame {
                cg_id: 1,
                surface: IOSurface::new(),
            })
            .unwrap();
        assert_eq!(
            fixture.wake(Wake::Ui),
            ["observations", "deadline FocusRequest"]
        );
        assert_eq!(fixture.wake(Wake::Capture), ["capture 1"]);
        assert_eq!(
            fixture.wake(Wake::FocusRequest),
            ["focus 1", "deadline FocusRequest"]
        );
        fixture
            .sender
            .send(HubMessage::Command(WindowCommand::Unminimize(1)))
            .unwrap();
        assert_eq!(fixture.wake(Wake::FocusRequest), ["deadline FocusRequest"]);
        assert_eq!(
            fixture.wake(Wake::Ui),
            ["observations", "unminimize 1", "deadline FocusRequest"]
        );
        assert_eq!(
            fixture.wake(Wake::WindowServer),
            ["native", "results", "deadline WindowServer"]
        );
    }
    #[test]
    fn drain_scene_focus_edges_releases_only_newest_pending_intent() {
        let mut fixture = Fixture::new();
        fixture.discover();
        let mut scene = fixture.scene();
        scene.focus_request = Some(FocusTarget::Window(1));
        for request in [
            Some(FocusTarget::Window(1)),
            None,
            Some(FocusTarget::Window(1)),
        ] {
            scene.focus_request = request;
            fixture
                .sender
                .send(HubMessage::Scene(scene.clone()))
                .unwrap();
        }
        let log = fixture.wake(Wake::Ui);
        assert!(!log.iter().any(|entry| entry.starts_with("focus ")));
        assert_eq!(
            fixture.focus_wakes.try_iter().collect::<Vec<_>>(),
            [Wake::FocusRequest, Wake::FocusRequest]
        );
        assert_eq!(
            fixture.wake(Wake::FocusRequest),
            ["focus 1", "deadline FocusRequest"]
        );
        fixture
            .sender
            .send(HubMessage::Scene(scene.clone()))
            .unwrap();
        fixture.wake(Wake::Ui);
        assert!(fixture.focus_wakes.try_recv().is_err());
        assert_eq!(fixture.wake(Wake::FocusRequest), ["deadline FocusRequest"]);

        fixture.now += Duration::from_millis(50);
        scene.focus_request = None;
        fixture
            .sender
            .send(HubMessage::Scene(scene.clone()))
            .unwrap();
        scene.focus_request = Some(FocusTarget::Window(1));
        fixture
            .sender
            .send(HubMessage::Scene(scene.clone()))
            .unwrap();
        fixture.wake(Wake::Ui);
        assert_eq!(fixture.focus_wakes.try_recv().unwrap(), Wake::FocusRequest);
        assert_eq!(
            fixture.focus.next_deadline(fixture.now),
            Some(fixture.now + Duration::from_millis(50))
        );
        assert_eq!(fixture.wake(Wake::FocusRequest), ["deadline FocusRequest"]);
        fixture.focus.request(2);
        fixture.focus.request(3);
        assert_eq!(fixture.focus_wakes.try_iter().count(), 2);
        fixture.now += Duration::from_millis(50);
        assert_eq!(
            fixture.wake(Wake::FocusRequest),
            ["focus 3", "deadline FocusRequest"]
        );

        scene.focus_request = Some(FocusTarget::Monitor(scene.monitors[0].id));
        fixture.sender.send(HubMessage::Scene(scene)).unwrap();
        let log = fixture.wake(Wake::Ui);
        let content = log.iter().position(|entry| entry == "monitors").unwrap();
        let focus = log
            .iter()
            .position(|entry| entry == "overlay focus")
            .unwrap();
        assert!(content < focus);
        assert!(!fixture.focus.is_submitted(3));
        fixture.focus.request(1);
        assert_eq!(fixture.wake(Wake::FocusRequest), ["deadline FocusRequest"]);
        fixture.now += Duration::from_millis(100);
        assert_eq!(
            fixture.wake(Wake::FocusRequest),
            ["focus 1", "deadline FocusRequest"]
        );
    }

    #[test]
    fn stop_discards_effects_and_hands_recovery_once() {
        let mut fixture = Fixture::new();
        fixture.discover();
        fixture
            .sender
            .send(HubMessage::Scene(fixture.scene()))
            .unwrap();
        fixture.wake(Wake::Ui);
        let mut changed = fixture.scene();
        changed.windows.get_mut(&1).unwrap().placement = WindowPlacement::Fullscreen {
            monitor: changed.monitors[0].id,
        };
        fixture.sender.send(HubMessage::Scene(changed)).unwrap();
        assert!(
            fixture
                .wake(Wake::Ui)
                .iter()
                .any(|entry| entry == "frame 1")
        );
        fixture.focus.request(1);
        assert_eq!(
            fixture.wake(Wake::FocusRequest),
            ["focus 1", "deadline FocusRequest"]
        );
        assert!(fixture.focus.is_submitted(1));
        fixture.focus.request(2);
        fixture
            .sender
            .send(HubMessage::Scene(fixture.scene()))
            .unwrap();
        fixture
            .sender
            .send(HubMessage::Command(WindowCommand::Close(1)))
            .unwrap();
        fixture.sender.send(HubMessage::Shutdown).unwrap();
        assert_eq!(
            fixture.wake(Wake::Ui),
            ["observations", "stop wakes", "shutdown", "terminate"]
        );
        fixture.handler.on_stopping();
        fixture.handler.ui.recover(fixture.handler.windows.as_mut());
        assert_eq!(
            *fixture.log.borrow(),
            ["observations", "stop wakes", "shutdown", "terminate"]
        );
        assert_eq!(fixture.recovery.borrow().len(), 1);
        assert_eq!(fixture.recovery.borrow()[0].window, 1);
        assert_eq!(
            fixture.recovery.borrow()[0].original_rect,
            PixelRect::new(810, 440, 300, 200)
        );
        assert!(!fixture.focus.is_submitted(1));
        assert!(fixture.focus.next_deadline(fixture.now).is_none());
        assert!(fixture.wake(Wake::WindowServer).is_empty());
        assert!(fixture.wake(Wake::FocusRequest).is_empty());
        assert!(fixture.wake(Wake::Capture).is_empty());
    }
}
