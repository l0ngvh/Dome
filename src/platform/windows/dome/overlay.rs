use std::sync::Arc;

use crate::font::FontConfig;
use crate::platform::windows::dpi;
use crate::platform::windows::{HubSender, WM_APP_DPI_CHANGE, WM_GETDPISCALEDSIZE};
use crate::theme::Flavor;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, MONITOR_DEFAULTTONEAREST, MonitorFromWindow, PAINTSTRUCT,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GWLP_USERDATA, GetClientRect,
    GetWindowLongPtrW, PostThreadMessageW, SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOREDRAW,
    SWP_NOZORDER, SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_DPICHANGED,
    WM_ERASEBKGND, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_PAINT, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{Interface, PCWSTR};

use super::HubEvent;
use crate::config::Config;
use crate::core::{
    ContainerPlacement, Dimension, FloatWindowPlacement, TilingWindowPlacement, WindowId,
};
use crate::overlay;
use crate::picker::PickerEntry;
use crate::platform::windows::dome::CreateOverlay;
use crate::platform::windows::dome::picker;
use crate::platform::windows::external::{HwndId, ZOrder};

/// Owns an HWND and calls `DestroyWindow` on drop.
/// Fields declared before this in a struct are dropped first,
/// ensuring renderer resources are cleaned up while the window's HDC is still alive.
pub(super) struct OwnedHwnd {
    hwnd: HWND,
    is_visible: bool,
}

impl OwnedHwnd {
    pub(super) fn new(class: PCWSTR, ex_style: WINDOW_EX_STYLE) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style | WS_EX_NOREDIRECTIONBITMAP,
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

    pub(super) fn is_visible(&self) -> bool {
        self.is_visible
    }
}

impl Drop for OwnedHwnd {
    fn drop(&mut self) {
        unsafe { DestroyWindow(self.hwnd).ok() };
    }
}

/// wgpu + DirectComposition renderer for overlay windows.
///
/// Field order matters for drop safety: `surface` must drop before the DComp objects
/// it references, and `painter` before the device. Rust drops fields in declaration order.
pub(super) struct Renderer {
    surface: wgpu::Surface<'static>,
    surface_config: wgpu::SurfaceConfiguration,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    painter: egui_wgpu::Renderer,
    egui_ctx: egui::Context,
    // DComp objects kept alive for the surface lifetime.
    // dcomp_device is also used in resize() to commit after reconfiguration.
    _dcomp_visual: IDCompositionVisual,
    _dcomp_target: IDCompositionTarget,
    dcomp_device: IDCompositionDevice,
    opaque: bool,
}

