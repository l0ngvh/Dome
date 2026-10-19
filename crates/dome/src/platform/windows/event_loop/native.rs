use std::any::Any;
use std::cell::RefCell;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};

use windows::Win32::Foundation::LRESULT;
use windows::Win32::UI::WindowsAndMessaging::{EndMenu, PostQuitMessage};

/// Holds a fatal callback panic until execution returns across the native dispatch
/// boundary. This does not defer reentrant callbacks.
#[derive(Default)]
pub(in crate::platform::windows) struct CallbackPanic {
    payload: RefCell<Option<Box<dyn Any + Send>>>,
}

impl CallbackPanic {
    pub(in crate::platform::windows) fn is_pending(&self) -> bool {
        self.payload.borrow().is_some()
    }

    pub(in crate::platform::windows) fn catch(
        &self,
        callback: impl FnOnce() -> LRESULT,
    ) -> LRESULT {
        if self.is_pending() {
            return LRESULT(0);
        }
        match catch_unwind(AssertUnwindSafe(callback)) {
            Ok(reply) => reply,
            Err(payload) => {
                if !self.is_pending() {
                    *self.payload.borrow_mut() = Some(payload);
                }
                unsafe {
                    // A nested menu must return before unwinding can resume in Rust.
                    let _ = EndMenu();
                    PostQuitMessage(0);
                }
                LRESULT(0)
            }
        }
    }

    pub(super) fn resume(&self) {
        let payload = self.payload.borrow_mut().take();
        if let Some(payload) = payload {
            resume_unwind(payload);
        }
    }
}
