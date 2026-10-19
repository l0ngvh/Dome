use std::cell::RefCell;
use std::collections::VecDeque;
use std::future::{Future, poll_fn};
use std::rc::{Rc, Weak};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll, Waker};

use futures::channel::mpsc;
use futures::future::LocalBoxFuture;
use futures::stream::FuturesUnordered;
use futures::{Stream, StreamExt};

use super::dome::Dome;
use super::tray::{NativeTray, Tray};
use super::waker::LoopWaker;
pub(super) mod native;

/// A thread-safe event producer.
pub struct EventSender<Event> {
    sender: mpsc::UnboundedSender<Event>,
    waker: Arc<OnceLock<Mutex<LoopWaker>>>,
}

impl<Event> Clone for EventSender<Event> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
            waker: self.waker.clone(),
        }
    }
}

impl<Event> EventSender<Event> {
    pub fn send(&self, event: impl Into<Event>) -> anyhow::Result<()> {
        self.sender
            .unbounded_send(event.into())
            .map_err(|_| anyhow::anyhow!("native loop stopped"))?;
        if let Some(waker) = self.waker.get() {
            waker.lock().unwrap().wake();
        }
        Ok(())
    }
}

/// Requests native-loop termination from any thread.
#[derive(Clone)]
pub struct Shutdown {
    stopped: Arc<AtomicBool>,
    waker: Arc<OnceLock<Mutex<LoopWaker>>>,
}

impl Shutdown {
    pub fn terminate(&self) {
        self.stopped.store(true, Ordering::Relaxed);
        if let Some(waker) = self.waker.get() {
            waker.lock().unwrap().wake();
        }
    }
}

pub struct RunLoop {
    dome: RefCell<Option<Dome>>,
    submitted: RefCell<VecDeque<LocalBoxFuture<'static, ()>>>,
    tasks: RefCell<FuturesUnordered<LocalBoxFuture<'static, ()>>>,
    stopped: Arc<AtomicBool>,
    waker: Arc<OnceLock<Mutex<LoopWaker>>>,
    dispatcher: super::dispatcher::Dispatcher,
    wake_window: super::waker::WakeWindow,
    tray: Rc<NativeTray>,
}

impl RunLoop {
    /// The synchronous callback cannot return a borrow of the installed application.
    pub fn with_dome<Result>(callback: impl FnOnce(&mut Dome) -> Result) -> Result {
        let run_loop = Self::current();
        let mut application = run_loop.dome.borrow_mut();
        callback(application.as_mut().expect("application started"))
    }

    pub fn channel<Event>() -> (EventSender<Event>, mpsc::UnboundedReceiver<Event>) {
        let (sender, source) = mpsc::unbounded();
        (
            EventSender {
                sender,
                waker: Self::current().waker.clone(),
            },
            source,
        )
    }

    /// Each callback finishes its scoped application borrow before consuming the next item.
    pub fn insert<Source, Callback>(source: Source, mut callback: Callback)
    where
        Source: Stream + 'static,
        Callback: FnMut(Source::Item, &mut Dome) + 'static,
    {
        let run_loop = Self::current();
        let stopped = run_loop.stopped.clone();
        run_loop.spawn(async move {
            let mut source = Box::pin(source);
            while let Some(event) = source.next().await {
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
                Self::with_dome(|application| callback(event, application));
                if stopped.load(Ordering::Relaxed) {
                    break;
                }
            }
        });
    }

    /// Installs completed state without polling sources or queued work.
    pub fn install(application: Dome) {
        let run_loop = Self::current();
        let mut installed = run_loop.dome.borrow_mut();
        assert!(installed.is_none(), "application already installed");
        *installed = Some(application);
        drop(installed);
        run_loop.wake();
    }

    pub fn terminate() {
        Self::current().shutdown_handle().terminate();
    }

