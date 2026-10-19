use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::rc::autoreleasepool;

pub(super) fn dispatch<Output: Send + 'static>(
    work: impl FnOnce(&DispatcherMarker) -> Output + Send + 'static,
) -> futures::channel::oneshot::Receiver<Output> {
    let (sender, result) = futures::channel::oneshot::channel();
    let work = move || {
        let _ = sender.send(work(&DispatcherMarker(std::marker::PhantomData)));
    };
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || autoreleasepool(|_| work()));
    result
}

pub(super) struct DispatcherMarker(std::marker::PhantomData<std::rc::Rc<()>>);

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::time::{Duration, Instant};

    fn wait<Output>(work: impl Future<Output = Output>) -> Output {
        struct WaitingThread(std::thread::Thread);
        impl Wake for WaitingThread {
            fn wake(self: Arc<Self>) {
                self.0.unpark();
            }
        }
        let waker = Waker::from(Arc::new(WaitingThread(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut work = std::pin::pin!(work);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Poll::Ready(output) = work.as_mut().poll(&mut context) {
                return output;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "dispatch completion timed out");
            std::thread::park_timeout(remaining);
        }
    }

    #[test]
    fn dispatch_returns_result_on_worker_thread() {
        let owner = std::thread::current().id();
        let result = crate::platform::macos::dispatcher::dispatch(move |_| {
            assert_ne!(std::thread::current().id(), owner);
            42
        });
        assert_eq!(wait(result).unwrap(), 42);
    }
}
