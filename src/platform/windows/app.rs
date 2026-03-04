use std::collections::{HashMap, HashSet};

use calloop::channel::Sender;
use glutin::display::{Display, DisplayApiPreference};
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GW_HWNDPREV, GWLP_USERDATA, GetWindow,
    GetWindowLongPtrW, HWND_TOP, HWND_TOPMOST, PostMessageW, RegisterClassW, SWP_NOACTIVATE,
    SWP_NOMOVE, SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, WM_DISPLAYCHANGE, WM_PAINT, WM_QUIT,
    WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::PCWSTR;

use super::dome::{AppHandle, HubEvent, OverlayFrame, WM_APP_CONFIG, WM_APP_OVERLAY};
use super::get_all_screens;
use super::overlay::{
    CONTAINER_OVERLAY_CLASS, ContainerOverlay, WINDOW_OVERLAY_CLASS, WindowOverlay,
    container_wnd_proc, raw_window_handle,
};
use crate::config::Config;
use crate::core::{ContainerId, WindowId};

pub(super) struct App {
    hwnd: HWND,
    display: Display,
    hub_sender: Sender<HubEvent>,
    config: Config,
    window_overlays: HashMap<WindowId, WindowOverlay>,
    container_overlays: HashMap<ContainerId, Box<ContainerOverlay>>,
}

impl App {
    pub(super) fn new(
        hub_sender: Sender<HubEvent>,
        config: Config,
    ) -> windows::core::Result<Box<Self>> {
        let hinstance = unsafe { GetModuleHandleW(None)? };

        const APP_CLASS: PCWSTR = windows::core::w!("DomeApp");

        // Register app window class
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: APP_CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };

        // Register window overlay class
        let wc_window = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_overlay_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: WINDOW_OVERLAY_CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc_window) };

        // Register container overlay class
        let wc_container = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(container_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: CONTAINER_OVERLAY_CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc_container) };

        let hwnd = unsafe {
            CreateWindowExW(
                WS_EX_TOOLWINDOW,
                APP_CLASS,
                windows::core::w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(hinstance.into()),
                None,
            )?
        };

        let raw_display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
        let raw_window = raw_window_handle(hwnd);
        let display =
            unsafe { Display::new(raw_display, DisplayApiPreference::Wgl(Some(raw_window))) }
                .expect("failed to create GL display");

        let app = Box::new(Self {
            hwnd,
            display,
            hub_sender,
            config,
            window_overlays: HashMap::new(),
            container_overlays: HashMap::new(),
        });

        let ptr = &*app as *const _ as *mut App;
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, ptr as isize) };

        app.send_event(HubEvent::AppInitialized(AppHandle::new(hwnd)));

        Ok(app)
    }

    fn send_event(&self, event: HubEvent) {
        if self.hub_sender.send(event).is_err() {
            tracing::error!("Hub thread died, shutting down");
            unsafe { PostMessageW(Some(self.hwnd), WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
        }
    }

    fn handle_overlay_frame(&mut self, frame: OverlayFrame) {
        // Create new window overlays
        for create in frame.creates {
            match WindowOverlay::new(&self.display, create.hwnd, create.is_float) {
                Ok(overlay) => {
                    self.window_overlays.insert(create.window_id, overlay);
                }
                Err(e) => {
                    tracing::warn!(id = ?create.window_id, "Failed to create window overlay: {e:#}")
                }
            }
        }

        // Delete window overlays
        for id in frame.deletes {
            self.window_overlays.remove(&id);
        }

        let mut seen_windows: HashSet<WindowId> = HashSet::new();

        // Update window overlays
        for wp in &frame.windows {
            seen_windows.insert(wp.id);
            if let Some(overlay) = self.window_overlays.get_mut(&wp.id) {
                overlay.update(wp, &self.config);
                overlay.show();
            }
        }

        // Update container overlays
        let frame_container_ids: HashSet<_> =
            frame.containers.iter().map(|c| c.placement.id).collect();
        self.container_overlays
            .retain(|id, _| frame_container_ids.contains(id));

        for data in &frame.containers {
            let id = data.placement.id;
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.container_overlays.entry(id)
            {
                match ContainerOverlay::new(
                    &self.display,
                    self.config.clone(),
                    self.hub_sender.clone(),
                ) {
                    Ok(overlay) => {
                        entry.insert(overlay);
                    }
                    Err(e) => {
                        tracing::warn!(%id, "Failed to create container overlay: {e:#}");
                        continue;
                    }
                }
            }
            if let Some(overlay) = self.container_overlays.get_mut(&id) {
                overlay.update(data);
                overlay.show();
                unsafe {
                    SetWindowPos(
                        overlay.hwnd(),
                        Some(HWND_TOP),
                        0,
                        0,
                        0,
                        0,
                        SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                    )
                    .ok();
                }
            }
        }

        // Hide unseen window overlays (workspace switch)
        for (id, overlay) in &mut self.window_overlays {
            if !seen_windows.contains(id) {
                overlay.hide();
            }
        }

        // Bring focused window's border above its window
        if let Some(id) = frame.focused
            && let Some(overlay) = self.window_overlays.get(&id)
        {
            unsafe {
                let above = GetWindow(overlay.managed_hwnd, GW_HWNDPREV);
                match above {
                    Ok(hwnd) if hwnd != overlay.hwnd() => {
                        SetWindowPos(
                            overlay.hwnd(),
                            Some(hwnd),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        )
                        .ok();
                    }
                    _ => {
                        let top = if overlay.is_float {
                            Some(HWND_TOPMOST)
                        } else {
                            Some(HWND_TOP)
                        };
                        SetWindowPos(
                            overlay.hwnd(),
                            top,
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                        )
                        .ok();
                    }
                }
            }
        }
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_OVERLAY => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            let frame = unsafe { *Box::from_raw(wparam.0 as *mut OverlayFrame) };
            unsafe { (*ptr).handle_overlay_frame(frame) };
            LRESULT(0)
        }
        WM_APP_CONFIG => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            let config = unsafe { *Box::from_raw(wparam.0 as *mut Config) };
            unsafe {
                for overlay in (*ptr).container_overlays.values_mut() {
                    overlay.config = config.clone();
                }
                (*ptr).config = config;
            }
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            if !ptr.is_null() {
                match get_all_screens() {
                    Ok(screens) => unsafe { (*ptr).send_event(HubEvent::ScreensChanged(screens)) },
                    Err(e) => tracing::warn!("Failed to enumerate screens: {e}"),
                }
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn window_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}
