use super::*;
use crate::config::lua::test_support::loaded_runtime;
use crate::core::{Dimension, Length, Physical, PixelRect};
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::Duration;

#[test]
fn coordinate_startup_after_early_ui_exit_finishes_without_readiness() {
    let (finished, completion) = mpsc::channel();
    let (closing, closed_readiness) = mpsc::channel();
    let (release, exit_barrier) = mpsc::channel();
    let domain = thread::spawn(move || {
        let (ready, readiness) = mpsc::channel();
        let ui_thread = thread::spawn(move || {
            drop(ready);
            closing.send(()).unwrap();
            exit_barrier.recv_timeout(Duration::from_secs(5)).unwrap();
        });
        let (startup, startup_intake) = mpsc::channel();
        let (events, _intake) = mpsc::channel();
        coordinate_ui_startup(
            ui_thread,
            readiness,
            startup,
            HubSender {
                thread_id: 0,
                events,
            },
            |_| panic!("startup must not proceed without UI readiness"),
        );
        assert!(startup_intake.try_recv().is_err());
        finished.send(()).unwrap();
    });
    closed_readiness
        .recv_timeout(Duration::from_secs(5))
        .unwrap();
    assert!(
        matches!(
            completion.recv_timeout(Duration::from_millis(50)),
            Err(mpsc::RecvTimeoutError::Timeout)
        ),
        "coordinator returned before joining the failed UI thread"
    );
    release.send(()).unwrap();
    completion
        .recv_timeout(Duration::from_secs(5))
        .expect("early UI exit stranded the coordinator");
    domain.join().unwrap();
}

#[derive(Debug, PartialEq, Eq)]
enum ExitEvent {
    Shutdown,
    Add(HwndId),
    Delete(HwndId),
}

type ExitLog = Rc<RefCell<Vec<(ExitEvent, thread::ThreadId)>>>;
struct RecordingTaskbar(ExitLog);
impl taskbar::ManageTaskbar for RecordingTaskbar {
    fn add_tab(&self, key: HwndId) {
        self.0
            .borrow_mut()
            .push((ExitEvent::Add(key), thread::current().id()));
    }
    fn delete_tab(&self, key: HwndId) {
        self.0
            .borrow_mut()
            .push((ExitEvent::Delete(key), thread::current().id()));
    }
}
struct RecordingScenes(ExitLog);
impl SceneSender for RecordingScenes {
    fn send(&mut self, message: HubMessage) {
        if matches!(message, HubMessage::Shutdown) {
            self.0
                .borrow_mut()
                .push((ExitEvent::Shutdown, thread::current().id()));
        }
    }
}
struct OneDisplay;
impl dome::QueryDisplay for OneDisplay {
    fn get_all_monitors(&self) -> Result<Vec<dome::MonitorInfo>> {
        let bounds = rectangle(0, 0, 1920, 1080);
        Ok(vec![dome::MonitorInfo {
            handle: 1,
            full_rect: bounds,
            work_area: bounds,
            dots_per_inch: 96,
            name: "Startup test".into(),
            gdi_device: "Startup test display".into(),
            is_primary: true,
            scale: 1.0,
        }])
    }
}
fn rectangle(x: i32, y: i32, width: i32, height: i32) -> PixelRect<Physical> {
    PixelRect::from_dimension(Dimension::new(
        Length::new(x as f32),
        Length::new(y as f32),
        Length::new(width as f32),
        Length::new(height as f32),
    ))
}
fn discovery(key: HwndId) -> window_server::DiscoveredWindow {
    window_server::DiscoveredWindow {
        key,
        process_id: 123,
        is_maximized: false,
        metadata: dome::WindowsMetadata {
            title: Some(format!("Window {key}")),
            process: "startup-fixture.exe".into(),
            process_path: None,
            class: Some("StartupFixture".into()),
            aumid: None,
            app_name: None,
        },
        frame: rectangle(100, 100, 640, 480),
        monitor: 1,
        limits: None,
        fullscreen: None,
    }
}

#[test]
fn exit_domain_sends_shutdown_then_restores_live_taskbar_keys_on_domain() {
    for unwind in [false, true] {
        let domain_thread = thread::current().id();
        let log = ExitLog::default();
        let live = HwndId::test(401);
        let removed = HwndId::test(402);
        let minimized = HwndId::test(403);
        let result = catch_unwind(AssertUnwindSafe(|| {
            let (runtime, _file) = loaded_runtime("startup_taskbar", "return {}");
            let (keymap, _intake) = mpsc::channel();
            let mut dome = Dome::new(
                crate::config::tests::tiling_config(),
                PreferredLayouts::default(),
                Rc::new(RecordingTaskbar(log.clone())),
                Box::new(OneDisplay),
                Box::new(RecordingScenes(log.clone())),
                runtime,
                KeymapPublisher::new(KeymapView::new(), keymap),
                HashMap::new(),
            )
            .unwrap();
            dome.windows_discovered(vec![
                discovery(live),
                discovery(removed),
                discovery(minimized),
            ]);
            dome.window_destroyed(removed);
            dome.window_minimized(minimized);
            dome.apply_layout();
            log.borrow_mut().clear();
            assert!(!unwind, "exercise domain unwind");
        }));
        assert_eq!(result.is_err(), unwind);
        let events = log.borrow();
        assert_eq!(
            events.len(),
            3,
            "restore exactly the two live admitted keys"
        );
        assert_eq!(events[0].0, ExitEvent::Shutdown);
        let restored: std::collections::HashSet<_> = events[1..]
            .iter()
            .map(|(event, owner)| {
                assert_eq!(
                    *owner, domain_thread,
                    "Explorer work left the domain thread"
                );
                match event {
                    ExitEvent::Add(key) => *key,
                    other => panic!("unexpected exit action {other:?}"),
                }
            })
            .collect();
        assert_eq!(restored, [live, minimized].into_iter().collect());
        assert_eq!(events[0].1, domain_thread);
    }
}
