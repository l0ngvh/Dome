// Coordinate systems: wgpu surface sizing (SurfaceConfiguration.width/height) and SetWindowPos
// use physical pixels (cached as width_phys/height_phys via Dimension<Physical>::to_surface_size).
// The overlay paint boundary (TilingOverlay::rerender, FloatOverlay::update) projects physical
// Dimensions via .to_logical(scale) and passes
// pixels_per_point = scale so egui rescales back to physical. BorderMetrics/OverlayMetrics pass
// through unscaled -- never pre-multiply thickness/radius/tab-bar-height here.

use std::sync::Arc;

use crate::config::Config;
use crate::font::FontConfig;
use crate::platform::windows::{HubEvent, HubSender, WM_APP_DISPLAY_CHANGE};
use crate::theme::Flavor;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDevice, IDCompositionTarget, IDCompositionVisual,
};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, GW_HWNDPREV, GWLP_USERDATA, GetWindow,
    GetWindowLongPtrW, HWND_BOTTOM, HWND_TOP, MA_NOACTIVATE, PostThreadMessageW, SW_HIDE,
    SW_SHOWNA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOREDRAW, SWP_NOSIZE, SWP_NOZORDER, SWP_SHOWWINDOW,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, WINDOW_EX_STYLE, WM_DISPLAYCHANGE, WM_LBUTTONDOWN,
    WM_LBUTTONUP, WM_MOUSEACTIVATE, WM_MOUSEMOVE, WM_PAINT, WS_EX_LAYERED, WS_EX_NOACTIVATE,
    WS_EX_NOREDIRECTIONBITMAP, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::{Interface, PCWSTR};

