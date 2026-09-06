use std::marker::PhantomData;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostQuitMessage, PostThreadMessageW};

use super::WM_APP_WAKE;

/// Wakes the loop from any thread. Carries no payload, so the consumer owns its queue.
#[derive(Clone, Copy)]
pub struct LoopWaker {
    thread_id: u32,
}

impl LoopWaker {
    pub(crate) fn new(thread_id: u32) -> Self {
        Self { thread_id }
    }

    pub fn wake(&self) {
        unsafe { PostThreadMessageW(self.thread_id, WM_APP_WAKE, WPARAM(0), LPARAM(0)).ok() };
    }
}

/// `PhantomData<*const ()>` keeps it off any other thread, so `terminate` cannot post to
/// a foreign queue.
#[derive(Clone, Copy, Default)]
pub struct LoopHandle {
    _not_send: PhantomData<*const ()>,
}

impl LoopHandle {
    pub fn terminate(&self) {
        unsafe { PostQuitMessage(0) };
    }
}