    pub fn shutdown_handle(&self) -> Shutdown {
        Shutdown {
            stopped: self.stopped.clone(),
            waker: self.waker.clone(),
        }
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.stopped.load(Ordering::Relaxed)
    }

    pub(crate) fn clear_application(&self) {
        let _application = self.dome.borrow_mut().take();
    }

    pub fn spawn(&self, future: impl Future<Output = ()> + 'static) {
        if submit_task(&self.submitted, &self.stopped, future) {
            self.wake();
        }
    }

    pub fn waker(&self) -> LoopWaker {
        self.waker
            .get()
            .expect("app initialized")
            .lock()
            .unwrap()
            .clone()
    }

    /// Submits synchronous work immediately, even before the native loop starts.
    /// Dropping the receiver does not cancel work. The receiver reports cancellation
    /// if the sender is dropped without sending a result.
    pub fn dispatch<Work, Output>(work: Work) -> futures::channel::oneshot::Receiver<Output>
    where
        Work: FnOnce(&crate::platform::windows::dispatcher::DispatcherMarker) -> Output
            + Send
            + 'static,
        Output: Send + 'static,
    {
        Self::current().dispatcher.dispatch(work)
    }

    fn wake(&self) {
        if let Some(waker) = self.waker.get() {
            waker.lock().unwrap().wake();
        }
    }
}

impl Drop for RunLoop {
    fn drop(&mut self) {
        self.discard_tasks();
        self.clear_application();
        CURRENT.with(|current| *current.borrow_mut() = None);
    }
}

pub enum NativeEvent {
    DisplayChanged,
    WorkAreaChanged,
}

thread_local! {
    static CURRENT: RefCell<Option<Weak<RunLoop>>> = const { RefCell::new(None) };
}
impl RunLoop {
    pub(crate) fn new(
        icon: windows::Win32::UI::WindowsAndMessaging::HICON,
        on_action: Rc<dyn Fn(crate::action::Action)>,
    ) -> anyhow::Result<(
        Rc<Self>,
        mpsc::UnboundedReceiver<NativeEvent>,
        mpsc::UnboundedReceiver<usize>,
    )> {
        CURRENT.with(|current| {
            anyhow::ensure!(current.borrow().is_none(), "native loop already exists");
            Ok(())
        })?;
        let stopped = Arc::new(AtomicBool::new(false));
        let waker = Arc::new(OnceLock::new());
        let (sender, events) = mpsc::unbounded();
        let (timer_sender, timers) = mpsc::unbounded();
        let panic = Rc::new(native::CallbackPanic::default());
        let tray = Rc::new(NativeTray::new(
            icon,
            on_action,
            EventSender {
                sender,
                waker: waker.clone(),
            },
            panic.clone(),
        )?);
        let (wake_window, native_waker) = super::waker::WakeWindow::new(
            EventSender {
                sender: timer_sender,
                waker: waker.clone(),
            },
            panic,
        )?;
        assert!(waker.set(Mutex::new(native_waker)).is_ok());
        let owner = Rc::new(Self {
            dome: RefCell::new(None),
            submitted: RefCell::default(),
            tasks: RefCell::default(),
            stopped,
            waker,
            dispatcher: super::dispatcher::Dispatcher::new(),
            wake_window,
            tray,
        });
        CURRENT.with(|current| *current.borrow_mut() = Some(Rc::downgrade(&owner)));
        Ok((owner, events, timers))
    }

    pub(super) fn tray(&self) -> Rc<dyn Tray> {
        self.tray.clone()
    }

    pub(crate) fn timer_window(&self) -> windows::Win32::Foundation::HWND {
        self.wake_window.hwnd()
    }

    pub(crate) fn current() -> Rc<Self> {
        Self::try_current().expect("no native loop on this thread")
    }

    pub(super) fn callback_panic(&self) -> Rc<native::CallbackPanic> {
        self.wake_window.state.panic.clone()
    }

    pub(super) fn try_current() -> Option<Rc<Self>> {
        CURRENT.with(|current| current.borrow().as_ref().and_then(Weak::upgrade))
    }