use crate::core::{
    ContainerId, ContainerPlacement, Dimension, FloatWindowPlacement, Length, Logical, Physical,
    TilingWindowPlacement, WindowId,
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
    /// `WS_EX_NOREDIRECTIONBITMAP` is force-OR'd in because every Dome overlay
    /// uses DirectComposition and must suppress the GDI redirection bitmap.
    /// Click-through (`WS_EX_LAYERED | WS_EX_TRANSPARENT`) is per-call: only
    /// the tiling overlay opts in. Force-OR'ing it here would silently route
    /// pointer events past the picker (which needs keyboard and mouse) and
    /// the float overlay (which sits inside its window's pointer band).
    pub(super) fn new(
        class: PCWSTR,
        ex_style: WINDOW_EX_STYLE,
        x: i32,
        y: i32,
        width: u32,
        height: u32,
    ) -> anyhow::Result<Self> {
        let hwnd = unsafe {
            CreateWindowExW(
                ex_style | WS_EX_NOREDIRECTIONBITMAP,
                class,
                windows::core::w!(""),
                WS_POPUP,
                x,
                y,
                width as i32,
                height as i32,
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
}

impl Renderer {
    #[expect(clippy::too_many_arguments, reason = "TODO: refactor")]
    pub(super) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        hwnd: HWND,
        width: u32,
        height: u32,
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

        // PreMultiplied maps to DXGI_ALPHA_MODE_PREMULTIPLIED, giving native
        // per-pixel alpha compositing through DComp without DWM blur-behind hacks.
        let alpha_mode = wgpu::CompositeAlphaMode::PreMultiplied;
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
        if let Some(family) = font.family.as_deref() {
            match crate::platform::windows::font::resolve_system_font(family) {
                Ok(bytes) => crate::font::install_fonts(bytes, &egui_ctx),
                Err(e) => tracing::warn!(
                    family = %family,
                    error = %e,
                    "font resolution failed; using egui defaults"
                ),
            }
        }
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

    pub(super) fn egui_ctx(&self) -> &egui::Context {
        &self.egui_ctx
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
                a: 0.0,
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

pub(in crate::platform::windows) const TAB_BAR_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomeTabBarOverlay");

/// Per-monitor overlay that draws all tiling window borders and container tab bars.
/// `renderer` is declared before `window` so it drops first.
pub(in crate::platform::windows) struct TilingOverlay {
    renderer: Renderer,
    monitor: Dimension,
    // Physical-pixel cache for the last `surface_size_from_physical` result.
    width_phys: u32,
    height_phys: u32,
    windows: Vec<TilingWindowPlacement>,
    containers: Vec<(ContainerPlacement, Vec<String>)>,
    config: Config,
    tab_bar_height: Length<Logical>,
    window: OwnedHwnd,
    scale: f32,
}

impl TilingOverlay {
    pub(in crate::platform::windows) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: Config,
        tab_bar_height: Length<Logical>,
        monitor: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<Self>> {
        let flavor = config.theme;
        let font = &config.font;
        // Initialize the wgpu surface at the monitor's physical size so the
        // overlay is ready to render without a preceding update() call.
        // Monitor dimensions are already physical under the agnostic-core
        // design, so this is a cast-only conversion (same as update()).
        let (x_phys, y_phys, init_w, init_h) = monitor.to_surface_size();
        // WS_EX_NOACTIVATE prevents DefWindowProcW from returning MA_ACTIVATE on clicks,
        // stopping the overlay from being raised above managed windows by user input.
        // WS_EX_LAYERED | WS_EX_TRANSPARENT keeps the tiling overlay
        // click-through so pointer events reach managed windows below.
        let mut window = OwnedHwnd::new(
            TILING_OVERLAY_CLASS,
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_TRANSPARENT,
            x_phys,
            y_phys,
            init_w,
            init_h,
        )?;
        let hwnd = window.hwnd();
        let renderer = Renderer::new(instance, device, queue, hwnd, init_w, init_h, flavor, font)?;
        window.show();
        // Park the overlay at HWND_BOTTOM immediately after creation. Managed
        // windows created after this (via CreateWindowExW) naturally land above
        // it. Z-order is maintained thereafter by show_tiling's per-window lift
        // on transitions into the visible band.
        unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_BOTTOM),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .ok();
        }
        let mut boxed = Box::new(Self {
            renderer,
            monitor,
            width_phys: init_w,
            height_phys: init_h,
            windows: Vec::new(),
            containers: Vec::new(),
            config,
            tab_bar_height,
            window,
            scale,
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };
        Ok(boxed)
    }

    fn rerender(&mut self) {
        let scale = self.scale;
        let monitor_logical = self.monitor.to_logical(scale);
        let windows_logical: Vec<overlay::LogicalTiledWindow> = self
            .windows
            .iter()
            .map(|wp| overlay::LogicalTiledWindow {
                id: wp.id,
                frame: wp.frame.to_logical(scale),
                visible_frame: wp.visible_frame.to_logical(scale),
                is_highlighted: wp.is_highlighted,
                spawn_indicator: wp.spawn_indicator,
            })
            .collect();
        let containers_logical: Vec<overlay::LogicalTiledContainer> = self
            .containers
            .iter()
            .map(|(cp, titles)| overlay::LogicalTiledContainer {
                id: cp.id,
                frame: cp.frame.to_logical(scale),
                visible_frame: cp.visible_frame.to_logical(scale),
                is_highlighted: cp.is_highlighted,
                spawn_indicator: cp.spawn_indicator,
                is_tabbed: cp.is_tabbed,
                titles: titles.clone(),
            })
            .collect();
        let config = &self.config;
        let theme = config.theme();
        let metrics = overlay::OverlayMetrics {
            border: overlay::BorderMetrics::from_thickness(Length::<Logical>::new(
                config.border_size,
            )),
            tab_bar_height: self.tab_bar_height,
        };
        let w_phys = self.width_phys;
        let h_phys = self.height_phys;
        // Borders-only mode: tab bars live in dedicated per-container windows,
        // so the per-monitor overlay never sees pointer events. The returned
        // click vector is always empty.
        let _ = self.renderer.render(w_phys, h_phys, scale, vec![], |ctx| {
            overlay::paint_tiling_overlay(
                ctx,
                monitor_logical,
                &windows_logical,
                &containers_logical,
                &theme,
                metrics,
            )
        });
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
        let (x_phys, y_phys, w_phys, h_phys) = monitor.to_surface_size();

        if self.monitor != monitor {
            self.renderer.resize(w_phys, h_phys);
            unsafe {
                SetWindowPos(
                    self.window.hwnd(),
                    Some(HWND_BOTTOM),
                    x_phys,
                    y_phys,
                    w_phys as i32,
                    h_phys as i32,
                    SWP_NOACTIVATE | SWP_NOREDRAW,
                )
                .ok();
            }
            self.window.show();
        }
        // Same-monitor path: no SetWindowPos. Z-order is restored by the
        // per-window lift in show_tiling whenever a tiling window enters the
        // visible band from Float or Offscreen (or unminimizes via the flag).

        // All state assignments must precede rerender(), which reads cached
        // physical dimensions.
        self.monitor = monitor;
        self.width_phys = w_phys;
        self.height_phys = h_phys;
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

    fn set_config(&mut self, config: &Config) {
        if self.config.theme != config.theme {
            self.renderer.apply_theme(config.theme);
        }
        if self.config.font != config.font {
            if self.config.font.family != config.font.family {
                self.reinstall_fonts(config.font.family.as_deref());
            }
            self.renderer.apply_font(&config.font);
        }
        self.config = config.clone();
    }

    fn set_tab_bar_height(&mut self, height: Length<Logical>) {
        self.tab_bar_height = height;
    }

    fn window_above(&self) -> Option<HwndId> {
        let prev = unsafe { GetWindow(self.window.hwnd(), GW_HWNDPREV) }.ok();
        prev.map(HwndId::from)
    }

    fn demote_below(&mut self, managed: HwndId) {
        let target: HWND = managed.into();
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                Some(target),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
            .ok();
        }
    }

    fn focus(&self) {
        crate::platform::windows::handle::force_set_foreground(self.window.hwnd());
    }
}

impl TilingOverlay {
    fn reinstall_fonts(&mut self, family: Option<&str>) {
        if let Some(family) = family {
            match crate::platform::windows::font::resolve_system_font(family) {
                Ok(bytes) => crate::font::install_fonts(bytes, &self.renderer.egui_ctx),
                Err(e) => tracing::warn!(
                    family = %family,
                    error = %e,
                    "font reload failed"
                ),
            }
        }
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
    if let Some(lr) = crate::platform::windows::dome_wnd_proc_common(hwnd, msg, wparam, lparam) {
        return lr;
    }
    // One tiling overlay per monitor, so a WM_DISPLAYCHANGE broadcast can
    // produce multiple posts. The WM_APP_DISPLAY_CHANGE handler re-enumerates
    // monitors and is idempotent, mirroring the duplicate-post note on the
    // common helper's WM_DPICHANGED arm.
    if msg == WM_DISPLAYCHANGE {
        unsafe {
            PostThreadMessageW(
                GetCurrentThreadId(),
                WM_APP_DISPLAY_CHANGE,
                WPARAM(0),
                LPARAM(0),
            )
            .ok()
        };
        return LRESULT(0);
    }
    // Belt-and-braces guard: WS_EX_LAYERED + WS_EX_TRANSPARENT already routes
    // pointer events past the overlay, but the active-window-tracking
    // accessibility path can still dispatch WM_MOUSEACTIVATE. MA_NOACTIVATE
    // here keeps that rare path from raising the overlay above managed
    // windows. Placed before the USERDATA read because WM_MOUSEACTIVATE can
    // arrive during window creation before USERDATA is written.
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    // No USERDATA deref here: rendering is driven by RedrawWindow from the
    // dispatcher, so WM_PAINT only needs to satisfy the Begin/EndPaint contract.
    if msg == WM_PAINT {
        unsafe {
            let mut ps = PAINTSTRUCT::default();
            BeginPaint(hwnd, &mut ps);
            EndPaint(hwnd, &ps).ok().ok();
        }
        return LRESULT(0);
    }
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

pub(in crate::platform::windows) trait FloatOverlayApi {
    fn update(&mut self, wp: &FloatWindowPlacement, config: &Config, z: ZOrder, scale: f32);
    fn hide(&mut self);
    fn set_config(&mut self, config: &Config);
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
    fn set_config(&mut self, config: &Config);
    fn set_tab_bar_height(&mut self, height: Length<Logical>);
    /// Pulls keyboard focus to this monitor's overlay HWND. The Win32
    /// close-time focus walk lands here when the user closes a managed
    /// window with no obvious successor on the same monitor, replacing the
    /// process-wide focus-sink window the platform shell used to keep below
    /// every overlay. The overlay HWND is `WS_EX_TRANSPARENT`, so claiming
    /// foreground does not take pointer events away from anything below.
    fn focus(&self);
    /// Returns the HWND sitting directly above this overlay in z-order.
    /// Wraps `GetWindow(GW_HWNDPREV)` in production; used by `show_tiling`
    /// to slot tiling windows above the overlay on band transitions.
    fn window_above(&self) -> Option<HwndId>;
    /// Demotes the overlay below `managed` via a z-only `SetWindowPos`.
    /// Fallback for when `window_above()` returns None (overlay at top).
    fn demote_below(&mut self, managed: HwndId);
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
    fn set_config(&mut self, config: &Config);
}

pub(in crate::platform::windows) const FLOAT_OVERLAY_CLASS: PCWSTR =
    windows::core::w!("DomeFloatOverlay");

pub(in crate::platform::windows) struct FloatOverlay {
    renderer: Renderer,
    // Physical-pixel cache for the last `SetWindowPos` / `renderer.resize`.
    // Asserted positive on construction and update (zero would be a logic bug).
    width_phys: u32,
    height_phys: u32,
    window: OwnedHwnd,
    config: Config,
}

impl FloatOverlay {
    #[expect(
        clippy::too_many_arguments,
        reason = "x, y added for birth-at-rect invariant; restructuring deferred"
    )]
    fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: Config,
        x: i32,
        y: i32,
        width_phys: u32,
        height_phys: u32,
    ) -> anyhow::Result<Box<Self>> {
        let window = OwnedHwnd::new(
            FLOAT_OVERLAY_CLASS,
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            x,
            y,
            width_phys,
            height_phys,
        )?;
        let hwnd = window.hwnd();
        let renderer = Renderer::new(
            instance,
            device,
            queue,
            hwnd,
            width_phys,
            height_phys,
            config.theme,
            &config.font,
        )?;
        let boxed = Box::new(Self {
            renderer,
            width_phys,
            height_phys,
            window,
            config,
        });
        Ok(boxed)
    }
}

impl FloatOverlayApi for FloatOverlay {
    fn update(&mut self, wp: &FloatWindowPlacement, config: &Config, z: ZOrder, scale: f32) {
        let vf = wp.visible_frame;
        let (x_phys, y_phys, w_phys, h_phys) = vf.to_surface_size();

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

        let vf_logical = vf.to_logical(scale);
        let frame_logical = wp.frame.to_logical(scale);
        let theme = config.theme();
        let border =
            overlay::BorderMetrics::from_thickness(Length::<Logical>::new(config.border_size));
        let is_highlighted = wp.is_highlighted;

        self.renderer.render(w_phys, h_phys, scale, vec![], |ctx| {
            // layer_painter bypasses egui's Area sizing pass, avoiding
            // black/invisible borders on the first frame.
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("border"),
            ));
            let clip = egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(vf_logical.width.logical(), vf_logical.height.logical()),
            );
            overlay::paint_window_border(
                &painter.with_clip_rect(clip),
                frame_logical,
                vf_logical,
                is_highlighted,
                None,
                &theme,
                border,
                egui::vec2(0.0, 0.0),
            );
        });
    }

    fn hide(&mut self) {
        self.window.hide();
    }

    fn set_config(&mut self, config: &Config) {
        if self.config.theme != config.theme {
            self.renderer.apply_theme(config.theme);
        }
        if self.config.font != config.font {
            if self.config.font.family != config.font.family {
                self.reinstall_fonts(config.font.family.as_deref());
            }
            self.renderer.apply_font(&config.font);
        }
        self.config = config.clone();
    }
}

