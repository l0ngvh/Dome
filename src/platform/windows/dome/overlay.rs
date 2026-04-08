use std::mem::size_of;
use std::num::NonZeroU32;
use std::sync::Arc;

use crate::platform::windows::HubSender;
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
    BeginPaint, CombineRgn, CreateRectRgn, DeleteObject, EndPaint, HRGN, InvalidateRect, OffsetRgn,
    PAINTSTRUCT, RGN_OR, SetWindowRgn,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::MARGINS;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetForegroundWindow,
    GetWindowLongPtrW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOZORDER, SetForegroundWindow,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_LBUTTONDOWN, WM_LBUTTONUP,
    WM_MOUSEMOVE, WM_PAINT, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::PCWSTR;

use super::HubEvent;
use crate::config::Config;
use crate::core::{ContainerPlacement, Dimension, WindowPlacement};
use crate::overlay;
use crate::platform::windows::external::{HwndId, ZOrder};

pub(in crate::platform::windows) fn raw_window_handle(hwnd: HWND) -> RawWindowHandle {
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
        mut ctx_fn: impl FnMut(&egui::Context) -> R,
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

        let mut result = None;
        let output = self.egui_ctx.run(raw_input, |ctx| {
            result = Some(ctx_fn(ctx));
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

        // Unfocused containers exclude border strips so the per-window overlay
        // borders underneath remain visible (egui skips drawing them anyway).
        if placement.is_tabbed {
            let th = config.tab_bar_height as i32;
            let tab = clamped_rgn(ox, oy, ox + fw, oy + th);
            CombineRgn(Some(region), Some(region), Some(tab), RGN_OR);
            // DeleteObject only fails for invalid or DC-selected handles;
            // these regions are freshly created and never selected into a DC.
            DeleteObject(tab.into()).ok().ok();

            if placement.is_focused {
                let left = clamped_rgn(ox, oy + th, ox + b, oy + fh - b);
                CombineRgn(Some(region), Some(region), Some(left), RGN_OR);
                DeleteObject(left.into()).ok().ok();
                let right = clamped_rgn(ox + fw - b, oy + th, ox + fw, oy + fh - b);
                CombineRgn(Some(region), Some(region), Some(right), RGN_OR);
                DeleteObject(right.into()).ok().ok();
                let bottom = clamped_rgn(ox, oy + fh - b, ox + fw, oy + fh);
                CombineRgn(Some(region), Some(region), Some(bottom), RGN_OR);
                DeleteObject(bottom.into()).ok().ok();
            }
        } else if placement.is_focused {
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

pub(in crate::platform::windows) const TILING_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomeTilingOverlay");

/// Per-monitor overlay that draws all tiling window borders and container tab bars.
/// `renderer` is declared before `window` so it drops first.
pub(in crate::platform::windows) struct TilingOverlay {
    renderer: OverlayRenderer,
    events: Vec<egui::Event>,
    monitor: Dimension,
    windows: Vec<WindowPlacement>,
    containers: Vec<(ContainerPlacement, Vec<String>)>,
    config: Config,
    hub_sender: HubSender,
    window: OwnedHwnd,
}

impl TilingOverlay {
    pub(in crate::platform::windows) fn new(
        display: &Display,
        config: Config,
        hub_sender: HubSender,
    ) -> anyhow::Result<Box<Self>> {
        let window = OwnedHwnd::new(TILING_OVERLAY_CLASS, WS_EX_TOOLWINDOW)?;
        let hwnd = window.hwnd();
        let renderer = OverlayRenderer::new(display, hwnd, 1, 1)?;
        let mut boxed = Box::new(Self {
            renderer,
            events: Vec::new(),
            monitor: Dimension::default(),
            windows: Vec::new(),
            containers: Vec::new(),
            config,
            hub_sender,
            window,
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };
        Ok(boxed)
    }

    fn rerender(&mut self) {
        if self.windows.is_empty() && self.containers.is_empty() {
            return;
        }
        let monitor = self.monitor;
        let config = &self.config;
        let events = std::mem::take(&mut self.events);
        let w = monitor.width.max(1.0) as u32;
        let h = monitor.height.max(1.0) as u32;
        let clicked_tabs = self.renderer.render(w, h, 1.0, events, |ctx| {
            overlay::paint_tiling_overlay(ctx, monitor, &self.windows, &self.containers, config)
        });
        for (container_id, tab_idx) in clicked_tabs {
            self.hub_sender
                .send(HubEvent::TabClicked(container_id, tab_idx));
        }
    }
}

impl TilingOverlayApi for TilingOverlay {
    fn id(&self) -> HwndId {
        HwndId::from(self.window.hwnd())
    }

    fn update(
        &mut self,
        monitor: Dimension,
        windows: &[WindowPlacement],
        containers: &[(ContainerPlacement, Vec<String>)],
    ) {
        let w = monitor.width.max(1.0) as u32;
        let h = monitor.height.max(1.0) as u32;

        if self.monitor != monitor {
            self.renderer.resize(w, h);
            self.monitor = monitor;
        }

        // Build combined region covering all border strips and tab bar rects
        let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
        for wp in windows {
            let wr = build_window_border_region(wp, &self.config);
            let ox = (wp.visible_frame.x - monitor.x) as i32;
            let oy = (wp.visible_frame.y - monitor.y) as i32;
            unsafe {
                OffsetRgn(wr, ox, oy);
                CombineRgn(Some(region), Some(region), Some(wr), RGN_OR);
                DeleteObject(wr.into()).ok().ok();
            }
        }
        for (cp, _) in containers {
            let cr = build_container_region(cp, &self.config);
            let ox = (cp.visible_frame.x - monitor.x) as i32;
            let oy = (cp.visible_frame.y - monitor.y) as i32;
            unsafe {
                OffsetRgn(cr, ox, oy);
                CombineRgn(Some(region), Some(region), Some(cr), RGN_OR);
                DeleteObject(cr.into()).ok().ok();
            }
        }
        unsafe { SetWindowRgn(self.window.hwnd(), Some(region), true) };

        self.windows = windows.to_vec();
        self.containers = containers.to_vec();
        self.rerender();

        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                None,
                monitor.x as i32,
                monitor.y as i32,
                w as i32,
                h as i32,
                SWP_NOACTIVATE | SWP_NOZORDER,
            )
            .ok();
        }
        self.window.show();
    }

    fn clear(&mut self) {
        self.windows.clear();
        self.containers.clear();
        let region = unsafe { CreateRectRgn(0, 0, 0, 0) };
        unsafe { SetWindowRgn(self.window.hwnd(), Some(region), true) };
    }

    fn focus(&self) {
        let hwnd = self.window.hwnd();
        if unsafe { GetForegroundWindow() } == hwnd {
            return;
        }
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
        ];
        unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed for tiling overlay");
        }
    }

    fn set_config(&mut self, config: Config) {
        self.config = config;
    }
}

impl Drop for TilingOverlay {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
    }
}

