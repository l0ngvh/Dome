use std::any::Any;

use calloop::channel::Sender as CalloopSender;
use dome_auxiliary_window::{
    AuxiliaryWindow, AuxiliaryWindowExtMacOs, AuxiliaryWindowHandler, MouseButton, NativeUnit,
    Point, Size, WindowAttributes, WindowLevel,
};
use objc2::MainThreadMarker;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2_application_services::AXUIElement;
use objc2_core_foundation::kCFBooleanTrue;
use objc2_core_graphics::CGWindowID;
use objc2_foundation::NSRect;
use objc2_io_surface::IOSurface;
use objc2_quartz_core::{CAAutoresizingMask, CALayer, CATransaction, kCAGravityResize};

use super::super::dome::{ContainerShow, HubEvent};
use super::compositor::{MacOsCompositor, physical_size};
use crate::config::Config;
use crate::core::{
    ContainerId, Dimension, FloatWindowPlacement, Length, Logical, TilingWindowPlacement,
};
use crate::font::FontConfig;
use crate::overlay::{self, BorderMetrics, LogicalTiledContainer, LogicalTiledWindow};
use crate::platform::macos::objc2_wrapper::{kAXFrontmostAttribute, set_attribute_value};
use crate::platform::render::{Renderer, WgpuContext};
use crate::platform::tab_bar::{TabBarMessage, TabBarWidget};
use crate::theme::Flavor;

fn frame_attrs(frame: NSRect) -> (Point<NativeUnit>, Size<NativeUnit>) {
    (
        Point::new(frame.origin.x.round() as i32, frame.origin.y.round() as i32),
        Size::new(
            frame.size.width.round() as u32,
            frame.size.height.round() as u32,
        ),
    )
}

struct FloatHandler {
    hub_sender: CalloopSender<HubEvent>,
    cg_id: CGWindowID,
}

impl AuxiliaryWindowHandler for FloatHandler {
    fn on_mouse_down(&mut self, _at: Point<NativeUnit>, _button: MouseButton) {
        self.hub_sender
            .send(HubEvent::MirrorClicked(self.cg_id))
            .ok();
    }
}

pub(super) struct FloatOverlay {
    window: AuxiliaryWindow,
    renderer: Renderer,
    mirror_layer: Retained<CALayer>,
    is_focused: bool,
    placement: Option<FloatWindowPlacement>,
    scale: f64,
    border_thickness: Length<Logical>,
}

impl FloatOverlay {
    pub(super) fn new(
        _mtm: MainThreadMarker,
        frame: NSRect,
        cg_id: CGWindowID,
        hub_sender: CalloopSender<HubEvent>,
        gpu: &WgpuContext,
        flavor: Flavor,
        font: &FontConfig,
    ) -> Self {
        // No window exists to read backingScaleFactor from yet. render() sets the real
        // scale before the first present.
        let scale = 1.0;
        let compositor = MacOsCompositor::new(scale, None);
        let metal_layer = compositor.layer();
        let (init_w, init_h) = physical_size(frame.size.width, frame.size.height, scale);
        let renderer = Renderer::new(
            gpu,
            Box::new(compositor),
            init_w,
            init_h,
            flavor,
            font,
            Box::new(crate::platform::macos::font::resolve_system_font),
        )
        .expect("float overlay renderer init");

        let root_layer = CALayer::layer();
        let mirror_layer = CALayer::layer();
        let mask = CAAutoresizingMask::LayerWidthSizable | CAAutoresizingMask::LayerHeightSizable;
        unsafe {
            mirror_layer.setAutoresizingMask(mask);
            mirror_layer.setContentsGravity(kCAGravityResize);
            mirror_layer.setContentsScale(scale);
            metal_layer.setAutoresizingMask(mask);
            root_layer.addSublayer(&mirror_layer);
            root_layer.addSublayer(&metal_layer);
        }

        let (position, size) = frame_attrs(frame);
        let window = AuxiliaryWindow::new(
            &WindowAttributes {
                position,
                size,
                click_through: true,
                focusable: false,
            },
            Box::new(FloatHandler { hub_sender, cg_id }),
        )
        .expect("auxiliary window on main thread");
        window.set_level(WindowLevel::Floating);
        window.set_content_layer(&root_layer);

        Self {
            window,
            renderer,
            mirror_layer,
            is_focused: false,
            placement: None,
            scale: 1.0,
            border_thickness: Length::new(0.0),
        }
    }