impl Renderer {
    #[expect(
        clippy::too_many_arguments,
        reason = "flavor added for theming; restructuring Renderer::new is out of scope"
    )]
    pub(super) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        hwnd: HWND,
        width: u32,
        height: u32,
        opaque: bool,
        flavor: Flavor,
        font: &FontConfig,
    ) -> anyhow::Result<Self> {
        // DCompositionCreateDevice2 (not v1) accepts None for dxgiDevice, letting wgpu
        // own its own DXGI device and swap chain internally.
        let dcomp_device: IDCompositionDevice = unsafe { DCompositionCreateDevice2(None)? };
        // topmost = true is conventional for DComp overlays. With WS_EX_NOREDIRECTIONBITMAP
        // there is no GDI surface, so the value is irrelevant.
        let dcomp_target = unsafe { dcomp_device.CreateTargetForHwnd(hwnd, true)? };
        let dcomp_visual = unsafe { dcomp_device.CreateVisual()? };

        // SurfaceTargetUnsafe::CompositionVisual is #[cfg(dx12)] in wgpu 25. It does not
        // appear on docs.rs (Linux build), but compiles on Windows with the dx12 feature.
        let target = wgpu::SurfaceTargetUnsafe::CompositionVisual(dcomp_visual.as_raw() as *mut _);
        let surface = unsafe { instance.create_surface_unsafe(target)? };

        unsafe { dcomp_target.SetRoot(&dcomp_visual)? };

        let alpha_mode = if opaque {
            wgpu::CompositeAlphaMode::Opaque
        } else {
            // PreMultiplied maps to DXGI_ALPHA_MODE_PREMULTIPLIED, giving native
            // per-pixel alpha compositing through DComp without DWM blur-behind hacks.
            wgpu::CompositeAlphaMode::PreMultiplied
        };
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            width: width.max(1),
            height: height.max(1),
            // Immediate matches the previous SwapInterval::DontWait -- no vsync wait.
            // Overlays render on-demand, not in a loop.
            present_mode: wgpu::PresentMode::Immediate,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);
        // Commit after configure: wgpu calls SetContent(swap_chain) inside configure(),
        // so Commit must come after for DWM to see the visual with its content.
        unsafe { dcomp_device.Commit()? };

        let painter = egui_wgpu::Renderer::new(
            &device,
            surface_config.format,
            None,  // no depth format
            1,     // msaa_samples
            false, // dithering
        );

        // Disable selectable labels so clicks on tab bars register as tab switches
        // instead of triggering egui's text selection behavior.
        let egui_ctx = egui::Context::default(); // only egui context in this overlay
        egui_ctx.style_mut(|s| s.interaction.selectable_labels = false);
        catppuccin_egui::set_theme(&egui_ctx, flavor.catppuccin_egui());
        font.apply_to(&egui_ctx);

        Ok(Self {
            surface,
            surface_config,
            device,
            queue,
            painter,
            egui_ctx,
            _dcomp_visual: dcomp_visual,
            _dcomp_target: dcomp_target,
            dcomp_device,
            opaque,
        })
    }

    pub(super) fn resize(&mut self, width: u32, height: u32) {
        self.surface_config.width = width.max(1);
        self.surface_config.height = height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
        // configure() may create a new swap chain and call SetContent again,
        // which requires a Commit for DWM to pick up the change.
        unsafe { self.dcomp_device.Commit() }.expect("DComp commit after resize");
    }

    pub(super) fn set_visuals(&self, visuals: egui::Visuals) {
        self.egui_ctx.set_visuals(visuals);
    }

    pub(super) fn apply_theme(&self, flavor: Flavor) {
        catppuccin_egui::set_theme(&self.egui_ctx, flavor.catppuccin_egui());
    }

    pub(super) fn apply_font(&self, font: &FontConfig) {
        font.apply_to(&self.egui_ctx);
    }

    pub(super) fn render<R>(
        &mut self,
        width: u32,
        height: u32,
        pixels_per_point: f32,
        events: Vec<egui::Event>,
        mut ctx_fn: impl FnMut(&egui::Context) -> R,
    ) -> R {
        let frame = self.surface.get_current_texture().expect("surface texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default()); // default view of the surface texture

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
                    ..Default::default() // remaining ViewportInfo fields not needed for overlay rendering
                },
            ))
            .collect(),
            events,
            ..Default::default() // remaining RawInput fields (focused, max_texture_side, etc.) not needed for overlay rendering
        };

        let mut result = None;
        let output = self.egui_ctx.run(raw_input, |ctx| {
            result = Some(ctx_fn(ctx));
        });
        let meshes = self
            .egui_ctx
            .tessellate(output.shapes, output.pixels_per_point);

        let screen = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [width, height],
            pixels_per_point: output.pixels_per_point,
        };

        for (id, delta) in &output.textures_delta.set {
            self.painter
                .update_texture(&self.device, &self.queue, *id, delta);
        }

        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        let user_cmds =
            self.painter
                .update_buffers(&self.device, &self.queue, &mut encoder, &meshes, &screen);

        {
            let clear_color = wgpu::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: if self.opaque { 1.0 } else { 0.0 },
            };
            let rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default() // no occlusion query, no timestamp writes
            });
            // forget_lifetime() is required because egui_wgpu::Renderer::render
            // needs a RenderPass with 'static lifetime.
            self.painter
                .render(&mut rpass.forget_lifetime(), &meshes, &screen);
            // rpass dropped here before encoder.finish()
        }

        self.queue.submit(
            user_cmds
                .into_iter()
                .chain(std::iter::once(encoder.finish())),
        );
        frame.present();

        for id in &output.textures_delta.free {
            self.painter.free_texture(id);
        }

        result.unwrap()
    }
}