impl FloatOverlay {
    fn reinstall_fonts(&mut self, family: Option<&str>) {
        if let Some(family) = family {
            match crate::platform::windows::font::resolve_system_font(family) {
                Ok(bytes) => crate::font::install_fonts(bytes, &self.renderer.egui_ctx),
                Err(e) => tracing::warn!(
                    family = %family,
                    error = %e,
                    "font reload failed"
                ),
            }
        }
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
        tab_bar_height: Length<Logical>,
        monitor: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        Ok(TilingOverlay::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            config,
            tab_bar_height,
            monitor,
            scale,
        )?)
    }
    fn create_float_overlay(
        &self,
        config: Config,
        _scale: f32,
        visible_frame: Dimension,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        let (x_phys, y_phys, w_phys, h_phys) = visible_frame.to_surface_size();
        Ok(FloatOverlay::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            config,
            x_phys,
            y_phys,
            w_phys,
            h_phys,
        )?)
    }
    fn create_picker(
        &self,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        config: Config,
        scale: f32,
    ) -> anyhow::Result<Box<dyn PickerApi>> {
        Ok(picker::PickerWindow::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            entries,
            monitor_dim,
            self.hub_sender.clone(),
            config,
            scale,
        )?)
    }
    fn create_tab_bar(
        &self,
        config: Config,
        container_id: ContainerId,
        rect: Dimension,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TabBarOverlayApi>> {
        Ok(TabBarOverlay::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            config,
            container_id,
            rect,
            scale,
            self.hub_sender.clone(),
        )?)
    }
}

