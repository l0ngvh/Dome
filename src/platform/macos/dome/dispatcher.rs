use calloop::futures::Scheduler;
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::rc::autoreleasepool;

use super::runloop::State;

type ApplyFn = Box<dyn FnOnce(&mut State)>;

pub(super) struct GcdDispatcher {
    scheduler: Scheduler<ApplyFn>,
}

impl GcdDispatcher {
    pub(super) fn new(scheduler: Scheduler<ApplyFn>) -> Self {
        Self { scheduler }
    }

    pub(super) fn dispatch<W, R, A>(&self, work: W, apply: A)
    where
        W: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
        A: FnOnce(R, &mut State) + 'static,
    {
        self.scheduler
            .schedule(async move {
                let result = gcd_spawn(work).await;
                Box::new(move |state: &mut State| apply(result, state)) as ApplyFn
            })
            .ok();
    }
}

async fn gcd_spawn<R: Send + 'static>(work: impl FnOnce() -> R + Send + 'static) -> R {
    let (tx, rx) = futures_channel::oneshot::channel();
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let _ = tx.send(work());
        });
    });
    rx.await.expect("GCD task was cancelled")
}
