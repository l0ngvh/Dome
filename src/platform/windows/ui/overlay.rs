use std::cell::RefCell;
use std::rc::Rc;

use dome_auxiliary_window::{
    AuxiliaryWindow, AuxiliaryWindowExtWindows, AuxiliaryWindowHandler, MouseButton,
    PhysicalPosition, PhysicalSize, WindowAttributes, WindowLevel,
};

use crate::config::Config;
use crate::platform::render::{Compositor, Renderer, WgpuContext};
use crate::platform::tab_bar::TabBarWidget;
use crate::platform::windows::{HubEvent, HubSender};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::DirectComposition::{
    DCompositionCreateDevice2, IDCompositionDevice, IDCompositionVisual,
};
use windows::Win32::UI::WindowsAndMessaging::{
    SW_HIDE, SW_SHOWNA, SWP_NOACTIVATE, SWP_NOREDRAW, SWP_NOZORDER, SetWindowPos, ShowWindow,
};
use windows::core::Interface;

use crate::core::{
    ContainerId, ContainerPlacement, Dimension, FloatWindowPlacement, Length, Logical, Physical,
    PixelRect, Pixels, TilingWindowPlacement,
};
use crate::overlay;
use crate::platform::windows::dome::CreateOverlay;
use crate::platform::windows::external::{HwndId, ZOrder};
use crate::platform::windows::foreground::force_set_foreground;

/// The window-independent half of the DirectComposition surface: the composition device
/// and this overlay's visual, which wgpu renders into with no HWND. The window-bound half
/// (a target rooting this visual) is created later by `AuxiliaryWindow::set_content_visual`,
/// so the renderer and its state can be built before the window exists.
struct WindowsCompositor {
    dcomp_visual: IDCompositionVisual,
    dcomp_device: IDCompositionDevice,
}

impl WindowsCompositor {
    fn new(dcomp_device: &IDCompositionDevice) -> anyhow::Result<Self> {
        let dcomp_visual = unsafe { dcomp_device.CreateVisual()? };
        Ok(Self {
            dcomp_visual,
            dcomp_device: dcomp_device.clone(),
        })
    }

    fn visual(&self) -> IDCompositionVisual {
        self.dcomp_visual.clone()
    }
}

impl Compositor for WindowsCompositor {
    fn surface_target(&self) -> wgpu::SurfaceTargetUnsafe {
        // CompositionVisual is #[cfg(dx12)] in wgpu 29, absent from docs.rs (Linux build).
        wgpu::SurfaceTargetUnsafe::CompositionVisual(self.dcomp_visual.as_raw() as *mut _)
    }

    fn format(&self) -> wgpu::TextureFormat {
        wgpu::TextureFormat::Bgra8UnormSrgb
    }

    fn after_configure(&self, _surface: &wgpu::Surface<'static>) -> anyhow::Result<()> {
        // configure() calls SetContent(swap_chain). Commit must follow so DWM sees the
        // visual with content.
        unsafe { self.dcomp_device.Commit()? };
        Ok(())
    }
}

/// Forwards the per-monitor DPI signal. The tiling overlay is the only window on
/// every monitor, so it alone forwards DPI.
///
/// Activation relies on the crate default `Decline`. WS_EX_LAYERED + WS_EX_TRANSPARENT
/// already routes pointer events past the overlay, but the active-window-tracking
/// accessibility path can still dispatch WM_MOUSEACTIVATE. Declining keeps that rare
/// path from raising the overlay above managed windows.
struct TilingHandler {
    hub_sender: HubSender,
}

impl AuxiliaryWindowHandler for TilingHandler {
    fn on_scale_changed(&mut self, _scale: f32) {
        // DpiChanged is a bare signal. The domain reconciles every monitor's scale.
        self.hub_sender.send(HubEvent::DpiChanged);
    }
}

/// Per-monitor overlay that draws all tiling window borders.
pub(in crate::platform::windows) struct TilingOverlay {
    /// Declared before `aux` so it drops before the window is destroyed.
    renderer: Renderer,
    monitor: PixelRect,
    width_phys: u32,
    height_phys: u32,
    windows: Vec<TilingWindowPlacement>,
    containers: Vec<ContainerPlacement>,
    border_thickness: Pixels<Physical>,
    aux: AuxiliaryWindow,
    scale: f32,
}