/// Windows-only conversions on physical-pixel dimensions for the wgpu/egui overlay pipeline.
trait PhysicalDimensionExt {
    fn to_logical(self, scale: f32) -> Dimension<Logical>;
    fn to_surface_size(self) -> (i32, i32, u32, u32);
}

impl PhysicalDimensionExt for Dimension<Physical> {
    fn to_logical(self, scale: f32) -> Dimension<Logical> {
        debug_assert!(scale > 0.0, "scale must be positive, got {scale}");
        Dimension::new(
            Length::new(self.x.value() / scale),
            Length::new(self.y.value() / scale),
            Length::new(self.width.value() / scale),
            Length::new(self.height.value() / scale),
        )
    }

    fn to_surface_size(self) -> (i32, i32, u32, u32) {
        let w = self.width.round().value() as u32;
        let h = self.height.round().value() as u32;
        assert!(
            w > 0 && h > 0,
            "overlay surface size must be positive; got {w}x{h}"
        );
        (
            self.x.round().value() as i32,
            self.y.round().value() as i32,
            w,
            h,
        )
    }
}

pub(in crate::platform::windows) trait TabBarOverlayApi {
    fn update(
        &mut self,
        rect: Dimension,
        titles: Vec<String>,
        active_index: usize,
        is_highlighted: bool,
        scale: f32,
    );
    #[expect(
        dead_code,
        reason = "hide() is invoked when a tabbed container's active window minimizes. Wired up in the follow-up minimize/restore pass."
    )]
    fn hide(&mut self);
    fn set_config(&mut self, config: &Config);
}

