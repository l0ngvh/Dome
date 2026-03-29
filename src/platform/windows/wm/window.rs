use std::collections::HashMap;
use std::sync::Arc;

use glutin::display::Display;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::SetWindowRgn;
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::PCWSTR;

use super::super::dome::WindowShow;
use super::super::external::{HwndId, ManageExternalHwnd, ShowCmd, ZOrder};
use super::super::handle::WindowMode;
use super::overlay::{OverlayRenderer, OwnedHwnd, build_window_border_region};
use crate::config::Config;
use crate::core::{Child, Dimension, WindowId, WindowPlacement};
use crate::overlay;
use crate::platform::windows::taskbar::Taskbar;

pub(super) trait OverlayApi {
    fn id(&self) -> HwndId;
    fn update(&mut self, ws: &WindowShow, is_focused: bool, config: &Config, z: ZOrder);
    fn hide(&mut self);
}

pub(super) struct ManagedWindow {
    handle: WindowHandle,
    overlay: Option<Box<dyn OverlayApi>>,
}

impl ManagedWindow {
    pub(super) fn new(
        inner: Arc<dyn ManageExternalHwnd>,
        title: Option<String>,
        process: String,
        mode: WindowMode,
        overlay: Option<Box<dyn OverlayApi>>,
    ) -> Self {
        let handle = WindowHandle {
            inner,
            title,
            process,
            mode,
        };
        Self { handle, overlay }
    }

    pub(super) fn id(&self) -> HwndId {
        self.handle.inner.id()
    }

    pub(super) fn mode(&self) -> WindowMode {
        self.handle.mode
    }

    pub(super) fn set_mode(&mut self, mode: WindowMode) {
        self.handle.mode = mode;
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.handle.title.as_deref()
    }

    pub(super) fn add_to_taskbar(&self, taskbar: &Taskbar) {
        self.handle.inner.add_to_taskbar(taskbar);
    }

