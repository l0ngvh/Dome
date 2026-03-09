use std::num::NonZeroU32;
use std::sync::Arc;

use calloop::channel::Sender;
use glow::HasContext;
use glutin::config::ConfigTemplateBuilder;
use glutin::context::{ContextApi, ContextAttributesBuilder, PossiblyCurrentContext};
use glutin::display::Display;
use glutin::prelude::*;
use glutin::surface::{Surface, SurfaceAttributesBuilder, SwapInterval, WindowSurface};
use raw_window_handle::{RawWindowHandle, Win32WindowHandle};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Dwm::DwmExtendFrameIntoClientArea;
use windows::Win32::Graphics::Gdi::{
    BeginPaint, CombineRgn, CreateRectRgn, DeleteObject, EndPaint, HRGN, InvalidateRect,
    PAINTSTRUCT, RGN_OR, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetWindowLongPtrW, SW_HIDE,
    SW_SHOWNA, SWP_NOACTIVATE, SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    WINDOW_EX_STYLE, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::PCWSTR;

use super::dome::HubEvent;
use crate::config::Config;
use crate::core::{ContainerPlacement, WindowPlacement};
use crate::overlay;

pub(super) const CONTAINER_OVERLAY_CLASS: PCWSTR = windows::core::w!("DomeContainerOverlay");

/// `renderer` is declared before `window` so it drops first —
/// GL cleanup runs while the window's HDC is still alive.
pub(super) struct ContainerOverlay {
    renderer: OverlayRenderer,
    events: Vec<egui::Event>,
    width: u32,
    height: u32,
    placement: Option<ContainerPlacement>,
    tab_titles: Vec<String>,
    pub(super) config: Config,
    hub_sender: Sender<HubEvent>,
    window: OwnedHwnd,
}

impl ContainerOverlay {
    pub(super) fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    pub(super) fn new(
        display: &Display,
        config: Config,
        hub_sender: Sender<HubEvent>,
    ) -> anyhow::Result<Box<Self>> {
        let window = OwnedHwnd::new(CONTAINER_OVERLAY_CLASS, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)?;
        let hwnd = window.hwnd();

        let renderer = OverlayRenderer::new(display, hwnd, 1, 1)?;

        let mut boxed = Box::new(Self {
            renderer,
            events: Vec::new(),
            width: 1,
            height: 1,
            placement: None,
            tab_titles: Vec::new(),
            config,
            hub_sender,
            window,
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };

        Ok(boxed)
    }

    pub(super) fn update(&mut self, placement: ContainerPlacement, tab_titles: Vec<String>) {
        let vf = placement.visible_frame;
        let w = vf.width.max(1.0) as u32;
        let h = vf.height.max(1.0) as u32;

        if self.width != w || self.height != h {
            self.renderer.resize(w, h);
            self.width = w;
            self.height = h;
        }

        let region = build_container_region(&placement, &self.config);
        unsafe { SetWindowRgn(self.window.hwnd(), Some(region), true) };

        self.placement = Some(placement);
        self.tab_titles = tab_titles;
        self.rerender();

        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                vf.x as i32,
                vf.y as i32,
                w as i32,
                h as i32,
                SWP_NOZORDER | SWP_NOACTIVATE,
            )
            .ok();
        }
    }

    pub(super) fn show(&mut self) {
        self.window.show();
    }

    fn rerender(&mut self) {
        let Some(placement) = self.placement.as_ref() else {
            return;
        };
        let tab_titles = &self.tab_titles;
        let config = &self.config;
        let events = std::mem::take(&mut self.events);
        let clicked = self
            .renderer
            .render(self.width, self.height, 1.0, events, |ui| {
                overlay::show_container(ui, placement, tab_titles, config)
            });
        if let Some(tab_idx) = clicked {
            self.hub_sender
                .send(HubEvent::TabClicked(placement.id, tab_idx))
                .ok();
        }
    }
}

impl Drop for ContainerOverlay {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
    }
}

