mod overlay;
mod recovery;
mod taskbar;
mod window;

use std::collections::{HashMap, HashSet};

use calloop::channel::Sender;
use glutin::display::{Display, DisplayApiPreference};
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, GWLP_USERDATA, GetForegroundWindow,
    GetWindowLongPtrW, HWND_NOTOPMOST, HWND_TOPMOST, PostMessageW, RegisterClassW,
    SetWindowLongPtrW, WM_DISPLAYCHANGE, WM_PAINT, WM_QUIT, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::PCWSTR;

use self::overlay::{
    CONTAINER_OVERLAY_CLASS, ContainerOverlay, container_wnd_proc, raw_window_handle,
};
use self::taskbar::Taskbar;
use self::window::{
    ManagedWindow, Registry, WINDOW_OVERLAY_CLASS, is_d3d_exclusive_fullscreen_active,
};
use super::dome::{
    AppHandle, ContainerRender, HubEvent, LayoutFrame, TitleUpdate, WM_APP_CONFIG, WM_APP_LAYOUT,
    WM_APP_TITLE, WindowShow,
};
use super::get_all_screens;
use super::handle::{ManagedHwnd, WindowMode, get_dimension};
use crate::config::Config;
use crate::core::{ContainerId, WindowId};

pub(super) struct Wm {
    hwnd: HWND,
    display: Display,
    hub_sender: Sender<HubEvent>,
    config: Config,
    registry: Registry,
    taskbar: Taskbar,
    last_focused: Option<WindowId>,
    container_overlays: HashMap<ContainerId, Box<ContainerOverlay>>,
    exclusive_fullscreen: HashSet<WindowId>,
}

impl Wm {
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
        recovery::install_handlers();

        let app = Box::new(Self {
            hwnd,
            display,
            hub_sender,
            config,
            registry: Registry::new(),
            taskbar,
            last_focused: None,
            container_overlays: HashMap::new(),
            exclusive_fullscreen: HashSet::new(),
        });

        let ptr = &*app as *const _ as *mut Wm;
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
        // --- Lifecycle ---
        for create in &frame.created_windows {
            let dim = get_dimension(create.hwnd);
            recovery::track(create.hwnd, dim);
            let mut mw = ManagedWindow::new(
                &self.display,
                create.hwnd,
                create.title.clone(),
                create.process.clone(),
                create.mode,
            );
            // Hide before first frame — window may end up offscreen due to
            // viewport scrolling. apply_layout will show the visible ones.
            // Fullscreen windows are always inside the viewport, and hiding them
            // interferes with D3D exclusive fullscreen transitions.
            if !mw.mode().is_fullscreen() {
                mw.hide();
            }
            self.registry.insert(mw, create.id);
        }

        for &id in &frame.deleted_windows {
            self.exclusive_fullscreen.remove(&id);
            if let Some(mw) = self.registry.get(id) {
                self.taskbar.delete_tab(mw.hwnd()).ok();
                recovery::untrack(mw.managed_hwnd());
            }
            self.registry.remove(id);
        }

        for &id in &frame.created_containers {
            match ContainerOverlay::new(&self.display, self.config.clone(), self.hub_sender.clone())
            {
                Ok(overlay) => {
                    self.container_overlays.insert(id, overlay);
                }
                Err(e) => {
                    tracing::warn!(%id, "Failed to create container overlay: {e:#}");
                }
            }
        }

        for &id in &frame.deleted_containers {
            self.container_overlays.remove(&id);
        }

        // --- Hide phase ---
        for &id in &frame.to_hide {
            if self.exclusive_fullscreen.contains(&id) {
                continue;
            }
            if let Some(mw) = self.registry.get_mut(id) {
                mw.hide();
                self.taskbar.delete_tab(mw.hwnd()).ok();
            }
        }

        for &id in &frame.containers_to_hide {
            if let Some(overlay) = self.container_overlays.get_mut(&id) {
                overlay.hide();
            }
        }

        // --- Position phase ---
        self.position_windows(&frame);

        // --- Taskbar ---
        for &id in &frame.tabs_to_add {
            if let Some(mw) = self.registry.get(id) {
                self.taskbar.add_tab(mw.hwnd()).ok();
            }
        }

