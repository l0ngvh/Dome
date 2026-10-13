use super::Wake;
use super::accessibility::{AXApp, AXWindow};
use super::focus::SharedFocus;
use super::listeners::RetainedObserverRegistration;
use super::window_server::inspect::ApplicationAccess;
use super::window_server::schedule::ApplicationReadSchedule;
use super::window_server::{
    ApplicationEvent, ApplicationLifetime, ApplicationResult, InspectionKind, NativeEvent,
    ObserverContext, RecoveryEntry, TitleElement,
};
use crate::core::{Length, Logical, PixelRect};
use dome_auxiliary_window::LoopWaker;
use objc2::rc::autoreleasepool;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes};
use objc2_core_graphics::CGWindowID;
use std::collections::{HashMap, VecDeque};
use std::io;
use std::sync::{Arc, OnceLock, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Instant;

/// Proof of access from the serial application worker.
pub(in crate::platform::macos) struct DispatcherMarker(());

pub(super) enum ApplicationCommand {
    Event(ApplicationEvent),
    Read(InspectionKind),
    Operation(ApplicationOperation),
    Shutdown(Vec<RecoveryEntry>),
    Terminate,
}
pub(super) enum ApplicationOperation {
    SetFrame {
        window: AXWindow,
        frame: PixelRect<Logical>,
    },
    MoveOffscreen {
        window: AXWindow,
        position: (Length<Logical>, Length<Logical>),
    },
    Minimize(AXWindow),
    Recover {
        window: AXWindow,
        frame: PixelRect<Logical>,
    },
    Focus(AXWindow),
    Close(AXWindow),
    Unminimize(AXWindow),
    Read(InspectionKind),
    ResolveTitle(TitleElement),
    RegisterObserver,
    RemoveObserver(RetainedObserverRegistration),
    Restore(Vec<RecoveryEntry>),
    Terminate,
}

pub(super) trait ApplicationNative: Send {
    fn probe(&mut self, marker: &DispatcherMarker);
    fn execute(&mut self, operation: ApplicationOperation, marker: &DispatcherMarker);
    #[cfg(test)]
    fn selected(&mut self, _operation: &ApplicationOperation, _marker: &DispatcherMarker) {}
}
type WorkerJob = Box<dyn FnOnce() + Send>;
type SpawnWorker = Box<dyn FnMut(WorkerJob) -> io::Result<JoinHandle<()>>>;
type NativeFactory = Box<
    dyn Fn(
        mpsc::Sender<ApplicationResult>,
        HashMap<CGWindowID, AXWindow>,
    ) -> Box<dyn ApplicationNative>,
>;
struct ApplicationWorkerConnection {
    identity: Arc<()>,
    commands: mpsc::Sender<ApplicationCommand>,
    results: mpsc::Receiver<ApplicationResult>,
    _thread: JoinHandle<()>,
}
struct AttachedObserver {
    source: CFRetained<CFRunLoopSource>,
    _context: Arc<ObserverContext>,
    run_loop: CFRetained<CFRunLoop>,
}
impl Drop for AttachedObserver {
    fn drop(&mut self) {
        self.run_loop
            .remove_source(Some(&self.source), unsafe { kCFRunLoopCommonModes });
    }
}
pub(super) struct AccessibilityApplication {
    pub(super) lifetime: Arc<ApplicationLifetime>,
    worker: Option<ApplicationWorkerConnection>,
    waiting_to_start: VecDeque<ApplicationCommand>,
    retry_on_refresh: bool,
    accepting: bool,
    hidden: bool,
    suspended: bool,
    handles: HashMap<CGWindowID, AXWindow>,
    attached: Vec<AttachedObserver>,
    native: NativeFactory,
    spawn: SpawnWorker,
    #[cfg(test)]
    wait: Option<WorkerWait>,
}
impl AccessibilityApplication {
    pub(super) fn new(
        lifetime: Arc<ApplicationLifetime>,
        app: Arc<AXApp>,
        focus: Arc<SharedFocus>,
        events: mpsc::Sender<NativeEvent>,
        loop_waker: Arc<OnceLock<LoopWaker<Wake>>>,
        hidden: bool,
        suspended: bool,
    ) -> Self {
        let native_lifetime = lifetime.clone();
        let native = Box::new(move |results, handles| -> Box<dyn ApplicationNative> {
            Box::new(ApplicationAccess::new(
                native_lifetime.clone(),
                app.clone(),
                focus.clone(),
                events.clone(),
                results,
                loop_waker.clone(),
                handles,
            ))
        });
        Self::with_native(
            lifetime,
            hidden,
            suspended,
            native,
            Box::new(|job| {
                thread::Builder::new()
                    .name("dome-application".into())
                    .spawn(job)
            }),
        )
    }
    pub(in crate::platform::macos) fn with_native(
        lifetime: Arc<ApplicationLifetime>,
        hidden: bool,
        suspended: bool,
        native: NativeFactory,
        spawn: SpawnWorker,
    ) -> Self {
        Self {
            lifetime,
            worker: None,
            waiting_to_start: VecDeque::new(),
            retry_on_refresh: false,
            accepting: true,
            hidden,
            suspended,
            handles: HashMap::new(),
            attached: Vec::new(),
            native,
            spawn,
            #[cfg(test)]
            wait: None,
        }
    }
    pub(super) fn update_state(&mut self, hidden: bool, suspended: bool) {
        self.hidden = hidden;
        self.suspended = suspended;
    }
    pub(super) fn install_handle(&mut self, window: AXWindow) {
        self.handles.insert(window.cg_id(), window);
    }
    pub(super) fn remove_handle(&mut self, key: CGWindowID) {
        self.handles.remove(&key);
    }
    pub(super) fn send(&mut self, command: ApplicationCommand) {
        if !self.accepting {
            return;
        }
        if matches!(
            command,
            ApplicationCommand::Shutdown(_) | ApplicationCommand::Terminate
        ) {
            self.accepting = false;
            self.detach_observers();
        }
        self.waiting_to_start.push_back(command);
        if !self.retry_on_refresh {
            self.flush();
        }
    }
    fn start_worker(&mut self) -> io::Result<()> {
        let (commands, receiver) = mpsc::channel();
        let (results, result_receiver) = mpsc::channel();
        let access = (self.native)(results, self.handles.clone());
        let schedule = ApplicationReadSchedule::new(self.hidden, self.suspended);
        let wait = WorkerWait::default();
        #[cfg(test)]
        let wait = self.wait.take().unwrap_or(wait);
        let thread = (self.spawn)(Box::new(move || {
            run_dispatcher_thread(receiver, schedule, access, wait)
        }))?;
        self.worker = Some(ApplicationWorkerConnection {
            identity: Arc::new(()),
            commands,
            results: result_receiver,
            _thread: thread,
        });
        Ok(())
    }
    fn flush(&mut self) {
        while let Some(command) = self.waiting_to_start.pop_front() {
            if self.worker.is_none() && self.start_worker().is_err() {
                self.waiting_to_start.push_front(command);
                self.retry_on_refresh = true;
                return;
            }
            let terminal = matches!(
                command,
                ApplicationCommand::Shutdown(_) | ApplicationCommand::Terminate
            );
            match self
                .worker
                .as_ref()
                .expect("worker started")
                .commands
                .send(command)
            {
                Ok(()) => {
                    if terminal {
                        self.worker = None;
                        return;
                    }
                }
                Err(error) => {
                    self.worker = None;
                    self.detach_observers();
                    self.waiting_to_start.push_front(error.0);
                }
            }
        }
    }
    pub(super) fn retry_start_on_refresh(&mut self) {
        if self.retry_on_refresh {
            self.retry_on_refresh = false;
            self.flush();
        }
    }
    pub(super) fn terminal_waiting(&self) -> bool {
        !self.accepting && !self.waiting_to_start.is_empty()
    }
    pub(super) fn connection(&self) -> Option<Arc<()>> {
        self.worker.as_ref().map(|worker| worker.identity.clone())
    }

    pub(super) fn take_results(&mut self) -> Vec<ApplicationResult> {
        self.worker
            .as_ref()
            .map_or_else(Vec::new, |worker| worker.results.try_iter().collect())
    }
    pub(super) fn attach_observer(
        &mut self,
        source: CFRetained<CFRunLoopSource>,
        context: Arc<ObserverContext>,
    ) {
        if !self.accepting || !Arc::ptr_eq(&self.lifetime, &context.lifetime) {
            return;
        }
        let run_loop = CFRunLoop::current().expect("UI run loop");
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
        self.attached.push(AttachedObserver {
            source,
            _context: context,
            run_loop,
        });
    }
    pub(super) fn detach_observers(&mut self) {
        self.attached.clear();
    }
    pub(super) fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        self.send(ApplicationCommand::Shutdown(recovery));
    }
    pub(super) fn terminate(&mut self) {
        self.send(ApplicationCommand::Terminate);
    }
}
impl Drop for AccessibilityApplication {
    fn drop(&mut self) {
        self.detach_observers();
        self.worker = None;
    }
}