/// Per-container window that owns its tab bar's pixels and pointer events.
/// `renderer` is declared before `window` so it drops first while the HWND is
/// still valid (mirrors `TilingOverlay` and `FloatOverlay`).
pub(in crate::platform::windows) struct TabBarOverlay {
    renderer: Renderer,
    events: Vec<egui::Event>,
    container_id: ContainerId,
    width_phys: u32,
    height_phys: u32,
    titles: Vec<String>,
    active_index: usize,
    is_highlighted: bool,
    config: Config,
    hub_sender: HubSender,
    window: OwnedHwnd,
    scale: f32,
    // First update positions and shows; later updates skip SWP_SHOWWINDOW so a
    // hide() does not get clobbered by the next paint pass.
    placed: bool,
}

impl TabBarOverlay {
    #[expect(
        clippy::too_many_arguments,
        reason = "wgpu handles, identity, geometry, and the hub sender all travel together at construction"
    )]
    pub(in crate::platform::windows) fn new(
        instance: &wgpu::Instance,
        device: Arc<wgpu::Device>,
        queue: Arc<wgpu::Queue>,
        config: Config,
        container_id: ContainerId,
        rect: Dimension,
        scale: f32,
        hub_sender: HubSender,
    ) -> anyhow::Result<Box<Self>> {
        let (x_phys, y_phys, w_phys, h_phys) = rect.to_surface_size();
        // WS_EX_NOACTIVATE prevents foreground theft on click. Tab clicks are
        // dispatched as `HubEvent::TabClicked`, not by raising the window.
        let window = OwnedHwnd::new(
            TAB_BAR_OVERLAY_CLASS,
            WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            x_phys,
            y_phys,
            w_phys,
            h_phys,
        )?;
        let hwnd = window.hwnd();
        let renderer = Renderer::new(
            instance,
            device,
            queue,
            hwnd,
            w_phys,
            h_phys,
            config.theme,
            &config.font,
        )?;
        let mut boxed = Box::new(Self {
            renderer,
            events: Vec::new(),
            container_id,
            width_phys: w_phys,
            height_phys: h_phys,
            titles: Vec::new(),
            active_index: 0,
            is_highlighted: false,
            config,
            hub_sender,
            window,
            scale,
            placed: false,
        });
        unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, &mut *boxed as *mut Self as isize) };
        Ok(boxed)
    }

    /// Renders one frame and returns `Some((container_id, tab_index))` when a
    /// click landed on a tab. Caller dispatches the click via the hub sender.
    fn rerender(&mut self) -> Option<(ContainerId, usize)> {
        let events = std::mem::take(&mut self.events);
        let scale = self.scale;
        let w_phys = self.width_phys;
        let h_phys = self.height_phys;
        let titles = self.titles.clone();
        let active_index = self.active_index;
        let is_highlighted = self.is_highlighted;
        let container_id = self.container_id;
        let config = &self.config;
        let theme = config.theme();
        // Canvas is the full bar, so paint_tab_bar's own height drives the
        // metric; otherwise the highlight underline math would diverge from
        // the bar's actual logical height.
        let bar_h_logical = Length::<Logical>::new(h_phys as f32 / scale);
        let bar_w_logical = Length::<Logical>::new(w_phys as f32 / scale);
        let metrics = overlay::OverlayMetrics {
            border: overlay::BorderMetrics::from_thickness(Length::<Logical>::new(
                config.border_size,
            )),
            tab_bar_height: bar_h_logical,
        };
        let canvas_local =
            Dimension::<Logical>::new(Length::ZERO, Length::ZERO, bar_w_logical, bar_h_logical);
        self.renderer.render(w_phys, h_phys, scale, events, |ctx| {
            overlay::paint_tab_bar(
                ctx,
                container_id,
                canvas_local,
                &titles,
                active_index,
                is_highlighted,
                metrics,
                &theme,
            )
        })
    }
}

