use super::*;
use crate::config::lua::test_support::{TempFile, loaded_runtime};
use crate::config::{Appearance, PreferredLayouts};
use crate::core::{ContainerPlacement, FloatWindowPlacement, MonitorId};
use crate::platform::keymap::{KeymapPublisher, KeymapView};
use crate::platform::windows::dome::events::{
    FocusTarget, HubMessage, MonitorScene, Scene, SceneSender, TilingWindowShow, WindowCommand,
    WindowPlacement,
};
use crate::platform::windows::dome::{Dome, HubEvent, QueryDisplay};
use crate::platform::windows::handle::native::fixtures::{self, Reads, Window};
use crate::platform::windows::taskbar::ManageTaskbar;
use crate::platform::windows::ui::{FloatShow, InternalWindow, InternalWindows, UiState};
use crate::platform::windows::{HubSender, Wake, WindowLoopCallbacks, WindowLoopHandler};
use std::cell::{OnceCell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;
use windows::Win32::Foundation::RECT;

#[derive(Default)]
struct Output {
    painted: usize,
    focused: Vec<MonitorId>,
    stacked: usize,
}
impl InternalWindows for Rc<RefCell<Output>> {
    fn set_monitors(&mut self, _: &[MonitorScene]) {}
    fn show_tiling(&mut self, _: MonitorId, _: &[TilingWindowShow], _: &[ContainerPlacement]) {
        self.borrow_mut().painted += 1;
    }
    fn show_floats(&mut self, _: &[FloatShow]) {
        self.borrow_mut().painted += 1;
    }
    fn show_tab_bars(&mut self, _: &[MonitorScene]) {}
    fn stack(&mut self, _: InternalWindow, _: ZOrder) {
        self.borrow_mut().stacked += 1;
    }
    fn focus_tiling_overlay(&mut self, monitor: MonitorId) {
        self.borrow_mut().focused.push(monitor);
    }
    fn set_appearance(&mut self, _: &Appearance) {}
}
struct Taskbar;
impl ManageTaskbar for Taskbar {
    fn add_tab(&self, _: HwndId) {}
    fn delete_tab(&self, _: HwndId) {}
}
struct Displays(Vec<MonitorInfo>);
impl QueryDisplay for Displays {
    fn get_all_monitors(&self) -> anyhow::Result<Vec<MonitorInfo>> {
        Ok(self.0.clone())
    }
}
#[derive(Default)]
struct Scenes(VecDeque<HubMessage>);
impl SceneSender for Rc<RefCell<Scenes>> {
    fn send(&mut self, message: HubMessage) {
        self.borrow_mut().0.push_back(message);
    }
}
struct Harness {
    handler: WindowLoopHandler,
    dome: Dome,
    _configuration: TempFile,
    scenes: Rc<RefCell<Scenes>>,
    intake: mpsc::Sender<HubMessage>,
    reports: mpsc::Receiver<HubEvent>,
    native: NativeEventSender,
    wakes: mpsc::Receiver<Wake>,
    native_reads: Reads,
    schedule: SharedSchedule,
    output: Rc<RefCell<Output>>,
    now: Instant,
    latest: Option<Scene>,
}
impl Harness {
    fn new() -> Self {
        Self::configured(crate::config::tests::tiling_config(), |_, _| {})
    }
    fn configured(
        tiling: crate::core::TilingConfig,
        configure: impl FnOnce(&mut WindowsWindowServer, &mut Reads),
    ) -> Self {
        let mut harness = Self::before_initial_scene(tiling, configure);
        assert!(harness.deliver_scenes().is_empty());
        harness
    }
    fn before_initial_scene(
        tiling: crate::core::TilingConfig,
        configure: impl FnOnce(&mut WindowsWindowServer, &mut Reads),
    ) -> Self {
        let (wake, wakes) = mpsc::channel();
        let server_wake = wake.clone();
        let mut server = WindowsWindowServer::new(Arc::new(move || {
            server_wake.send(Wake::WindowServer).unwrap();
        }))
        .unwrap();
        let native = server.native_sender();
        let schedule = server.schedule.clone();
        let mut native_reads = Reads::default();
        configure(&mut server, &mut native_reads);
        server
            .dispatcher
            .as_mut()
            .unwrap()
            .set_native_reads(native_reads.clone());
        let scenes = Rc::new(RefCell::new(Scenes::default()));
        let mut source = monitor(PixelRect::new(-1920, 0, 1920, 1080), 96);
        source.name = "Source".into();
        source.gdi_device = "source".into();
        source.work_area = PixelRect::new(-1920, 30, 1920, 1050);
        source.is_primary = true;
        let mut destination = monitor(PixelRect::new(0, 0, 2560, 1440), 144);
        destination.handle = 2;
        destination.name = "Destination".into();
        destination.gdi_device = "destination".into();
        destination.work_area = PixelRect::new(0, 40, 2560, 1360);
        let (runtime, configuration) = loaded_runtime(
            "concrete_windows_handler",
            r#"
            return { keymaps = { main = { ["ctrl+l"] = function(a) a.move_to_monitor_right() end } } }
        "#,
        );
        let (keymap, _) = mpsc::channel();
        let mut dome = Dome::new(
            tiling,
            PreferredLayouts::default(),
            Rc::new(Taskbar),
            Box::new(Displays(vec![source, destination])),
            Box::new(scenes.clone()),
            runtime,
            KeymapPublisher::new(KeymapView::new(), keymap),
            HashMap::new(),
        )
        .unwrap();
        dome.apply_layout();
        let (intake, messages) = mpsc::channel();
        let (events, reports) = mpsc::channel();
        let hub = HubSender {
            thread_id: 0,
            events,
        };
        let callbacks = Rc::new(OnceCell::new());
        assert!(
            callbacks
                .set(WindowLoopCallbacks {
                    wake: Box::new(move |reason| {
                        wake.send(reason).unwrap();
                    }),
                    wake_at: Box::new(|_, _| {}),
                    stop_wakes: Box::new(|| {}),
                    terminate: Box::new(|| {}),
                })
                .is_ok()
        );
        let output = Rc::new(RefCell::new(Output::default()));
        let handler = WindowLoopHandler {
            ui: UiState::new(messages, hub.clone(), Box::new(output.clone())),
            windows: Box::new(server),
            callbacks,
            hub_sender: hub,
            observe_foreground: Box::new(|| {}),
            stopping: false,
        };
        Self {
            handler,
            dome,
            _configuration: configuration,
            scenes,
            intake,
            reports,
            native,
            wakes,
            native_reads,
            schedule,
            output,
            now: Instant::now() - Duration::from_secs(10),
            latest: None,
        }
    }
    fn discover(&mut self, key: HwndId, placement: PlacementRead) {
        self.inspect(key, placement);
        self.deliver_reports();
    }
    fn inspect(&mut self, key: HwndId, placement: PlacementRead) {
        self.inspect_frame(key, placement, PixelRect::new(-1700, 200, 600, 500));
    }
    fn inspect_frame(&mut self, key: HwndId, placement: PlacementRead, frame: PixelRect) {
        let (left, top, right, bottom) = placement.invisible_border;
        let outer = RECT {
            left: frame.x().value() - left.value(),
            top: frame.y().value() - top.value(),
            right: frame.right().value() + right.value(),
            bottom: frame.bottom().value() + bottom.value(),
        };
        let parent = Window {
            outer: Some(outer),
            frame: Some(rectangle(frame)),
            owner: None,
            thread: 1,
            valid: true,
            visible: true,
            unaware: placement.unaware,
            minimized: false,
            maximized: true,
            monitor: 1,
        };
        let mut windows = self.native_reads.windows.lock().unwrap();
        windows.insert(key, parent.clone());
        for child in placement.owned_windows {
            let (x, y) = child.offset.unwrap();
            windows.insert(
                child.window,
                Window {
                    outer: Some(RECT {
                        left: outer.left + x.value(),
                        top: outer.top + y.value(),
                        right: outer.left + x.value() + 100,
                        bottom: outer.top + y.value() + 100,
                    }),
                    frame: None,
                    owner: Some(key),
                    visible: false,
                    ..parent.clone()
                },
            );
        }
        drop(windows);
        self.native.send(NativeEvent::Discover(key));
        self.complete_reads(1);
        assert!(self.ui().is_empty(), "discovery cannot admit writes");
    }
    fn complete_reads(&mut self, count: usize) {
        self.dispatch_reads(count);
        self.handler.process_window_server_at(self.now);
    }
    fn dispatch_reads(&mut self, count: usize) {
        while self.wakes.try_recv().is_ok() {}
        self.handler.process_window_server_at(self.now);
        for _ in 0..count {
            assert!(matches!(
                self.wakes.recv_timeout(Duration::from_secs(2)).unwrap(),
                Wake::WindowServer
            ));
        }
    }
    fn ui(&mut self) -> Vec<Submission> {
        let reads = self.native_reads.clone();
        fixtures::with(reads, || {
            submission::record_submissions(|| {
                submission::without_queries(|| {
                    assert!(
                        self.handler
                            .process_wake_at(Wake::Ui, self.now)
                            .is_continue()
                    );
                })
            })
        })
    }
    fn deliver_reports(&mut self) {
        while let Ok(report) = self.reports.try_recv() {
            assert!(self.dome.handle_report(report).is_ok());
        }
    }
    fn queue_scenes(&mut self) {
        while let Some(message) = self.scenes.borrow_mut().0.pop_front() {
            if let HubMessage::Scene(scene) = &message {
                self.latest = Some(scene.clone());
            }
            self.intake.send(message).unwrap();
        }
    }
    fn deliver_scenes(&mut self) -> Vec<Submission> {
        self.queue_scenes();
        self.ui()
    }
    fn scene(&mut self, scene: Scene) -> Vec<Submission> {
        self.latest = Some(scene.clone());
        self.intake.send(HubMessage::Scene(scene)).unwrap();
        self.ui()
    }
    fn focus(&mut self) -> Vec<Submission> {
        submission::record_submissions(|| {
            submission::without_queries(|| {
                assert!(
                    self.handler
                        .process_wake_at(Wake::FocusRequest, self.now)
                        .is_continue()
                );
            })
        })
    }
    fn settle(&mut self, key: HwndId) {
        self.now += Duration::from_millis(200);
        self.native.send(NativeEvent::Moved {
            window: key,
            observed_at: self.now,
        });
        self.now += Duration::from_millis(100);
        self.complete_reads(1);
    }
}
fn rectangle(frame: PixelRect) -> RECT {
    RECT {
        left: frame.x().value(),
        top: frame.y().value(),
        right: frame.right().value(),
        bottom: frame.bottom().value(),
    }
}
fn set_borders(reads: &Reads, key: HwndId, borders: (i32, i32, i32, i32)) {
    let mut windows = reads.windows.lock().unwrap();
    let window = windows.get_mut(&key).unwrap();
    let frame = window.frame.unwrap();
    window.outer = Some(RECT {
        left: frame.left - borders.0,
        top: frame.top - borders.1,
        right: frame.right + borders.2,
        bottom: frame.bottom + borders.3,
    });
}
fn count_read(harness: &Harness, key: HwndId, operation: &str) -> usize {
    harness
        .native_reads
        .calls
        .lock()
        .unwrap()
        .iter()
        .filter(|(window, read)| *window == key && *read == operation)
        .count()
}
fn positions(writes: &[Submission], key: HwndId) -> Vec<(i32, i32, i32, i32)> {
    writes
        .iter()
        .filter_map(|write| match write {
            Submission::Position {
                window,
                rectangle,
                flags,
                ..
            } if *window == key && !flags.contains(SWP_NOMOVE) => Some(*rectangle),
            _ => None,
        })
        .collect()
}
fn tile_frame(scene: &Scene, key: HwndId) -> PixelRect {
    let WindowPlacement::Tiling { placement, .. } = scene.windows[&key].placement else {
        panic!("expected tiled scene");
    };
    placement.content_box
}
fn change_tile(scene: &mut Scene, key: HwndId, frame: PixelRect) {
    let WindowPlacement::Tiling { monitor, placement } =
        &mut scene.windows.get_mut(&key).unwrap().placement
    else {
        panic!("expected tile");
    };
    placement.content_box = frame;
    let replacement = *placement;
    let monitor = scene
        .monitors
        .iter_mut()
        .find(|candidate| candidate.id == *monitor)
        .unwrap();
    *monitor
        .tiling_windows
        .iter_mut()
        .find(|candidate| candidate.id == replacement.id)
        .unwrap() = replacement;
}
fn expected_native(
    frame: PixelRect,
    borders: (i32, i32, i32, i32),
    dots: u32,
) -> (i32, i32, i32, i32) {
    let (left, top, right, bottom) = borders;
    let scale = f64::from(dots) / 96.0;
    let edge = |value: i32| (f64::from(value) / scale).round() as i32;
    let x = edge(frame.x().value() - left);
    let y = edge(frame.y().value() - top);
    (
        x,
        y,
        edge(frame.right().value() + right) - x,
        edge(frame.bottom().value() + bottom) - y,
    )
}

#[test]
fn discover_before_initial_scene_uses_monitor_policy_before_admission() {
    for scene_already_queued in [false, true] {
        for fullscreen in [false, true] {
            let mut harness =
                Harness::before_initial_scene(crate::config::tests::tiling_config(), |_, _| {});
            let key = HwndId::test(240);
            if scene_already_queued {
                harness.queue_scenes();
            }
            let frame = if fullscreen {
                PixelRect::new(-1920, 30, 1920, 1050)
            } else {
                PixelRect::new(-1700, 200, 600, 500)
            };
            harness.inspect_frame(key, placement(false, None), frame);
            if !scene_already_queued {
                assert!(harness.reports.try_recv().is_err());
                assert!(harness.deliver_scenes().is_empty());
            }
            let report = harness.reports.try_recv().unwrap();
            let HubEvent::WindowsDiscovered(discoveries) = &report else {
                panic!("expected discovery after first monitor policy");
            };
            assert_eq!(discoveries.len(), 1);
            assert_eq!(discoveries[0].frame, frame);
            assert_eq!(
                discoveries[0].fullscreen,
                fullscreen.then_some(FullscreenKind::Borderless)
            );
            assert!(harness.reports.try_recv().is_err());
            assert!(harness.dome.handle_report(report).is_ok());
            let writes = harness.deliver_scenes();
            let scene = harness.latest.as_ref().unwrap();
            if fullscreen {
                assert!(matches!(
                    scene.windows[&key].placement,
                    WindowPlacement::Fullscreen { .. }
                ));
                assert!(writes.is_empty(), "native fullscreen must not be tiled");
            } else {
                let target = tile_frame(scene, key);
                assert_eq!(
                    positions(&writes, key),
                    vec![expected_native(target, (7, 2, 9, 6), 96)]
                );
            }
        }
    }
}

#[test]
fn deliver_initial_empty_monitor_scene_retains_discovery_until_nonempty_policy() {
    let mut harness =
        Harness::before_initial_scene(crate::config::tests::tiling_config(), |_, _| {});
    let key = HwndId::test(241);
    harness.inspect_frame(
        key,
        placement(false, None),
        PixelRect::new(-1920, 30, 1920, 1050),
    );
    let mut empty = harness
        .scenes
        .borrow()
        .0
        .iter()
        .find_map(|message| match message {
            HubMessage::Scene(scene) => Some(scene.clone()),
            _ => None,
        })
        .unwrap();
    empty.monitors.clear();
    empty.workspaces.clear();
    empty.focus_request = None;
    for _ in 0..2 {
        harness
            .intake
            .send(HubMessage::DisplayRefreshed(Vec::new()))
            .unwrap();
        assert!(harness.scene(empty.clone()).is_empty());
        assert!(harness.reports.try_recv().is_err());
        assert!(harness.ui().is_empty());
        assert!(harness.reports.try_recv().is_err());
    }
    assert!(harness.deliver_scenes().is_empty());
    let report = harness.reports.try_recv().unwrap();
    assert!(matches!(&report, HubEvent::WindowsDiscovered(discoveries)
        if discoveries.len() == 1 && discoveries[0].fullscreen == Some(FullscreenKind::Borderless)));
    assert!(harness.dome.handle_report(report).is_ok());
    assert!(harness.deliver_scenes().is_empty());
    assert!(matches!(
        harness.latest.as_ref().unwrap().windows[&key].placement,
        WindowPlacement::Fullscreen { .. }
    ));
    assert!(harness.reports.try_recv().is_err());
}

#[test]
fn handle_gone_before_initial_scene_preserves_report_order_without_resurrection() {
    let mut harness =
        Harness::before_initial_scene(crate::config::tests::tiling_config(), |_, _| {});
    let key = HwndId::test(242);
    harness.inspect(key, placement(false, None));
    harness.native.send(NativeEvent::Title(key));
    harness.complete_reads(1);
    assert!(harness.ui().is_empty());
    harness.native.send(NativeEvent::Minimized(key));
    harness.handler.process_window_server_at(harness.now);
    assert!(harness.ui().is_empty());
    harness.native.send(NativeEvent::Gone(key));
    harness.handler.process_window_server_at(harness.now);
    assert!(harness.ui().is_empty());
    assert!(harness.reports.try_recv().is_err());
    assert!(harness.deliver_scenes().is_empty());
    let reports: Vec<_> = harness.reports.try_iter().collect();
    assert!(matches!(reports.as_slice(), [
        HubEvent::WindowsDiscovered(discoveries),
        HubEvent::TitleChanged { key: titled, title },
        HubEvent::WindowMinimized(minimized),
        HubEvent::WindowGone(gone),
    ] if discoveries[0].key == key && *titled == key && title == "Native fixture"
        && *minimized == key && *gone == key));
    let mut stale = None;
    for report in reports {
        assert!(harness.dome.handle_report(report).is_ok());
        if stale.is_none() {
            stale = harness.scenes.borrow().0.iter().find_map(|message| {
                if let HubMessage::Scene(scene) = message {
                    Some(scene.clone())
                } else {
                    None
                }
            });
        }
    }
    assert!(harness.deliver_scenes().is_empty());
    assert!(harness.latest.as_ref().unwrap().windows.is_empty());
    let stale = stale.unwrap();
    assert!(stale.windows.contains_key(&key));
    assert!(harness.scene(stale).is_empty());
    assert!(harness.focus().is_empty());
    assert!(harness.reports.try_recv().is_err());
}

#[test]
fn observe_exclusive_around_initial_discovery_preserves_report_order() {
    for exclusive_first in [false, true] {
        let mut harness =
            Harness::before_initial_scene(crate::config::tests::tiling_config(), |_, _| {});
        let key = HwndId::test(243);
        let exclusive = NativeEvent::Foreground {
            window: key,
            internal: false,
            exclusive: Some(key),
        };
        if exclusive_first {
            harness.native.send(exclusive);
            harness.handler.process_window_server_at(harness.now);
            assert!(harness.ui().is_empty());
        }
        harness.inspect(key, placement(false, None));
        if !exclusive_first {
            harness.native.send(exclusive);
            harness.handler.process_window_server_at(harness.now);
            assert!(harness.ui().is_empty());
        }
        assert!(harness.reports.try_recv().is_err());
        assert!(harness.deliver_scenes().is_empty());
        let reports: Vec<_> = harness.reports.try_iter().collect();
        if exclusive_first {
            assert!(matches!(reports.as_slice(), [
                HubEvent::FocusReported(focused), HubEvent::WindowsDiscovered(discoveries)
            ] if *focused == key && discoveries[0].fullscreen == Some(FullscreenKind::Exclusive)));
        } else {
            assert!(matches!(reports.as_slice(), [
                HubEvent::WindowsDiscovered(discoveries),
                HubEvent::FullscreenChanged { key: changed, fullscreen: Some(FullscreenKind::Exclusive) },
                HubEvent::FocusReported(focused),
            ] if discoveries[0].fullscreen.is_none() && *changed == key && *focused == key));
        }
        for report in reports {
            assert!(harness.dome.handle_report(report).is_ok());
        }
        assert!(harness.deliver_scenes().is_empty());
        assert!(harness.focus().is_empty());
        assert!(matches!(
            harness.latest.as_ref().unwrap().windows[&key].placement,
            WindowPlacement::Fullscreen { .. }
        ));
    }
}

#[test]
fn discover_before_monitor_policy_preserves_later_pre_admission_maximized_fact() {
    let mut harness =
        Harness::before_initial_scene(crate::config::tests::tiling_config(), |_, _| {});
    let key = HwndId::test(244);
    harness.inspect(key, placement(false, None));
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .maximized = false;
    harness.settle(key);
    assert!(harness.ui().is_empty());
    assert!(harness.reports.try_recv().is_err());
    assert!(harness.deliver_scenes().is_empty());
    harness.deliver_reports();
    assert!(!harness.deliver_scenes().is_empty());
    let recovery = submission::record_submissions(|| {
        submission::without_queries(|| harness.handler.stop());
    });
    assert_eq!(positions(&recovery, key), vec![(100, 100, 0, 0)]);
    assert_eq!(recovery.len(), 1, "latest fact must prevent re-maximizing");
}

#[test]
fn shut_down_before_monitor_policy_discards_pending_discovery() {
    for scene_already_queued in [false, true] {
        let mut harness =
            Harness::before_initial_scene(crate::config::tests::tiling_config(), |_, _| {});
        let key = HwndId::test(245);
        harness.inspect(key, placement(false, None));
        if scene_already_queued {
            harness.queue_scenes();
        }
        harness.intake.send(HubMessage::Shutdown).unwrap();
        let writes = submission::record_submissions(|| {
            submission::without_queries(|| {
                assert!(harness.handler.process_ui_at(harness.now).is_break());
                harness.handler.stop();
            });
        });
        assert!(writes.is_empty());
        assert!(harness.reports.try_recv().is_err());
        assert!(harness.schedule.lock().stopped);
    }
}

#[test]
fn request_ui_limits_defers_dispatch_until_empty_server_batch() {
    let mut harness = Harness::new();
    let key = HwndId::test(201);
    harness.discover(key, placement(false, None));
    assert_eq!(count_read(&harness, key, "limits"), 1);
    assert!(!harness.deliver_scenes().is_empty());
    assert!(
        count_read(&harness, key, "limits") == 1,
        "UI demand must not dispatch inline"
    );
    assert!(harness.schedule.lock().windows[&key].size_limits.pending);
    assert!(
        harness
            .wakes
            .try_iter()
            .any(|wake| wake == Wake::WindowServer)
    );
    harness.complete_reads(1);
    assert_eq!(count_read(&harness, key, "limits"), 2);
    harness
        .intake
        .send(HubMessage::DisplayRefreshed(vec![monitor(
            PixelRect::new(0, 0, 3000, 2000),
            144,
        )]))
        .unwrap();
    assert!(harness.ui().is_empty());
    assert_eq!(count_read(&harness, key, "limits"), 2);
    harness.complete_reads(1);
    assert_eq!(count_read(&harness, key, "limits"), 3);
}

#[test]
fn accept_settle_installs_native_inputs_before_handler_correction() {
    let mut harness = Harness::new();
    let key = HwndId::test(202);
    harness.discover(key, placement(false, None));
    let first = harness.deliver_scenes();
    let frame = tile_frame(harness.latest.as_ref().unwrap(), key);
    assert_eq!(
        positions(&first, key),
        vec![expected_native(frame, (7, 2, 9, 6), 96)]
    );
    harness.complete_reads(1);
    set_borders(&harness.native_reads, key, (13, 5, 17, 11));
    let before = (
        harness.output.borrow().painted,
        harness.output.borrow().stacked,
    );
    harness.settle(key);
    let corrected = harness.ui();
    assert_eq!(
        positions(&corrected, key),
        vec![expected_native(frame, (13, 5, 17, 11), 96)]
    );
    assert_eq!(
        (
            harness.output.borrow().painted,
            harness.output.borrow().stacked
        ),
        before
    );
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .frame = Some(rectangle(frame));
    set_borders(&harness.native_reads, key, (13, 5, 17, 11));
    harness.settle(key);
    assert!(
        harness.ui().is_empty(),
        "accepted target frame needs no correction"
    );
}

#[test]
fn move_window_between_monitors_uses_destination_dpi_until_accepted_settle() {
    let mut harness = Harness::new();
    let key = HwndId::test(203);
    harness.discover(key, placement(true, None));
    harness.deliver_scenes();
    harness.complete_reads(1);
    harness.native_reads.calls.lock().unwrap().clear();
    let accepted_before = harness.native_reads.windows.lock().unwrap()[&key].frame;
    harness
        .dome
        .run_binding("main", &"ctrl+l".parse().unwrap(), 0);
    let writes = harness.deliver_scenes();
    let frame = tile_frame(harness.latest.as_ref().unwrap(), key);
    assert!(
        frame.x().value() >= 0,
        "binding must move onto destination monitor"
    );
    assert_eq!(
        positions(&writes, key),
        vec![expected_native(frame, (7, 2, 9, 6), 144)]
    );
    assert!(
        count_read(&harness, key, "outer") == 0,
        "crossing a monitor must not force a native read"
    );
    assert_eq!(
        harness.native_reads.windows.lock().unwrap()[&key].frame,
        accepted_before
    );
    assert!(
        harness.ui().is_empty(),
        "scene-free delivery cannot correct cached drift"
    );
    assert!(positions(&harness.scene(harness.latest.clone().unwrap()), key).is_empty());
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .monitor = 2;
    set_borders(&harness.native_reads, key, (15, 4, 21, 8));
    harness.settle(key);
    let writes = harness.ui();
    assert_eq!(
        positions(&writes, key),
        vec![expected_native(frame, (15, 4, 21, 8), 144)]
    );
    assert!(harness.schedule.lock().windows[&key].size_limits.pending);
}

#[test]
fn write_scene_targets_preserves_owned_physical_offsets_until_refresh() {
    let mut harness = Harness::new();
    let parent = HwndId::test(204);
    let child = HwndId::test(205);
    harness.discover(parent, placement(true, Some((child, 20, 30))));
    harness.deliver_scenes();
    harness.complete_reads(1);
    let mut scene = harness.latest.clone().unwrap();
    for (frame, child_target) in [
        (PixelRect::new(1500, 200, 500, 400), (1513, 228, 0, 0)),
        (PixelRect::new(1800, 300, 500, 400), (1813, 328, 0, 0)),
    ] {
        change_tile(&mut scene, parent, frame);
        let writes = harness.scene(scene.clone());
        assert_eq!(
            positions(&writes, parent),
            vec![expected_native(frame, (7, 2, 9, 6), 144)]
        );
        assert_eq!(positions(&writes, child), vec![child_target]);
        let context_restore = writes
            .iter()
            .position(|write| *write == Submission::Context(-4))
            .unwrap();
        let child_write = writes
            .iter()
            .position(
                |write| matches!(write, Submission::Position { window, .. } if *window == child),
            )
            .unwrap();
        assert!(context_restore < child_write);
    }
    {
        let mut windows = harness.native_reads.windows.lock().unwrap();
        let parent_outer = windows[&parent].outer.unwrap();
        let child_outer = windows.get_mut(&child).unwrap().outer.as_mut().unwrap();
        assert_eq!(
            (
                child_outer.left - parent_outer.left,
                child_outer.top - parent_outer.top
            ),
            (20, 30)
        );
        child_outer.left = parent_outer.left + 80;
        child_outer.top = parent_outer.top + 90;
    }
    harness.settle(parent);
    assert_eq!(positions(&harness.ui(), child), vec![(1873, 388, 0, 0)]);
    change_tile(&mut scene, parent, PixelRect::new(1600, 250, 500, 400));
    assert_eq!(
        positions(&harness.scene(scene), child),
        vec![(1673, 338, 0, 0)]
    );
}

#[test]
fn move_scene_across_full_display_bounds_preserves_destination_controls() {
    let mut harness = Harness::new();
    let key = HwndId::test(210);
    harness.discover(key, placement(true, None));
    harness.deliver_scenes();
    harness.complete_reads(1);
    harness
        .dome
        .run_binding("main", &"ctrl+l".parse().unwrap(), 0);
    harness.deliver_scenes();
    let mut scene = harness.latest.clone().unwrap();
    for (outer, dots) in [
        (PixelRect::<Physical>::new(-100, 0, 400, 300), 144),
        (PixelRect::new(-200, 0, 400, 300), 96),
        (PixelRect::new(-1500, 100, 600, 600), 96),
        (PixelRect::new(3000, 150, 600, 600), 144),
        (PixelRect::new(-200, -600, 400, 300), 96),
    ] {
        let frame = PixelRect::new(
            outer.x().value() + 7,
            outer.y().value() + 2,
            outer.width().value() - 16,
            outer.height().value() - 8,
        );
        change_tile(&mut scene, key, frame);
        assert_eq!(
            positions(&harness.scene(scene.clone()), key),
            vec![expected_native(frame, (7, 2, 9, 6), dots)]
        );
    }
    harness
        .intake
        .send(HubMessage::DisplayRefreshed(vec![monitor(
            PixelRect::new(0, 0, 2560, 1440),
            0,
        )]))
        .unwrap();
    let frame = PixelRect::new(150, 150, 600, 600);
    change_tile(&mut scene, key, frame);
    assert_eq!(
        positions(&harness.scene(scene), key),
        vec![(143, 148, 616, 608)]
    );
}

#[test]
fn read_placement_with_failed_and_partial_geometry_preserves_fallbacks() {
    let mut harness = Harness::new();
    let key = HwndId::test(211);
    harness.discover(key, placement(false, None));
    harness.deliver_scenes();
    harness.complete_reads(1);
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .valid = false;
    harness.settle(key);
    assert!(harness.ui().is_empty());
    let mut scene = harness.latest.clone().unwrap();
    let frame = PixelRect::new(-1300, 200, 700, 650);
    change_tile(&mut scene, key, frame);
    assert_eq!(
        positions(&harness.scene(scene), key),
        vec![(-1307, 198, 716, 658)]
    );
    {
        let mut windows = harness.native_reads.windows.lock().unwrap();
        let window = windows.get_mut(&key).unwrap();
        window.valid = true;
        window.outer = None;
    }
    harness.settle(key);
    assert_eq!(positions(&harness.ui(), key), vec![(-1300, 200, 700, 650)]);
}

#[test]
fn ingest_movement_and_removal_before_settle_results_preserves_handler_inputs() {
    let mut harness = Harness::new();
    let key = HwndId::test(212);
    harness.discover(key, placement(false, None));
    harness.deliver_scenes();
    harness.complete_reads(1);
    set_borders(&harness.native_reads, key, (13, 5, 17, 11));
    harness.now += Duration::from_millis(200);
    harness.native.send(NativeEvent::MoveEnded {
        window: key,
        observed_at: harness.now,
    });
    harness.dispatch_reads(1);
    harness.native.send(NativeEvent::Moved {
        window: key,
        observed_at: harness.now,
    });
    harness.handler.process_window_server_at(harness.now);
    assert!(
        harness.ui().is_empty(),
        "moving-result rejection must precede UI correction"
    );
    let mut scene = harness.latest.clone().unwrap();
    let target = PixelRect::new(-1300, 200, 700, 650);
    change_tile(&mut scene, key, target);
    assert_eq!(
        positions(&harness.scene(scene.clone()), key),
        vec![(-1307, 198, 716, 658)]
    );
    harness.now += Duration::from_millis(100);
    harness.complete_reads(1);
    assert_eq!(positions(&harness.ui(), key), vec![(-1313, 195, 730, 666)]);
    harness.now += Duration::from_millis(200);
    harness.native.send(NativeEvent::MoveEnded {
        window: key,
        observed_at: harness.now,
    });
    harness.dispatch_reads(1);
    harness.native.send(NativeEvent::Gone(key));
    harness.handler.process_window_server_at(harness.now);
    assert!(harness.ui().is_empty());
    change_tile(&mut scene, key, PixelRect::new(-1200, 300, 700, 650));
    assert!(
        harness.scene(scene).is_empty(),
        "a removed window cannot consume a late native input"
    );
}

#[test]
fn route_internal_focus_invalidates_observed_external_shortcut() {
    let mut harness = Harness::new();
    let key = HwndId::test(206);
    harness.discover(key, placement(false, None));
    harness.deliver_scenes();
    harness.complete_reads(1);
    *harness.native_reads.foreground.lock().unwrap() = Some(key);
    fixtures::with(harness.native_reads.clone(), || {
        harness.native.observe_foreground(Some(key))
    });
    harness.handler.process_window_server_at(harness.now);
    harness.ui();
    assert!(
        harness.focus().is_empty(),
        "observed external foreground skips redundant submission"
    );
    let mut scene = harness.latest.clone().unwrap();
    let monitor = scene.monitors[0].id;
    scene.focus_request = Some(FocusTarget::Monitor(monitor));
    harness.scene(scene.clone());
    assert_eq!(harness.output.borrow().focused.last(), Some(&monitor));
    scene.focus_request = Some(FocusTarget::Window(key));
    harness.scene(scene);
    harness.now += Duration::from_millis(100);
    assert_eq!(harness.focus(), vec![Submission::Foreground(key)]);
    harness.reports.try_iter().for_each(drop);
    let internal = HwndId::test(900);
    let mut own_window = harness.native_reads.windows.lock().unwrap()[&key].clone();
    own_window.thread = u32::MAX;
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .insert(internal, own_window);
    *harness.native_reads.foreground.lock().unwrap() = Some(internal);
    fixtures::with(harness.native_reads.clone(), || {
        harness.native.observe_foreground(Some(internal))
    });
    harness.handler.process_window_server_at(harness.now);
    harness.ui();
    assert!(
        !harness
            .reports
            .try_iter()
            .any(|report| matches!(report, HubEvent::FocusReported(_)))
    );
    let other = HwndId::test(901);
    let external_window = harness.native_reads.windows.lock().unwrap()[&key].clone();
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .insert(other, external_window);
    *harness.native_reads.foreground.lock().unwrap() = Some(other);
    fixtures::with(harness.native_reads.clone(), || {
        harness.native.observe_foreground(Some(other))
    });
    harness.handler.process_window_server_at(harness.now);
    harness.ui();
    assert!(
        harness
            .reports
            .try_iter()
            .any(|report| matches!(report, HubEvent::FocusReported(key) if key == other))
    );
}

#[test]
fn submit_handler_scenes_commands_focus_and_recovery_without_native_queries() {
    let mut harness = Harness::new();
    let key = HwndId::test(207);
    harness.discover(key, placement(false, None));
    let queries = submission::query_count();
    assert!(!harness.deliver_scenes().is_empty());
    assert!(
        submission::query_count() > queries,
        "explicit scene radius read is a separate read phase"
    );
    harness.complete_reads(1);
    let mut scene = harness.latest.clone().unwrap();
    harness.native.send(NativeEvent::MoveStarted {
        window: key,
        observed_at: harness.now,
    });
    change_tile(&mut scene, key, PixelRect::new(-1400, 200, 800, 700));
    assert_eq!(
        positions(&harness.scene(scene.clone()), key),
        vec![(-1407, 198, 816, 708)]
    );
    let monitor = scene.monitors.iter().find(|m| m.native == 1).unwrap().id;
    let id = scene.windows[&key].id;
    let float = FloatWindowPlacement {
        id,
        border_box: PixelRect::new(-1000, 200, 500, 400),
        visible_border_box: PixelRect::new(-1000, 200, 500, 400),
        content_box: PixelRect::new(-1000, 200, 500, 400),
        is_highlighted: true,
    };
    scene.windows.get_mut(&key).unwrap().placement = WindowPlacement::Float {
        monitor,
        placement: float,
    };
    let output = scene.monitors.iter_mut().find(|m| m.id == monitor).unwrap();
    output.tiling_windows.clear();
    output.float_windows = vec![float];
    let floated = harness.scene(scene.clone());
    assert_eq!(positions(&floated, key), vec![(-1007, 198, 516, 408)]);
    assert!(floated.iter().any(
        |write| matches!(write, Submission::Position { flags, .. } if flags.contains(SWP_NOMOVE))
    ));
    scene.windows.get_mut(&key).unwrap().placement = WindowPlacement::Fullscreen { monitor };
    scene
        .monitors
        .iter_mut()
        .find(|m| m.id == monitor)
        .unwrap()
        .float_windows
        .clear();
    assert_eq!(positions(&harness.scene(scene.clone()), key).len(), 1);
    scene.windows.get_mut(&key).unwrap().placement = WindowPlacement::Hidden;
    assert_eq!(
        positions(&harness.scene(scene.clone()), key),
        vec![(-32000, -32000, 0, 0)]
    );
    let fullscreen = scene
        .monitors
        .iter()
        .find(|candidate| candidate.id == monitor)
        .unwrap()
        .system_work_area;
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .frame = Some(rectangle(fullscreen));
    set_borders(&harness.native_reads, key, (7, 2, 9, 6));
    harness.settle(key);
    assert_eq!(
        harness.ui(),
        vec![Submission::Show {
            window: key,
            command: windows::Win32::UI::WindowsAndMessaging::SW_MINIMIZE
        }]
    );
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .frame = Some(rectangle(PixelRect::new(-1700, 200, 600, 500)));
    set_borders(&harness.native_reads, key, (7, 2, 9, 6));
    harness.settle(key);
    let resurfaced = harness.ui();
    assert!(
        matches!(resurfaced.first(), Some(Submission::Show { window, command }) if *window == key && *command == windows::Win32::UI::WindowsAndMessaging::SW_RESTORE)
    );
    assert_eq!(positions(&resurfaced, key), vec![(-32000, -32000, 0, 0)]);
    harness
        .intake
        .send(HubMessage::Command(WindowCommand::Close(key)))
        .unwrap();
    harness
        .intake
        .send(HubMessage::Command(WindowCommand::Close(key)))
        .unwrap();
    harness
        .intake
        .send(HubMessage::Command(WindowCommand::Unminimize(key)))
        .unwrap();
    assert_eq!(
        harness.ui(),
        vec![
            Submission::Close(key),
            Submission::Close(key),
            Submission::Show {
                window: key,
                command: windows::Win32::UI::WindowsAndMessaging::SW_RESTORE
            }
        ]
    );
    assert_eq!(harness.focus(), vec![Submission::Foreground(key)]);
    harness.intake.send(HubMessage::Shutdown).unwrap();
    let recovery = submission::record_submissions(|| {
        submission::without_queries(|| {
            assert!(
                harness
                    .handler
                    .process_wake_at(Wake::Ui, harness.now)
                    .is_break()
            );
            harness.handler.stop();
        })
    });
    assert_eq!(
        recovery,
        vec![
            Submission::Show {
                window: key,
                command: windows::Win32::UI::WindowsAndMessaging::SW_RESTORE
            },
            Submission::Position {
                window: key,
                after: None,
                rectangle: (100, 100, 0, 0),
                flags: SWP_ASYNCWINDOWPOS | SWP_NOACTIVATE | SWP_NOZORDER | SWP_NOSIZE
            },
            Submission::Show {
                window: key,
                command: windows::Win32::UI::WindowsAndMessaging::SW_MAXIMIZE
            }
        ]
    );
}

#[test]
fn discover_unmanaged_child_with_quiet_movement_refreshes_owner_offset() {
    let mut server = server();
    let parent = HwndId::test(208);
    let child = HwndId::test(209);
    let now = Instant::now() - Duration::from_secs(2);
    register(&mut server, parent, now);
    server
        .schedule
        .lock()
        .windows
        .get_mut(&parent)
        .unwrap()
        .discovery
        .pending = false;
    install(&mut server, parent, placement(false, None), now);
    let reads = Reads::default();
    reads.windows.lock().unwrap().insert(
        parent,
        Window {
            outer: Some(RECT {
                left: 100,
                top: 100,
                right: 700,
                bottom: 600,
            }),
            frame: Some(RECT {
                left: 107,
                top: 102,
                right: 691,
                bottom: 594,
            }),
            owner: None,
            thread: 42,
            valid: true,
            visible: true,
            unaware: false,
            minimized: false,
            maximized: false,
            monitor: 1,
        },
    );
    reads.windows.lock().unwrap().insert(
        child,
        Window {
            outer: Some(RECT {
                left: 120,
                top: 130,
                right: 220,
                bottom: 230,
            }),
            frame: None,
            owner: Some(parent),
            thread: 42,
            valid: true,
            visible: true,
            unaware: false,
            minimized: false,
            maximized: false,
            monitor: 1,
        },
    );
    server
        .dispatcher
        .as_mut()
        .unwrap()
        .set_native_reads(reads.clone());
    register(&mut server, child, now);
    server.read_settled_windows(now);
    let result = server.results.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(result.kind, InspectionKind::Discovery);
    server.accept_result(result, now);
    assert!(server.take_observations().is_empty());
    assert_eq!(server.owners[&child], parent);
    let calls = reads.calls.lock().unwrap().clone();
    assert_eq!(
        &calls[..3],
        &[(child, "root_owner"), (child, "thread"), (parent, "thread")]
    );
    assert!(
        calls
            .iter()
            .position(|call| *call == (child, "owner"))
            .unwrap()
            > 2
    );
    let before = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(parent, PixelRect::new(300, 300, 500, 400)))
    });
    assert!(
        positions(&before, child).is_empty(),
        "relationship alone has no accepted offset"
    );
    server.read_settled_windows(now);
    let result = server.results.recv_timeout(Duration::from_secs(2)).unwrap();
    assert_eq!(result.kind, InspectionKind::Placement);
    server.accept_result(result, now);
    assert!(server.take_observations().is_empty());
    assert_eq!(
        server.placement[&parent].owned_windows[0].offset,
        Some((Pixels::new(20), Pixels::new(30)))
    );
    reads.calls.lock().unwrap().clear();
    server.native_sender().send(NativeEvent::Moved {
        window: child,
        observed_at: now,
    });
    server.ingest_native_events(now);
    server.read_settled_windows(now + Duration::from_millis(99));
    assert!(server.results.try_recv().is_err());
    assert!(!reads.calls.lock().unwrap().contains(&(child, "outer")));
    let during = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(parent, PixelRect::new(400, 400, 500, 400)))
    });
    assert_eq!(positions(&during, child), vec![(413, 428, 0, 0)]);
    reads.windows.lock().unwrap().get_mut(&child).unwrap().outer = Some(RECT {
        left: 180,
        top: 190,
        right: 280,
        bottom: 290,
    });
    let quiet = now + Duration::from_millis(100);
    server.read_settled_windows(quiet);
    let result = server.results.recv_timeout(Duration::from_secs(2)).unwrap();
    server.native_sender().send(NativeEvent::Moved {
        window: child,
        observed_at: quiet,
    });
    server.ingest_native_events(quiet);
    server.accept_result(result, quiet);
    assert_eq!(
        server.placement[&parent].owned_windows[0].offset,
        Some((Pixels::new(20), Pixels::new(30)))
    );
    server.read_settled_windows(quiet + Duration::from_millis(100));
    let result = server.results.recv_timeout(Duration::from_secs(2)).unwrap();
    server.accept_result(result, quiet + Duration::from_millis(100));
    assert_eq!(
        server.placement[&parent].owned_windows[0].offset,
        Some((Pixels::new(80), Pixels::new(90)))
    );
    assert!(server.take_observations().is_empty());
    server.native_sender().send(NativeEvent::Gone(child));
    server.ingest_native_events(quiet);
    let retired = submission::record_submissions(|| {
        submission::without_queries(|| server.set_frame(parent, PixelRect::new(500, 500, 500, 400)))
    });
    assert!(positions(&retired, child).is_empty());
}

