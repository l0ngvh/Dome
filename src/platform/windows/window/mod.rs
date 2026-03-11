mod handle;
mod overlay;

pub(super) use handle::{
    ManagedHwnd, WindowMode, enum_windows, get_dimension, get_process_name, get_size_constraints,
    get_window_title, initial_window_mode, is_d3d_exclusive_fullscreen_active, is_fullscreen,
    is_manageable,
};
pub(super) use overlay::WINDOW_OVERLAY_CLASS;

use std::collections::HashMap;

use glutin::display::Display;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::WindowsAndMessaging::{
    GW_HWNDPREV, GetWindow, HWND_NOTOPMOST, HWND_TOP, HWND_TOPMOST, SWP_ASYNCWINDOWPOS,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos,
};

use self::handle::WindowHandle;
use self::overlay::WindowOverlay;
use crate::config::Config;
use crate::core::{Child, WindowId};
use crate::core::{Dimension, WindowPlacement};

pub(super) struct ManagedWindow {
    handle: WindowHandle,
    overlay: Option<WindowOverlay>,
    is_float: bool,
}

impl ManagedWindow {
    pub(super) fn new(
        display: &Display,
        hwnd: HWND,
        title: Option<String>,
        process: String,
        mode: WindowMode,
    ) -> Self {
        let handle = WindowHandle::new_from_create(hwnd, title, process, mode);
        // Hide before first frame — window may end up offscreen due to
        // viewport scrolling. apply_layout will show the visible ones.
        // Fullscreen windows are always inside the viewport, and hiding them
        // interferes with D3D exclusive fullscreen transitions.
        if !mode.is_fullscreen() {
            handle.hide();
        }
        let overlay = match WindowOverlay::new(display) {
            Ok(o) => Some(o),
            Err(e) => {
                tracing::warn!("Failed to create window overlay: {e:#}");
                None
            }
        };
        let mut mw = Self {
            handle,
            overlay,
            is_float: mode == WindowMode::Float,
        };
        mw.sync_z_order();
        mw
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.handle.hwnd()
    }

    pub(super) fn managed_hwnd(&self) -> ManagedHwnd {
        ManagedHwnd::new(self.handle.hwnd())
    }

    pub(super) fn mode(&self) -> WindowMode {
        self.handle.mode()
    }

    pub(super) fn set_mode(&mut self, mode: WindowMode) {
        self.handle.set_mode(mode);
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.handle.title()
    }

    pub(super) fn show(
        &mut self,
        wp: &WindowPlacement,
        border: f32,
        config: &Config,
        is_focused: bool,
    ) {
        let float_changed = self.is_float != wp.is_float;
        if float_changed {
            self.is_float = wp.is_float;
        }

        let overlay_z = self.overlay_z(float_changed, wp.is_float, is_focused);
        let handle_z = if float_changed {
            match &self.overlay {
                Some(o) => Some(o.hwnd()),
                None => Some(if wp.is_float {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                }),
            }
        } else {
            None
        };

        if let Some(overlay) = &mut self.overlay {
            overlay.update(wp, config, overlay_z);
        }
        self.handle.show(&wp.frame, border, wp.is_float, handle_z);
    }

    pub(super) fn hide(&mut self) {
        self.handle.hide();
        self.hide_overlay();
    }

    pub(super) fn set_fullscreen(&mut self, dim: &Dimension) {
        self.handle.set_fullscreen(dim);
        self.hide_overlay();
    }

    pub(super) fn focus(&self) {
        self.handle.focus();
    }

    pub(super) fn set_title(&mut self, title: Option<String>) {
        self.handle.set_title(title);
    }

    pub(super) fn hide_overlay(&mut self) {
        if let Some(overlay) = &mut self.overlay {
            overlay.hide();
        }
    }

    fn overlay_z(&self, float_changed: bool, is_float: bool, is_focused: bool) -> Option<HWND> {
        if float_changed {
            return Some(if is_float {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            });
        }
        if !is_focused {
            return None;
        }
        let overlay_hwnd = self.overlay.as_ref().map(|o| o.hwnd());
        unsafe {
            let above = GetWindow(self.handle.hwnd(), GW_HWNDPREV);
            match above {
                Ok(h) if Some(h) != overlay_hwnd => Some(h),
                _ => Some(if is_float { HWND_TOPMOST } else { HWND_TOP }),
            }
        }
    }

    // Position managed window behind border overlay
    fn sync_z_order(&mut self) {
        let Some(overlay) = &self.overlay else {
            return;
        };
        if self.is_float {
            unsafe {
                SetWindowPos(
                    overlay.hwnd(),
                    Some(HWND_TOPMOST),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                )
                .ok();
            }
        }
        unsafe {
            SetWindowPos(
                self.handle.hwnd(),
                Some(overlay.hwnd()),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            )
            .ok();
        }
    }
}

impl std::fmt::Display for ManagedWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.handle.fmt(f)
    }
}

pub(super) struct Registry {
    windows: HashMap<ManagedHwnd, WindowId>,
    reverse: HashMap<WindowId, ManagedWindow>,
}

impl Registry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, mw: ManagedWindow, id: WindowId) {
        tracing::info!(%mw, %id, "Window inserted");
        self.windows.insert(mw.managed_hwnd(), id);
        self.reverse.insert(id, mw);
    }

    pub(super) fn remove(&mut self, id: WindowId) {
        if let Some(mw) = self.reverse.remove(&id) {
            tracing::info!(%mw, %id, "Window removed");
            self.windows.remove(&mw.managed_hwnd());
        }
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&ManagedWindow> {
        self.reverse.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut ManagedWindow> {
        self.reverse.get_mut(&id)
    }

    pub(super) fn get_by_hwnd(&self, hwnd: ManagedHwnd) -> Option<&ManagedWindow> {
        self.windows.get(&hwnd).and_then(|id| self.reverse.get(id))
    }

    pub(super) fn get_id(&self, hwnd: ManagedHwnd) -> Option<WindowId> {
        self.windows.get(&hwnd).copied()
    }

    pub(super) fn get_by_hwnd_mut(&mut self, hwnd: ManagedHwnd) -> Option<&mut ManagedWindow> {
        self.windows
            .get(&hwnd)
            .and_then(|id| self.reverse.get_mut(id))
    }

    pub(super) fn set_title(&mut self, hwnd: ManagedHwnd, title: Option<String>) {
        if let Some(&id) = self.windows.get(&hwnd)
            && let Some(mw) = self.reverse.get_mut(&id)
        {
            mw.set_title(title);
        }
    }

    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = (WindowId, &mut ManagedWindow)> {
        self.reverse.iter_mut().map(|(&id, mw)| (id, mw))
    }

    pub(super) fn resolve_tab_titles(&self, children: &[Child]) -> Vec<String> {
        children
            .iter()
            .map(|c| match c {
                Child::Window(wid) => self
                    .get(*wid)
                    .and_then(|mw| mw.title())
                    .unwrap_or("<no title>")
                    .to_owned(),
                Child::Container(_) => "Container".to_owned(),
            })
            .collect()
    }
}
