use super::schedule::{QUIET_INTERVAL, geometry_read, record_movement};
use super::*;
use crate::platform::macos::dispatcher::{
    AccessibilityApplication, ApplicationCommand, ApplicationOperation,
};
use crate::platform::macos::focus::SharedFocus;
use crate::platform::macos::listeners::{EventListener, SYNC_INTERVAL, TITLE_INTERVAL};
use crate::platform::macos::running_application::RunningApp;
use crate::platform::macos::throttle::Throttle;
use dome_auxiliary_window::LoopWaker;
use objc2_core_graphics::CGWindowID;
#[cfg(test)]
#[path = "tests.rs"]
mod tests;

#[cfg(feature = "native-ui-test")]
#[path = "native_observer_test.rs"]
pub(super) mod native_observer_test;

use std::collections::{HashMap, HashSet};
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock, mpsc};
use std::time::Instant;

struct ApplicationState {
    lifetime: Arc<ApplicationLifetime>,
    hidden: bool,
    suspended: bool,
    movement: Option<DebounceBurst>,
}

impl ApplicationState {
    fn accepts_result(&self, result: &InspectionResult, now: Instant) -> bool {
        Arc::ptr_eq(&self.lifetime, &result.lifetime)
            && !self.hidden
            && !self.suspended
            && (!geometry_read(result.kind)
                || self
                    .movement
                    .is_none_or(|burst| now >= burst.last + QUIET_INTERVAL))
    }
}

struct Application {
    state: ApplicationState,
    owner: AccessibilityApplication,
}

type TitleNotice = (Arc<ApplicationLifetime>, TitleElement);
pub(in crate::platform::macos) struct MacOsWindowServer {
    native_events: mpsc::Receiver<NativeEvent>,
    native_sender: mpsc::Sender<NativeEvent>,
    focus: Arc<SharedFocus>,
    loop_waker: Arc<OnceLock<LoopWaker<Wake>>>,
    applications: HashMap<i32, Application>,
    retired: Vec<AccessibilityApplication>,
    handles: HashMap<CGWindowID, AXWindow>,
    observations: Vec<Observation>,
    next_refresh: Instant,
    suspended: bool,
    is_suspended: Arc<AtomicBool>,
    stopped: bool,
    listener: Option<EventListener>,
    titles: mpsc::Receiver<TitleNotice>,
    title_throttle: Option<Pin<Box<Throttle<TitleNotice>>>>,
}

impl MacOsWindowServer {
    pub(in crate::platform::macos) fn new(
        native_events: mpsc::Receiver<NativeEvent>,
        native_sender: mpsc::Sender<NativeEvent>,
        focus: Arc<SharedFocus>,
        loop_waker: Arc<OnceLock<LoopWaker<Wake>>>,
        is_suspended: Arc<AtomicBool>,
        now: Instant,
    ) -> Self {
        let (title_sender, titles) = mpsc::channel();
        let title_waker = loop_waker.clone();
        let title_throttle = Throttle::new(TITLE_INTERVAL, move |notice| {
            if title_sender.send(notice).is_ok()
                && let Some(waker) = title_waker.get()
            {
                waker.wake(Wake::WindowServer);
            }
        });
        Self {
            native_events,
            native_sender,
            focus,
            loop_waker,
            applications: HashMap::new(),
            retired: Vec::new(),
            handles: HashMap::new(),
            observations: Vec::new(),
            next_refresh: now,
            suspended: is_suspended.load(Ordering::Relaxed),
            is_suspended,
            stopped: false,
            listener: None,
            titles,
            title_throttle: Some(title_throttle),
        }
    }

    fn discover(&mut self, app: RunningApp) {
        let process_id = app.pid();
        if self.applications.contains_key(&process_id) {
            return;
        }
        let lifetime = Arc::new(ApplicationLifetime { pid: process_id });
        let hidden = app.is_hidden();
        let state = ApplicationState {
            lifetime: lifetime.clone(),
            hidden,
            suspended: self.suspended,
            movement: None,
        };
        let owner = AccessibilityApplication::new(
            lifetime,
            app.ax_app(),
            self.focus.clone(),
            self.native_sender.clone(),
            self.loop_waker.clone(),
            hidden,
            self.suspended,
        );
        self.admit_application(Application { state, owner }, app.is_active());
    }