impl TilingOverlay {
    pub(in crate::platform::windows) fn new(
        gpu: &WgpuContext,
        dcomp_device: &IDCompositionDevice,
        config: Config,
        monitor: PixelRect,
        scale: f32,
        hub_sender: HubSender,
    ) -> anyhow::Result<Box<Self>> {
        // Initialize the wgpu surface at the monitor's physical size so the
        // overlay is ready to render without a preceding update() call.
        let (x_phys, y_phys, init_w, init_h) = monitor.to_surface_size();
        // click_through keeps the tiling overlay transparent to the pointer, so events
        // reach managed windows below.
        let attributes = WindowAttributes {
            position: PhysicalPosition {
                x: x_phys,
                y: y_phys,
            },
            size: PhysicalSize {
                width: init_w,
                height: init_h,
            },
            click_through: true,
            focusable: true,
        };
        let aux = AuxiliaryWindow::new(&attributes, Box::new(TilingHandler { hub_sender }))?;
        let compositor = WindowsCompositor::new(dcomp_device)?;
        let dcomp_visual = compositor.visual();
        let renderer = Renderer::new(
            gpu,
            Box::new(compositor),
            init_w,
            init_h,
            config.theme,
            &config.font,
            Box::new(crate::platform::windows::font::resolve_system_font),
        )?;
        aux.set_content_visual(dcomp_device, &dcomp_visual)?;
        aux.set_visible(true);
        // Park below managed windows after showing. Managed windows created later land
        // above it, and show_tiling's per-window lift maintains the band thereafter.
        aux.set_level(WindowLevel::Bottom);
        let boxed = Box::new(Self {
            renderer,
            monitor,
            width_phys: init_w,
            height_phys: init_h,
            windows: Vec::new(),
            containers: Vec::new(),
            border_thickness: Pixels::ZERO,
            aux,
            scale,
        });
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
                frame: wp.border_box.to_logical(scale),
                visible_frame: wp.visible_border_box.to_logical(scale),
                is_highlighted: wp.is_highlighted,
                spawn_indicator: wp.spawn_indicator,
            })
            .collect();
        let containers_logical: Vec<overlay::LogicalTiledContainer> = self
            .containers
            .iter()
            .map(|cp| overlay::LogicalTiledContainer {
                id: cp.id,
                frame: cp.border_box.to_logical(scale),
                visible_frame: cp.visible_border_box.to_logical(scale),
                tab_bar_height: Length::from_pixels(cp.tab_bar_band.height()).to_logical(scale),
                is_highlighted: cp.is_highlighted,
                spawn_indicator: cp.spawn_indicator,
                is_tabbed: cp.is_tabbed,
                titles: cp.titles.clone(),
            })
            .collect();
        let theme = self.renderer.theme();
        let border = overlay::BorderMetrics::from_thickness(
            Length::from_pixels(self.border_thickness).to_logical(scale),
        );
        // Borders-only mode: tab bars live in dedicated per-container windows,
        // so the per-monitor overlay never sees pointer events. The returned
        // click vector is always empty.
        let _ = self.renderer.render(scale, vec![], |ui| {
            overlay::paint_tiling_overlay(
                ui.ctx(),
                monitor_logical,
                &windows_logical,
                &containers_logical,
                &theme,
                border,
            )
        });
    }
}

