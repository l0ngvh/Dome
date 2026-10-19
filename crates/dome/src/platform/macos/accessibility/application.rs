use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex, mpsc};
use std::time::Instant;

use anyhow::{Context, Result};
use dome_core::{FractionalCoordinate, Logical, PixelRect};
use futures::channel::oneshot;
use objc2::rc::autoreleasepool;
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopCommonModes};
use objc2_core_graphics::CGWindowID;

use super::observer::{CallbackContext, RegisteredObserver};
use super::queue::{ApplicationRequest, enqueue_request};
use super::{AXApp, AXWindow};
use crate::platform::macos::event_loop::EventSender;
use crate::platform::macos::running_application::RunningApp;
use crate::platform::macos::waker::{LoopWaker, create_wake_source};
use crate::platform::macos::window_server::{
    NativeAppObservation, NativeEvent, NativeFocusedWindow,
};

#[derive(Default)]
pub(in crate::platform::macos) struct FocusState {
    pub(in crate::platform::macos) pending: Option<CGWindowID>,
    pub(in crate::platform::macos) submitted: Option<CGWindowID>,
    pub(in crate::platform::macos) last_external_submission_at: Option<Instant>,
}

pub(in crate::platform::macos) struct AccessibilityApplication {
    connection: RefCell<Option<Connection>>,
    pid: i32,
    app_name: Option<String>,
    bundle_id: Option<String>,
    focus: Arc<Mutex<FocusState>>,
    events: EventSender<NativeEvent>,
    suspended: Arc<AtomicBool>,
}

pub(super) struct NativeAccessibility {
    receiver: mpsc::Receiver<ApplicationRequest>,
    pending: VecDeque<ApplicationRequest>,
    pub(super) application: Arc<AXApp>,
    pub(super) windows: HashMap<CGWindowID, AXWindow>,
    observer: Option<RegisteredObserver>,
    focus: Arc<Mutex<FocusState>>,
}

impl NativeAccessibility {
    pub(super) fn ensure_window(&mut self, id: CGWindowID) -> Result<()> {
        if !self.windows.contains_key(&id) {
            for window in self.application.clone().windows()? {
                self.windows.entry(window.cg_id()).or_insert(window);
            }
        }
        anyhow::ensure!(
            self.windows.contains_key(&id),
            "Unknown Accessibility window {id}"
        );
        Ok(())
    }

    fn refresh_observer(&mut self, callbacks: &CallbackContext) {
        self.observer.take();
        self.observer = RegisteredObserver::new(self.application.clone(), callbacks);
    }

    fn execute(&mut self, request: ApplicationRequest, callbacks: &CallbackContext) -> Result<()> {
        match request {
            ApplicationRequest::RefreshObserver => self.refresh_observer(callbacks),
            ApplicationRequest::ForgetWindow(id) => {
                self.windows.remove(&id);
            }
            ApplicationRequest::ReadWindows(ids, reply) => {
                send_reply(reply, self.read_windows(ids))
            }
            ApplicationRequest::ReadFocus(reply) => send_reply(reply, self.read_focus()),
            ApplicationRequest::ReadTitle(id, reply) => {
                let result = self.window(id).map(AXWindow::read_title);
                send_reply(reply, result);
            }
            ApplicationRequest::SetFrame(id, rect) => {
                self.ensure_window(id)?;
                let window = self.windows.get_mut(&id).expect("resolved window");
                window.placement = Some((rect, Instant::now()));
                window.set_frame(rect)?;
            }
            ApplicationRequest::HideAt(id, x, y) => self.window(id)?.hide_at(x, y)?,
            ApplicationRequest::Minimize(id) => self.window(id)?.minimize()?,
            ApplicationRequest::Unminimize(id) => self.window(id)?.unminimize()?,
            ApplicationRequest::Close(id) => self.window(id)?.close()?,
            ApplicationRequest::Focus(id) => {
                self.ensure_window(id)?;
                let authorized = self.focus.lock().expect("focus lock").submitted == Some(id);
                if authorized {
                    self.windows[&id].focus()?;
                }
            }
        }
        Ok(())
    }

    fn window(&mut self, id: CGWindowID) -> Result<&AXWindow> {
        self.ensure_window(id)?;
        Ok(&self.windows[&id])
    }
}