pub(super) unsafe extern "system" fn container_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut ContainerOverlay;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let overlay = unsafe { &mut *ptr };
    match msg {
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            overlay
                .events
                .push(egui::Event::PointerMoved(egui::pos2(x, y)));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            overlay.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            overlay.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x, y),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            invalidate_rect(hwnd);
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                let mut ps = std::mem::zeroed::<PAINTSTRUCT>();
                BeginPaint(hwnd, &mut ps);
                overlay.rerender();
                EndPaint(hwnd, &ps).ok().ok(); // always returns nonzero per MSDN
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub(super) fn raw_window_handle(hwnd: HWND) -> RawWindowHandle {
    let mut handle = Win32WindowHandle::new(std::num::NonZeroIsize::new(hwnd.0 as isize).unwrap());
    let hinstance = unsafe { GetModuleHandleW(None).unwrap() };
    handle.hinstance = std::num::NonZeroIsize::new(hinstance.0 as isize);
    RawWindowHandle::Win32(handle)
}

/// Owns an HWND and calls `DestroyWindow` on drop.
/// Fields declared before this in a struct are dropped first,
/// ensuring GL resources are cleaned up while the window's HDC is still alive.
pub(super) struct OwnedHwnd {
    hwnd: HWND,
    is_visible: bool,
}

impl OwnedHwnd {
    pub(super) fn new(class: PCWSTR, ex_style: WINDOW_EX_STYLE) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style,
                class,
                windows::core::w!(""),
                WS_POPUP,
                0,
                0,
                1,
                1,
                None,
                None,
                Some(GetModuleHandleW(None)?.into()),
                None,
            )?
        };
        enable_blur_behind(hwnd);
        Ok(Self {
            hwnd,
            is_visible: false,
        })
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.hwnd
    }

    pub(super) fn show(&mut self) {
        if !self.is_visible {
            // BOOL is previous visibility state, not an error indicator
            unsafe { ShowWindow(self.hwnd, SW_SHOWNA).ok().ok() };
            self.is_visible = true;
        }
    }

    pub(super) fn hide(&mut self) {
        if self.is_visible {
            // BOOL is previous visibility state, not an error indicator
            unsafe { ShowWindow(self.hwnd, SW_HIDE).ok().ok() };
            self.is_visible = false;
        }
    }
}

impl Drop for OwnedHwnd {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd).ok() };
    }
}

pub(super) struct OverlayRenderer {
    surface: Surface<WindowSurface>,
    gl_context: PossiblyCurrentContext,
    gl: Arc<glow::Context>,
    painter: egui_glow::Painter,
    egui_ctx: egui::Context,
}

impl OverlayRenderer {
    pub(super) fn new(
        display: &Display,
        hwnd: HWND,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        let raw_window = raw_window_handle(hwnd);

        let config_template = ConfigTemplateBuilder::new().with_alpha_size(8).build();
        let gl_config = unsafe { display.find_configs(config_template) }?
            .next()
            .ok_or_else(|| anyhow::anyhow!("no suitable GL config"))?;

        let context_attrs = ContextAttributesBuilder::new()
            .with_context_api(ContextApi::OpenGl(None))
            .build(Some(raw_window));
        let context = unsafe { display.create_context(&gl_config, &context_attrs) }?;

        let w = NonZeroU32::new(width.max(1)).unwrap();
        let h = NonZeroU32::new(height.max(1)).unwrap();
        let surface_attrs =
            SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window, w, h);
        let surface = unsafe { display.create_window_surface(&gl_config, &surface_attrs) }?;

        let gl_context = context.make_current(&surface)?;
        surface
            .set_swap_interval(&gl_context, SwapInterval::DontWait)
            .ok();

        let gl = unsafe {
            Arc::new(glow::Context::from_loader_function_cstr(|s| {
                display.get_proc_address(s)
            }))
        };

        let painter = egui_glow::Painter::new(Arc::clone(&gl), "", None, false)
            .map_err(|e| anyhow::anyhow!("{e}"))?;

