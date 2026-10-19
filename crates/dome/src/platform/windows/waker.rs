use std::rc::Rc;
use std::sync::{Arc, Mutex, Once};

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, PostMessageW, SetWindowLongPtrW, WM_TIMER,
};

use super::{WAKE_CLASS_NAME, WM_APP_WAKE};
use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::event_loop::{EventSender, RunLoop};
use crate::platform::windows::ui::window::native::{OwnedHwnd, register_class};

/// Wakes the loop from any thread. The lock serializes posting with window teardown,
/// so a retained waker cannot post to a recycled HWND after the app drops.
#[derive(Clone)]
pub struct LoopWaker {
    window: Arc<Mutex<Option<usize>>>,
}

impl LoopWaker {
    pub fn wake(&self) {
        let window = self.window.lock().unwrap();
        if let Some(handle) = *window {
            unsafe {
                PostMessageW(
                    Some(HWND(handle as *mut _)),
                    WM_APP_WAKE,
                    WPARAM(0),
                    LPARAM(0),
                )
                .ok();
            }
        }
    }
}

pub(super) struct WakeWindow {
    pub(super) state: Box<WakeState>,
    window: OwnedHwnd,
    waker: LoopWaker,
}

pub(super) struct WakeState {
    timers: EventSender<usize>,
    pub(super) panic: Rc<CallbackPanic>,
}

impl WakeWindow {
    pub(super) fn new(
        timers: EventSender<usize>,
        panic: Rc<CallbackPanic>,
    ) -> anyhow::Result<(Self, LoopWaker)> {
        static REGISTER: Once = Once::new();
        REGISTER.call_once(|| register_class(WAKE_CLASS_NAME, Some(wake_wnd_proc)));
        let window = OwnedHwnd::message_only(WAKE_CLASS_NAME)?;
        let state = Box::new(WakeState { timers, panic });
        unsafe {
            SetWindowLongPtrW(
                window.hwnd(),
                GWLP_USERDATA,
                &*state as *const WakeState as isize,
            )
        };
        let waker = LoopWaker {
            window: Arc::new(Mutex::new(Some(window.hwnd().0 as usize))),
        };
        Ok((
            Self {
                window,
                state,
                waker: waker.clone(),
            },
            waker,
        ))
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }
}

impl Drop for WakeWindow {
    fn drop(&mut self) {
        *self.waker.window.lock().unwrap() = None;
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
    }
}

impl LoopWaker {
    pub fn task_waker(&self) -> std::task::Waker {
        struct NativeWake(std::sync::Mutex<LoopWaker>);

        impl std::task::Wake for NativeWake {
            fn wake(self: std::sync::Arc<Self>) {
                self.wake_by_ref();
            }

            fn wake_by_ref(self: &std::sync::Arc<Self>) {
                self.0.lock().unwrap().wake();
            }
        }

        std::task::Waker::from(std::sync::Arc::new(NativeWake(std::sync::Mutex::new(
            self.clone(),
        ))))
    }
}

unsafe extern "system" fn wake_wnd_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let state = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *const WakeState;
    if matches!(message, WM_APP_WAKE | WM_TIMER) && !state.is_null() {
        let state = unsafe { &*state };
        return state.panic.catch(|| {
            if message == WM_TIMER {
                let _ = state.timers.send(wparam.0);
            }
            let Some(app) = RunLoop::try_current() else {
                return LRESULT(0);
            };
            app.wake_received();
            LRESULT(0)
        });
    }
    unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
}
