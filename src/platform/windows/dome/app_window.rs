use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    ChangeWindowMessageFilterEx, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW, MSGFLT_ALLOW,
    PostThreadMessageW, RegisterWindowMessageW, SPI_SETWORKAREA, SetWindowLongPtrW, WM_DESTROY,
    WM_DISPLAYCHANGE, WM_SETTINGCHANGE,
};
use windows::core::{PCWSTR, w};

use crate::core::WorkspaceInfo;
use crate::platform::windows::dome::overlay::OwnedHwnd;
use crate::platform::windows::dome::tray::{TRAY_CALLBACK_MSG, TrayIndicator};
use crate::platform::windows::{HubSender, WM_APP_DISPLAY_CHANGE, WM_APP_WORKAREA_CHANGE};

pub(in crate::platform::windows) const APP_WINDOW_CLASS: PCWSTR = w!("DomeAppWindow");

pub(in crate::platform::windows) trait AppWindowApi {
    fn update_tray(&self, workspaces: &[WorkspaceInfo]);
}

pub(in crate::platform::windows) struct AppWindow {
    // Tray field precedes hwnd so NIM_DELETE runs while the callback HWND is still alive.
    // Option only bridges construction. After new() returns it is Some for the object's life.
    tray: Option<Box<TrayIndicator>>,
    hwnd: OwnedHwnd,
    taskbar_created_msg: u32,
}

impl AppWindow {
    pub(in crate::platform::windows) fn new(
        instance: HINSTANCE,
        hub_sender: HubSender,
    ) -> anyhow::Result<Box<Self>> {
        let hwnd = OwnedHwnd::new_hidden_top_level(APP_WINDOW_CLASS, instance)?;

        let mut app = Box::new(Self {
            tray: None,
            hwnd,
            taskbar_created_msg: 0,
        });

        // Install GWLP_USERDATA before TrayIndicator::new so shell callbacks after NIM_ADD see an initialized AppWindow.
        unsafe {
            SetWindowLongPtrW(
                app.hwnd.hwnd(),
                GWLP_USERDATA,
                app.as_ref() as *const AppWindow as isize,
            );
        }

        let msg = unsafe { RegisterWindowMessageW(w!("TaskbarCreated")) };
        if msg == 0 {
            tracing::warn!(
                "RegisterWindowMessageW(TaskbarCreated) returned 0, tray will not survive explorer restart"
            );
        } else {
            app.taskbar_created_msg = msg;
            if let Err(e) =
                unsafe { ChangeWindowMessageFilterEx(app.hwnd.hwnd(), msg, MSGFLT_ALLOW, None) }
            {
                tracing::warn!(?e, "ChangeWindowMessageFilterEx(TaskbarCreated) failed");
            }
        }

        let tray = TrayIndicator::new(hub_sender, app.hwnd.hwnd())?;
        app.tray = Some(tray);
        Ok(app)
    }
}

impl AppWindowApi for AppWindow {
    fn update_tray(&self, workspaces: &[WorkspaceInfo]) {
        if let Some(tray) = self.tray.as_ref() {
            tray.update(workspaces);
        }
    }
}

pub(in crate::platform::windows) unsafe extern "system" fn app_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(lr) = crate::platform::windows::dome_wnd_proc_common(hwnd, msg, wparam, lparam) {
        return lr;
    }

    if msg == WM_SETTINGCHANGE && wparam.0 == SPI_SETWORKAREA.0 as usize {
        unsafe {
            PostThreadMessageW(
                GetCurrentThreadId(),
                WM_APP_WORKAREA_CHANGE,
                WPARAM(0),
                LPARAM(0),
            )
            .ok()
        };
        return LRESULT(0);
    }
    if msg == WM_DISPLAYCHANGE {
        unsafe {
            PostThreadMessageW(
                GetCurrentThreadId(),
                WM_APP_DISPLAY_CHANGE,
                WPARAM(0),
                LPARAM(0),
            )
            .ok()
        };
        return LRESULT(0);
    }

    if let Some(app) = unsafe { app_from_hwnd(hwnd) } {
        // AppWindow::new installs GWLP_USERDATA and finishes tray construction
        // before returning, so any wnd-proc callback observed here has tray = Some.
        let tray = app
            .tray
            .as_ref()
            .expect("tray populated for wnd proc's live lifetime");
        if msg == TRAY_CALLBACK_MSG {
            tray.show_menu(hwnd);
            return LRESULT(0);
        }
        if app.taskbar_created_msg != 0 && msg == app.taskbar_created_msg {
            if let Err(e) = tray.add_icon() {
                tracing::warn!(?e, "failed to re-add tray icon after taskbar restart");
            }
            return LRESULT(0);
        }
    }

    if msg == WM_DESTROY {
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
    }

    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

unsafe fn app_from_hwnd<'a>(hwnd: HWND) -> Option<&'a AppWindow> {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) };
    if ptr == 0 {
        None
    } else {
        Some(unsafe { &*(ptr as *const AppWindow) })
    }
}