        Ok(Self {
            surface,
            gl_context,
            gl,
            painter,
            egui_ctx: egui::Context::default(),
        })
    }

    pub(super) fn resize(&self, width: u32, height: u32) {
        let w = NonZeroU32::new(width.max(1)).unwrap();
        let h = NonZeroU32::new(height.max(1)).unwrap();
        self.surface.resize(&self.gl_context, w, h);
    }

    pub(super) fn render<R>(
        &mut self,
        width: u32,
        height: u32,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        ui_fn: impl FnOnce(&mut egui::Ui) -> R,
    ) -> R {
        self.gl_context.make_current(&self.surface).ok();
        unsafe {
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
        }

        let w_pts = width as f32 / pixels_per_point;
        let h_pts = height as f32 / pixels_per_point;
        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(w_pts, h_pts),
            )),
            viewports: std::iter::once((
                egui::ViewportId::ROOT,
                egui::ViewportInfo {
                    native_pixels_per_point: Some(pixels_per_point),
                    ..Default::default()
                },
            ))
            .collect(),
            events,
            ..Default::default()
        };

        let mut ui_fn = Some(ui_fn);
        let mut result = None;
        let output = self.egui_ctx.run(raw_input, |ctx| {
            egui::Area::new(egui::Id::new("overlay"))
                .fixed_pos(egui::pos2(0.0, 0.0))
                .show(ctx, |ui| {
                    if let Some(f) = ui_fn.take() {
                        result = Some(f(ui));
                    }
                });
        });
        let meshes = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);
        self.painter.paint_and_update_textures(
            [width, height],
            output.pixels_per_point,
            &meshes,
            &output.textures_delta,
        );
        self.surface.swap_buffers(&self.gl_context).ok();
        result.unwrap()
    }
}

impl Drop for OverlayRenderer {
    fn drop(&mut self) {
        self.gl_context.make_current(&self.surface).ok();
        self.painter.destroy();
    }
}

/// MSDN says InvalidateRect "returns zero if the function fails" but
/// documents no specific failure conditions. We have no actionable
/// recovery path, so we discard the result.
fn invalidate_rect(hwnd: HWND) {
    unsafe { InvalidateRect(Some(hwnd), None, false).ok().ok() };
}

fn enable_blur_behind(hwnd: HWND) {
    let margins = MARGINS {
        cxLeftWidth: -1,
        cxRightWidth: -1,
        cyTopHeight: -1,
        cyBottomHeight: -1,
    };
    unsafe { DwmExtendFrameIntoClientArea(hwnd, &margins).ok() };
}

pub(super) fn build_window_border_region(placement: &WindowPlacement, config: &Config) -> HRGN {
    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = (f.x - vf.x) as i32;
    let oy = (f.y - vf.y) as i32;
    let fw = f.width as i32;
    let fh = f.height as i32;
    let vw = vf.width as i32;
    let vh = vf.height as i32;
    let b = config.border_size as i32;

    let clamped_rgn = |x1: i32, y1: i32, x2: i32, y2: i32| -> HRGN {
        unsafe {
            CreateRectRgn(
                x1.max(0).min(vw),
                y1.max(0).min(vh),
                x2.max(0).min(vw),
                y2.max(0).min(vh),
            )
        }
    };

    unsafe {
        let top = clamped_rgn(ox, oy, ox + fw, oy + b);
        let bottom = clamped_rgn(ox, oy + fh - b, ox + fw, oy + fh);
        let left = clamped_rgn(ox, oy + b, ox + b, oy + fh - b);
        let right = clamped_rgn(ox + fw - b, oy + b, ox + fw, oy + fh - b);
        let region = CreateRectRgn(0, 0, 0, 0);
        CombineRgn(Some(region), Some(top), Some(bottom), RGN_OR);
        CombineRgn(Some(region), Some(region), Some(left), RGN_OR);
        CombineRgn(Some(region), Some(region), Some(right), RGN_OR);
        // DeleteObject only fails for invalid or DC-selected handles;
        // these regions are freshly created and never selected into a DC.
        DeleteObject(top.into()).ok().ok();
        DeleteObject(bottom.into()).ok().ok();
        DeleteObject(left.into()).ok().ok();
        DeleteObject(right.into()).ok().ok();
        region
    }
}

