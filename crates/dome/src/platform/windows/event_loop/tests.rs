use super::*;
use std::cell::Cell;
use std::pin::Pin;
use std::sync::atomic::AtomicUsize;
use std::task::Wake;

#[derive(Default)]
struct TestTasks {
    submitted: RefCell<VecDeque<LocalBoxFuture<'static, ()>>>,
    tasks: RefCell<FuturesUnordered<LocalBoxFuture<'static, ()>>>,
    stopped: Arc<AtomicBool>,
    wake: Arc<WakeCount>,
}

impl TestTasks {
    fn submit(&self, future: impl Future<Output = ()> + 'static) -> bool {
        submit_task(&self.submitted, &self.stopped, future)
    }

    fn poll(&self) {
        poll_tasks(
            &self.submitted,
            &self.tasks,
            &self.stopped,
            &Waker::from(self.wake.clone()),
        );
    }

    fn discard(&self) {
        discard_tasks(&self.submitted, &self.tasks, &self.stopped);
    }
}

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
}

#[test]
fn submit_during_poll_runs_child_after_parent_yields() {
    let tasks = Rc::new(TestTasks::default());
    let child_tasks = tasks.clone();
    let delivered = Rc::new(Cell::new(false));
    let observed = delivered.clone();
    tasks.submit(async move {
        let child_observed = observed.clone();
        assert!(child_tasks.submit(async move { child_observed.set(true) }));
        assert!(!observed.get());
        std::future::pending::<()>().await;
    });
    tasks.poll();
    assert!(delivered.get());
    tasks.discard();
}

#[test]
fn submit_from_completed_future_drop_delivers_work() {
    struct SubmitOnDrop {
        tasks: Rc<TestTasks>,
        delivered: Rc<Cell<bool>>,
    }
    impl Future for SubmitOnDrop {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            Poll::Ready(())
        }
    }
    impl Drop for SubmitOnDrop {
        fn drop(&mut self) {
            let delivered = self.delivered.clone();
            assert!(self.tasks.submit(async move { delivered.set(true) }));
        }
    }
    let tasks = Rc::new(TestTasks::default());
    let delivered = Rc::new(Cell::new(false));
    tasks.submit(SubmitOnDrop {
        tasks: tasks.clone(),
        delivered: delivered.clone(),
    });
    tasks.poll();
    assert!(delivered.get());
}

#[test]
fn stop_inside_pending_poll_suppresses_siblings_in_same_collection_poll() {
    let tasks = TestTasks::default();
    let stopped = tasks.stopped.clone();
    let finished = Rc::new(Cell::new(false));
    let observed = finished.clone();
    tasks.submit(poll_fn(move |_| {
        stopped.store(true, Ordering::Relaxed);
        observed.set(true);
        Poll::Pending
    }));
    let sibling = Rc::new(());
    let released = Rc::downgrade(&sibling);
    tasks.submit(async move {
        let _sibling = sibling;
        panic!("sibling polled after stop");
    });
    tasks.poll();
    assert!(finished.get());
    assert!(released.upgrade().is_none());
    tasks.discard();
}

#[test]
fn poll_when_stopped_avoids_task_borrow_and_rejects_submission() {
    let tasks = TestTasks::default();
    tasks.stopped.store(true, Ordering::Relaxed);
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    let _borrow = tasks.tasks.borrow_mut();
    assert!(!tasks.submit(async move {
        let _capture = capture;
        panic!("stopped submission ran");
    }));
    tasks.poll();
    assert!(released.upgrade().is_none());
}

#[test]
fn poll_recursively_panics_and_cleanup_releases_pending_work() {
    let tasks = Rc::new(TestTasks::default());
    let reentrant = tasks.clone();
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    tasks.submit(async move {
        let _capture = capture;
        reentrant.poll();
    });
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| tasks.poll()));
    assert!(result.is_err());
    tasks.discard();
    assert!(released.upgrade().is_none());
}