pub(in crate::platform::windows) const TILING_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomeTilingOverlay");

/// Per-monitor overlay that draws all tiling window borders and container tab bars.
/// `renderer` is declared before `window` so it drops first.
pub(in crate::platform::windows) struct TilingOverlay {
    renderer: Renderer,
    events: Vec<egui::Event>,
    monitor: Dimension,
    windows: Vec<TilingWindowPlacement>,
    containers: Vec<(ContainerPlacement, Vec<String>)>,
    config: Config,
    hub_sender: HubSender,
    window: OwnedHwnd,
    scale: f32,
}

impl TilingOverlay {
    pub(in crate::platform::windows) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: Config,
        hub_sender: HubSender,
        monitor: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<Self>> {
        let flavor = config.theme;
        let font = &config.font;
        let window = OwnedHwnd::new(TILING_OVERLAY_CLASS, WS_EX_TOOLWINDOW)?;
        let hwnd = window.hwnd();
        // Initialize the wgpu surface at the monitor's physical size so the
        // overlay is ready to render without a preceding update() call.
        // Monitor dimensions are already physical under the agnostic-core
        // design, so this is a cast-only conversion (same as update()).
        let (_x, _y, init_w, init_h) = dpi::surface_size_from_physical(monitor);
        let init_w = init_w.max(1);
        let init_h = init_h.max(1);
        let renderer = Renderer::new(
            instance, device, queue, hwnd, init_w, init_h, false, flavor, font,
        )?;
        let mut boxed = Box::new(Self {
            renderer,
            events: Vec::new(),
            monitor,
            windows: Vec::new(),
            containers: Vec::new(),
            config,
            hub_sender,
            window,
            scale,
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };
        Ok(boxed)
    }

    fn rerender(&mut self) {
        let monitor = self.monitor;
        let config = &self.config;
        let events = std::mem::take(&mut self.events);
        let w_phys = dpi::logical_to_physical(self.monitor.width, self.scale) as u32;
        let h_phys = dpi::logical_to_physical(self.monitor.height, self.scale) as u32;
        let clicked_tabs = self
            .renderer
            .render(w_phys, h_phys, self.scale, events, |ctx| {
                overlay::paint_tiling_overlay(ctx, monitor, &self.windows, &self.containers, config)
            });
        for (container_id, tab_idx) in clicked_tabs {
            self.hub_sender
                .send(HubEvent::TabClicked(container_id, tab_idx));
        }
    }
}

impl TilingOverlayApi for TilingOverlay {
    fn update(
        &mut self,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        containers: &[(ContainerPlacement, Vec<String>)],
        scale: f32,
    ) {
        let (x_phys, y_phys, w_phys, h_phys) = dpi::surface_size_from_physical(monitor);
        // Floor at 1 to avoid zero-sized wgpu surface validation.
        let w_phys = w_phys.max(1);
        let h_phys = h_phys.max(1);

        if self.monitor != monitor {
            self.renderer.resize(w_phys, h_phys);
            unsafe {
                SetWindowPos(
                    self.window.hwnd(),
                    None,
                    x_phys,
                    y_phys,
                    w_phys as i32,
                    h_phys as i32,
                    SWP_NOACTIVATE | SWP_NOREDRAW | SWP_NOZORDER,
                )
                .ok();
            }
            self.window.show();
        }

        // All state assignments must precede rerender(), which derives
        // physical dimensions from monitor + scale.
        self.monitor = monitor;
        self.windows = windows.to_vec();
        self.containers = containers.to_vec();
        self.scale = scale;
        self.rerender();
    }