impl TabBarOverlayApi for TabBarOverlay {
    fn update(
        &mut self,
        rect: Dimension,
        titles: Vec<String>,
        active_index: usize,
        is_highlighted: bool,
        scale: f32,
    ) {
        self.titles = titles;
        self.active_index = active_index;
        self.is_highlighted = is_highlighted;
        self.scale = scale;
        let (x_phys, y_phys, w_phys, h_phys) = rect.to_surface_size();
        if w_phys != self.width_phys || h_phys != self.height_phys {
            self.renderer.resize(w_phys, h_phys);
            self.width_phys = w_phys;
            self.height_phys = h_phys;
        }
        let mut flags = SWP_NOACTIVATE | SWP_SHOWWINDOW;
        if self.placed {
            // Z-order is owned by show_tiling's per-window lift on band
            // transitions. Subsequent SetWindowPos calls must not perturb it.
            flags |= SWP_NOZORDER;
        }
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                Some(HWND_TOP),
                x_phys,
                y_phys,
                w_phys as i32,
                h_phys as i32,
                flags,
            )
            .ok();
        }
        self.placed = true;
        let _ = self.rerender();
    }

    fn hide(&mut self) {
        self.window.hide();
    }

    fn set_config(&mut self, config: &Config) {
        if self.config.theme != config.theme {
            self.renderer.apply_theme(config.theme);
        }
        if self.config.font != config.font {
            self.renderer.apply_font(&config.font);
        }
        self.config = config.clone();
    }
}

impl Drop for TabBarOverlay {
    fn drop(&mut self) {
        unsafe { SetWindowLongPtrW(self.window.hwnd(), GWLP_USERDATA, 0) };
    }
}

pub(in crate::platform::windows) unsafe extern "system" fn tab_bar_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(lr) = crate::platform::windows::dome_wnd_proc_common(hwnd, msg, wparam, lparam) {
        return lr;
    }
    // The bar must not raise itself on click. Tab clicks dispatch a hub event;
    // foreground stays with whatever managed window owned it.
    if msg == WM_MOUSEACTIVATE {
        return LRESULT(MA_NOACTIVATE as isize);
    }
    let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut TabBarOverlay;
    if ptr.is_null() {
        return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
    }
    let overlay = unsafe { &mut *ptr };
    match msg {
        WM_MOUSEMOVE => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            // Bar pixels are the canvas; coords in window-local physical pixels
            // map directly to egui logical points after Renderer::render's
            // pixels_per_point = scale rescale.
            let scale = overlay.scale;
            overlay
                .events
                .push(egui::Event::PointerMoved(egui::pos2(x / scale, y / scale)));
            LRESULT(0)
        }
        WM_LBUTTONDOWN => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            let scale = overlay.scale;
            overlay.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x / scale, y / scale),
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            LRESULT(0)
        }
        WM_LBUTTONUP => {
            let x = (lparam.0 & 0xFFFF) as i16 as f32;
            let y = ((lparam.0 >> 16) & 0xFFFF) as i16 as f32;
            let scale = overlay.scale;
            overlay.events.push(egui::Event::PointerButton {
                pos: egui::pos2(x / scale, y / scale),
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            // Button-up is the edge paint_tab_bar's Sense::click() observes.
            // Both Down and Up must be present in the same render pass.
            if let Some((cid, idx)) = overlay.rerender() {
                overlay.hub_sender.send(HubEvent::TabClicked(cid, idx));
            }
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