        // --- Focus ---
        if !self.exclusive_fullscreen.is_empty() {
            return;
        }
        if frame.focused != self.last_focused {
            self.last_focused = frame.focused;
            if let Some(id) = frame.focused {
                if let Some(mw) = self.registry.get(id) {
                    mw.focus();
                }
            }
        }
    }

    fn apply_title_update(&mut self, update: TitleUpdate) {
        for (hwnd, title) in &update.titles {
            self.registry.set_title(*hwnd, title.clone());
        }

        for data in &update.container_renders {
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&data.placement.id) {
                overlay.update(data.placement, titles, None);
            }
        }
    }

    /// Position all visible windows and container overlays.
    ///
    /// Fullscreen windows are positioned directly (outside the z-order bands).
    /// Everything else is placed in two z-bands:
    ///
    /// ```text
    /// ── topmost band ──────────────────
    ///   newly active floats (HWND_TOPMOST) — newly converted or newly focused
    ///   steady floats (chained by specific HWND, reverse id)
    /// ── non-topmost band ──────────────
    ///   focused container overlay OR focused tiling (HWND_NOTOPMOST)
    ///   steady container overlays (chained, reverse id)
    ///   steady tiling (chained, reverse id)
    /// ```
    ///
    /// HWND_TOPMOST only on transitions. HWND_NOTOPMOST on the first non-topmost
    /// item (documented no-op if already non-topmost). Steady-state uses specific-HWND
    /// chaining — true no-op when already in position.
    fn position_windows(&mut self, frame: &LayoutFrame) {
        let border = self.config.border_size;
        let focus_changed = frame.focused != self.last_focused;

        // Separate fullscreen from normal windows
        let mut normal_windows: Vec<&WindowShow> = Vec::new();
        for ws in &frame.to_show {
            if self.exclusive_fullscreen.contains(&ws.id) {
                continue;
            }
            if let Some(dim) = &ws.fullscreen_dim {
                if let Some(mw) = self.registry.get_mut(ws.id) {
                    mw.set_fullscreen(dim);
                }
            } else {
                normal_windows.push(ws);
            }
        }

        // Classify into z-order groups
        let mut newly_active_float: Vec<&WindowShow> = Vec::new();
        let mut steady_float: Vec<&WindowShow> = Vec::new();
        let mut focused_tiling: Option<&WindowShow> = None;
        let mut steady_tiling: Vec<&WindowShow> = Vec::new();

        for ws in &normal_windows {
            let float_changed = self
                .registry
                .get(ws.id)
                .map(|mw| (mw.mode() == WindowMode::Float) != ws.is_float)
                .unwrap_or(false);
            let is_newly_focused_float =
                ws.is_float && focus_changed && frame.focused == Some(ws.id);

            if ws.is_float {
                if float_changed || is_newly_focused_float {
                    newly_active_float.push(ws);
                } else {
                    steady_float.push(ws);
                }
            } else if frame.focused == Some(ws.id) {
                focused_tiling = Some(ws);
            } else {
                steady_tiling.push(ws);
            }
        }

        // Reverse-id sort (newest/highest first). Move focused to front of newly_active.
        newly_active_float.sort_by(|a, b| b.id.cmp(&a.id));
        if let Some(focused_id) = frame.focused {
            if let Some(pos) = newly_active_float.iter().position(|ws| ws.id == focused_id) {
                let item = newly_active_float.remove(pos);
                newly_active_float.insert(0, item);
            }
        }
        steady_float.sort_by(|a, b| b.id.cmp(&a.id));
        steady_tiling.sort_by(|a, b| b.id.cmp(&a.id));

        let focused_container = frame
            .containers_to_show
            .iter()
            .find(|c| c.placement.is_focused);
        let mut steady_containers: Vec<&ContainerRender> = frame
            .containers_to_show
            .iter()
            .filter(|c| !c.placement.is_focused)
            .collect();
        steady_containers.sort_by(|a, b| b.placement.id.cmp(&a.placement.id));

        // Position in z-order
        let mut anchor: Option<HWND> = None;

        // 1. Newly active floats (iterate in reverse so focused ends up highest)
        for ws in newly_active_float.iter().rev() {
            let is_focused = frame.focused == Some(ws.id);
            if let Some(mw) = self.registry.get_mut(ws.id) {
                mw.show(ws, is_focused, border, &self.config, Some(HWND_TOPMOST));
                if anchor.is_none() {
                    anchor = Some(mw.hwnd());
                }
            }
        }

        // 2. Steady floats — chain below anchor
        for ws in &steady_float {
            let is_focused = frame.focused == Some(ws.id);
            if let Some(mw) = self.registry.get_mut(ws.id) {
                mw.show(ws, is_focused, border, &self.config, anchor);
                anchor = Some(mw.hwnd());
            }
        }

        // 3. Focused container or focused tiling (first non-topmost item)
        if let Some(data) = focused_container {
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&data.placement.id) {
                overlay.update(data.placement, titles, Some(HWND_NOTOPMOST));
                overlay.show();
                anchor = Some(overlay.hwnd());
            }
        } else if let Some(ws) = focused_tiling {
            if let Some(mw) = self.registry.get_mut(ws.id) {
                mw.show(ws, true, border, &self.config, Some(HWND_NOTOPMOST));
                anchor = Some(mw.hwnd());
            }
        } else {
            anchor = None;
        }

        // 4. Steady containers — chain below anchor
        for data in &steady_containers {
            let titles = self.registry.resolve_tab_titles(&data.children);
            if let Some(overlay) = self.container_overlays.get_mut(&data.placement.id) {
                overlay.update(data.placement, titles, anchor);
                overlay.show();
                anchor = Some(overlay.hwnd());
            }
        }

        // 5. Steady tiling — chain below anchor
        for ws in &steady_tiling {
            if let Some(mw) = self.registry.get_mut(ws.id) {
                mw.show(ws, false, border, &self.config, anchor);
                anchor = Some(mw.hwnd());
            }
        }
    }
}

impl Drop for Wm {
    fn drop(&mut self) {
        recovery::restore_all();
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
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
            let frame = unsafe { *Box::from_raw(wparam.0 as *mut LayoutFrame) };
            unsafe { (*ptr).apply_layout_frame(frame) };
            LRESULT(0)
        }
        WM_APP_TITLE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
            let update = unsafe { *Box::from_raw(wparam.0 as *mut TitleUpdate) };
            unsafe { (*ptr).apply_title_update(update) };
            LRESULT(0)
        }
        WM_APP_CONFIG => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
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
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
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
                        app.exclusive_fullscreen.insert(id);
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
