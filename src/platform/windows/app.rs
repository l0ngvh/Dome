use std::collections::{HashMap, HashSet};

use calloop::channel::Sender;
use glutin::display::{Display, DisplayApiPreference};
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetWindowLongPtrW,
    HWND_TOP, PostMessageW, RegisterClassW, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SetWindowLongPtrW, SetWindowPos, WM_DISPLAYCHANGE, WM_PAINT, WM_QUIT, WNDCLASSW,
    WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::PCWSTR;

use super::dome::{
    AppHandle, ContainerOverlayData, HubEvent, LayoutFrame, TitleUpdate, WM_APP_CONFIG,
    WM_APP_LAYOUT, WM_APP_TITLE,
};
use super::get_all_screens;
use super::overlay::{
    CONTAINER_OVERLAY_CLASS, ContainerOverlay, container_wnd_proc, raw_window_handle,
};
use super::taskbar::Taskbar;
use super::window::{ManagedHwnd, ManagedWindow, Registry, WINDOW_OVERLAY_CLASS};
use crate::config::Config;
use crate::core::{ContainerId, MonitorLayout, WindowId};

pub(super) struct App {
    hwnd: HWND,
    display: Display,
    hub_sender: Sender<HubEvent>,
    config: Config,
    registry: Registry,
    taskbar: Taskbar,
    displayed_windows: HashSet<ManagedHwnd>,
    last_focused: Option<WindowId>,
    container_overlays: HashMap<ContainerId, Box<ContainerOverlay>>,
}

impl App {
    pub(super) fn new(
        hub_sender: Sender<HubEvent>,
        config: Config,
    ) -> windows::core::Result<Box<Self>> {
        let hinstance = unsafe { GetModuleHandleW(None)? };

        const APP_CLASS: PCWSTR = windows::core::w!("DomeApp");

        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: APP_CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc) };

        let wc_window = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(window_overlay_wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: WINDOW_OVERLAY_CLASS,
            ..Default::default()
        };
        unsafe { RegisterClassW(&wc_window) };

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

        let taskbar = Taskbar::new()?;

        let app = Box::new(Self {
            hwnd,
            display,
            hub_sender,
            config,
            registry: Registry::new(),
            taskbar,
            displayed_windows: HashSet::new(),
            last_focused: None,
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

    fn apply_layout_frame(&mut self, frame: LayoutFrame) {
        for create in &frame.creates {
            let mw = ManagedWindow::new(
                &self.display,
                create.hwnd,
                create.title.clone(),
                create.process.clone(),
                create.mode,
            );
            self.registry.insert(mw, create.id);
        }

        for id in &frame.deletes {
            self.registry.remove(*id);
        }

        let mut new_displayed = HashSet::new();
        let mut seen_windows = HashSet::new();
        let border = self.config.border_size;

        for fm in &frame.monitors {
            match &fm.layout {
                MonitorLayout::Fullscreen(window_id) => {
                    if let Some(mw) = self.registry.get_mut(*window_id) {
                        new_displayed.insert(mw.managed_hwnd());
                        mw.set_fullscreen(&fm.dimension);
                    }
                }
                MonitorLayout::Normal { windows, .. } => {
                    for wp in windows {
                        if let Some(mw) = self.registry.get_mut(wp.id) {
                            new_displayed.insert(mw.managed_hwnd());
                            mw.show(wp, border, &self.config, frame.focused == Some(wp.id));
                            seen_windows.insert(wp.id);
                        }
                    }
                }
            }
        }

        for hwnd in new_displayed.difference(&self.displayed_windows) {
            if let Some(mw) = self.registry.get_by_hwnd(*hwnd) {
                self.taskbar.add_tab(mw.hwnd()).ok();
            }
        }
        for hwnd in self.displayed_windows.difference(&new_displayed) {
            if let Some(mw) = self.registry.get_by_hwnd_mut(*hwnd) {
                mw.hide();
                self.taskbar.delete_tab(mw.hwnd()).ok();
            }
        }
        self.displayed_windows = new_displayed;

        if frame.focused != self.last_focused {
            self.last_focused = frame.focused;
            if let Some(id) = frame.focused {
                if let Some(mw) = self.registry.get(id) {
                    mw.focus();
                }
            }
        }

        for (id, mw) in self.registry.iter_mut() {
            if !seen_windows.contains(&id) {
                mw.hide_overlay();
            }
        }

        self.update_container_overlays(&frame.container_overlays);
    }

    fn apply_title_update(&mut self, update: TitleUpdate) {
        for (hwnd, title) in &update.titles {
            self.registry.set_title(*hwnd, title.clone());
        }

        for data in &update.container_overlays {
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&data.placement.id) {
                overlay.update(data.placement, titles);
            }
        }
    }

    fn update_container_overlays(&mut self, container_data: &[ContainerOverlayData]) {
        let frame_container_ids: HashSet<_> =
            container_data.iter().map(|c| c.placement.id).collect();
        self.container_overlays
            .retain(|id, _| frame_container_ids.contains(id));

        for data in container_data {
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
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&id) {
                overlay.update(data.placement, titles);
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
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_LAYOUT => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            let frame = unsafe { *Box::from_raw(wparam.0 as *mut LayoutFrame) };
            unsafe { (*ptr).apply_layout_frame(frame) };
            LRESULT(0)
        }
        WM_APP_TITLE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut App;
            let update = unsafe { *Box::from_raw(wparam.0 as *mut TitleUpdate) };
            unsafe { (*ptr).apply_title_update(update) };
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