    pub(super) fn render(
        &mut self,
        placement: &FloatWindowPlacement,
        cocoa_frame: NSRect,
        scale: f64,
        border_thickness: Length<Logical>,
        is_focused: bool,
    ) {
        self.placement = Some(*placement);
        self.scale = scale;
        self.border_thickness = border_thickness;
        self.is_focused = is_focused;

        let (position, size) = frame_attrs(cocoa_frame);
        self.window.set_frame(position, size);
        let (pw, ph) = physical_size(cocoa_frame.size.width, cocoa_frame.size.height, scale);
        self.renderer.resize(scale as f32, pw, ph);
        self.mirror_layer.setContentsScale(scale);

        if !is_focused {
            self.window.set_click_through(false);
            self.mirror_layer.setHidden(false);
        } else {
            self.window.set_click_through(true);
            self.mirror_layer.setHidden(true);
        }

        let border = BorderMetrics::from_thickness(self.border_thickness);
        let theme = self.renderer.theme();
        self.renderer.render(scale as f32, Vec::new(), |ui| {
            overlay::paint_float_border(
                ui.ctx(),
                placement.border_box.to_dimension(),
                placement.visible_border_box.to_dimension(),
                placement.is_highlighted,
                &theme,
                border,
            );
        });
        self.window.set_visible(true);
    }

    pub(super) fn set_config(&mut self, config: &Config) {
        // Borders only, no text, so the font is not applied.
        self.renderer.set_theme(config.theme);
        if let Some(placement) = self.placement {
            let border = BorderMetrics::from_thickness(self.border_thickness);
            let theme = self.renderer.theme();
            self.renderer.render(self.scale as f32, Vec::new(), |ui| {
                overlay::paint_float_border(
                    ui.ctx(),
                    placement.border_box.to_dimension(),
                    placement.visible_border_box.to_dimension(),
                    placement.is_highlighted,
                    &theme,
                    border,
                );
            });
        }
    }

    pub(super) fn apply_frame(&mut self, surface: &IOSurface) {
        if self.is_focused {
            return;
        }
        // Core Animation applies a 0.25s implicit crossfade when contents changes.
        // Wrapping in a transaction with disabled actions swaps surfaces atomically.
        unsafe {
            CATransaction::begin();
            CATransaction::setDisableActions(true);
            // Explicit typed binding avoids deref-coercion ambiguity through the
            // IOSurface -> NSObject -> AnyObject chain in argument position.
            let obj: &AnyObject = surface;
            self.mirror_layer.setContents(Some(obj));
            CATransaction::commit();
        }
    }
}

struct TilingHandler;

impl AuxiliaryWindowHandler for TilingHandler {}

pub(super) struct TilingOverlay {
    window: AuxiliaryWindow,
    renderer: Renderer,
    monitor: Dimension,
    windows: Vec<TilingWindowPlacement>,
    containers: Vec<ContainerShow>,
    border_thickness: Length<Logical>,
    scale: f64,
}

