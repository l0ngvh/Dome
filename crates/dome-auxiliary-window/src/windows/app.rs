use std::hash::Hash;
use std::panic::{AssertUnwindSafe, catch_unwind, resume_unwind};
use std::rc::Rc;
use std::sync::mpsc;

use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, LoadImageW, MSG,
    TranslateMessage,
};
use windows::core::PCWSTR;

use super::run_loop::{LoopHandle, LoopState, NativeCallbacks, Registration, WakeTarget};
use super::tray::{Tray, TrayTooltip};
use super::window::OwnedHwnd;
use crate::delivery::{Callbacks, WakeDelivery};
use crate::{AppHandler, LoopWaker};

/// A tray icon loaded from a compiled resource. `LR_SHARED` lets Windows cache the handle,
/// so no `DestroyIcon` is owed and `Icon` needs no `Drop`.
pub(crate) struct Icon {
    hicon: HICON,
}

impl Icon {
    #[cfg(test)]
    pub(super) fn for_test() -> Self {
        use windows::Win32::UI::WindowsAndMessaging::{IDI_APPLICATION, LoadIconW};
        Self {
            hicon: unsafe { LoadIconW(None, IDI_APPLICATION) }.expect("load test icon"),
        }
    }

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
    #[cfg(test)]
    pub(super) fn for_test() -> Self {
        Self {
            tooltip: TrayTooltip::for_test(),
        }
    }
    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        self.tooltip.set(tooltip);
    }
}

pub(crate) struct App<W: Clone + Eq + Hash + Send + 'static> {
    waker: LoopWaker<W>,
    state: Rc<LoopState<W>>,
    tray: Option<Tray>,
    wake_window: Option<OwnedHwnd>,
    registration: Option<Registration>,
}

impl<W: Clone + Eq + Hash + Send + 'static> App<W> {
    pub(crate) fn new(icon: Icon, handler: Box<dyn AppHandler<Wake = W>>) -> anyhow::Result<Self> {
        let (sender, receiver) = mpsc::channel();
        let delivery = Rc::new(WakeDelivery::new(receiver, handler));
        let state = Rc::new(LoopState::new(delivery, sender.clone()));
        let registration = Registration::new(state.clone())?;
        let wake_window = OwnedHwnd::message_only()?;
        let tray = Tray::new(icon.hicon, state.clone())?;
        Ok(Self {
            waker: LoopWaker {
                sender,
                target: WakeTarget::new(wake_window.hwnd()),
            },
            state,
            tray: Some(tray),
            wake_window: Some(wake_window),
            registration: Some(registration),
        })
    }

    #[cfg(test)]
    pub(super) fn tray_window_for_test(&self) -> windows::Win32::Foundation::HWND {
        self.tray.as_ref().unwrap().window_for_test()
    }

    pub(crate) fn waker(&self) -> LoopWaker<W> {
        self.waker.clone()
    }

    pub(crate) fn handle(&self) -> LoopHandle<W> {
        LoopHandle::new(&self.state)
    }

    pub(crate) fn run(mut self) {
        let result = catch_unwind(AssertUnwindSafe(|| {
            if !self.state.should_stop() {
                let shell = crate::Shell::new(Shell {
                    tooltip: self.tray.as_ref().unwrap().tooltip(),
                });
                self.state.started(shell);
            }
            let mut message = MSG::default();
            while !self.state.should_stop() {
                let retrieved = unsafe { GetMessageW(&mut message, None, 0, 0) }.0;
                if retrieved == -1 {
                    panic!(
                        "GetMessageW failed: {}",
                        windows::core::Error::from_thread()
                    );
                }
                if self.state.should_stop() || retrieved == 0 {
                    break;
                }
                unsafe {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                if self.state.should_stop() {
                    break;
                }
            }
        }));
        if let Err(payload) = result {
            self.state.record_panic(payload);
        }
        self.state.finish();
        self.teardown();
        let payload = self.state.take_panic();
        drop(self);
        if let Some(payload) = payload {
            resume_unwind(payload);
        }
    }

    fn teardown(&mut self) {
        self.state.stop_wakes();
        drop(self.tray.take());
        drop(self.wake_window.take());
        drop(self.registration.take());
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> Drop for App<W> {
    fn drop(&mut self) {
        self.teardown();
    }
}