pub(in crate::platform::windows) unsafe extern "system" fn tiling_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut TilingOverlay;
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
                EndPaint(hwnd, &ps).ok().ok();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub(in crate::platform::windows) trait FloatOverlayApi {
    fn id(&self) -> HwndId;
    fn update(&mut self, wp: &WindowPlacement, config: &Config, z: ZOrder);
    fn hide(&mut self);
}

pub(in crate::platform::windows) trait TilingOverlayApi {
    fn id(&self) -> HwndId;
    fn update(
        &mut self,
        monitor: Dimension,
        windows: &[WindowPlacement],
        containers: &[(ContainerPlacement, Vec<String>)],
    );
    fn clear(&mut self);
    fn focus(&self);
    fn set_config(&mut self, config: Config);
}

pub(in crate::platform::windows) const FLOAT_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomeFloatOverlay");

pub(in crate::platform::windows) fn create_float_overlay(
    display: &Display,
) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
    Ok(Box::new(FloatOverlay::new(display)?))
}

struct FloatOverlay {
    renderer: OverlayRenderer,
    width: u32,
    height: u32,
    window: OwnedHwnd,
}

impl FloatOverlay {
    fn new(display: &Display) -> anyhow::Result<Self> {
        let window = OwnedHwnd::new(FLOAT_OVERLAY_CLASS, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)?;
        let renderer = OverlayRenderer::new(display, window.hwnd(), 1, 1)?;
        Ok(Self {
            renderer,
            width: 1,
            height: 1,
            window,
        })
    }
}

impl FloatOverlayApi for FloatOverlay {
    fn id(&self) -> HwndId {
        HwndId::from(self.window.hwnd())
    }

    fn update(&mut self, wp: &WindowPlacement, config: &Config, z: ZOrder) {
        let vf = wp.visible_frame;
        let w = vf.width.max(1.0) as u32;
        let h = vf.height.max(1.0) as u32;

        if self.width != w || self.height != h {
            self.renderer.resize(w, h);
            self.width = w;
            self.height = h;
        }

        self.renderer.render(w, h, 1.0, vec![], |ctx| {
            let origin = egui::vec2(0.0, 0.0);
            egui::Area::new(egui::Id::new(("border", wp.id)))
                .fixed_pos(origin.to_pos2())
                .show(ctx, |ui| {
                    ui.set_clip_rect(egui::Rect::from_min_size(
                        origin.to_pos2(),
                        egui::vec2(vf.width, vf.height),
                    ));
                    overlay::paint_window_border(ui.painter(), wp, config, origin);
                });
        });

        let region = build_window_border_region(wp, config);
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
