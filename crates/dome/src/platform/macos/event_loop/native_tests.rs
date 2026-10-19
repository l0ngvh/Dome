use super::*;
use crate::platform::macos::signals::StopSignals;
use crate::platform::macos::throttle::Throttle;
use crate::platform::macos::timer::Timer;
use objc2_core_foundation::{CFRunLoop, kCFRunLoopDefaultMode};
use std::cell::Cell;
use std::time::{Duration, Instant};

pub(super) fn worker_event_reaches_the_running_native_pump() {
    let (owner, _) = RunLoop::new(icon(), Rc::new(|_| {})).unwrap();
    let (sender, mut events) = RunLoop::channel();
    let delivered = Rc::new(Cell::new(false));
    let observed = delivered.clone();
    let shutdown = owner.shutdown_handle();
    owner.spawn(async move {
        while let Some(()) = events.next().await {
            observed.set(true);
            shutdown.terminate();
        }
    });
    let (entered, waiting) = std::sync::mpsc::channel();
    // Native dispatch must begin before the worker sends.
    let _timer = Timer::once(Duration::from_millis(1), move || entered.send(()).unwrap());
    let worker = std::thread::spawn(move || {
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        sender.send(()).unwrap();
    });
    owner.run().unwrap();
    worker.join().unwrap();
    assert!(delivered.get());
}

pub(super) fn stop_signal_defers_to_native_timer(signal: libc::c_int) {
    let calls = Rc::new(Cell::new(0));
    let delivered = calls.clone();
    let _guard = StopSignals::new(move || delivered.set(delivered.get() + 1)).unwrap();
    assert_eq!(unsafe { libc::raise(signal) }, 0);
    assert_eq!(
        calls.get(),
        0,
        "signal handler must not invoke application code"
    );
    drive_until(|| calls.get() == 1);
    assert_eq!(calls.get(), 1, "signal {signal} did not request stop");
}

pub(super) fn schedule_one_shot_delivers_once_and_drop_cancels_delivery() {
    let calls = Rc::new(Cell::new(0));
    let delivered = calls.clone();
    let _timer = Timer::once(Duration::from_millis(5), move || {
        delivered.set(delivered.get() + 1)
    });
    drive_until(|| calls.get() == 1);
    drive_for(Duration::from_millis(20));
    assert_eq!(calls.get(), 1);

    let delivered = calls.clone();
    let timer = Timer::once(Duration::from_millis(5), move || delivered.set(99));
    drop(timer);
    drive_for(Duration::from_millis(20));
    assert_eq!(calls.get(), 1);
}

pub(super) fn submit_burst_delivers_leading_and_latest_trailing_values() {
    let received = Rc::new(RefCell::new(Vec::new()));
    let delivered = received.clone();
    let throttle = Throttle::new(Duration::from_millis(100), move |value| {
        delivered.borrow_mut().push(value)
    });
    throttle.submit(1);
    throttle.submit(2);
    throttle.submit(3);
    assert_eq!(*received.borrow(), [1]);
    drive_until(|| received.borrow().len() == 2);
    assert_eq!(*received.borrow(), [1, 3]);
    throttle.submit(4);
    std::thread::sleep(Duration::from_millis(120));
    throttle.submit(5);
    assert_eq!(*received.borrow(), [1, 3, 5]);
    drive_for(Duration::from_millis(120));
    assert_eq!(*received.borrow(), [1, 3, 5]);
}

pub(super) fn cancel_repeating_timer_from_callback_stops_delivery_and_releases_captures() {
    let timer = Rc::new(RefCell::new(None::<Timer>));
    let cancel = Rc::downgrade(&timer);
    let calls = Rc::new(Cell::new(0));
    let delivered = calls.clone();
    let capture = Rc::new(());
    let released = Rc::downgrade(&capture);
    *timer.borrow_mut() = Some(Timer::repeating(Duration::from_millis(5), move || {
        let _capture = &capture;
        delivered.set(delivered.get() + 1);
        if delivered.get() == 3 {
            cancel.upgrade().unwrap().borrow_mut().take();
        }
    }));
    drive_until(|| calls.get() == 3);
    drive_for(Duration::from_millis(20));
    assert_eq!(calls.get(), 3);
    assert!(released.upgrade().is_none());
}

fn icon() -> objc2::rc::Retained<objc2_app_kit::NSImage> {
    crate::platform::macos::menu_bar::load_icon(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../resources/macos/status_bar_icon.png"
    )))
    .unwrap()
}

fn drive_until(mut done: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !done() {
        assert!(Instant::now() < deadline, "native timer did not fire");
        CFRunLoop::run_in_mode(unsafe { kCFRunLoopDefaultMode }, 0.005, false);
    }
}

fn drive_for(duration: Duration) {
    let deadline = Instant::now() + duration;
    drive_until(|| Instant::now() >= deadline);
}
