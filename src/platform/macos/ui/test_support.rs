use super::*;
use crate::core::{Length, PixelRect, WindowId};
use crate::platform::macos::dome::events::{
    DiscoveredWindow, ExpectedWindow, FocusTarget, MacOsMetadata, MonitorScene, Scene,
    WindowPlacement,
};
use crate::platform::macos::window_server::{ApplicationLifetime, Observation, RecoveryEntry};
use internal_windows::{ContainerShow, FloatShow, InternalWindows, MonitorTilingData};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::time::Duration;

pub(super) type Log = Rc<RefCell<Vec<String>>>;
struct ScriptedServer {
    log: Log,
    output: Rc<RefCell<Vec<Observation>>>,
    recovery: Rc<RefCell<Vec<RecoveryEntry>>>,
    deadline: Rc<Cell<Instant>>,
}
impl WindowServer for ScriptedServer {
    fn ingest_native_events(&mut self, _: Instant) {
        self.log.borrow_mut().push("native".into());
    }
    fn ingest_read_results(&mut self, _: Instant) {
        self.log.borrow_mut().push("results".into());
    }
    fn has_observations(&self) -> bool {
        !self.output.borrow().is_empty()
    }
    fn take_observations(&mut self) -> Vec<Observation> {
        self.log.borrow_mut().push("observations".into());
        std::mem::take(&mut *self.output.borrow_mut())
    }
    fn next_deadline(&self) -> Option<Instant> {
        Some(self.deadline.get())
    }
    fn refresh_applications(&mut self, now: Instant) {
        if now >= self.deadline.get() {
            self.log.borrow_mut().push("refresh".into());
            self.deadline.set(now + Duration::from_secs(5));
        }
    }
    fn set_frame(&mut self, key: CGWindowID, _: PixelRect) {
        self.log.borrow_mut().push(format!("frame {key}"));
    }
    fn move_offscreen(&mut self, key: CGWindowID, _: (Length, Length)) {
        self.log.borrow_mut().push(format!("park {key}"));
    }
    fn minimize(&mut self, key: CGWindowID) {
        self.log.borrow_mut().push(format!("minimize {key}"));
    }
    fn unminimize(&mut self, key: CGWindowID) {
        self.log.borrow_mut().push(format!("unminimize {key}"));
    }
    fn close(&mut self, key: CGWindowID) {
        self.log.borrow_mut().push(format!("close {key}"));
    }
    fn focus(&mut self, key: CGWindowID) {
        self.log.borrow_mut().push(format!("focus {key}"));
    }
    fn recover(&mut self, _: CGWindowID, _: PixelRect) {
        panic!("handler hands records to shutdown");
    }
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        self.log.borrow_mut().push("shutdown".into());
        *self.recovery.borrow_mut() = recovery;
    }
}
struct RecordingInternal {
    log: Log,
}
impl InternalWindows for RecordingInternal {
    fn set_monitors(&mut self, _: &[MonitorScene]) {
        self.log.borrow_mut().push("monitors".into());
    }
    fn show_tiling(&mut self, _: &MonitorTilingData) {
        self.log.borrow_mut().push("tiling".into());
    }
    fn show_floats(&mut self, _: &[FloatShow], _: Option<CGWindowID>) {
        self.log.borrow_mut().push("floats".into());
    }
    fn show_tab_bars(&mut self, _: &[ContainerShow]) {
        self.log.borrow_mut().push("tabs".into());
    }
    fn apply_capture_messages(&mut self, messages: Vec<CaptureMessage>) {
        self.log
            .borrow_mut()
            .push(format!("capture {}", messages.len()));
    }
    fn focus_tiling_overlay(&mut self, _: crate::core::MonitorId) {
        self.log.borrow_mut().push("overlay focus".into());
    }
    fn set_appearance(&mut self, _: &Appearance) {
        self.log.borrow_mut().push("appearance".into());
    }
}
pub(super) struct Fixture {
    pub(super) handler: WindowLoopHandler,
    pub(super) sender: mpsc::Sender<HubMessage>,
    pub(super) capture_sender: mpsc::Sender<CaptureMessage>,
    pub(super) log: Log,
    pub(super) output: Rc<RefCell<Vec<Observation>>>,
    pub(super) recovery: Rc<RefCell<Vec<RecoveryEntry>>>,
    #[cfg(test)]
    pub(super) focus: Arc<SharedFocus>,
    #[cfg(test)]
    pub(super) focus_wakes: mpsc::Receiver<Wake>,
    #[cfg(feature = "native-ui-test")]
    pub(super) server_deadline: Rc<Cell<Instant>>,
    #[cfg(test)]
    pub(super) reports: calloop::channel::Channel<HubEvent>,
    pub(super) now: Instant,
}
impl Fixture {
    pub(super) fn new() -> Self {
        let now = Instant::now();
        let log = Rc::new(RefCell::new(Vec::new()));
        let output = Rc::new(RefCell::new(Vec::new()));
        let recovery = Rc::new(RefCell::new(Vec::new()));
        let (sender, intake) = mpsc::channel();
        let (capture_sender, capture_intake) = mpsc::channel();
        let (hub, _reports) = calloop::channel::channel();
        let focus = Arc::new(SharedFocus::new());
        let (focus_sender, _focus_wakes) = mpsc::channel();
        focus.bind_waker(move || focus_sender.send(Wake::FocusRequest).unwrap());
        let ui = UiState::new(
            intake,
            hub,
            Box::new(RecordingInternal { log: log.clone() }),
            focus.clone(),
            capture_intake,
        );
        let callbacks = Rc::new(OnceCell::new());
        let wake_log = log.clone();
        let deadline_log = log.clone();
        let stop_log = log.clone();
        let terminate_log = log.clone();
        assert!(
            callbacks
                .set(WindowLoopCallbacks {
                    wake: Box::new(move |wake| wake_log
                        .borrow_mut()
                        .push(format!("wake {wake:?}"))),
                    wake_at: Box::new(move |wake, _| deadline_log
                        .borrow_mut()
                        .push(format!("deadline {wake:?}"))),
                    stop_wakes: Box::new(move || stop_log.borrow_mut().push("stop wakes".into())),
                    terminate: Box::new(move || terminate_log
                        .borrow_mut()
                        .push("terminate".into())),
                })
                .is_ok()
        );
        let server_deadline = Rc::new(Cell::new(now));
        let server = ScriptedServer {
            log: log.clone(),
            output: output.clone(),
            recovery: recovery.clone(),
            deadline: server_deadline.clone(),
        };
        let handler = WindowLoopHandler::new(ui, Box::new(server), callbacks);
        Self {
            handler,
            sender,
            capture_sender,
            log,
            output,
            recovery,
            #[cfg(test)]
            focus,
            #[cfg(test)]
            focus_wakes: _focus_wakes,
            #[cfg(feature = "native-ui-test")]
            server_deadline,
            #[cfg(test)]
            reports: _reports,
            now,
        }
    }
    pub(super) fn discover(&self) {
        self.output.borrow_mut().push(Observation::AppWindows {
            lifetime: Arc::new(ApplicationLifetime { pid: 99 }),
            discovered: vec![DiscoveredWindow {
                key: 1,
                process_id: 99,
                metadata: MacOsMetadata {
                    title: Some("Window".into()),
                    app_name: Some("App".into()),
                    bundle_id: None,
                },
                frame: Some(PixelRect::new(40, 50, 300, 200)),
                corner_radius: Length::new(8.0),
                minimized: false,
                fullscreen: None,
            }],
            existing: Vec::new(),
            removed: Vec::new(),
        });
    }
    pub(super) fn scene(&self) -> Scene {
        let (keymaps, _) = mpsc::channel();
        let mut keymaps = crate::platform::keymap::KeymapPublisher::new(
            crate::platform::keymap::KeymapView::new(),
            keymaps,
        );
        let (runtime, config) =
            crate::config::LuaRuntime::load(String::new(), &mut keymaps).unwrap();
        let monitor = crate::core::Hub::new(
            crate::core::ReportedMonitor {
                device_name: "test".into(),
                work_area: PixelRect::new(0, 0, 1920, 1080),
                scale: 1.0,
                cg_display_id: None,
                gdi_device: None,
            },
            config.tiling,
            crate::core::PreferredLayouts::default(),
            runtime,
        )
        .primary_monitor();
        Scene {
            windows: HashMap::from([(
                1,
                ExpectedWindow {
                    id: WindowId::new(1),
                    placement: WindowPlacement::Hidden,
                },
            )]),
            monitors: vec![MonitorScene {
                id: monitor,
                native: 1,
                work_area: PixelRect::new(0, 0, 1920, 1080),
                system_work_area: PixelRect::new(0, 0, 1920, 1080),
                bounds: PixelRect::new(0, 0, 1920, 1080).to_dimension(),
                is_primary: true,
                full_height: 1080.0,
                scale: 2.0,
                border_thickness: Length::new(4.0),
                containers: Vec::new(),
            }],
            focused_window: None,
            focus_request: Some(FocusTarget::Monitor(monitor)),
            workspaces: Vec::new(),
            displays_changed_at: self.now - Duration::from_secs(20),
        }
    }
    #[cfg(test)]
    pub(super) fn wake(&mut self, wake: Wake) -> Vec<String> {
        self.log.borrow_mut().clear();
        let _ = self.handler.process_wake_at(wake, self.now);
        self.log.borrow().clone()
    }
}
