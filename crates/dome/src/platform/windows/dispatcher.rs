use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx, CoUninitialize};

pub(crate) struct Dispatcher {
    pool: rayon::ThreadPool,
}

impl Dispatcher {
    pub(crate) fn new() -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(50)
            .thread_name(|index| format!("dome-worker-{index}"))
            .start_handler(|_| {
                // Property-store queries require COM. Workers have no message pump.
                let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                assert!(
                    result.is_ok(),
                    "CoInitializeEx failed on worker: {result:?}"
                );
            })
            .exit_handler(|_| unsafe { CoUninitialize() })
            .build()
            .expect("Failed to create dispatcher thread pool");
        Self { pool }
    }

    pub(crate) fn dispatch<Output: Send + 'static>(
        &self,
        work: impl FnOnce(&DispatcherMarker) -> Output + Send + 'static,
    ) -> futures::channel::oneshot::Receiver<Output> {
        let (sender, result) = futures::channel::oneshot::channel();
        let work = move || {
            let _ = sender.send(work(&DispatcherMarker(std::marker::PhantomData)));
        };
        self.pool.spawn(work);
        result
    }
}

pub(crate) struct DispatcherMarker(std::marker::PhantomData<std::rc::Rc<()>>);

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
        let dispatcher = crate::platform::windows::dispatcher::Dispatcher::new();
        let owner = std::thread::current().id();
        let result = dispatcher.dispatch(move |_| {
            assert_ne!(std::thread::current().id(), owner);
            42
        });
        assert_eq!(wait(result).unwrap(), 42);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn dispatch_runs_in_multithreaded_com_apartment() {
        use windows::Win32::System::Com::{
            APTTYPE, APTTYPE_MTA, APTTYPEQUALIFIER, CoGetApartmentType,
        };
        let dispatcher = crate::platform::windows::dispatcher::Dispatcher::new();
        let result = dispatcher.dispatch(|_| {
            let mut apartment = APTTYPE::default();
            let mut qualifier = APTTYPEQUALIFIER::default();
            unsafe { CoGetApartmentType(&mut apartment, &mut qualifier) }.unwrap();
            apartment == APTTYPE_MTA
        });
        assert!(wait(result).unwrap());
    }
}