    fn admit_application(&mut self, mut app: Application, active: bool) {
        app.owner
            .send(ApplicationCommand::Event(ApplicationEvent::Refresh));
        if active {
            app.owner
                .send(ApplicationCommand::Event(ApplicationEvent::FocusChanged));
        }
        self.applications.insert(app.state.lifetime.pid, app);
    }

    fn retire(&mut self, process_id: i32) {
        let Some(mut app) = self.applications.remove(&process_id) else {
            return;
        };
        self.handles.retain(|_, window| window.pid() != process_id);
        self.observations.retain(|observation| {
            !Arc::ptr_eq(observation_lifetime(observation), &app.state.lifetime)
        });
        app.owner.terminate();
        self.observations.push(Observation::AppGone {
            lifetime: app.state.lifetime,
        });
        if app.owner.terminal_waiting() {
            self.retired.push(app.owner);
        }
    }

    fn forward_event(&mut self, lifetime: &Arc<ApplicationLifetime>, event: ApplicationEvent) {
        let Some(app) = self.applications.get_mut(&lifetime.pid) else {
            return;
        };
        if !Arc::ptr_eq(lifetime, &app.state.lifetime) {
            return;
        }
        match &event {
            ApplicationEvent::MovedOrResized { observed_at } => {
                record_movement(&mut app.state.movement, *observed_at)
            }
            ApplicationEvent::Hidden => {
                app.state.hidden = true;
                self.observations.retain(|observation| {
                    !Arc::ptr_eq(observation_lifetime(observation), lifetime)
                });
                self.observations.push(Observation::AppHidden {
                    lifetime: lifetime.clone(),
                });
            }
            ApplicationEvent::Shown => app.state.hidden = false,
            ApplicationEvent::Suspended(suspended) => app.state.suspended = *suspended,
            _ => {}
        }
        app.owner
            .update_state(app.state.hidden, app.state.suspended);
        if let ApplicationEvent::TitleElementChanged(element) = event {
            if let Some(throttle) = &mut self.title_throttle {
                throttle.submit((lifetime.clone(), element));
            }
        } else {
            app.owner.send(ApplicationCommand::Event(event));
        }
    }

    fn accept_result(&mut self, result: InspectionResult, now: Instant) {
        let Some(app) = self.applications.get_mut(&result.lifetime.pid) else {
            return;
        };
        if !Arc::ptr_eq(&app.state.lifetime, &result.lifetime) {
            return;
        }
        if !app.state.accepts_result(&result, now) {
            if !app.state.hidden && !app.state.suspended && geometry_read(result.kind) {
                app.owner.send(ApplicationCommand::Read(result.kind));
            }
            return;
        }
        let InspectionOutcome::Completed(observation) = result.outcome else {
            return;
        };
        if !Arc::ptr_eq(observation_lifetime(&observation), &app.state.lifetime) {
            return;
        }
        for handle in result.handle_updates {
            if handle.pid() != app.state.lifetime.pid {
                continue;
            }
            app.owner.install_handle(handle.clone());
            self.handles.insert(handle.cg_id(), handle);
        }
        if let Observation::AppWindows { removed, .. } = &observation {
            for key in removed {
                if self
                    .handles
                    .get(key)
                    .is_some_and(|window| window.pid() == app.state.lifetime.pid)
                {
                    self.handles.remove(key);
                }
                app.owner.remove_handle(*key);
            }
        }
        self.observations.push(observation);
    }

