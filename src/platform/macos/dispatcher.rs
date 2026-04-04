use calloop::futures::Scheduler;
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::rc::autoreleasepool;

use super::event_loop::DomeRunner;

/// Zero-sized proof token that the current code is running on a GCD
/// dispatch queue, not the dome thread. The private field prevents
/// construction outside `gcd_spawn`.
pub(in crate::platform::macos) struct DispatcherMarker(());

type ApplyFn = Box<dyn FnOnce(&mut DomeRunner)>;

pub(super) struct GcdDispatcher {
    scheduler: Scheduler<ApplyFn>,
}

impl GcdDispatcher {
    pub(super) fn new(scheduler: Scheduler<ApplyFn>) -> Self {
        Self { scheduler }
    }

    pub(super) fn dispatch<W, R, A>(&self, work: W, apply: A)
    where
        W: FnOnce(&DispatcherMarker) -> R + Send + 'static,
        R: Send + 'static,
        A: FnOnce(R, &mut DomeRunner) + 'static,
    {
        self.scheduler
            .schedule(async move {
                let result = gcd_spawn(work).await;
                Box::new(move |runner: &mut DomeRunner| apply(result, runner)) as ApplyFn
            })
            .ok();
    }
}

async fn gcd_spawn<R: Send + 'static>(
    work: impl FnOnce(&DispatcherMarker) -> R + Send + 'static,
) -> R {
    let (tx, rx) = futures_channel::oneshot::channel();
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let marker = DispatcherMarker(());
            let _ = tx.send(work(&marker));
        });
    });
    rx.await.expect("GCD task was cancelled")
}