#[test]
fn wake_one_task_leaves_unrelated_tasks_sleeping() {
    let tasks = TestTasks::default();
    let polls = Rc::new(Cell::new(0));
    let selected = Rc::new(RefCell::new(None));
    for index in 0..128 {
        let polls = polls.clone();
        let selected = selected.clone();
        tasks.submit(poll_fn(move |context| {
            polls.set(polls.get() + 1);
            if index == 42 {
                *selected.borrow_mut() = Some(context.waker().clone());
            }
            Poll::Pending
        }));
    }
    tasks.poll();
    assert_eq!(polls.get(), 128);
    tasks.poll();
    assert_eq!(polls.get(), 128);
    tasks.wake.0.store(0, Ordering::Relaxed);
    for _ in 0..10 {
        selected.borrow().as_ref().unwrap().wake_by_ref();
    }
    assert!(tasks.wake.0.load(Ordering::Relaxed) > 0);
    tasks.poll();
    assert_eq!(polls.get(), 129);
    tasks.discard();
}

#[test]
fn wake_from_worker_and_during_poll_delivers_work() {
    let tasks = TestTasks::default();
    let (sender, receiver) = futures::channel::oneshot::channel::<()>();
    let delivered = Rc::new(Cell::new(false));
    let observed = delivered.clone();
    tasks.submit(async move {
        receiver.await.unwrap();
        let mut yielded = false;
        poll_fn(|context| {
            if yielded {
                return Poll::Ready(());
            }
            yielded = true;
            context.waker().wake_by_ref();
            Poll::Pending
        })
        .await;
        observed.set(true);
    });
    tasks.poll();
    assert!(!delivered.get());
    tasks.wake.0.store(0, Ordering::Relaxed);
    std::thread::spawn(move || sender.send(()).unwrap())
        .join()
        .unwrap();
    assert!(tasks.wake.0.load(Ordering::Relaxed) > 0);
    for _ in 0..4 {
        tasks.wake.0.store(0, Ordering::Relaxed);
        tasks.poll();
        if delivered.get() {
            break;
        }
        assert!(tasks.wake.0.load(Ordering::Relaxed) > 0);
    }
    assert!(delivered.get());
}

#[test]
fn discard_with_retained_waker_releases_future() {
    let tasks = TestTasks::default();
    let retained_waker = Rc::new(RefCell::new(None));
    let saved = retained_waker.clone();
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    tasks.submit(poll_fn(move |context| {
        let _capture = &capture;
        *saved.borrow_mut() = Some(context.waker().clone());
        Poll::Pending
    }));
    tasks.poll();
    tasks.discard();
    assert!(released.upgrade().is_none());
    retained_waker.borrow().as_ref().unwrap().wake_by_ref();
    tasks.poll();
}

#[test]
fn discard_releases_both_collections_before_dropping_futures() {
    struct InspectOnDrop {
        tasks: Rc<TestTasks>,
        drops: Rc<Cell<usize>>,
    }
    impl Future for InspectOnDrop {
        type Output = ();
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<()> {
            Poll::Pending
        }
    }
    impl Drop for InspectOnDrop {
        fn drop(&mut self) {
            assert!(self.tasks.stopped.load(Ordering::Relaxed));
            assert!(self.tasks.submitted.borrow_mut().is_empty());
            assert!(self.tasks.tasks.borrow_mut().is_empty());
            assert!(
                !self
                    .tasks
                    .submit(async { panic!("submitted during disposal") })
            );
            self.drops.set(self.drops.get() + 1);
        }
    }
    let tasks = Rc::new(TestTasks::default());
    let drops = Rc::new(Cell::new(0));
    tasks.submit(InspectOnDrop {
        tasks: tasks.clone(),
        drops: drops.clone(),
    });
    tasks.poll();
    tasks.submit(InspectOnDrop {
        tasks: tasks.clone(),
        drops: drops.clone(),
    });
    tasks.discard();
    assert_eq!(drops.get(), 2);
}

#[test]
fn discard_never_polled_submissions_releases_without_running() {
    let tasks = TestTasks::default();
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    tasks.submit(async move {
        let _capture = capture;
        panic!("never-run task polled during disposal");
    });
    tasks.discard();
    assert!(released.upgrade().is_none());
}
