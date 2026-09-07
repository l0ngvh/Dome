use std::cell::RefCell;
use std::rc::Rc;

use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, LoadImageW, MSG,
    TranslateMessage,
};
use windows::core::PCWSTR;

use super::WM_APP_WAKE;
use super::run_loop::{LoopHandle, LoopWaker};
use super::tray::{SharedHandler, Tray, TrayTooltip};
use crate::AppHandler;

/// A tray icon loaded from a compiled resource. `LR_SHARED` lets Windows cache the handle,
/// so no `DestroyIcon` is owed and `Icon` needs no `Drop`.
pub(crate) struct Icon {
    hicon: HICON,
}

impl Icon {
    pub(crate) fn from_resource_id(id: u16) -> anyhow::Result<Self> {
        let hmodule = unsafe { GetModuleHandleW(None) }?;
        let instance = HINSTANCE(hmodule.0);
        let handle = unsafe {
            LoadImageW(
                Some(instance),
                PCWSTR(id as usize as *const u16),
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE | LR_SHARED,
            )
        }?;
        Ok(Self {
            hicon: HICON(handle.0),
        })
    }
}

/// A handle to the tray icon's tooltip. Holds no lifetime, so the public `Shell` needs
/// none. It stays valid while the owning `App` and its tray live.
pub(crate) struct Shell {
    tooltip: TrayTooltip,
}

impl Shell {
    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        self.tooltip.set(tooltip);
    }
}

pub(crate) struct App {
    thread_id: u32,
    handler: SharedHandler,
    tray: Tray,
}

impl App {
    pub(crate) fn new(icon: Icon, handler: Box<dyn AppHandler>) -> anyhow::Result<Self> {
        let handler: SharedHandler = Rc::new(RefCell::new(handler));
        let tray = Tray::new(icon.hicon, Rc::clone(&handler))?;
        Ok(Self {
            thread_id: unsafe { GetCurrentThreadId() },
            handler,
            tray,
        })
    }

    pub(crate) fn waker(&self) -> LoopWaker {
        LoopWaker::new(self.thread_id)
    }

    pub(crate) fn handle(&self) -> LoopHandle {
        LoopHandle::default()
    }

    pub(crate) fn run(self) {
        let shell = crate::Shell::new(Shell {
            tooltip: self.tray.tooltip(),
        });
        self.handler.borrow_mut().on_started(&shell);
        let mut msg = MSG::default();
        while unsafe { GetMessageW(&mut msg, None, 0, 0) }.into() {
            if msg.message == WM_APP_WAKE {
                self.handler.borrow_mut().on_wake(&shell);
            } else {
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
        self.handler.borrow_mut().on_stopping();
    }
}
