use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use super::timer::Timer;

static SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);
const SIGNAL_CHECK_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct StopSignals {
    timer: Option<Timer>,
    previous: Vec<(libc::c_int, libc::sighandler_t)>,
}

impl StopSignals {
    pub(super) fn new(request_stop: impl Fn() + 'static) -> anyhow::Result<Self> {
        SIGNAL_RECEIVED.store(false, Ordering::Relaxed);
        let mut signals = Self {
            timer: None,
            previous: Vec::new(),
        };
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
            let previous = unsafe { libc::signal(signal, signal_handler as *const () as usize) };
            if previous == libc::SIG_ERR {
                return Err(std::io::Error::last_os_error().into());
            }
            signals.previous.push((signal, previous));
        }
        signals.timer = Some(Timer::repeating(SIGNAL_CHECK_INTERVAL, move || {
            if SIGNAL_RECEIVED.swap(false, Ordering::Relaxed) {
                request_stop();
            }
        }));
        Ok(signals)
    }
}

impl Drop for StopSignals {
    fn drop(&mut self) {
        self.timer.take();
        for &(signal, previous) in &self.previous {
            unsafe { libc::signal(signal, previous) };
        }
    }
}

extern "C" fn signal_handler(_signal: libc::c_int) {
    SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
}
