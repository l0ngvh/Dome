use super::*;
use crate::platform::macos::dispatcher::{
    AccessibilityApplication, ApplicationCommand, ApplicationNative, ApplicationOperation,
};
use crate::platform::macos::window_server::ApplicationLifetime;
use std::cell::RefCell;
use std::thread;

#[derive(Debug)]
enum Call {
    Add(String, thread::ThreadId),
    Remove(String, thread::ThreadId),
    Dropped(thread::ThreadId),
}
struct Recorder {
    calls: mpsc::Sender<Call>,
    resource: Arc<()>,
    fail_first_add: bool,
    fail_removal: bool,
    removal_gate: Option<mpsc::Receiver<()>>,
}
impl ObserverNative for Recorder {
    fn source(&self) -> CFRetained<objc2_core_foundation::CFRunLoopSource> {
        panic!("barrier and bookkeeping tests do not attach sources")
    }
    fn context(&self) -> Arc<ObserverContext> {
        panic!("barrier and bookkeeping tests do not attach sources")
    }
    fn add(&mut self, _: &DispatcherMarker, notification: &CFString) -> Result<(), AXError> {
        self.calls
            .send(Call::Add(notification.to_string(), thread::current().id()))
            .unwrap();
        if std::mem::take(&mut self.fail_first_add) {
            Err(AXError::InvalidUIElement)
        } else {
            Ok(())
        }
    }
    fn remove(&mut self, _: &DispatcherMarker, notification: &CFString) -> Result<(), AXError> {
        self.calls
            .send(Call::Remove(
                notification.to_string(),
                thread::current().id(),
            ))
            .unwrap();
        if let Some(gate) = self.removal_gate.take() {
            gate.recv().unwrap();
        }
        if self.fail_removal {
            Err(AXError::InvalidUIElement)
        } else {
            Ok(())
        }
    }
}
impl Drop for Recorder {
    fn drop(&mut self) {
        assert!(Arc::strong_count(&self.resource) >= 1);
        let _ = self.calls.send(Call::Dropped(thread::current().id()));
    }
}
pub(in crate::platform::macos) fn removal_barrier() -> RetainedObserverRegistration {
    let (calls, _) = mpsc::channel();
    RetainedObserverRegistration {
        native: Some(Box::new(Recorder {
            calls,
            resource: Arc::new(()),
            fail_first_add: false,
            fail_removal: false,
            removal_gate: None,
        })),
        registered: Vec::new(),
    }
}
struct RegistrationAccess {
    registration: Option<RetainedObserverRegistration>,
}
impl ApplicationNative for RegistrationAccess {
    fn probe(&mut self, _: &DispatcherMarker) {}
    fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker) {
        match operation {
            ApplicationOperation::RegisterObserver => {
                self.registration.as_mut().unwrap().repair(marker)
            }
            ApplicationOperation::Terminate => self.registration.take().unwrap().remove(marker),
            _ => {}
        }
    }
}
#[test]
fn unwind_during_partial_registration_retains_resources_without_messaging_from_drop() {
    struct PartialRegistration {
        recorder: Recorder,
        attempts: usize,
    }
    impl ObserverNative for PartialRegistration {
        fn source(&self) -> CFRetained<objc2_core_foundation::CFRunLoopSource> {
            self.recorder.source()
        }
        fn context(&self) -> Arc<ObserverContext> {
            self.recorder.context()
        }
        fn add(
            &mut self,
            marker: &DispatcherMarker,
            notification: &CFString,
        ) -> Result<(), AXError> {
            self.attempts += 1;
            assert_ne!(self.attempts, 2, "injected partial registration panic");
            self.recorder.add(marker, notification)
        }
        fn remove(
            &mut self,
            marker: &DispatcherMarker,
            notification: &CFString,
        ) -> Result<(), AXError> {
            self.recorder.remove(marker, notification)
        }
    }
    struct Exit(mpsc::Sender<()>);
    impl Drop for Exit {
        fn drop(&mut self) {
            let _ = self.0.send(());
        }
    }
    let (calls, receiver) = mpsc::channel();
    let resource = Arc::new(());
    let weak = Arc::downgrade(&resource);
    let registration = RefCell::new(Some(RetainedObserverRegistration {
        native: Some(Box::new(PartialRegistration {
            recorder: Recorder {
                calls,
                resource,
                fail_first_add: false,
                fail_removal: false,
                removal_gate: None,
            },
            attempts: 0,
        })),
        registered: Vec::new(),
    }));
    let (exited, exits) = mpsc::channel();
    let mut owner = AccessibilityApplication::with_native(
        Arc::new(ApplicationLifetime { pid: 123 }),
        false,
        false,
        Box::new(move |_, _| {
            Box::new(RegistrationAccess {
                registration: registration.borrow_mut().take(),
            })
        }),
        Box::new(move |job| {
            let exited = exited.clone();
            thread::Builder::new().spawn(move || {
                let _exit = Exit(exited);
                job();
            })
        }),
    );
    owner.send(ApplicationCommand::Operation(
        ApplicationOperation::RegisterObserver,
    ));
    let Call::Add(_, on_thread) = receiver.recv_timeout(Duration::from_secs(3)).unwrap() else {
        panic!("expected first registration")
    };
    assert_ne!(on_thread, thread::current().id());
    exits.recv_timeout(Duration::from_secs(3)).unwrap();
    drop(owner);
    assert!(weak.upgrade().is_some());
    assert!(
        receiver.try_recv().is_err(),
        "unwind must not issue native removal or drop uncertain resources"
    );
}