    fn refresh_applications_with<T>(
        &mut self,
        now: Instant,
        snapshot: impl FnOnce() -> Vec<(i32, bool, T)>,
        mut discover: impl FnMut(&mut Self, T),
    ) {
        if self.stopped || now < self.next_refresh {
            return;
        }
        self.next_refresh = now + SYNC_INTERVAL;
        for owner in &mut self.retired {
            owner.retry_start_on_refresh();
        }
        self.retired
            .retain(AccessibilityApplication::terminal_waiting);
        if self.listener.is_none()
            && let Some(waker) = self.loop_waker.get()
        {
            self.listener = Some(EventListener::new(
                self.native_sender.clone(),
                waker.clone(),
                self.is_suspended.clone(),
            ));
        }
        if self.suspended {
            return;
        }
        let running = snapshot();
        let present: HashSet<_> = running.iter().map(|(pid, _, _)| *pid).collect();
        let absent: Vec<_> = self
            .applications
            .keys()
            .filter(|pid| !present.contains(pid))
            .copied()
            .collect();
        for pid in absent {
            self.retire(pid);
        }
        for (pid, hidden, running_app) in running {
            if !self.applications.contains_key(&pid) {
                discover(self, running_app);
                continue;
            }
            let app = self.applications.get_mut(&pid).expect("existing app");
            app.owner.retry_start_on_refresh();
            let lifetime = app.state.lifetime.clone();
            if hidden != app.state.hidden {
                self.forward_event(
                    &lifetime,
                    if hidden {
                        ApplicationEvent::Hidden
                    } else {
                        ApplicationEvent::Shown
                    },
                );
            }
            self.forward_event(&lifetime, ApplicationEvent::Refresh);
        }
    }

    fn operation(
        &mut self,
        key: CGWindowID,
        operation: impl FnOnce(AXWindow) -> ApplicationOperation,
    ) {
        if self.stopped {
            return;
        }
        let Some(window) = self.handles.get(&key).cloned() else {
            return;
        };
        if let Some(app) = self.applications.get_mut(&window.pid()) {
            app.owner
                .send(ApplicationCommand::Operation(operation(window)));
        }
    }
}

impl WindowServer for MacOsWindowServer {
    fn ingest_native_events(&mut self, _now: Instant) {
        if self.stopped {
            return;
        }
        while let Ok(event) = self.native_events.try_recv() {
            match event {
                NativeEvent::Application { lifetime, event } => {
                    self.forward_event(&lifetime, event)
                }
                NativeEvent::ApplicationLaunched { process_id } => {
                    if let Some(app) = RunningApp::new(process_id) {
                        self.discover(app);
                    }
                }
                NativeEvent::ApplicationTerminated { process_id } => self.retire(process_id),
                NativeEvent::ApplicationActivated { process_id } => {
                    if !RunningApp::is_active_process(process_id) {
                        continue;
                    }
                    if let Some(app) = RunningApp::new(process_id) {
                        self.discover(app);
                        let lifetime = self.applications[&process_id].state.lifetime.clone();
                        self.forward_event(&lifetime, ApplicationEvent::FocusChanged);
                    } else {
                        self.observations.push(Observation::FocusReported {
                            lifetime: Arc::new(ApplicationLifetime { pid: process_id }),
                            window: None,
                        });
                    }
                }
                NativeEvent::SpaceChanged => {
                    let lifetimes: Vec<_> = self
                        .applications
                        .values()
                        .filter(|app| {
                            RunningApp::new(app.state.lifetime.pid)
                                .is_some_and(|app| app.is_active())
                        })
                        .map(|app| app.state.lifetime.clone())
                        .collect();
                    for lifetime in lifetimes {
                        self.forward_event(&lifetime, ApplicationEvent::SpaceChanged);
                    }
                }
                NativeEvent::Suspended(suspended) => {
                    self.suspended = suspended;
                    self.is_suspended.store(suspended, Ordering::Relaxed);
                    let lifetimes: Vec<_> = self
                        .applications
                        .values()
                        .map(|app| app.state.lifetime.clone())
                        .collect();
                    for lifetime in lifetimes {
                        self.forward_event(&lifetime, ApplicationEvent::Suspended(suspended));
                    }
                }
            }
        }
        while let Ok((lifetime, element)) = self.titles.try_recv() {
            if let Some(app) = self.applications.get_mut(&lifetime.pid)
                && Arc::ptr_eq(&lifetime, &app.state.lifetime)
                && !app.state.hidden
                && !app.state.suspended
            {
                app.owner.send(ApplicationCommand::Event(
                    ApplicationEvent::TitleElementChanged(element),
                ));
            }
        }
    }

