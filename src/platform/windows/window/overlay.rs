use glutin::display::Display;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::SetWindowRgn;
use windows::Win32::UI::WindowsAndMessaging::{
    SWP_NOACTIVATE, SWP_NOZORDER, SetWindowPos, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::PCWSTR;

use crate::config::Config;
use crate::core::WindowPlacement;
use crate::overlay;
use crate::platform::windows::overlay::{OverlayRenderer, OwnedHwnd, build_window_border_region};

pub(crate) const WINDOW_OVERLAY_CLASS: PCWSTR = windows::core::w!("DomeWindowOverlay");

/// `renderer` is declared before `window` so it drops first —
/// GL cleanup runs while the window's HDC is still alive.
pub(super) struct WindowOverlay {
    renderer: OverlayRenderer,
    width: u32,
    height: u32,
    window: OwnedHwnd,
}

impl WindowOverlay {
    pub(super) fn new(display: &Display) -> anyhow::Result<Self> {
        let window = OwnedHwnd::new(WINDOW_OVERLAY_CLASS, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)?;
        let renderer = OverlayRenderer::new(display, window.hwnd(), 1, 1)?;
        Ok(Self {
            renderer,
            width: 1,
            height: 1,
            window,
        })
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    pub(super) fn update(
        &mut self,
        placement: &WindowPlacement,
        config: &Config,
        z_after: Option<HWND>,
    ) {
        let vf = placement.visible_frame;
        let w = vf.width.max(1.0) as u32;
        let h = vf.height.max(1.0) as u32;

        if self.width != w || self.height != h {
            self.renderer.resize(w, h);
            self.width = w;
            self.height = h;
        }

        self.renderer.render(w, h, 1.0, vec![], |ui| {
            overlay::paint_window_border(ui.painter(), placement, config);
        });

        let region = build_window_border_region(placement, config);
        unsafe { SetWindowRgn(self.window.hwnd(), Some(region), true) };

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

    pub(super) fn hide(&mut self) {
        self.window.hide();
    }
}
