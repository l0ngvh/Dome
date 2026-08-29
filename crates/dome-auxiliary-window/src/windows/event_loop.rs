use std::marker::PhantomData;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostQuitMessage, PostThreadMessageW, TranslateMessage,
};

use super::WM_APP_WAKE;
use crate::AuxiliaryLoopHandler;

pub struct EventLoop {
    thread_id: u32,
    handler: Box<dyn AuxiliaryLoopHandler>,
}

impl EventLoop {
    pub fn new(handler: Box<dyn AuxiliaryLoopHandler>) -> Self {
        Self {
            thread_id: unsafe { GetCurrentThreadId() },
            handler,
        }
    }

    pub fn waker(&self) -> LoopWaker {
        LoopWaker {
            thread_id: self.thread_id,
        }
    }

    pub fn handle(&self) -> LoopHandle {
        LoopHandle {
            _not_send: PhantomData,
        }
    }

    pub fn run(mut self) {
        self.handler.on_started();
        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.into() {
            if msg.message == WM_APP_WAKE {
                self.handler.on_wake();
            } else {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
        self.handler.on_stopping();
    }
}

/// `PhantomData<*const ()>` keeps it off any other thread, so `terminate` cannot post to
/// a foreign queue.
#[derive(Clone, Copy)]
pub struct LoopHandle {
    _not_send: PhantomData<*const ()>,
}

impl LoopHandle {
    pub fn terminate(&self) {
        unsafe { PostQuitMessage(0) };
    }
}

/// Wakes the loop from any thread. Carries no payload, so the consumer owns its queue.
#[derive(Clone, Copy)]
pub struct LoopWaker {
    thread_id: u32,
}

impl LoopWaker {
    pub fn wake(&self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_APP_WAKE, WPARAM(0), LPARAM(0)).ok() };
    }
}
