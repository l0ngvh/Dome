mod handle;
mod overlay;

pub(super) use handle::{
    ManagedHwnd, enum_windows, get_dimension, get_process_name, get_size_constraints,
    get_window_title, initial_display_mode, is_fullscreen, is_manageable,
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
use crate::core::{Dimension, WindowId, WindowPlacement};

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
        is_float: bool,
    ) -> Self {
        let handle = WindowHandle::new_from_create(hwnd, title, process);
        handle.hide();
        let overlay = WindowOverlay::new(display).ok();
        let mut mw = Self {
            handle,
            overlay,
            is_float,
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
        self.windows.insert(mw.managed_hwnd(), id);
        self.reverse.insert(id, mw);
    }

    pub(super) fn remove(&mut self, id: WindowId) {
        if let Some(mw) = self.reverse.remove(&id) {
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
}