/// Builds a hit-test region covering border strips and (if tabbed) the tab bar.
/// Coordinates are window-local (the overlay window is sized to `visible_frame`).
/// The drawing code in overlay.rs uses offsets `ox = frame.x - visible_frame.x`,
/// `oy = frame.y - visible_frame.y`, so the region must use the same offsets.
fn build_container_region(placement: &ContainerPlacement, config: &Config) -> HRGN {
    let vf = placement.visible_frame;
    let f = placement.frame;
    let ox = (f.x - vf.x) as i32;
    let oy = (f.y - vf.y) as i32;
    let fw = f.width as i32;
    let fh = f.height as i32;
    let vw = vf.width as i32;
    let vh = vf.height as i32;
    let b = config.border_size as i32;

    // Clamp helper: intersect a rect with the visible window bounds (0,0)-(vw,vh)
    let clamped_rgn = |x1: i32, y1: i32, x2: i32, y2: i32| -> HRGN {
        let cx1 = x1.max(0).min(vw);
        let cy1 = y1.max(0).min(vh);
        let cx2 = x2.max(0).min(vw);
        let cy2 = y2.max(0).min(vh);
        unsafe { CreateRectRgn(cx1, cy1, cx2, cy2) }
    };

    unsafe {
        let region = CreateRectRgn(0, 0, 0, 0);

        if placement.is_tabbed {
            let th = config.tab_bar_height as i32;
            // Tab bar: (ox, oy) to (ox + fw, oy + th)
            let tab = clamped_rgn(ox, oy, ox + fw, oy + th);
            CombineRgn(Some(region), Some(region), Some(tab), RGN_OR);
            // DeleteObject only fails for invalid or DC-selected handles;
            // these regions are freshly created and never selected into a DC.
            DeleteObject(tab.into()).ok().ok();
            // Left border below tab bar
            let left = clamped_rgn(ox, oy + th, ox + b, oy + fh - b);
            CombineRgn(Some(region), Some(region), Some(left), RGN_OR);
            DeleteObject(left.into()).ok().ok();
            // Right border below tab bar
            let right = clamped_rgn(ox + fw - b, oy + th, ox + fw, oy + fh - b);
            CombineRgn(Some(region), Some(region), Some(right), RGN_OR);
            DeleteObject(right.into()).ok().ok();
            // Bottom border
            let bottom = clamped_rgn(ox, oy + fh - b, ox + fw, oy + fh);
            CombineRgn(Some(region), Some(region), Some(bottom), RGN_OR);
            DeleteObject(bottom.into()).ok().ok();
        } else {
            // Four border strips
            let top = clamped_rgn(ox, oy, ox + fw, oy + b);
            let bottom = clamped_rgn(ox, oy + fh - b, ox + fw, oy + fh);
            let left = clamped_rgn(ox, oy + b, ox + b, oy + fh - b);
            let right = clamped_rgn(ox + fw - b, oy + b, ox + fw, oy + fh - b);
            CombineRgn(Some(region), Some(top), Some(bottom), RGN_OR);
            CombineRgn(Some(region), Some(region), Some(left), RGN_OR);
            CombineRgn(Some(region), Some(region), Some(right), RGN_OR);
            // DeleteObject only fails for invalid or DC-selected handles;
            // these regions are freshly created and never selected into a DC.
            DeleteObject(top.into()).ok().ok();
            DeleteObject(bottom.into()).ok().ok();
            DeleteObject(left.into()).ok().ok();
            DeleteObject(right.into()).ok().ok();
        }

        region
    }
}
