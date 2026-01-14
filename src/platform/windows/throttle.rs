use std::cell::UnsafeCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::marker::PhantomData;
use std::pin::Pin;
use std::time::{Duration, Instant};

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{KillTimer, SetTimer};

thread_local! {
    static TIMER_MAP: UnsafeCell<HashMap<usize, *mut c_void>> = UnsafeCell::new(HashMap::new());
}

pub(super) struct Throttle<T: 'static> {
    interval: Duration,
    last_sent: Option<Instant>,
    pending: Option<T>,
    timer_id: Option<usize>,
    callback: Box<dyn Fn(T)>,
    _bound_to_thread: PhantomData<*const ()>,
}

impl<T: 'static> Throttle<T> {
    pub(super) fn new(interval: Duration) -> Pin<Box<Self>> {
        Box::pin(Self {
            interval,
            last_sent: None,
            pending: None,
            timer_id: None,
            callback: Box::new(|_| {}),
            _bound_to_thread: PhantomData,
        })
    }

    pub(super) fn set_callback(self: &mut Pin<Box<Self>>, callback: impl Fn(T) + 'static) {
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        this.callback = Box::new(callback);
    }

    pub(super) fn submit(self: &mut Pin<Box<Self>>, value: T) {
        let now = Instant::now();
        let this = unsafe { self.as_mut().get_unchecked_mut() };
        let can_send = this
            .last_sent
            .map(|last| now.duration_since(last) >= this.interval)
            .unwrap_or(true);

        if can_send {
            if let Some(id) = this.timer_id.take() {
                TIMER_MAP.with(|m| unsafe { (*m.get()).remove(&id) });
                if let Err(e) = unsafe { KillTimer(None, id) } {
                    tracing::warn!("KillTimer failed: {e}");
                }
            }
            this.last_sent = Some(now);
            this.pending = None;
            (this.callback)(value);
        } else {
            this.pending = Some(value);
            if this.timer_id.is_none() {
                let delay = this
                    .interval
                    .saturating_sub(this.last_sent.unwrap().elapsed());
                self.as_mut().schedule_timer(delay);
            }
        }
    }

    fn schedule_timer(self: Pin<&mut Self>, delay: Duration) {
        let this = unsafe { self.get_unchecked_mut() };
        let ptr = this as *mut Self as *mut c_void;
        let id = unsafe { SetTimer(None, 0, delay.as_millis() as u32, Some(timer_callback::<T>)) };
        TIMER_MAP.with(|m| unsafe { (*m.get()).insert(id, ptr) });
        this.timer_id = Some(id);
    }

    fn on_timer(&mut self) {
        self.timer_id = None;
        if let Some(value) = self.pending.take() {
            self.last_sent = Some(Instant::now());
            (self.callback)(value);
        }
    }
}

impl<T> Drop for Throttle<T> {
    fn drop(&mut self) {
        if let Some(id) = self.timer_id.take() {
            TIMER_MAP.with(|m| unsafe { (*m.get()).remove(&id) });
            if let Err(e) = unsafe { KillTimer(None, id) } {
                tracing::warn!("KillTimer failed: {e}");
            }
        }
    }
}

unsafe extern "system" fn timer_callback<T: 'static>(
    _hwnd: HWND,
    _msg: u32,
    id: usize,
    _time: u32,
) {
    let ptr = TIMER_MAP.with(|m| unsafe { (*m.get()).remove(&id) });
    if let Some(ptr) = ptr {
        if let Err(e) = unsafe { KillTimer(None, id) } {
            tracing::warn!("KillTimer failed: {e}");
        }
        let throttle = unsafe { &mut *(ptr as *mut Throttle<T>) };
        throttle.on_timer();
    }
}