    fn ingest_read_results(&mut self, now: Instant) {
        if self.stopped {
            return;
        }
        let mut inspections = Vec::new();
        for app in self.applications.values_mut() {
            let Some(connection) = app.owner.connection() else {
                continue;
            };
            for result in app.owner.take_results() {
                match result {
                    ApplicationResult::Inspection(result) => {
                        inspections.push((connection.clone(), result))
                    }
                    ApplicationResult::ObserverReady {
                        lifetime,
                        source,
                        context,
                    } => {
                        if Arc::ptr_eq(&lifetime, &app.state.lifetime) {
                            app.owner.attach_observer(source.0, context);
                        }
                    }
                }
            }
        }
        for (connection, result) in inspections {
            if self
                .applications
                .get(&result.lifetime.pid)
                .and_then(|app| app.owner.connection())
                .is_some_and(|current| Arc::ptr_eq(&current, &connection))
            {
                self.accept_result(result, now);
            }
        }
    }

    fn has_observations(&self) -> bool {
        !self.observations.is_empty()
    }
    fn take_observations(&mut self) -> Vec<Observation> {
        std::mem::take(&mut self.observations)
    }
    fn next_deadline(&self) -> Option<Instant> {
        (!self.stopped).then_some(self.next_refresh)
    }

    fn refresh_applications(&mut self, now: Instant) {
        self.refresh_applications_with(
            now,
            || {
                RunningApp::all()
                    .map(|app| (app.pid(), app.is_hidden(), app))
                    .collect()
            },
            Self::discover,
        );
    }

    fn set_frame(&mut self, window: CGWindowID, frame: PixelRect<Logical>) {
        self.operation(window, |window| ApplicationOperation::SetFrame {
            window,
            frame,
        });
    }
    fn move_offscreen(&mut self, window: CGWindowID, position: (Length<Logical>, Length<Logical>)) {
        self.operation(window, |window| ApplicationOperation::MoveOffscreen {
            window,
            position,
        });
    }
    fn minimize(&mut self, window: CGWindowID) {
        self.operation(window, ApplicationOperation::Minimize);
    }
    fn unminimize(&mut self, window: CGWindowID) {
        self.operation(window, ApplicationOperation::Unminimize);
    }
    fn close(&mut self, window: CGWindowID) {
        self.operation(window, ApplicationOperation::Close);
    }
    fn focus(&mut self, window: CGWindowID) {
        self.operation(window, ApplicationOperation::Focus);
    }
    fn recover(&mut self, window: CGWindowID, frame: PixelRect<Logical>) {
        self.operation(window, |window| ApplicationOperation::Recover {
            window,
            frame,
        });
    }
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        self.listener = None;
        self.title_throttle = None;
        self.focus.clear_external();
        self.observations.clear();
        for (_, mut app) in self.applications.drain() {
            let recovery = recovery
                .iter()
                .filter(|entry| {
                    self.handles
                        .get(&entry.window)
                        .is_some_and(|window| window.pid() == app.state.lifetime.pid)
                })
                .copied()
                .collect();
            app.owner.shutdown(recovery);
            if app.owner.terminal_waiting() {
                self.retired.push(app.owner);
            }
        }
        self.handles.clear();
    }
}

fn observation_lifetime(observation: &Observation) -> &Arc<ApplicationLifetime> {
    match observation {
        Observation::AppWindows { lifetime, .. }
        | Observation::Settled { lifetime, .. }
        | Observation::Space { lifetime, .. }
        | Observation::FocusReported { lifetime, .. }
        | Observation::TitleChanged { lifetime, .. }
        | Observation::AppHidden { lifetime }
        | Observation::AppGone { lifetime } => lifetime,
    }
}