impl AccessibilityApplication {
    pub(in crate::platform::macos) fn new(
        app: &RunningApp,
        focus: Arc<Mutex<FocusState>>,
        events: EventSender<NativeEvent>,
        suspended: Arc<AtomicBool>,
    ) -> Self {
        let facade = Self {
            connection: RefCell::new(None),
            pid: app.pid(),
            app_name: app.app_name(),
            bundle_id: app.bundle_id(),
            focus,
            events,
            suspended,
        };
        match facade.start() {
            Ok(connection) => *facade.connection.borrow_mut() = Some(connection),
            Err(error) => {
                tracing::warn!(pid = facade.pid, %error, "Failed to start Accessibility worker")
            }
        }
        facade
    }

    pub(in crate::platform::macos) fn close(&self, id: CGWindowID) {
        self.submit(ApplicationRequest::Close(id));
    }

    pub(in crate::platform::macos) fn focus(&self, id: CGWindowID) {
        self.submit(ApplicationRequest::Focus(id));
    }

    pub(in crate::platform::macos) fn refresh_observer(&self) {
        self.submit(ApplicationRequest::RefreshObserver);
    }

    pub(in crate::platform::macos) fn minimize(&self, id: CGWindowID) {
        self.submit(ApplicationRequest::Minimize(id));
    }

    pub(in crate::platform::macos) fn unminimize(&self, id: CGWindowID) {
        self.submit(ApplicationRequest::Unminimize(id));
    }

    pub(in crate::platform::macos) fn forget_window(&self, id: CGWindowID) {
        self.submit(ApplicationRequest::ForgetWindow(id));
    }

    pub(in crate::platform::macos) fn set_frame(&self, id: CGWindowID, rect: PixelRect<Logical>) {
        self.submit(ApplicationRequest::SetFrame(id, rect));
    }

    pub(in crate::platform::macos) fn hide_at(
        &self,
        id: CGWindowID,
        x: FractionalCoordinate<Logical>,
        y: FractionalCoordinate<Logical>,
    ) {
        self.submit(ApplicationRequest::HideAt(id, x, y));
    }

    pub(in crate::platform::macos) fn read_focus(
        &self,
    ) -> impl Future<Output = Result<Option<NativeFocusedWindow>>> + 'static + use<> {
        let (sender, receiver) = oneshot::channel();
        self.submit(ApplicationRequest::ReadFocus(sender));
        read_reply(receiver)
    }

    pub(in crate::platform::macos) fn read_title(
        &self,
        id: CGWindowID,
    ) -> impl Future<Output = Result<Option<String>>> + 'static + use<> {
        let (sender, receiver) = oneshot::channel();
        self.submit(ApplicationRequest::ReadTitle(id, sender));
        read_reply(receiver)
    }

    pub(in crate::platform::macos) fn read_windows(
        &self,
        ids: Vec<CGWindowID>,
    ) -> impl Future<Output = Result<Option<NativeAppObservation>>> + 'static + use<> {
        let (sender, receiver) = oneshot::channel();
        self.submit(ApplicationRequest::ReadWindows(ids, sender));
        read_reply(receiver)
    }

    fn start(&self) -> Result<Connection> {
        let (sender, receiver) = mpsc::channel();
        let wake = Arc::new(Mutex::new(None));
        let worker_wake = wake.clone();
        let pid = self.pid;
        let app_name = self.app_name.clone();
        let bundle_id = self.bundle_id.clone();
        let focus = self.focus.clone();
        let events = self.events.clone();
        let suspended = self.suspended.clone();
        std::thread::Builder::new()
            .name(format!("accessibility-{pid}"))
            .spawn(move || {
                autoreleasepool(|_| {
                    let run_loop = CFRunLoop::current().expect("worker run loop");
                    let application = Arc::new(AXApp::new(pid, app_name, bundle_id));
                    let mut context = Box::new(WorkerContext {
                        native: RefCell::new(NativeAccessibility {
                            receiver,
                            pending: VecDeque::new(),
                            application,
                            windows: HashMap::new(),
                            observer: None,
                            focus,
                        }),
                        callbacks: CallbackContext::new(events, suspended),
                    });
                    let source =
                        create_wake_source((&mut *context as *mut WorkerContext).cast(), perform);
                    let waker = LoopWaker::new(source.clone(), run_loop.clone());
                    context.callbacks.set_waker(waker.clone());
                    *worker_wake.lock().expect("worker wake lock") = Some(waker.clone());
                    run_loop.add_source(Some(&source), unsafe { kCFRunLoopCommonModes });
                    let _command_source = CommandSource {
                        source,
                        run_loop,
                        wake: worker_wake,
                    };
                    context
                        .native
                        .borrow_mut()
                        .refresh_observer(&context.callbacks);
                    waker.wake();
                    CFRunLoop::run();
                    context.native.borrow_mut().observer.take();
                });
            })
            .context("spawn Accessibility worker")?;
        Ok(Connection {
            sender: Some(sender),
            wake,
        })
    }

    fn submit(&self, request: ApplicationRequest) {
        let mut connection = self.connection.borrow_mut();
        let request = if let Some(active) = connection.as_ref() {
            match active.sender.as_ref().expect("live sender").send(request) {
                Ok(()) => {
                    active.wake();
                    return;
                }
                Err(error) => error.0,
            }
        } else {
            request
        };
        connection.take();
        match self.start() {
            Ok(replacement) => {
                if replacement
                    .sender
                    .as_ref()
                    .expect("live sender")
                    .send(request)
                    .is_err()
                {
                    tracing::warn!(pid = self.pid, "Accessibility worker closed during restart");
                }
                replacement.wake();
                *connection = Some(replacement);
            }
            Err(error) => {
                tracing::warn!(pid = self.pid, %error, "Failed to restart Accessibility worker")
            }
        }
    }
}