impl TilingOverlayApi for TilingOverlay {
    fn update(
        &mut self,
        monitor: PixelRect,
        windows: &[TilingWindowPlacement],
        containers: &[ContainerPlacement],
        scale: f32,
        border_thickness: Pixels<Physical>,
    ) {
        let (x_phys, y_phys, w_phys, h_phys) = monitor.to_surface_size();

        if self.monitor != monitor {
            self.renderer.resize(scale, w_phys, h_phys);
            self.aux.set_frame(
                PhysicalPosition {
                    x: x_phys,
                    y: y_phys,
                },
                PhysicalSize {
                    width: w_phys,
                    height: h_phys,
                },
            );
            self.aux.set_level(WindowLevel::Bottom);
            self.aux.set_visible(true);
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
        self.border_thickness = border_thickness;
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
        // Borders only, no text, so the font is not applied.
        self.renderer.set_theme(config.theme);
    }

    fn focus(&self) {
        force_set_foreground(self.aux.hwnd());
    }

    fn id(&self) -> HwndId {
        HwndId::from(self.aux.hwnd())
    }
}

pub(in crate::platform::windows) trait FloatOverlayApi {
    fn update(
        &mut self,
        wp: &FloatWindowPlacement,
        z: ZOrder,
        scale: f32,
        border_thickness: Pixels<Physical>,
    );
    fn hide(&mut self);
    fn set_config(&mut self, config: &Config);
}

pub(in crate::platform::windows) trait TilingOverlayApi {
    fn update(
        &mut self,
        monitor: PixelRect,
        windows: &[TilingWindowPlacement],
        containers: &[ContainerPlacement],
        scale: f32,
        border_thickness: Pixels<Physical>,
    );
    fn clear(&mut self);
    fn set_config(&mut self, config: &Config);
    /// The Win32 close-time focus walk lands here when the user closes a
    /// managed window with no obvious successor on the same monitor, replacing
    /// the process-wide focus-sink window the platform shell used to keep below
    /// every overlay. The overlay HWND is `WS_EX_TRANSPARENT`, so claiming
    /// foreground does not take pointer events away from anything below.
    fn focus(&self);
    fn id(&self) -> HwndId;
}

/// Empty: the float overlay renders from its seam's `update`, so it needs no window-event
/// callback. The crate declines click-activation for every window.
struct FloatHandler;
impl AuxiliaryWindowHandler for FloatHandler {}

pub(in crate::platform::windows) struct FloatOverlay {
    renderer: Renderer,
    width_phys: u32,
    height_phys: u32,
    aux: AuxiliaryWindow,
}

impl FloatOverlay {
    fn new(
        gpu: &WgpuContext,
        dcomp_device: &IDCompositionDevice,
        config: Config,
        x: i32,
        y: i32,
        width_phys: u32,
        height_phys: u32,
    ) -> anyhow::Result<Box<Self>> {
        // Not focusable, so clicking the float border never steals foreground.
        let attributes = WindowAttributes {
            position: PhysicalPosition { x, y },
            size: PhysicalSize {
                width: width_phys,
                height: height_phys,
            },
            click_through: true,
            focusable: false,
        };
        let aux = AuxiliaryWindow::new(&attributes, Box::new(FloatHandler))?;
        let compositor = WindowsCompositor::new(dcomp_device)?;
        let dcomp_visual = compositor.visual();
        let renderer = Renderer::new(
            gpu,
            Box::new(compositor),
            width_phys,
            height_phys,
            config.theme,
            &config.font,
            Box::new(crate::platform::windows::font::resolve_system_font),
        )?;
        aux.set_content_visual(dcomp_device, &dcomp_visual)?;
        let boxed = Box::new(Self {
            renderer,
            width_phys,
            height_phys,
            aux,
        });
        Ok(boxed)
    }
}

impl FloatOverlayApi for FloatOverlay {
    fn update(
        &mut self,
        wp: &FloatWindowPlacement,
        z: ZOrder,
        scale: f32,
        border_thickness: Pixels<Physical>,
    ) {
        let vf = wp.visible_border_box;
        let (x_phys, y_phys, w_phys, h_phys) = vf.to_surface_size();

        if self.width_phys != w_phys || self.height_phys != h_phys {
            self.renderer.resize(scale, w_phys, h_phys);
            self.width_phys = w_phys;
            self.height_phys = h_phys;
        }

        // Position before showing, or the window flashes at its previous position.
        let z_after: Option<HWND> = z.into();
        let mut flags = SWP_NOACTIVATE | SWP_NOREDRAW;
        if z_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        unsafe {
            SetWindowPos(
                self.aux.hwnd(),
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
        unsafe { ShowWindow(self.aux.hwnd(), SW_SHOWNA).ok().ok() };

        let vf_logical = vf.to_logical(scale);
        let frame_logical = wp.border_box.to_logical(scale);
        let theme = self.renderer.theme();
        let border = overlay::BorderMetrics::from_thickness(
            Length::from_pixels(border_thickness).to_logical(scale),
        );
        let is_highlighted = wp.is_highlighted;

        self.renderer.render(scale, vec![], |ui| {
            overlay::paint_float_border(
                ui.ctx(),
                frame_logical,
                vf_logical,
                is_highlighted,
                &theme,
                border,
            );
        });
    }

    fn hide(&mut self) {
        unsafe { ShowWindow(self.aux.hwnd(), SW_HIDE).ok().ok() };
    }

    fn set_config(&mut self, config: &Config) {
        // Borders only, no text, so the font is not applied.
        self.renderer.set_theme(config.theme);
    }
}

pub(in crate::platform::windows) struct WgpuOverlayFactory {
    gpu: WgpuContext,
    hub_sender: HubSender,
    // Any overlay's Commit() flushes this shared device, which is safe because all
    // overlays run on the one window thread.
    dcomp_device: IDCompositionDevice,
}

impl WgpuOverlayFactory {
    pub(in crate::platform::windows) fn new(
        gpu: WgpuContext,
        hub_sender: HubSender,
    ) -> anyhow::Result<Self> {
        // DCompositionCreateDevice2 accepts None so wgpu owns its own DXGI device.
        let dcomp_device: IDCompositionDevice = unsafe { DCompositionCreateDevice2(None)? };
        Ok(Self {
            gpu,
            hub_sender,
            dcomp_device,
        })
    }
}

impl CreateOverlay for WgpuOverlayFactory {
    fn create_tiling_overlay(
        &self,
        config: Config,
        monitor: PixelRect,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TilingOverlayApi>> {
        Ok(TilingOverlay::new(
            &self.gpu,
            &self.dcomp_device,
            config,
            monitor,
            scale,
            self.hub_sender.clone(),
        )?)
    }
    fn create_float_overlay(
        &self,
        config: Config,
        _scale: f32,
        visible_border_box: PixelRect,
    ) -> anyhow::Result<Box<dyn FloatOverlayApi>> {
        let (x_phys, y_phys, w_phys, h_phys) = visible_border_box.to_surface_size();
        Ok(FloatOverlay::new(
            &self.gpu,
            &self.dcomp_device,
            config,
            x_phys,
            y_phys,
            w_phys,
            h_phys,
        )?)
    }
    fn create_tab_bar(
        &self,
        config: Config,
        container_id: ContainerId,
        rect: PixelRect,
        scale: f32,
    ) -> anyhow::Result<Box<dyn TabBarOverlayApi>> {
        Ok(TabBarOverlay::new(
            &self.gpu,
            &self.dcomp_device,
            config,
            container_id,
            rect,
            scale,
            self.hub_sender.clone(),
        )?)
    }
}

trait PhysicalRectExt {
    fn to_logical(self, scale: f32) -> Dimension<Logical>;
    fn to_surface_size(self) -> (i32, i32, u32, u32);
}

trait PhysicalLengthExt {
    fn to_logical(self, scale: f32) -> Length<Logical>;
}

impl PhysicalLengthExt for Length<Physical> {
    /// Deliberately does not round. Core insets by an integral physical thickness, so
    /// dividing recovers it exactly on multiplication back, and rounding here would
    /// reintroduce the disagreement between the painted band and the inset.
    fn to_logical(self, scale: f32) -> Length<Logical> {
        debug_assert!(scale > 0.0, "scale must be positive, got {scale}");
        Length::new(self.value() / scale)
    }
}

impl PhysicalRectExt for PixelRect<Physical> {
    fn to_logical(self, scale: f32) -> Dimension<Logical> {
        Dimension::new(
            Length::from_pixels(self.x()).to_logical(scale),
            Length::from_pixels(self.y()).to_logical(scale),
            Length::from_pixels(self.width()).to_logical(scale),
            Length::from_pixels(self.height()).to_logical(scale),
        )
    }

    fn to_surface_size(self) -> (i32, i32, u32, u32) {
        // Assert before the cast, not after: `i32 as u32` wraps a negative extent
        // into a huge positive one, where the `f32` path saturates to zero.
        assert!(
            self.width() > Pixels::ZERO && self.height() > Pixels::ZERO,
            "overlay surface size must be positive; got {}x{}",
            self.width().value(),
            self.height().value()
        );
        (
            self.x().value(),
            self.y().value(),
            self.width().value() as u32,
            self.height().value() as u32,
        )
    }
}

pub(in crate::platform::windows) trait TabBarOverlayApi {
    fn update(
        &mut self,
        rect: PixelRect,
        titles: Vec<String>,
        active_index: usize,
        is_highlighted: bool,
        scale: f32,
        border_thickness: Pixels<Physical>,
    );
    #[expect(
        dead_code,
        reason = "hide() is invoked when a tabbed container's active window minimizes. Wired up in the follow-up minimize/restore pass."
    )]
    fn hide(&mut self);
    fn set_config(&mut self, config: &Config);
}

/// The bar must not raise itself on click. The crate declines click-activation for every
/// window, so foreground stays with whatever managed window owned it.
struct TabBarHandler {
    widget: Rc<RefCell<TabBarWidget>>,
    hub_sender: HubSender,
}

impl AuxiliaryWindowHandler for TabBarHandler {
    fn on_mouse_moved(&mut self, at: PhysicalPosition) {
        // Window-local physical pixels divide by scale to reach the logical points
        // TabBarWidget paints in.
        let mut widget = self.widget.borrow_mut();
        let scale = widget.scale();
        widget.push_pointer_moved(egui::pos2(at.x as f32 / scale, at.y as f32 / scale));
    }