    fn clear(&mut self) {
        self.windows.clear();
        self.containers.clear();
        // Render a transparent frame so the overlay becomes invisible.
        // No region clipping needed: the overlay sits behind managed windows.
        self.rerender();
    }

    fn set_config(&mut self, config: Config) {
        self.config = config;
    }

    fn apply_theme(&mut self, flavor: Flavor) {
        self.renderer.apply_theme(flavor);
    }

    fn apply_font(&mut self, font: &FontConfig) {
        self.renderer.apply_font(font);
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
    if msg == WM_ERASEBKGND {
        return LRESULT(1);
    }
    // WM_DPICHANGED is per-window. Duplicate posts from multiple Dome
    // wnd-procs on the same monitor are absorbed by monitor_dpi_changed.
    if msg == WM_DPICHANGED {
        let dpi = (wparam.0 & 0xFFFF) as u32;
        let handle = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }.0 as isize;
        unsafe {
            PostThreadMessageW(
                GetCurrentThreadId(),
                WM_APP_DPI_CHANGE,
                WPARAM(dpi as usize),
                LPARAM(handle),
            )
            .ok()
        };
        return LRESULT(0);
    }
    if msg == WM_GETDPISCALEDSIZE {
        let mut rect = RECT::default();
        unsafe { GetClientRect(hwnd, &mut rect).ok() };
        let size = SIZE {
            cx: rect.right - rect.left,
            cy: rect.bottom - rect.top,
        };
        let out = lparam.0 as *mut SIZE;
        unsafe { *out = crate::platform::windows::wm_getdpiscaledsize_reply(size) };
        return LRESULT(1);
    }
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
            overlay.rerender();
            LRESULT(0)
        }
        WM_PAINT => {
            unsafe {
                let mut ps = PAINTSTRUCT::default();
                BeginPaint(hwnd, &mut ps);
                EndPaint(hwnd, &ps).ok().ok();
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub(in crate::platform::windows) trait FloatOverlayApi {
    fn update(&mut self, wp: &FloatWindowPlacement, config: &Config, z: ZOrder, scale: f32);
    fn hide(&mut self);
    // &mut self keeps the receiver consistent with the other trait
    // methods; apply_theme only needs &self on the underlying Renderer.
    fn apply_theme(&mut self, flavor: Flavor);
    fn apply_font(&mut self, font: &FontConfig);
}

pub(in crate::platform::windows) trait TilingOverlayApi {
    fn update(
        &mut self,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        containers: &[(ContainerPlacement, Vec<String>)],
        scale: f32,
    );
    fn clear(&mut self);
    fn set_config(&mut self, config: Config);
    // &mut self keeps the receiver consistent with the other trait
    // methods; apply_theme only needs &self on the underlying Renderer.
    fn apply_theme(&mut self, flavor: Flavor);
    fn apply_font(&mut self, font: &FontConfig);
}

pub(in crate::platform::windows) trait PickerApi {
    fn show(&mut self, entries: Vec<PickerEntry>, monitor_dim: Dimension, scale: f32);
    fn hide(&mut self);
    fn is_visible(&self) -> bool;
    fn icons_to_load(
        &mut self,
        lookup_hwnd: &dyn Fn(WindowId) -> Option<HwndId>,
    ) -> Vec<(String, HwndId)>;
    fn receive_icon(&mut self, app_id: String, image: egui::ColorImage);
    fn rerender(&mut self);
}

pub(in crate::platform::windows) const FLOAT_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomeFloatOverlay");

struct FloatOverlay {
    renderer: Renderer,
    // Physical-pixel cache for the last `SetWindowPos` / `renderer.resize`.
    // Floored at 1 in `new` to avoid zero-sized wgpu surface validation.
    width_phys: u32,
    height_phys: u32,
    window: OwnedHwnd,
}

impl FloatOverlay {
    fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        flavor: Flavor,
        font: &FontConfig,
        width_phys: u32,
        height_phys: u32,
    ) -> anyhow::Result<Self> {
        // Floor at 1 to avoid zero-sized wgpu surface validation.
        let width_phys = width_phys.max(1);
        let height_phys = height_phys.max(1);
        let window = OwnedHwnd::new(FLOAT_OVERLAY_CLASS, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)?;
        let renderer = Renderer::new(
            instance,
            device,
            queue,
            window.hwnd(),
            width_phys,
            height_phys,
            false,
            flavor,
            font,
        )?;
        Ok(Self {
            renderer,
            width_phys,
            height_phys,
            window,
        })
    }
}

impl FloatOverlayApi for FloatOverlay {
    fn update(&mut self, wp: &FloatWindowPlacement, config: &Config, z: ZOrder, scale: f32) {
        let vf = wp.visible_frame;
        let (x_phys, y_phys, w_phys, h_phys) = dpi::surface_size_from_physical(vf);
        // Floor at 1 to avoid zero-sized wgpu surface validation.
        let w_phys = w_phys.max(1);
        let h_phys = h_phys.max(1);

        if self.width_phys != w_phys || self.height_phys != h_phys {
            self.renderer.resize(w_phys, h_phys);
            self.width_phys = w_phys;
            self.height_phys = h_phys;
        }

        // ORDERING INVARIANT: SetWindowPos, show, render.
        let z_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE | SWP_NOREDRAW;
        if z_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                z_after,
                x_phys,
                y_phys,
                w_phys as i32,
                h_phys as i32,
                flags,
            )
            .ok();
        }

        // Show before render so the window is visible when the first frame presents.
        self.window.show();

        self.renderer.render(w_phys, h_phys, scale, vec![], |ctx| {
            // layer_painter bypasses egui's Area sizing pass, avoiding
            // black/invisible borders on the first frame.
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("border"),
            ));
            let clip =
                egui::Rect::from_min_size(egui::pos2(0.0, 0.0), egui::vec2(vf.width, vf.height));
            overlay::paint_window_border(
                &painter.with_clip_rect(clip),
                wp.frame,
                wp.visible_frame,
                wp.is_highlighted,
                None,
                config,
                egui::vec2(0.0, 0.0),
            );
        });
    }

    fn hide(&mut self) {
        self.window.hide();
    }

    fn apply_theme(&mut self, flavor: Flavor) {
        self.renderer.apply_theme(flavor);
    }

    fn apply_font(&mut self, font: &FontConfig) {
        self.renderer.apply_font(font);
    }
}