#[test]
fn shut_down_with_blocked_read_returns_without_waiting_and_ignores_late_completion() {
    use std::sync::Mutex;
    use std::sync::atomic::AtomicBool;
    let (entered, body_entered) = mpsc::channel();
    let (release, released) = mpsc::channel();
    let (stopped, shutdown_returned) = mpsc::channel();
    let (finished, checked) = mpsc::channel();
    let worker = std::thread::spawn(move || {
        let armed = Arc::new(AtomicBool::new(false));
        let block = armed.clone();
        let released = Mutex::new(released);
        let (completed, completions) = mpsc::channel();
        let mut harness =
            Harness::configured(crate::config::tests::tiling_config(), |server, reads| {
                reads.before_read = Some(Arc::new(move |_, operation| {
                    if operation == "outer" && block.swap(false, Ordering::SeqCst) {
                        entered.send(()).unwrap();
                        released
                            .lock()
                            .unwrap()
                            .recv_timeout(Duration::from_secs(5))
                            .unwrap();
                    }
                }));
                server.dispatcher.as_mut().unwrap().set_after_job(Arc::new(
                    move |delivered, read_completed| {
                        completed.send((delivered, read_completed)).unwrap()
                    },
                ));
            });
        let key = HwndId::test(220);
        harness.discover(key, placement(false, None));
        harness.deliver_scenes();
        harness.complete_reads(1);
        for _ in 0..2 {
            assert!(completions.recv_timeout(Duration::from_secs(2)).unwrap().0);
        }
        while harness.wakes.try_recv().is_ok() {}
        harness.native_reads.calls.lock().unwrap().clear();
        armed.store(true, Ordering::SeqCst);
        harness.native.send(NativeEvent::MoveEnded {
            window: key,
            observed_at: harness.now,
        });
        harness.handler.process_window_server_at(harness.now);
        body_entered.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(completions.try_recv().is_err());
        harness.intake.send(HubMessage::Shutdown).unwrap();
        let recovery = submission::record_submissions(|| {
            submission::without_queries(|| {
                assert!(
                    harness
                        .handler
                        .process_wake_at(Wake::Ui, harness.now)
                        .is_break()
                );
                harness.handler.stop();
            });
        });
        assert_eq!(positions(&recovery, key), vec![(100, 100, 0, 0)]);
        assert_eq!(recovery.len(), 3);
        assert!(
            completions.try_recv().is_err(),
            "body remains blocked at shutdown"
        );
        assert!(harness.schedule.lock().stopped);
        let paints = harness.output.borrow().painted;
        while harness.wakes.try_recv().is_ok() {}
        drop(harness.handler);
        stopped.send(()).unwrap();
        assert_eq!(
            completions.recv_timeout(Duration::from_secs(2)).unwrap(),
            (false, true)
        );
        assert!(
            harness.wakes.try_recv().is_err(),
            "receiver loss must not wake UI"
        );
        assert_eq!(harness.output.borrow().painted, paints);
        assert!(harness.reports.try_recv().is_err());
        assert!(
            harness
                .native_reads
                .calls
                .lock()
                .unwrap()
                .contains(&(key, "awareness"))
        );
        finished.send(()).unwrap();
    });
    shutdown_returned
        .recv_timeout(Duration::from_secs(2))
        .unwrap();
    release.send(()).unwrap();
    checked.recv_timeout(Duration::from_secs(2)).unwrap();
    worker.join().unwrap();
}