    fn on_mouse_down(&mut self, at: PhysicalPosition, button: MouseButton) {
        if button != MouseButton::Primary {
            return;
        }
        let mut widget = self.widget.borrow_mut();
        let scale = widget.scale();
        widget.push_pointer_button(egui::pos2(at.x as f32 / scale, at.y as f32 / scale), true);
    }

    fn on_mouse_up(&mut self, at: PhysicalPosition, button: MouseButton) {
        if button != MouseButton::Primary {
            return;
        }
        // Button-up is the edge paint_tab_bar's Sense::click() observes, with the queued
        // press still present in the same render pass.
        let click = {
            let mut widget = self.widget.borrow_mut();
            let scale = widget.scale();
            widget.push_pointer_button(egui::pos2(at.x as f32 / scale, at.y as f32 / scale), false);
            widget.render()
        };
        if let Some((cid, idx)) = click {
            self.hub_sender.send(HubEvent::TabClicked(cid, idx));
        }
    }
}

pub(in crate::platform::windows) struct TabBarOverlay {
    // widget precedes aux so the Renderer drops before DestroyWindow.
    widget: Rc<RefCell<TabBarWidget>>,
    aux: AuxiliaryWindow,
}

impl TabBarOverlay {
    pub(in crate::platform::windows) fn new(
        gpu: &WgpuContext,
        dcomp_device: &IDCompositionDevice,
        config: Config,
        container_id: ContainerId,
        rect: PixelRect,
        scale: f32,
        hub_sender: HubSender,
    ) -> anyhow::Result<Box<Self>> {
        let (x_phys, y_phys, w_phys, h_phys) = rect.to_surface_size();
        // Not focusable, so a tab click never steals foreground. Clicks dispatch as
        // `HubEvent::TabClicked` rather than raising the window.
        let attributes = WindowAttributes {
            position: PhysicalPosition {
                x: x_phys,
                y: y_phys,
            },
            size: PhysicalSize {
                width: w_phys,
                height: h_phys,
            },
            click_through: false,
            focusable: false,
        };
        let compositor = WindowsCompositor::new(dcomp_device)?;
        let dcomp_visual = compositor.visual();
        let renderer = Renderer::new(
            gpu,
            Box::new(compositor),
            w_phys,
            h_phys,
            config.theme,
            &config.font,
            Box::new(crate::platform::windows::font::resolve_system_font),
        )?;
        let widget = Rc::new(RefCell::new(TabBarWidget::new(
            renderer,
            container_id,
            scale,
            (w_phys, h_phys),
        )));
        let aux = AuxiliaryWindow::new(
            &attributes,
            Box::new(TabBarHandler {
                widget: Rc::clone(&widget),
                hub_sender,
            }),
        )?;
        aux.set_content_visual(dcomp_device, &dcomp_visual)?;
        Ok(Box::new(Self { widget, aux }))
    }
}

impl TabBarOverlayApi for TabBarOverlay {
    fn update(
        &mut self,
        rect: PixelRect,
        titles: Vec<String>,
        active_index: usize,
        is_highlighted: bool,
        scale: f32,
        border_thickness: Pixels<Physical>,
    ) {
        let (x_phys, y_phys, w_phys, h_phys) = rect.to_surface_size();
        let bar_size = (
            Length::new(w_phys as f32 / scale),
            Length::new(h_phys as f32 / scale),
        );
        let border = Length::from_pixels(border_thickness).to_logical(scale);
        self.widget.borrow_mut().set_content(
            scale,
            bar_size,
            border,
            titles,
            active_index,
            is_highlighted,
        );
        // No z-order lift needed. The tab bar is created above the bottom-parked
        // border overlay, the only window it shares pixels with, and set_frame
        // preserves that order.
        self.aux.set_frame(
            PhysicalPosition {
                x: x_phys,
                y: y_phys,
            },
            PhysicalSize {
                width: w_phys,
                height: h_phys,
            },
        );
        self.aux.set_visible(true);
        let _ = self.widget.borrow_mut().render();
    }

    fn hide(&mut self) {
        unsafe { ShowWindow(self.aux.hwnd(), SW_HIDE).ok().ok() };
    }

    fn set_config(&mut self, config: &Config) {
        self.widget.borrow_mut().set_config(config);
    }
}