#[test]
fn repair_after_partial_registration_retries_failures_and_retains_resources_during_removal() {
    for fail_removal in [false, true] {
        let (calls, receiver) = mpsc::channel();
        let (release, gate) = mpsc::channel();
        let resource = Arc::new(());
        let weak = Arc::downgrade(&resource);
        let registration = RefCell::new(Some(RetainedObserverRegistration {
            native: Some(Box::new(Recorder {
                calls,
                resource,
                fail_first_add: true,
                fail_removal,
                removal_gate: Some(gate),
            })),
            registered: Vec::new(),
        }));
        let (exited, exit) = mpsc::channel();
        let mut owner = AccessibilityApplication::with_native(
            Arc::new(ApplicationLifetime { pid: 123 }),
            false,
            false,
            Box::new(move |_, _| {
                Box::new(RegistrationAccess {
                    registration: registration.borrow_mut().take(),
                })
            }),
            Box::new(move |job| {
                let exited = exited.clone();
                thread::Builder::new().spawn(move || {
                    job();
                    exited.send(()).unwrap();
                })
            }),
        );
        let next = || receiver.recv_timeout(Duration::from_secs(3)).unwrap();
        owner.send(ApplicationCommand::Operation(
            ApplicationOperation::RegisterObserver,
        ));
        let mut registered = Vec::new();
        let mut worker = None;
        for _ in 0..10 {
            let Call::Add(notification, on_thread) = next() else {
                panic!("expected initial registration")
            };
            assert_ne!(on_thread, thread::current().id());
            assert_eq!(*worker.get_or_insert(on_thread), on_thread);
            registered.push(notification);
        }
        let Call::Add(retried, on_thread) = next() else {
            panic!("expected one failed notification retry")
        };
        assert_eq!(retried, registered[0]);
        assert_eq!(Some(on_thread), worker);
        owner.terminate();
        let Call::Remove(first, on_thread) = next() else {
            panic!("successful notifications must not register twice")
        };
        assert_eq!(first, registered[1]);
        assert_eq!(Some(on_thread), worker);
        drop(owner);
        assert!(
            weak.upgrade().is_some(),
            "receiver loss must not release native resources"
        );
        release.send(()).unwrap();
        let mut removed = vec![first];
        for _ in 1..10 {
            let Call::Remove(notification, on_thread) = next() else {
                panic!("expected worker removal")
            };
            assert_eq!(Some(on_thread), worker);
            removed.push(notification);
        }
        registered.sort();
        removed.sort();
        assert_eq!(registered, removed);
        exit.recv_timeout(Duration::from_secs(3)).unwrap();
        if fail_removal {
            assert!(receiver.try_recv().is_err());
            assert!(
                weak.upgrade().is_some(),
                "uncertain native ownership must retain resources"
            );
        } else {
            let Call::Dropped(on_thread) = next() else {
                panic!("successful removal must release resources")
            };
            assert_eq!(Some(on_thread), worker);
            assert!(weak.upgrade().is_none());
        }
    }
}