impl TilingOverlay {
    pub(super) fn new(
        _mtm: MainThreadMarker,
        gpu: &WgpuContext,
        config: Config,
        cocoa_frame: NSRect,
        scale: f64,
    ) -> Self {
        let flavor = config.theme;
        let font = config.font.clone();
        let compositor = MacOsCompositor::new(scale, None);
        let metal_layer = compositor.layer();
        let (init_w, init_h) = physical_size(0.0, 0.0, scale);
        let renderer = Renderer::new(
            gpu,
            Box::new(compositor),
            init_w,
            init_h,
            flavor,
            &font,
            Box::new(crate::platform::macos::font::resolve_system_font),
        )
        .expect("tiling overlay renderer init");

        // Click-through so mouse events reach the application window beneath. Tab
        // clicks land on the sibling TabBarOverlay instead.
        let (position, size) = frame_attrs(cocoa_frame);
        let window = AuxiliaryWindow::new(
            &WindowAttributes {
                position,
                size,
                click_through: true,
                focusable: true,
            },
            Box::new(TilingHandler),
        )
        .expect("auxiliary window on main thread");
        window.set_level(WindowLevel::Bottom);
        window.set_content_layer(&metal_layer);
        window.set_visible(true);

        Self {
            window,
            renderer,
            monitor: Dimension::default(),
            windows: Vec::new(),
            containers: Vec::new(),
            border_thickness: Length::new(0.0),
            scale,
        }
    }

    pub(super) fn render(
        &mut self,
        cocoa_frame: NSRect,
        scale: f64,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        containers: &[ContainerShow],
    ) {
        let (position, size) = frame_attrs(cocoa_frame);
        self.window.set_frame(position, size);
        self.update(monitor, windows, containers, scale);
    }

    pub(super) fn set_border_thickness(&mut self, t: Length<Logical>) {
        self.border_thickness = t;
    }

    pub(super) fn clear(&mut self) {
        self.windows.clear();
        self.containers.clear();
        self.render_now();
    }

    pub(super) fn focus(&self, _mtm: MainThreadMarker) {
        activate_self();
        self.window.focus();
    }

    pub(super) fn set_config(&mut self, config: &Config) {
        // Borders only, no text, so the font is not applied.
        self.renderer.set_theme(config.theme);
        self.render_now();
    }

    fn update(
        &mut self,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        containers: &[ContainerShow],
        scale: f64,
    ) {
        self.monitor = monitor;
        self.scale = scale;
        self.windows = windows.to_vec();
        self.containers = containers.to_vec();
        let (pw, ph) = physical_size(
            monitor.width.logical() as f64,
            monitor.height.logical() as f64,
            scale,
        );
        self.renderer.resize(scale as f32, pw, ph);
        self.render_now();
    }