    pub(super) fn wake_received(&self) {
        self.poll_tasks();
        if self.is_stopped() {
            unsafe { windows::Win32::UI::WindowsAndMessaging::PostQuitMessage(0) };
        }
    }

    pub fn run(self: Rc<Self>) -> anyhow::Result<()> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_tasks();
            if self.is_stopped() {
                Ok(())
            } else {
                self.run_native()
            }
        }));
        self.discard_tasks();
        self.clear_application();
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn run_native(&self) -> anyhow::Result<()> {
        use windows::Win32::UI::WindowsAndMessaging::{
            DispatchMessageW, GetMessageW, MSG, TranslateMessage,
        };
        let panic = &self.wake_window.state.panic;
        panic.resume();
        let mut message = MSG::default();
        while !self.is_stopped() {
            let result = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
            let error = (result == -1).then(windows::core::Error::from_thread);
            panic.resume();
            if let Some(error) = error {
                return Err(error.into());
            }
            if result == 0 {
                break;
            }
            unsafe {
                let _ = TranslateMessage(&message);
                DispatchMessageW(&message);
            }
            panic.resume();
        }
        Ok(())
    }

    fn poll_tasks(&self) {
        if self.is_stopped() {
            return;
        }
        poll_tasks(
            &self.submitted,
            &self.tasks,
            &self.stopped,
            &self.waker().task_waker(),
        );
    }

    fn discard_tasks(&self) {
        discard_tasks(&self.submitted, &self.tasks, &self.stopped);
    }
}

#[cfg(feature = "windows-lifecycle-tests")]
pub fn run_windows_lifecycle_test(case: &str) {
    match case {
        "wake-delivery" => native_tests::worker_event_reaches_the_running_native_pump(),
        _ => panic!("unknown native test {case}"),
    }
}

fn submit_task(
    submitted: &RefCell<VecDeque<LocalBoxFuture<'static, ()>>>,
    stopped: &Arc<AtomicBool>,
    future: impl Future<Output = ()> + 'static,
) -> bool {
    if stopped.load(Ordering::Relaxed) {
        return false;
    }
    let stopped = stopped.clone();
    let mut future = Box::pin(future);
    let guarded = poll_fn(move |context| {
        if stopped.load(Ordering::Relaxed) {
            Poll::Ready(())
        } else {
            future.as_mut().poll(context)
        }
    });
    submitted.borrow_mut().push_back(Box::pin(guarded));
    true
}

fn poll_tasks(
    submitted: &RefCell<VecDeque<LocalBoxFuture<'static, ()>>>,
    tasks: &RefCell<FuturesUnordered<LocalBoxFuture<'static, ()>>>,
    stopped: &AtomicBool,
    waker: &Waker,
) {
    if stopped.load(Ordering::Relaxed) {
        return;
    }
    let mut tasks = tasks.borrow_mut();
    let mut context = Context::from_waker(waker);
    loop {
        tasks.extend(submitted.borrow_mut().drain(..));
        let result = tasks.poll_next_unpin(&mut context);
        if stopped.load(Ordering::Relaxed) {
            return;
        }
        if !submitted.borrow().is_empty() {
            continue;
        }
        match result {
            Poll::Ready(Some(())) => continue,
            Poll::Ready(None) | Poll::Pending => return,
        }
    }
}

fn discard_tasks(
    submitted: &RefCell<VecDeque<LocalBoxFuture<'static, ()>>>,
    tasks: &RefCell<FuturesUnordered<LocalBoxFuture<'static, ()>>>,
    stopped: &AtomicBool,
) {
    stopped.store(true, Ordering::Relaxed);
    let submitted = submitted.take();
    let tasks = tasks.take();
    drop(submitted);
    drop(tasks);
}

#[cfg(feature = "windows-lifecycle-tests")]
mod native_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ipc_tests;