#[derive(Clone, Default)]
struct AdmissionLog(Arc<std::sync::Mutex<Vec<u8>>>);
impl std::io::Write for AdmissionLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
impl<'writer> tracing_subscriber::fmt::MakeWriter<'writer> for AdmissionLog {
    type Writer = Self;
    fn make_writer(&'writer self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn admit_after_native_loss_formats_and_logs_discovered_process_identity() {
    for ignored in [false, true] {
        let mut tiling = crate::config::tests::tiling_config();
        if ignored {
            tiling.ignore.push(crate::core::WindowMatcher {
                process: Some("fixture.exe".into()),
                ..Default::default()
            });
        }
        let mut harness = Harness::configured(tiling, |_, _| {});
        let key = HwndId::test(221);
        harness.inspect(key, placement(false, None));
        let report = harness.reports.try_recv().unwrap();
        let HubEvent::WindowsDiscovered(discoveries) = report else {
            panic!("expected concrete discovery");
        };
        assert_eq!(discoveries.len(), 1);
        assert_eq!(discoveries[0].process_id, 1);
        assert!(count_read(&harness, key, "process_id") > 0);
        harness.native_reads.windows.lock().unwrap().clear();
        let log = AdmissionLog::default();
        let subscriber = tracing_subscriber::fmt()
            .with_max_level(tracing::Level::TRACE)
            .without_time()
            .with_ansi(false)
            .with_writer(log.clone())
            .finish();
        fixtures::with(harness.native_reads.clone(), || {
            submission::without_queries(|| {
                assert!(discoveries[0].to_string().starts_with("[pid=1|hwnd="));
                tracing::subscriber::with_default(subscriber, || {
                    assert!(
                        harness
                            .dome
                            .handle_report(HubEvent::WindowsDiscovered(discoveries))
                            .is_ok()
                    );
                });
            });
        });
        let log = String::from_utf8(log.0.lock().unwrap().clone()).unwrap();
        let message = if ignored {
            "ignored by rule"
        } else {
            "New window"
        };
        let admission = log.lines().find(|line| line.contains(message)).unwrap();
        assert!(
            admission
                .split_whitespace()
                .any(|field| field == "process_id=1"),
            "{admission}"
        );
        let scene = harness
            .scenes
            .borrow()
            .0
            .iter()
            .rev()
            .find_map(|message| {
                if let HubMessage::Scene(scene) = message {
                    Some(scene.clone())
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(scene.windows.contains_key(&key), !ignored);
    }
}

#[test]
fn discover_child_with_other_thread_or_self_root_does_not_refresh_parent() {
    for different_thread in [false, true] {
        let mut harness = Harness::new();
        let parent = HwndId::test(222);
        let child = HwndId::test(223);
        harness.discover(parent, placement(false, None));
        harness.deliver_scenes();
        harness.complete_reads(1);
        let mut child_read = harness.native_reads.windows.lock().unwrap()[&parent].clone();
        child_read.owner = Some(if different_thread { parent } else { child });
        child_read.thread = if different_thread { 2 } else { 1 };
        harness
            .native_reads
            .windows
            .lock()
            .unwrap()
            .insert(child, child_read);
        harness.native_reads.calls.lock().unwrap().clear();
        harness.native.send(NativeEvent::Discover(child));
        harness.complete_reads(1);
        assert!(harness.ui().is_empty());
        assert!(harness.reports.try_recv().is_err());
        let schedule = harness.schedule.lock();
        assert!(!schedule.windows[&parent].placement.pending);
        assert!(!schedule.windows[&parent].placement.in_flight);
        drop(schedule);
        assert_eq!(count_read(&harness, parent, "outer"), 0);
        assert_eq!(count_read(&harness, child, "root_owner"), 1);
        assert_eq!(
            count_read(&harness, parent, "thread"),
            usize::from(different_thread)
        );
    }
}

#[test]
fn accept_owned_enumeration_without_prior_child_stops_propagation() {
    for removal in ["thread", "root_owner", "enumeration"] {
        let mut harness = Harness::new();
        let parent = HwndId::test(224);
        let child = HwndId::test(225);
        harness.discover(parent, placement(false, Some((child, 20, 30))));
        assert!(!positions(&harness.deliver_scenes(), child).is_empty());
        harness.complete_reads(1);
        {
            let mut windows = harness.native_reads.windows.lock().unwrap();
            match removal {
                "thread" => windows.get_mut(&child).unwrap().thread = 2,
                "root_owner" => windows.get_mut(&child).unwrap().owner = None,
                "enumeration" => {
                    windows.remove(&child);
                }
                _ => unreachable!(),
            }
        }
        let mut scene = harness.latest.clone().unwrap();
        change_tile(&mut scene, parent, PixelRect::new(-1300, 200, 700, 650));
        assert_eq!(
            positions(&harness.scene(scene.clone()), child),
            vec![(-1287, 228, 0, 0)]
        );
        harness.native_reads.calls.lock().unwrap().clear();
        harness.settle(parent);
        let corrected = harness.ui();
        assert!(!positions(&corrected, parent).is_empty(), "{removal}");
        assert!(positions(&corrected, child).is_empty(), "{removal}");
        assert_eq!(count_read(&harness, child, "outer"), 0);
        change_tile(&mut scene, parent, PixelRect::new(-1200, 200, 700, 650));
        assert!(
            positions(&harness.scene(scene), child).is_empty(),
            "{removal}"
        );
    }
}

#[test]
fn accept_unreadable_owned_child_preserves_prior_offset() {
    let mut harness = Harness::new();
    let parent = HwndId::test(226);
    let child = HwndId::test(227);
    harness.discover(parent, placement(false, Some((child, 20, 30))));
    harness.deliver_scenes();
    harness.complete_reads(1);
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&child)
        .unwrap()
        .outer = None;
    let mut scene = harness.latest.clone().unwrap();
    let target = PixelRect::new(-1300, 200, 700, 650);
    change_tile(&mut scene, parent, target);
    harness.scene(scene);
    harness.native_reads.calls.lock().unwrap().clear();
    harness.settle(parent);
    assert_eq!(count_read(&harness, child, "outer"), 1);
    assert_eq!(positions(&harness.ui(), child), vec![(-1287, 228, 0, 0)]);
    let parent_outer = harness.native_reads.windows.lock().unwrap()[&parent]
        .outer
        .unwrap();
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&child)
        .unwrap()
        .outer = Some(RECT {
        left: parent_outer.left + 80,
        top: parent_outer.top + 90,
        right: parent_outer.left + 180,
        bottom: parent_outer.top + 190,
    });
    harness.settle(parent);
    assert_eq!(positions(&harness.ui(), child), vec![(-1227, 288, 0, 0)]);
}

#[test]
fn settle_without_dwm_frame_uses_outer_geometry_and_zero_borders() {
    let mut harness = Harness::new();
    let key = HwndId::test(228);
    harness.discover(key, placement(false, None));
    harness.deliver_scenes();
    harness.complete_reads(1);
    let target = tile_frame(harness.latest.as_ref().unwrap(), key);
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .frame = None;
    harness.native_reads.calls.lock().unwrap().clear();
    harness.settle(key);
    assert!(count_read(&harness, key, "extended_frame") > 0);
    assert!(count_read(&harness, key, "outer") > 0);
    assert_eq!(
        positions(&harness.ui(), key),
        vec![expected_native(target, (0, 0, 0, 0), 96)]
    );
    harness
        .native_reads
        .windows
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .outer = Some(rectangle(target));
    harness.settle(key);
    assert!(
        harness.ui().is_empty(),
        "accepted outer target needs no correction"
    );
}

#[test]
fn settle_with_failed_owned_enumeration_retains_borders_and_membership() {
    let mut harness = Harness::new();
    let parent = HwndId::test(229);
    let child = HwndId::test(230);
    harness.discover(parent, placement(false, Some((child, 20, 30))));
    harness.deliver_scenes();
    harness.complete_reads(1);
    set_borders(&harness.native_reads, parent, (13, 5, 17, 11));
    harness.native_reads.windows.lock().unwrap().remove(&child);
    *harness.native_reads.enumeration_failed.lock().unwrap() = true;
    harness.settle(parent);
    assert!(harness.ui().is_empty());
    let mut scene = harness.latest.clone().unwrap();
    let target = PixelRect::new(-1300, 200, 700, 650);
    change_tile(&mut scene, parent, target);
    let retained = harness.scene(scene);
    assert_eq!(
        positions(&retained, parent),
        vec![expected_native(target, (7, 2, 9, 6), 96)]
    );
    assert_eq!(positions(&retained, child), vec![(-1287, 228, 0, 0)]);
    *harness.native_reads.enumeration_failed.lock().unwrap() = false;
    harness.settle(parent);
    let refreshed = harness.ui();
    assert_eq!(
        positions(&refreshed, parent),
        vec![expected_native(target, (13, 5, 17, 11), 96)]
    );
    assert!(positions(&refreshed, child).is_empty());
}