#[derive(Default)]
struct WorkerWait {
    #[cfg(test)]
    control: Option<tests::WaitControl>,
}
impl WorkerWait {
    fn now(&self) -> Instant {
        #[cfg(test)]
        if let Some(control) = &self.control {
            return control.now;
        }
        Instant::now()
    }

    fn receive(
        &mut self,
        commands: &mpsc::Receiver<ApplicationCommand>,
        deadline: Option<Instant>,
    ) -> Result<ApplicationCommand, mpsc::RecvTimeoutError> {
        #[cfg(test)]
        if let Some(control) = &mut self.control
            && let Some(received) = control.receive(commands, deadline)
        {
            return received;
        }
        if let Some(deadline) = deadline {
            commands.recv_timeout(deadline.saturating_duration_since(self.now()))
        } else {
            commands
                .recv()
                .map_err(|_| mpsc::RecvTimeoutError::Disconnected)
        }
    }

    fn sleep_until(&mut self, deadline: Instant) {
        #[cfg(test)]
        if let Some(control) = &mut self.control {
            control.sleep_until(deadline);
            return;
        }
        thread::sleep(deadline.saturating_duration_since(self.now()));
    }
}

fn run_dispatcher_thread(
    commands: mpsc::Receiver<ApplicationCommand>,
    mut schedule: ApplicationReadSchedule,
    mut access: Box<dyn ApplicationNative>,
    mut wait: WorkerWait,
) {
    let Ok(first) = commands.recv() else {
        return;
    };
    let marker = DispatcherMarker(());
    super::set_current_thread_qos(libc::qos_class_t::QOS_CLASS_USER_INITIATED);
    schedule.collect(first);
    run_pending_operations(
        &commands,
        &mut schedule,
        access.as_mut(),
        &marker,
        &mut wait,
    );
    drop(access);
    drop(schedule);
    drop(commands);
}
fn run_pending_operations(
    commands: &mpsc::Receiver<ApplicationCommand>,
    schedule: &mut ApplicationReadSchedule,
    access: &mut dyn ApplicationNative,
    marker: &DispatcherMarker,
    wait: &mut WorkerWait,
) {
    let mut disconnected = false;
    let mut probed = false;
    loop {
        loop {
            match commands.try_recv() {
                Ok(command) => schedule.collect(command),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    disconnected = true;
                    break;
                }
            }
        }
        if disconnected {
            schedule.disconnected();
        }
        let now = wait.now();
        schedule.consume_quiet_deadline(now);
        if !probed && schedule.permits_reads() {
            autoreleasepool(|_| access.probe(marker));
            probed = true;
            continue;
        }
        if let Some(operation) = schedule.take_next_eligible(now) {
            #[cfg(test)]
            access.selected(&operation, marker);
            let terminal = matches!(operation, ApplicationOperation::Terminate);
            autoreleasepool(|_| access.execute(operation, marker));
            if terminal {
                return;
            }
            continue;
        }
        if disconnected {
            if let Some(deadline) = schedule.next_deadline() {
                wait.sleep_until(deadline);
                continue;
            }
            autoreleasepool(|_| access.execute(ApplicationOperation::Terminate, marker));
            return;
        }
        let received = wait.receive(commands, schedule.next_deadline());
        match received {
            Ok(command) => schedule.collect(command),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => disconnected = true,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Dimension;
    use crate::platform::macos::window_server::{DebounceBurst, PositionReadOrigin};
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;
    use std::time::Duration;

    include!("dispatcher_failure_tests.rs");
    include!("dispatcher_submission_tests.rs");
    include!("dispatcher_wait_tests.rs");

    #[derive(Debug, PartialEq)]
    struct Call {
        kind: &'static str,
        key: u32,
        value: i32,
        origin: Option<PositionReadOrigin>,
        worker: thread::ThreadId,
    }
    impl Call {
        fn new(kind: &'static str, key: u32, value: i32) -> Self {
            Self {
                kind,
                key,
                value,
                origin: None,
                worker: thread::current().id(),
            }
        }
    }
    struct NativeRecorder {
        calls: mpsc::Sender<Call>,
        probe_gate: Option<mpsc::Receiver<()>>,
        operation_gate: Option<(&'static str, mpsc::Receiver<()>)>,
    }
    impl ApplicationNative for NativeRecorder {
        fn probe(&mut self, _: &DispatcherMarker) {
            self.calls.send(Call::new("probe", 0, 0)).unwrap();
            if let Some(gate) = self.probe_gate.take() {
                gate.recv().unwrap();
            }
        }
        fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
            use ApplicationOperation::*;
            let call = match operation {
                SetFrame { window, frame } => Call::new(
                    "frame",
                    window.cg_id(),
                    frame.to_dimension().x.value() as i32,
                ),
                MoveOffscreen { window, position } => {
                    Call::new("offscreen", window.cg_id(), position.0.value() as i32)
                }
                Minimize(window) => Call::new("minimize", window.cg_id(), 0),
                Recover { window, frame } => Call::new(
                    "recover",
                    window.cg_id(),
                    frame.to_dimension().x.value() as i32,
                ),
                Focus(window) => Call::new("focus", window.cg_id(), 0),
                Close(window) => Call::new("close", window.cg_id(), 0),
                Unminimize(window) => Call::new("unminimize", window.cg_id(), 0),
                Read(InspectionKind::Positions(origin)) => {
                    let mut call = Call::new("positions", 0, 0);
                    call.origin = Some(origin);
                    call
                }
                Read(InspectionKind::AppWindows) => Call::new("enumerate", 0, 0),
                Read(InspectionKind::Space) => Call::new("space", 0, 0),
                Read(InspectionKind::FocusedWindow) => Call::new("focused", 0, 0),
                Read(InspectionKind::Title(key)) => Call::new("title", key, 0),
                ResolveTitle(_) => Call::new("resolve-title", 0, 0),
                RegisterObserver => Call::new("register", 0, 0),
                RemoveObserver(_) => Call::new("remove", 0, 0),
                Restore(entries) => Call::new(
                    "restore",
                    entries.first().map_or(0, |entry| entry.window),
                    entries.len() as i32,
                ),
                Terminate => Call::new("terminate", 0, 0),
            };
            let kind = call.kind;
            self.calls.send(call).unwrap();
            if self
                .operation_gate
                .as_ref()
                .is_some_and(|(held, _)| *held == kind)
            {
                self.operation_gate.take().unwrap().1.recv().unwrap();
            }
        }
    }
    struct Fixture {
        owner: AccessibilityApplication,
        calls: mpsc::Receiver<Call>,
        probe_release: Option<mpsc::Sender<()>>,
        operation_release: Option<mpsc::Sender<()>>,
    }
    impl Fixture {
        fn new(hold_operation: Option<&'static str>) -> Self {
            let (calls, receiver) = mpsc::channel();
            let (release, gate) = mpsc::channel();
            let gate = RefCell::new(Some(gate));
            let (operation_release, operation_gate) = mpsc::channel();
            let operation_gate = RefCell::new(hold_operation.map(|kind| (kind, operation_gate)));
            let native = Box::new(move |_, _| -> Box<dyn ApplicationNative> {
                Box::new(NativeRecorder {
                    calls: calls.clone(),
                    probe_gate: gate.borrow_mut().take(),
                    operation_gate: operation_gate.borrow_mut().take(),
                })
            });
            let owner = AccessibilityApplication::with_native(
                Arc::new(ApplicationLifetime { pid: 123 }),
                false,
                false,
                native,
                Box::new(|job| thread::Builder::new().spawn(job)),
            );
            Self {
                owner,
                calls: receiver,
                probe_release: Some(release),
                operation_release: Some(operation_release),
            }
        }
        fn replace_native(&mut self) {
            let (calls, receiver) = mpsc::channel();
            let (release, gate) = mpsc::channel();
            let gate = RefCell::new(Some(gate));
            self.owner.native = Box::new(move |_, _| -> Box<dyn ApplicationNative> {
                Box::new(NativeRecorder {
                    calls: calls.clone(),
                    probe_gate: gate.borrow_mut().take(),
                    operation_gate: None,
                })
            });
            self.calls = receiver;
            self.probe_release = Some(release);
        }
        fn enter_probe(&mut self) {
            self.owner
                .send(ApplicationCommand::Read(InspectionKind::Title(999)));
            assert_eq!(self.next().kind, "probe");
        }
        fn release_probe(&mut self) {
            self.probe_release.take().unwrap().send(()).unwrap();
        }
        fn release_operation(&mut self) {
            self.operation_release.take().unwrap().send(()).unwrap();
        }
        fn next(&self) -> Call {
            self.calls
                .recv_timeout(Duration::from_secs(3))
                .expect("native worker made progress")
        }
        fn skip_initial(&self) {
            assert_eq!(self.next().kind, "register");
            let title = self.next();
            assert_eq!((title.kind, title.key), ("title", 999));
        }
        fn operation(&mut self, operation: ApplicationOperation) {
            self.owner.send(ApplicationCommand::Operation(operation));
        }
        fn read(&mut self, kind: InspectionKind) {
            self.owner.send(ApplicationCommand::Read(kind));
        }
        fn finish(&mut self) {
            self.owner.terminate();
            assert_eq!(self.next().kind, "terminate");
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            if let Some(release) = self.probe_release.take() {
                let _ = release.send(());
            }
            if let Some(release) = self.operation_release.take() {
                let _ = release.send(());
            }
        }
    }
    fn window(key: u32) -> AXWindow {
        AXWindow::test_handle(123, key)
    }
    fn frame(x: i32) -> PixelRect<Logical> {
        PixelRect::from_dimension(Dimension::new(
            Length::new(x as f32),
            Length::new(0.0),
            Length::new(80.0),
            Length::new(60.0),
        ))
    }
    fn setter(kind: usize, key: u32, value: i32) -> ApplicationOperation {
        match kind {
            0 => ApplicationOperation::SetFrame {
                window: window(key),
                frame: frame(value),
            },
            1 => ApplicationOperation::MoveOffscreen {
                window: window(key),
                position: (Length::new(value as f32), Length::new(0.0)),
            },
            2 => ApplicationOperation::Minimize(window(key)),
            _ => ApplicationOperation::Recover {
                window: window(key),
                frame: frame(value),
            },
        }
    }

    #[test]
    fn pending_setters_replace_in_place_without_crossing_barriers() {
        for kind in 0..4 {
            let mut fixture = Fixture::new(None);
            fixture.enter_probe();
            fixture.operation(setter(0, 1, 10));
            fixture.operation(setter(0, 2, 20));
            fixture.operation(setter(kind, 1, 30));
            fixture.release_probe();
            fixture.skip_initial();
            let first = fixture.next();
            assert_eq!(
                (first.kind, first.key, first.value),
                (
                    ["frame", "offscreen", "minimize", "recover"][kind],
                    1,
                    if kind == 2 { 0 } else { 30 }
                )
            );
            let second = fixture.next();
            assert_eq!((second.kind, second.key, second.value), ("frame", 2, 20));
            fixture.finish();
        }
        for barrier in 0..4 {
            let mut fixture = Fixture::new(None);
            fixture.enter_probe();
            fixture.operation(setter(0, 1, 10));
            fixture.operation(match barrier {
                0 => ApplicationOperation::Close(window(2)),
                1 => ApplicationOperation::Unminimize(window(2)),
                2 => ApplicationOperation::RegisterObserver,
                _ => ApplicationOperation::RemoveObserver(
                    super::super::listeners::tests::removal_barrier(),
                ),
            });
            fixture.operation(setter(0, 1, 30));
            fixture.release_probe();
            fixture.skip_initial();
            assert_eq!(fixture.next().value, 10);
            assert_eq!(
                fixture.next().kind,
                ["close", "unminimize", "register", "remove"][barrier]
            );
            assert_eq!(fixture.next().value, 30);
            fixture.finish();
        }
    }

    #[test]
    fn consecutive_pending_commands_deduplicate_only_at_tail() {
        for unminimize in [false, true] {
            let command = |key| {
                if unminimize {
                    ApplicationOperation::Unminimize(window(key))
                } else {
                    ApplicationOperation::Close(window(key))
                }
            };
            let expected = if unminimize { "unminimize" } else { "close" };
            let mut fixture = Fixture::new(Some(expected));
            fixture.enter_probe();
            fixture.operation(command(1));
            fixture.operation(command(1));
            fixture.operation(command(2));
            fixture.operation(command(1));
            fixture.release_probe();
            fixture.skip_initial();
            assert_eq!(fixture.next().key, 1);
            fixture.operation(command(1));
            fixture.release_operation();
            assert_eq!(fixture.next().key, 2);
            assert_eq!(fixture.next().key, 1);
            fixture.operation(command(1));
            assert_eq!(fixture.next().key, 1);
            fixture.finish();
        }
    }

    #[test]
    fn read_demand_replacement_moves_to_tail() {
        let mut fixture = Fixture::new(None);
        fixture.enter_probe();
        let now = Instant::now();
        let burst = DebounceBurst {
            first: now,
            last: now,
        };
        fixture.read(InspectionKind::Positions(PositionReadOrigin::Settle(burst)));
        fixture.operation(setter(0, 1, 10));
        fixture.operation(setter(0, 2, 20));
        fixture.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
        fixture.operation(setter(0, 1, 30));
        for read in [
            InspectionKind::AppWindows,
            InspectionKind::Space,
            InspectionKind::FocusedWindow,
            InspectionKind::Title(1),
            InspectionKind::Title(2),
        ] {
            fixture.read(read);
        }
        fixture.release_probe();
        fixture.skip_initial();
        assert_eq!(fixture.next().value, 30);
        assert_eq!(fixture.next().value, 20);
        let positions = fixture.next();
        assert_eq!(positions.origin, Some(PositionReadOrigin::Settle(burst)));
        for (kind, key) in [
            ("enumerate", 0),
            ("space", 0),
            ("focused", 0),
            ("title", 1),
            ("title", 2),
        ] {
            let call = fixture.next();
            assert_eq!((call.kind, call.key), (kind, key));
        }
        fixture.finish();
    }

    #[test]
    fn applications_serialize_without_cross_app_blocking() {
        let mut first = Fixture::new(Some("frame"));
        first.enter_probe();
        first.operation(setter(0, 1, 10));
        first.release_probe();
        first.skip_initial();
        let entry = first.next();
        assert_ne!(entry.worker, thread::current().id());
        first.operation(setter(0, 1, 20));
        first.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
        first.operation(setter(0, 1, 30));
        let mut second = Fixture::new(None);
        second.enter_probe();
        second.operation(setter(0, 2, 40));
        second.release_probe();
        second.skip_initial();
        let other = second.next();
        assert_eq!(other.value, 40);
        assert_ne!(entry.worker, other.worker);
        assert!(first.calls.try_recv().is_err());
        first.release_operation();
        assert_eq!(first.next().value, 30);
        assert_eq!(first.next().kind, "positions");
        first.finish();
        second.finish();
    }

    #[test]
    fn initial_probe_waits_for_first_receive() {
        let mut fixture = Fixture::new(None);
        assert!(fixture.calls.try_recv().is_err());
        fixture.owner.start_worker().unwrap();
        assert!(fixture.calls.try_recv().is_err());
        fixture
            .owner
            .send(ApplicationCommand::Read(InspectionKind::Title(999)));
        let probe = fixture.next();
        assert_eq!(probe.kind, "probe");
        assert_ne!(probe.worker, thread::current().id());
        fixture.release_probe();
        fixture.skip_initial();
        fixture.finish();
    }

    #[test]
    fn spawn_failure_buffers_only_never_enqueued_commands() {
        let mut fixture = Fixture::new(None);
        let attempts = Rc::new(Cell::new(0));
        let attempted = attempts.clone();
        fixture.owner.spawn = Box::new(move |job| {
            attempted.set(attempted.get() + 1);
            if attempted.get() < 3 {
                Err(io::Error::other("injected spawn failure"))
            } else {
                thread::Builder::new().spawn(job)
            }
        });
        fixture.operation(setter(0, 1, 10));
        fixture.operation(setter(0, 2, 20));
        fixture.operation(setter(0, 1, 30));
        assert_eq!(attempts.get(), 1);
        assert_eq!(fixture.owner.waiting_to_start.len(), 3);
        fixture.owner.retry_start_on_refresh();
        assert_eq!(attempts.get(), 2);
        fixture.operation(setter(0, 3, 40));
        assert_eq!(attempts.get(), 2);
        assert!(fixture.calls.try_recv().is_err());
        fixture.replace_native();
        fixture.owner.retry_start_on_refresh();
        assert_eq!(fixture.next().kind, "probe");
        fixture.release_probe();
        assert_eq!(fixture.next().kind, "register");
        for (key, value) in [(1, 30), (2, 20), (3, 40)] {
            let call = fixture.next();
            assert_eq!((call.key, call.value), (key, value));
        }
        assert_eq!(attempts.get(), 3);
        fixture.finish();
    }

    #[test]
    fn lifecycle_collection_prunes_before_next_selection() {
        for lifecycle in 0..3 {
            let mut fixture = Fixture::new(Some("frame"));
            fixture.enter_probe();
            fixture.operation(setter(0, 1, 10));
            fixture.release_probe();
            fixture.skip_initial();
            assert_eq!(fixture.next().value, 10);
            fixture.operation(setter(0, 1, 20));
            fixture.read(InspectionKind::AppWindows);
            fixture.owner.send(match lifecycle {
                0 => ApplicationCommand::Event(ApplicationEvent::Hidden),
                1 => ApplicationCommand::Terminate,
                _ => ApplicationCommand::Shutdown(vec![RecoveryEntry {
                    window: 1,
                    original_rect: frame(5),
                }]),
            });
            fixture.release_operation();
            if lifecycle == 0 {
                fixture.operation(setter(0, 2, 40));
                let next = fixture.next();
                assert_eq!((next.kind, next.value), ("frame", 40));
                fixture.finish();
            } else {
                if lifecycle == 2 {
                    assert_eq!(fixture.next().kind, "restore");
                }
                assert_eq!(fixture.next().kind, "terminate");
            }
        }
    }

    #[test]
    fn disconnect_drains_buffered_and_pending_work() {
        let mut fixture = Fixture::new(None);
        fixture.enter_probe();
        fixture.operation(setter(0, 1, 10));
        let observed_at = Instant::now();
        fixture.owner.send(ApplicationCommand::Event(
            ApplicationEvent::MovedOrResized { observed_at },
        ));
        fixture.read(InspectionKind::AppWindows);
        fixture.owner.worker = None;
        fixture.release_probe();
        fixture.skip_initial();
        assert_eq!(fixture.next().kind, "frame");
        assert_eq!(fixture.next().kind, "enumerate");
        assert!(Instant::now() >= observed_at + Duration::from_millis(300));
        assert_eq!(fixture.next().kind, "positions");
        assert_eq!(fixture.next().kind, "terminate");
    }

    #[test]
    fn disconnect_when_hidden_or_suspended_discards_unresumable_reads() {
        for event in [ApplicationEvent::Hidden, ApplicationEvent::Suspended(true)] {
            let mut fixture = Fixture::new(None);
            fixture.enter_probe();
            fixture.owner.send(ApplicationCommand::Event(event));
            fixture.read(InspectionKind::AppWindows);
            fixture.operation(setter(0, 1, 10));
            fixture.owner.worker = None;
            fixture.release_probe();
            assert_eq!(fixture.next().kind, "frame");
            assert_eq!(fixture.next().kind, "terminate");
            assert!(fixture.calls.try_recv().is_err());
        }
    }

    #[test]
    fn collect_during_movement_delays_geometry_without_blocking_title_focus_or_writes() {
        for kind in [
            InspectionKind::AppWindows,
            InspectionKind::Space,
            InspectionKind::Positions(PositionReadOrigin::Periodic),
            InspectionKind::Positions(PositionReadOrigin::Settle(DebounceBurst {
                first: Instant::now(),
                last: Instant::now(),
            })),
        ] {
            let mut fixture = Fixture::new(None);
            fixture.enter_probe();
            let observed_at = Instant::now();
            fixture.owner.send(ApplicationCommand::Event(
                ApplicationEvent::MovedOrResized { observed_at },
            ));
            fixture.read(kind);
            fixture.operation(setter(0, 1, 10));
            fixture.read(InspectionKind::Title(777));
            fixture.operation(ApplicationOperation::Focus(window(1)));
            fixture.release_probe();
            fixture.skip_initial();
            assert_eq!(fixture.next().kind, "frame");
            let title = fixture.next();
            assert_eq!((title.kind, title.key), ("title", 777));
            assert_eq!(fixture.next().kind, "focus");
            let geometry = fixture.next();
            assert!(Instant::now() >= observed_at + Duration::from_millis(300));
            match kind {
                InspectionKind::AppWindows => assert_eq!(geometry.kind, "enumerate"),
                InspectionKind::Space => assert_eq!(geometry.kind, "space"),
                InspectionKind::Positions(_) => assert_eq!(geometry.kind, "positions"),
                _ => unreachable!(),
            }
            let settled = if matches!(kind, InspectionKind::Positions(_)) {
                geometry
            } else {
                fixture.next()
            };
            assert_eq!(
                settled.origin,
                Some(PositionReadOrigin::Settle(DebounceBurst {
                    first: observed_at,
                    last: observed_at,
                }))
            );
            fixture.finish();
        }
    }

    #[test]
    fn send_after_worker_unwind_replaces_only_on_disconnection() {
        struct PanicAccess {
            dropping: mpsc::Sender<()>,
            restore: mpsc::Receiver<()>,
        }
        impl ApplicationNative for PanicAccess {
            fn probe(&mut self, _: &DispatcherMarker) {}
            fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
                if matches!(operation, ApplicationOperation::Read(_)) {
                    panic!("injected read panic");
                }
            }
        }
        impl Drop for PanicAccess {
            fn drop(&mut self) {
                let _ = self.dropping.send(());
                let _ = self.restore.recv();
            }
        }
        struct Exit(mpsc::Sender<()>);
        impl Drop for Exit {
            fn drop(&mut self) {
                let _ = self.0.send(());
            }
        }
        let mut fixture = Fixture::new(None);
        let (dropping, in_restore) = mpsc::channel();
        let (restore, restore_gate) = mpsc::channel();
        let restore_gate = RefCell::new(Some(restore_gate));
        fixture.owner.native = Box::new(move |_, _| {
            Box::new(PanicAccess {
                dropping: dropping.clone(),
                restore: restore_gate.borrow_mut().take().unwrap(),
            })
        });
        let attempts = Rc::new(Cell::new(0));
        let attempted = attempts.clone();
        let (exited, exits) = mpsc::channel();
        fixture.owner.spawn = Box::new(move |job| {
            attempted.set(attempted.get() + 1);
            let exited = exited.clone();
            thread::Builder::new().spawn(move || {
                let _exit = Exit(exited);
                job();
            })
        });
        let lifetime = fixture.owner.lifetime.clone();
        fixture.read(InspectionKind::AppWindows);
        in_restore.recv_timeout(Duration::from_secs(3)).unwrap();
        fixture.operation(setter(0, 1, 10));
        assert_eq!(attempts.get(), 1);
        fixture.owner.take_results();
        fixture.owner.retry_start_on_refresh();
        assert_eq!(attempts.get(), 1);
        restore.send(()).unwrap();
        exits.recv_timeout(Duration::from_secs(3)).unwrap();
        fixture.owner.take_results();
        fixture.owner.retry_start_on_refresh();
        assert_eq!(attempts.get(), 1);
        fixture.replace_native();
        fixture.operation(setter(0, 1, 20));
        assert_eq!(attempts.get(), 2);
        assert!(Arc::ptr_eq(&lifetime, &fixture.owner.lifetime));
        assert_eq!(fixture.next().kind, "probe");
        fixture.release_probe();
        assert_eq!(fixture.next().kind, "register");
        assert_eq!(fixture.next().value, 20);
        fixture.finish();
    }

    #[test]
    fn terminate_after_accepted_terminal_death_does_not_restart_cleanup() {
        struct TerminalPanic(mpsc::Sender<()>);
        impl ApplicationNative for TerminalPanic {
            fn probe(&mut self, _: &DispatcherMarker) {}
            fn execute(&mut self, operation: ApplicationOperation, _: &DispatcherMarker) {
                if matches!(
                    operation,
                    ApplicationOperation::Restore(_) | ApplicationOperation::Terminate
                ) {
                    self.0.send(()).unwrap();
                    panic!("injected terminal panic");
                }
            }
        }
        for shutdown in [false, true] {
            let mut fixture = Fixture::new(None);
            let (entered, entries) = mpsc::channel();
            fixture.owner.native = Box::new(move |_, _| Box::new(TerminalPanic(entered.clone())));
            let attempts = Rc::new(Cell::new(0));
            let attempted = attempts.clone();
            let (exited, exits) = mpsc::channel();
            fixture.owner.spawn = Box::new(move |job| {
                attempted.set(attempted.get() + 1);
                let exited = exited.clone();
                thread::Builder::new().spawn(move || {
                    let _exit = ThreadExit(exited);
                    job();
                })
            });
            fixture.owner.send(if shutdown {
                ApplicationCommand::Shutdown(vec![RecoveryEntry {
                    window: 1,
                    original_rect: frame(5),
                }])
            } else {
                ApplicationCommand::Terminate
            });
            entries.recv_timeout(Duration::from_secs(3)).unwrap();
            exits.recv_timeout(Duration::from_secs(3)).unwrap();
            fixture.owner.retry_start_on_refresh();
            fixture.owner.send(ApplicationCommand::Terminate);
            fixture.operation(setter(0, 1, 40));
            assert_eq!(attempts.get(), 1);
            assert!(!fixture.owner.terminal_waiting());
            assert!(fixture.owner.take_results().is_empty());
            assert!(entries.try_recv().is_err());
        }
        let mut fixture = Fixture::new(None);
        fixture.owner.spawn = Box::new(|_| Err(io::Error::other("injected spawn failure")));
        fixture.owner.terminate();
        assert!(fixture.owner.terminal_waiting());
        fixture.replace_native();
        fixture.owner.spawn = Box::new(|job| thread::Builder::new().spawn(job));
        fixture.owner.retry_start_on_refresh();
        assert!(!fixture.owner.terminal_waiting());
        assert_eq!(fixture.next().kind, "probe");
        fixture.release_probe();
        assert_eq!(fixture.next().kind, "terminate");
    }

    #[test]
    fn worker_quiet_deadline_advances_without_ui_service() {
        let mut fixture = Fixture::new(None);
        fixture.enter_probe();
        let observed_at = Instant::now();
        fixture.owner.send(ApplicationCommand::Event(
            ApplicationEvent::MovedOrResized { observed_at },
        ));
        fixture.read(InspectionKind::AppWindows);
        fixture.operation(setter(0, 1, 10));
        fixture.release_probe();
        fixture.skip_initial();
        assert_eq!(fixture.next().kind, "frame");
        assert_eq!(fixture.next().kind, "enumerate");
        assert!(Instant::now() >= observed_at + Duration::from_millis(300));
        let settled = fixture.next();
        assert_eq!(
            settled.origin,
            Some(PositionReadOrigin::Settle(DebounceBurst {
                first: observed_at,
                last: observed_at
            }))
        );
        fixture.finish();
    }
}