struct Connection {
    sender: Option<mpsc::Sender<ApplicationRequest>>,
    wake: Arc<Mutex<Option<LoopWaker>>>,
}

struct WorkerContext {
    native: RefCell<NativeAccessibility>,
    callbacks: CallbackContext,
}

struct CommandSource {
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
    wake: Arc<Mutex<Option<LoopWaker>>>,
}

impl Drop for CommandSource {
    fn drop(&mut self) {
        self.source.invalidate();
        self.run_loop
            .remove_source(Some(&self.source), unsafe { kCFRunLoopCommonModes });
        self.wake.lock().expect("worker wake lock").take();
    }
}

async fn read_reply<T>(receiver: oneshot::Receiver<Result<Option<T>>>) -> Result<Option<T>> {
    receiver.await.context("Accessibility read cancelled")?
}

impl Connection {
    fn wake(&self) {
        if let Some(waker) = self.wake.lock().expect("worker wake lock").as_ref() {
            waker.wake();
        }
    }
}

impl Drop for Connection {
    fn drop(&mut self) {
        self.sender.take();
        self.wake();
    }
}

fn send_reply<T>(sender: oneshot::Sender<Result<Option<T>>>, result: Result<Option<T>>) {
    if let Err(error) = &result {
        tracing::warn!(%error, "Accessibility read failed");
    }
    if sender.send(result).is_err() {
        tracing::trace!("Accessibility read receiver dropped");
    }
}

unsafe extern "C-unwind" fn perform(info: *mut std::ffi::c_void) {
    let context = unsafe { &*info.cast::<WorkerContext>() };
    let Ok(mut native) = context.native.try_borrow_mut() else {
        context.callbacks.wake();
        return;
    };
    autoreleasepool(|_| {
        loop {
            match native.receiver.try_recv() {
                Ok(request) => enqueue_request(&mut native.pending, request),
                Err(mpsc::TryRecvError::Empty) => break,
                Err(mpsc::TryRecvError::Disconnected) => {
                    native.pending.clear();
                    CFRunLoop::current().expect("worker run loop").stop();
                    return;
                }
            }
        }
        if let Some(event) = context.callbacks.pop() {
            context.callbacks.decode(event);
        } else if let Some(request) = native.pending.pop_front()
            && let Err(error) = native.execute(request, &context.callbacks)
        {
            tracing::warn!(%error, "Accessibility operation failed");
        }
        if !native.pending.is_empty() || context.callbacks.has_events() {
            context.callbacks.wake();
        }
    });
}
