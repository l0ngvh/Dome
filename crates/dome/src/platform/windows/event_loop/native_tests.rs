use super::*;
use std::cell::Cell;
use std::time::Duration;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

pub(super) fn worker_event_reaches_the_running_native_pump() {
    let (owner, _, mut timers) = RunLoop::new(icon(), Rc::new(|_| {})).unwrap();
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
    let mut entered = Some(entered);
    let window = owner.timer_window();
    // Native dispatch must begin before the worker sends.
    let timer = unsafe { SetTimer(Some(window), 1, 1, None) };
    assert_ne!(timer, 0);
    owner.spawn(async move {
        while let Some(id) = timers.next().await {
            if id == timer {
                unsafe { KillTimer(Some(window), timer).unwrap() };
                entered.take().unwrap().send(()).unwrap();
            }
        }
    });
    let worker = std::thread::spawn(move || {
        waiting.recv_timeout(Duration::from_secs(5)).unwrap();
        sender.send(()).unwrap();
    });
    owner.run().unwrap();
    worker.join().unwrap();
    assert!(delivered.get());
}

fn icon() -> windows::Win32::UI::WindowsAndMessaging::HICON {
    use windows::Win32::UI::WindowsAndMessaging::{IDI_APPLICATION, LoadIconW};
    unsafe { LoadIconW(None, IDI_APPLICATION) }.unwrap()
}
