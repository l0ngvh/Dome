use std::collections::{HashMap, HashSet};

use calloop::channel::Sender;
use glutin::display::{Display, DisplayApiPreference};
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetForegroundWindow,
    GetWindowLongPtrW, HWND_TOP, PostMessageW, RegisterClassW, SWP_NOACTIVATE, SWP_NOMOVE,
    SWP_NOSIZE, SetWindowLongPtrW, SetWindowPos, WM_DISPLAYCHANGE, WM_PAINT, WM_QUIT, WNDCLASSW,
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
use super::window::{
    ManagedHwnd, ManagedWindow, Registry, WINDOW_OVERLAY_CLASS, WindowMode,
    is_d3d_exclusive_fullscreen_active,
};
use crate::config::Config;
use crate::core::{ContainerId, MonitorId, MonitorLayout, WindowId};

/// If a monitor has an exclusive fullscreen windows, ignore all incoming stale events from Dome
enum MonitorState {
    Normal {
        ids: Vec<WindowId>,
        focused: Option<WindowId>,
    },
    Exclusive(WindowId),
}

impl MonitorState {
    fn window_ids(&self) -> &[WindowId] {
        match self {
            MonitorState::Normal { ids, .. } => ids,
            MonitorState::Exclusive(id) => std::slice::from_ref(id),
        }
    }
}

pub(super) struct App {
    hwnd: HWND,
    display: Display,
    hub_sender: Sender<HubEvent>,
    config: Config,
    registry: Registry,
    taskbar: Taskbar,
    last_focused: Option<WindowId>,
    container_overlays: HashMap<ContainerId, Box<ContainerOverlay>>,
    monitor_state: HashMap<MonitorId, MonitorState>,
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
            last_focused: None,
            container_overlays: HashMap::new(),
            monitor_state: HashMap::new(),
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
            for state in self.monitor_state.values_mut() {
                if matches!(state, MonitorState::Exclusive(eid) if *eid == *id) {
                    *state = MonitorState::Normal {
                        ids: Vec::new(),
                        focused: None,
                    };
                }
            }
            self.registry.remove(*id);
        }

        // Derive old displayed set from monitor_state before rebuilding.
        // Global diff (not per-monitor) avoids hiding windows that moved between monitors,
        // since hide() uses SWP_ASYNCWINDOWPOS and could race with the show() on the new monitor.
        let old_displayed: HashSet<WindowId> = self
            .monitor_state
            .values()
            .flat_map(|s| s.window_ids())
            .copied()
            .collect();

        let mut new_displayed = HashSet::new();
        let mut new_monitor_state: HashMap<MonitorId, MonitorState> = HashMap::new();
        let border = self.config.border_size;

        for fm in &frame.monitors {
            let layout_ids: Vec<WindowId> = match &fm.layout {
                MonitorLayout::Fullscreen(id) => vec![*id],
                MonitorLayout::Normal { windows, .. } => windows.iter().map(|wp| wp.id).collect(),
            };
            new_displayed.extend(&layout_ids);

            let new_state = match self.monitor_state.remove(&fm.monitor_id) {
                Some(MonitorState::Exclusive(id)) => MonitorState::Exclusive(id),
                _ => {
                    match &fm.layout {
                        MonitorLayout::Fullscreen(window_id) => {
                            if let Some(mw) = self.registry.get_mut(*window_id) {
                                mw.set_fullscreen(&fm.dimension);
                            }
                        }
                        MonitorLayout::Normal { windows, .. } => {
                            for wp in windows {
                                if let Some(mw) = self.registry.get_mut(wp.id) {
                                    mw.show(wp, border, &self.config, frame.focused == Some(wp.id));
                                }
                            }
                        }
                    }
                    MonitorState::Normal {
                        ids: layout_ids,
                        focused: frame.focused.filter(|id| new_displayed.contains(id)),
                    }
                }
            };
            new_monitor_state.insert(fm.monitor_id, new_state);
        }
        self.monitor_state = new_monitor_state;

        for &id in new_displayed.difference(&old_displayed) {
            if let Some(mw) = self.registry.get(id) {
                self.taskbar.add_tab(mw.hwnd()).ok();
            }
        }
        for &id in old_displayed.difference(&new_displayed) {
            if let Some(mw) = self.registry.get_mut(id) {
                mw.hide();
                self.taskbar.delete_tab(mw.hwnd()).ok();
            }
        }

        // Turns out exclusive
        let has_exclusive = self
            .monitor_state
            .values()
            .any(|s| matches!(s, MonitorState::Exclusive(_)));
        if !has_exclusive {
            let focused = self.monitor_state.values().find_map(|s| match s {
                MonitorState::Normal { focused, .. } => *focused,
                MonitorState::Exclusive(_) => None,
            });
            if focused != self.last_focused {
                self.last_focused = focused;
                if let Some(id) = focused {
                    if let Some(mw) = self.registry.get(id) {
                        mw.focus();
                    }
                }
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
                let app = unsafe { &mut *ptr };
                match get_all_screens() {
                    Ok(screens) => app.send_event(HubEvent::ScreensChanged(screens)),
                    Err(e) => tracing::warn!("Failed to enumerate screens: {e}"),
                }

                // If there is an active D3D context, immediately set the active window as
                // exclusive fullscreen. We won't clear the exclusive fullscreen flag when D3D
                // context got cleared due to 3 reasons:
                // - Exclusive fullscreen is fragile, and can suddenly exit if:
                //    - Another window got focus
                //    - Fullscreen window got partially obscured by another window
                //   All of which dome can accidentally cause. Dome tries its best to pause
                //   everything when a window go fullscreen, so this shouldn't happen often, but
                //   it might.
                // - Some apps are really aggressive in trying to take fullscreen status back,
                //   which can cause an infinite loop of dome accidentally takes control causing
                //   fullscreen to ext, and app tries to take back fullscreen status.
                // - Usually, game entering exclusive fullscreen only gives up exclusive
                //   fullscreen on exit. We may have to say sorry to users toggling the in-app
                //   fullscreen setting and tell them to relaunch the app as borderless.
                if is_d3d_exclusive_fullscreen_active() {
                    let fg = ManagedHwnd::new(unsafe { GetForegroundWindow() });
                    if let Some(id) = app.registry.get_id(fg) {
                        tracing::info!(%id, "D3D exclusive fullscreen entered");
                        if let Some(mw) = app.registry.get_mut(id) {
                            mw.set_mode(WindowMode::FullscreenExclusive);
                        }
                        if let Some((&monitor_id, _)) =
                            app.monitor_state.iter().find(|(_, state)| match state {
                                MonitorState::Normal { ids, .. } => ids.contains(&id),
                                MonitorState::Exclusive(eid) => *eid == id,
                            })
                        {
                            app.monitor_state
                                .insert(monitor_id, MonitorState::Exclusive(id));
                        }
                        app.send_event(HubEvent::SetFullscreen(id));
                    }
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