    fn render_now(&mut self) {
        let monitor_logical = self.monitor;
        let windows_logical: Vec<LogicalTiledWindow> = self
            .windows
            .iter()
            .map(|wp| LogicalTiledWindow {
                id: wp.id,
                frame: wp.border_box.to_dimension(),
                visible_frame: wp.visible_border_box.to_dimension(),
                is_highlighted: wp.is_highlighted,
                spawn_indicator: wp.spawn_indicator,
            })
            .collect();
        let containers_logical: Vec<LogicalTiledContainer> = self
            .containers
            .iter()
            .map(|cs| LogicalTiledContainer {
                id: cs.placement.id,
                frame: cs.placement.border_box.to_dimension(),
                visible_frame: cs.placement.visible_border_box.to_dimension(),
                tab_bar_height: Length::from_pixels(cs.placement.tab_bar_band.height()),
                is_highlighted: cs.placement.is_highlighted,
                spawn_indicator: cs.placement.spawn_indicator,
                is_tabbed: cs.placement.is_tabbed,
                titles: cs.placement.titles.clone(),
            })
            .collect();
        let border = BorderMetrics::from_thickness(self.border_thickness);
        let theme = self.renderer.theme();
        let scale = self.scale;

        self.renderer.render(scale as f32, Vec::new(), |ui| {
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

struct TabBarHandler {
    widget: TabBarWidget,
    hub_sender: CalloopSender<HubEvent>,
}

impl AuxiliaryWindowHandler for TabBarHandler {
    // Rendering on the press edge keeps the press queued for the click
    // TabBarWidget::render resolves on release.
    fn on_mouse_down(&mut self, at: Point<NativeUnit>, _button: MouseButton) {
        self.widget
            .push_pointer_button(egui::pos2(at.x() as f32, at.y() as f32), true);
        self.widget.render();
    }

    fn on_mouse_up(&mut self, at: Point<NativeUnit>, _button: MouseButton) {
        self.widget
            .push_pointer_button(egui::pos2(at.x() as f32, at.y() as f32), false);
        if let Some((cid, tab_idx)) = self.widget.render() {
            self.hub_sender
                .send(HubEvent::TabClicked(cid, tab_idx))
                .ok();
        }
    }

    fn on_message(&mut self, message: Box<dyn Any>) {
        let msg = message
            .downcast::<TabBarMessage>()
            .expect("tab bar window received a non-TabBarMessage payload");
        match *msg {
            TabBarMessage::Content {
                scale,
                size,
                border,
                titles,
                active_index,
                is_highlighted,
            } => {
                self.widget
                    .set_content(scale, size, border, titles, active_index, is_highlighted);
                self.widget.render();
            }
            TabBarMessage::Style { theme, font } => {
                self.widget.set_style(theme, &font);
                self.widget.render();
            }
        }
    }
}

/// A separate borderless window per tabbed container, so a tab click lands here
/// directly instead of traversing the click-through `TilingOverlay`.
pub(super) struct TabBarOverlay {
    window: AuxiliaryWindow,
}

impl TabBarOverlay {
    pub(super) fn new(
        _mtm: MainThreadMarker,
        gpu: &WgpuContext,
        config: Config,
        container_id: ContainerId,
        cocoa_frame: NSRect,
        scale: f64,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Self {
        let compositor = MacOsCompositor::new(scale, None);
        let metal_layer = compositor.layer();
        let (init_w, init_h) = physical_size(0.0, 0.0, scale);
        let renderer = Renderer::new(
            gpu,
            Box::new(compositor),
            init_w,
            init_h,
            config.theme,
            &config.font,
            Box::new(crate::platform::macos::font::resolve_system_font),
        )
        .expect("tab bar renderer init");
        let widget = TabBarWidget::new(renderer, container_id, scale as f32, (init_w, init_h));

        // Same level as the per-monitor tiling overlay. Stacking against
        // sibling same-level windows is fine because the tiling overlay is
        // mouse-transparent and visually empty in the strip the tab bar covers.
        let (position, size) = frame_attrs(cocoa_frame);
        let window = AuxiliaryWindow::new(
            &WindowAttributes {
                position,
                size,
                click_through: false,
                focusable: false,
            },
            Box::new(TabBarHandler { widget, hub_sender }),
        )
        .expect("auxiliary window on main thread");
        window.set_level(WindowLevel::Bottom);
        window.set_content_layer(&metal_layer);
        window.set_visible(true);

        Self { window }
    }

    pub(super) fn render(&self, cs: &ContainerShow, scale: f64, border_thickness: Length<Logical>) {
        let (position, size) = frame_attrs(cs.tab_bar_cocoa_frame);
        self.window.set_frame(position, size);
        let bar = cs.tab_bar_dim;
        self.window.deliver(Box::new(TabBarMessage::Content {
            scale: scale as f32,
            size: (bar.width, bar.height),
            border: border_thickness,
            titles: cs.placement.titles.clone(),
            active_index: cs.placement.active_tab_index,
            is_highlighted: cs.placement.is_highlighted,
        }));
        self.window.set_visible(true);
    }

    pub(super) fn set_config(&self, config: &Config) {
        self.window.deliver(Box::new(TabBarMessage::Style {
            theme: config.theme,
            font: config.font.clone(),
        }));
    }
}

/// macOS 14+ "cooperative activation" silently ignores `NSApplication::activate()` for
/// self-activation.
/// Without this, `makeKeyAndOrderFront` only makes a Dome-owned window key inside Dome's
/// AppKit context while the OS-level foreground app stays elsewhere.
fn activate_self() {
    let pid = std::process::id() as i32;
    let ax_app = unsafe { AXUIElement::new_application(pid) };
    set_attribute_value(&ax_app, &kAXFrontmostAttribute(), unsafe {
        kCFBooleanTrue.unwrap()
    })
    .ok();
}