pub(in crate::platform::windows) struct WgpuOverlayFactory {
    pub(in crate::platform::windows) instance: wgpu::Instance,
    pub(in crate::platform::windows) device: Arc<wgpu::Device>,
    pub(in crate::platform::windows) queue: Arc<wgpu::Queue>,
    pub(in crate::platform::windows) hub_sender: HubSender,
}

impl CreateOverlay for WgpuOverlayFactory {
    fn create_tiling_overlay(
        &self,
        config: Config,
        monitor: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        Ok(TilingOverlay::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            config,
            self.hub_sender.clone(),
            monitor,
            scale,
        )?)
    }
    fn create_float_overlay(
        &self,
        flavor: crate::theme::Flavor,
        font: &crate::font::FontConfig,
        _scale: f32,
        visible_frame: Dimension,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        let (_, _, w_phys, h_phys) = dpi::surface_size_from_physical(visible_frame);
        Ok(Box::new(FloatOverlay::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            flavor,
            font,
            w_phys,
            h_phys,
        )?))
    }
    fn create_picker(
        &self,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        flavor: crate::theme::Flavor,
        font: &crate::font::FontConfig,
        scale: f32,
    ) -> anyhow::Result<Box<dyn PickerApi>> {
        Ok(picker::PickerWindow::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            entries,
            monitor_dim,
            self.hub_sender.clone(),
            flavor,
            font,
            scale,
        )?)
    }
}