    pub(super) fn remove_from_taskbar(&self, taskbar: &Taskbar) {
        self.handle.inner.remove_from_taskbar(taskbar);
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn show(
        &mut self,
        ws: &WindowShow,
        is_focused: bool,
        border: f32,
        config: &Config,
        z: ZOrder,
    ) {
        tracing::debug!("show {self} frame={:?} float={}", ws.frame, ws.is_float);

        match z {
            ZOrder::Topmost => {
                self.handle
                    .show(&ws.frame, border, ws.is_float, ZOrder::Topmost);
                if let Some(overlay) = &mut self.overlay {
                    overlay.update(ws, is_focused, config, ZOrder::Topmost);
                }
            }
            _ => {
                if let Some(overlay) = &mut self.overlay {
                    overlay.update(ws, is_focused, config, z);
                }
                let window_z = match &self.overlay {
                    Some(o) => ZOrder::After(o.id()),
                    None => z,
                };
                self.handle.show(&ws.frame, border, ws.is_float, window_z);
            }
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn hide(&mut self) {
        tracing::debug!("hide {self}");
        self.handle.hide();
        if let Some(overlay) = &mut self.overlay {
            overlay.hide();
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn set_fullscreen(&mut self, dim: &Dimension) {
        tracing::debug!("set_fullscreen {self} dim={dim:?}");
        self.handle.set_fullscreen(dim);
        if let Some(overlay) = &mut self.overlay {
            overlay.hide();
        }
    }

    pub(super) fn focus(&self) {
        self.handle.inner.set_foreground_window();
    }

    pub(super) fn set_title(&mut self, title: Option<String>) {
        self.handle.title = title;
    }
}

impl std::fmt::Display for ManagedWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.handle.title.as_deref().unwrap_or("<no title>");
        write!(
            f,
            "'{title}' from '{}' id={:?} mode={}",
            self.handle.process,
            self.handle.inner.id(),
            self.handle.mode
        )
    }
}

pub(super) struct Registry {
    windows: HashMap<HwndId, WindowId>,
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
        self.windows.insert(mw.id(), id);
        self.reverse.insert(id, mw);
    }

    pub(super) fn remove(&mut self, id: WindowId) {
        if let Some(mw) = self.reverse.remove(&id) {
            tracing::info!(%mw, %id, "Window removed");
            self.windows.remove(&mw.id());
        }
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&ManagedWindow> {
        self.reverse.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut ManagedWindow> {
        self.reverse.get_mut(&id)
    }

    pub(super) fn get_id(&self, hwnd: HwndId) -> Option<WindowId> {
        self.windows.get(&hwnd).copied()
    }

    pub(super) fn set_title(&mut self, hwnd: HwndId, title: Option<String>) {
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

pub(super) fn is_d3d_exclusive_fullscreen_active() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

pub(super) const WINDOW_OVERLAY_CLASS: PCWSTR = windows::core::w!("DomeWindowOverlay");

pub(super) fn create_window_overlay(display: &Display) -> Option<Box<dyn OverlayApi>> {
    match WindowOverlay::new(display) {
        Ok(o) => Some(Box::new(o)),
        Err(e) => {
            tracing::debug!("Failed to create window overlay: {e:#}");
            None
        }
    }
}

struct WindowHandle {
    inner: Arc<dyn ManageExternalHwnd>,
    title: Option<String>,
    process: String,
    mode: WindowMode,
}

impl WindowHandle {
    fn set_fullscreen(&mut self, dim: &Dimension) {
        match self.mode {
            WindowMode::FullscreenBorderless | WindowMode::FullscreenExclusive => return,
            _ => {}
        }
        self.mode = WindowMode::ManagedFullscreen;
        self.inner.set_position(
            ZOrder::Unchanged,
            dim.x as i32,
            dim.y as i32,
            dim.width as i32,
            dim.height as i32,
        );
    }

    fn show(&mut self, dim: &Dimension, border: f32, is_float: bool, z: ZOrder) {
        if self.mode == WindowMode::FullscreenExclusive {
            return;
        }
        let content = apply_inset(*dim, border);
        if self.inner.is_iconic() {
            self.inner.show_cmd(ShowCmd::Restore);
        }
        self.inner.set_position(
            z,
            content.x as i32,
            content.y as i32,
            content.width as i32,
            content.height as i32,
        );
        self.mode = if is_float {
            WindowMode::Float
        } else {
            WindowMode::Tiling
        };
    }

    fn hide(&self) {
        match self.mode {
            WindowMode::FullscreenExclusive => {}
            WindowMode::FullscreenBorderless => {
                self.inner.show_cmd(ShowCmd::Minimize);
            }
            _ => {
                self.inner.move_offscreen();
            }
        }
    }
}

/// `renderer` is declared before `window` so it drops first —
/// GL cleanup runs while the window's HDC is still alive.
struct WindowOverlay {
    renderer: OverlayRenderer,
    width: u32,
    height: u32,
    window: OwnedHwnd,
}

impl WindowOverlay {
    fn new(display: &Display) -> anyhow::Result<Self> {
        let window = OwnedHwnd::new(WINDOW_OVERLAY_CLASS, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)?;
        let renderer = OverlayRenderer::new(display, window.hwnd(), 1, 1)?;
        Ok(Self {
            renderer,
            width: 1,
            height: 1,
            window,
        })
    }
}

impl OverlayApi for WindowOverlay {
    fn id(&self) -> HwndId {
        HwndId::from(self.window.hwnd())
    }

    fn update(&mut self, ws: &WindowShow, is_focused: bool, config: &Config, z: ZOrder) {
        let vf = ws.visible_frame;
        let w = vf.width.max(1.0) as u32;
        let h = vf.height.max(1.0) as u32;

        if self.width != w || self.height != h {
            self.renderer.resize(w, h);
            self.width = w;
            self.height = h;
        }

        let placement = WindowPlacement {
            id: ws.id,
            frame: ws.frame,
            visible_frame: ws.visible_frame,
            is_float: ws.is_float,
            is_focused,
            spawn_mode: ws.spawn_mode,
        };

        self.renderer.render(w, h, 1.0, vec![], |ui| {
            overlay::paint_window_border(ui.painter(), &placement, config);
        });

        let region = build_window_border_region(&placement, config);
        unsafe { SetWindowRgn(self.window.hwnd(), Some(region), true) };

        let z_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE;
        if z_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                z_after,
                vf.x as i32,
                vf.y as i32,
                w as i32,
                h as i32,
                flags,
            )
            .ok();
        }

        self.window.show();
    }

    fn hide(&mut self) {
        self.window.hide();
    }
}

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}
