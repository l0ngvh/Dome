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
use windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST;

use self::handle::WindowHandle;
use self::overlay::WindowOverlay;
use crate::config::Config;
use crate::core::{Child, WindowId};
use crate::core::{Dimension, WindowPlacement};

pub(super) struct ManagedWindow {
    handle: WindowHandle,
    overlay: Option<WindowOverlay>,
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
        let overlay = match WindowOverlay::new(display) {
            Ok(o) => Some(o),
            Err(e) => {
                tracing::debug!("Failed to create window overlay: {e:#}");
                None
            }
        };
        Self { handle, overlay }
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

    /// Position this window and its border overlay at the given placement.
    ///
    /// `z` is precomputed by the app-level z-order logic:
    /// - `Some(HWND_TOPMOST)`: window first, overlay second — both get TOPMOST,
    ///   call order ensures overlay ends up above window.
    /// - All other values (`Some(HWND_NOTOPMOST)`, `Some(specific_hwnd)`, `None`):
    ///   overlay first (`z`), window second (`overlay.hwnd()`). Placing a window
    ///   below a non-topmost overlay implicitly strips WS_EX_TOPMOST, so
    ///   HWND_NOTOPMOST needs no special path.
    /// - `None` means overlay gets SWP_NOZORDER, but the window still anchors
    ///   below the overlay via `overlay.hwnd()`.
    #[tracing::instrument(skip_all)]
    pub(super) fn show(
        &mut self,
        wp: &WindowPlacement,
        border: f32,
        config: &Config,
        z: Option<HWND>,
    ) {
        tracing::debug!("show {self} frame={:?} float={}", wp.frame, wp.is_float);

        if z == Some(HWND_TOPMOST) {
            self.handle.show(&wp.frame, border, wp.is_float, z);
            if let Some(overlay) = &mut self.overlay {
                overlay.update(wp, config, z);
            }
        } else {
            if let Some(overlay) = &mut self.overlay {
                overlay.update(wp, config, z);
            }
            let window_z = self.overlay.as_ref().map(|o| o.hwnd());
            self.handle.show(&wp.frame, border, wp.is_float, window_z);
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn hide(&mut self) {
        tracing::debug!("hide {self}");
        self.handle.hide();
        self.hide_overlay();
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn set_fullscreen(&mut self, dim: &Dimension) {
        tracing::debug!("set_fullscreen {self} dim={dim:?}");
        self.handle.set_fullscreen(dim);
        self.hide_overlay();
    }

    pub(super) fn focus(&self) {
        self.handle.focus();
    }

    pub(super) fn set_title(&mut self, title: Option<String>) {
        self.handle.set_title(title);
    }

    fn hide_overlay(&mut self) {
        if let Some(overlay) = &mut self.overlay {
            overlay.hide();
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
