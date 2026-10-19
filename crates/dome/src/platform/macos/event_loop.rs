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
use super::menu_bar::{MenuBar, NativeMenuBar};
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
    menu_bar: Rc<NativeMenuBar>,
    app: objc2::rc::Retained<objc2_app_kit::NSApplication>,
    delegate: objc2::rc::Retained<native::AppDelegate>,
    source: objc2_core_foundation::CFRetained<objc2_core_foundation::CFRunLoopSource>,
    native_run_loop: objc2_core_foundation::CFRetained<objc2_core_foundation::CFRunLoop>,
    _signals: super::signals::StopSignals,
}

impl RunLoop {
    /// The synchronous callback cannot return a borrow of the installed application.
    pub(super) fn with_dome<Result>(callback: impl FnOnce(&mut Dome) -> Result) -> Result {
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
    pub(super) fn insert<Source, Callback>(source: Source, mut callback: Callback)
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
    pub(super) fn install(application: Dome) {
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
    pub(super) fn dispatch<Work, Output>(work: Work) -> futures::channel::oneshot::Receiver<Output>
    where
        Work: FnOnce(&crate::platform::macos::dispatcher::DispatcherMarker) -> Output
            + Send
            + 'static,
        Output: Send + 'static,
    {
        super::dispatcher::dispatch(work)
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
        self.source.invalidate();
        self.native_run_loop
            .remove_source(Some(&self.source), unsafe {
                objc2_core_foundation::kCFRunLoopDefaultMode
            });
        self.menu_bar.remove();
        self.app.setDelegate(None);
        unsafe {
            objc2_foundation::NSNotificationCenter::defaultCenter().removeObserver(&self.delegate)
        };
    }
}

pub enum NativeEvent {
    DisplayChanged,
}

thread_local! {
    static CURRENT: RefCell<Option<Weak<RunLoop>>> = const { RefCell::new(None) };
}
impl RunLoop {
    pub(crate) fn current() -> Rc<Self> {
        Self::try_current().expect("no native loop on this thread")
    }

    pub(super) fn menu_bar(&self) -> Rc<dyn MenuBar> {
        self.menu_bar.clone()
    }

    pub fn run(self: Rc<Self>) -> anyhow::Result<()> {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.poll_tasks();
            if self.is_stopped() {
                Ok(())
            } else {
                self.app.run();
                Ok(())
            }
        }));
        self.discard_tasks();
        self.clear_application();
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn try_current() -> Option<Rc<Self>> {
        CURRENT.with(|current| current.borrow().as_ref().and_then(Weak::upgrade))
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

#[cfg(feature = "macos-lifecycle-tests")]
pub fn run_macos_lifecycle_test(case: &str) {
    match case {
        "wake-delivery" => native_tests::worker_event_reaches_the_running_native_pump(),
        "timer-once-cancel" => {
            native_tests::schedule_one_shot_delivers_once_and_drop_cancels_delivery()
        }
        "timer-self-cancel" => {
            native_tests::cancel_repeating_timer_from_callback_stops_delivery_and_releases_captures(
            )
        }
        "throttle-burst" => {
            native_tests::submit_burst_delivers_leading_and_latest_trailing_values()
        }
        "signal-int" => native_tests::stop_signal_defers_to_native_timer(libc::SIGINT),
        "signal-term" => native_tests::stop_signal_defers_to_native_timer(libc::SIGTERM),
        "signal-hup" => native_tests::stop_signal_defers_to_native_timer(libc::SIGHUP),
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

#[cfg(feature = "macos-lifecycle-tests")]
mod native_tests;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod ipc_tests;
