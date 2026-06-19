// Coordinate system: logical points throughout (AppKit, AX, and Core Graphics are all
// logical-point-native). Renderer::render passes pixels_per_point = backingScaleFactor; shell
// passes core Dimension (= Dimension<Logical> on macOS) directly with no
// physical-to-logical division at any boundary.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use calloop::channel::Sender as CalloopSender;
use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSNormalWindowLevel, NSResponder,
    NSView, NSWindow, NSWindowCollectionBehavior, NSWindowLevel, NSWindowStyleMask,
};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_io_surface::IOSurface;
use objc2_quartz_core::{
    CAAutoresizingMask, CALayer, CAMetalLayer, CATransaction, kCAGravityResize,
};

use super::super::dome::{ContainerShow, HubEvent};
use super::renderer::{MetalBackend, Renderer};
use crate::config::Config;
use crate::core::{
    ContainerId, Dimension, FloatWindowPlacement, Length, Logical, TilingWindowPlacement,
};
use crate::font::FontConfig;
use crate::overlay::{
    self, BorderMetrics, LogicalTiledContainer, LogicalTiledWindow, OverlayMetrics,
};
use crate::theme::Flavor;

define_class!(
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    struct KeyableWindow;

    unsafe impl NSObjectProtocol for KeyableWindow {}

    impl KeyableWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

impl KeyableWindow {
    fn new(mtm: MainThreadMarker, frame: NSRect, style: NSWindowStyleMask) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe {
            msg_send![
                super(this),
                initWithContentRect: frame,
                styleMask: style,
                backing: NSBackingStoreType::Buffered,
                defer: false,
            ]
        }
    }
}

const FLOAT_OVERLAY_LEVEL: NSWindowLevel = NSFloatingWindowLevel;

pub(super) struct FloatOverlay {
    window: Retained<NSWindow>,
    renderer: Renderer,
    mirror_layer: Retained<CALayer>,
    is_focused: Cell<bool>,
    placement: Option<FloatWindowPlacement>,
    scale: f64,
    config: Config,
}

impl FloatOverlay {
    #[expect(
        clippy::too_many_arguments,
        reason = "font added for font config plumbing; restructuring FloatOverlay::new is out of scope"
    )]
    pub(super) fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        cg_id: CGWindowID,
        hub_sender: CalloopSender<HubEvent>,
        backend: Rc<MetalBackend>,
        config: Config,
        flavor: Flavor,
        font: &FontConfig,
    ) -> Self {
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setOpaque(false);
        window.setLevel(FLOAT_OVERLAY_LEVEL);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Auxiliary
                | NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };

        let scale = window.backingScaleFactor();
        let renderer = Renderer::new(
            backend,
            scale,
            frame.size.width,
            frame.size.height,
            false,
            flavor,
            font,
        );
        let metal_layer = renderer.layer();

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

        let view = FloatOverlayView::new(
            mtm,
            NSRect::new(NSPoint::new(0.0, 0.0), frame.size),
            root_layer.clone(),
            hub_sender,
            cg_id,
        );
        window.setContentView(Some(&view));

        Self {
            window,
            renderer,
            mirror_layer,
            is_focused: Cell::new(false),
            placement: None,
            scale: 1.0,
            config,
        }
    }

    pub(super) fn render(
        &mut self,
        placement: &FloatWindowPlacement,
        cocoa_frame: NSRect,
        scale: f64,
        is_focused: bool,
    ) {
        self.placement = Some(*placement);
        self.scale = scale;
        self.is_focused.set(is_focused);

        self.window.setFrame_display(cocoa_frame, true);
        self.renderer
            .resize(cocoa_frame.size.width, cocoa_frame.size.height, scale);
        self.mirror_layer.setContentsScale(scale);

        if !is_focused {
            self.window.setIgnoresMouseEvents(false);
            self.mirror_layer.setHidden(false);
        } else {
            self.window.setIgnoresMouseEvents(true);
            self.mirror_layer.setHidden(true);
        }

        let config = &self.config;
        let border = BorderMetrics::from_thickness(Length::<Logical>::new(config.border_size));
        let theme = config.theme();
        self.renderer.render(scale as f32, Vec::new(), |ctx| {
            // layer_painter bypasses egui's Area sizing pass, avoiding
            // black/invisible borders on the first frame.
            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Middle,
                egui::Id::new("border"),
            ));
            let clip = egui::Rect::from_min_size(
                egui::pos2(0.0, 0.0),
                egui::vec2(
                    placement.visible_frame.width.logical(),
                    placement.visible_frame.height.logical(),
                ),
            );
            overlay::paint_window_border(
                &painter.with_clip_rect(clip),
                placement.frame,
                placement.visible_frame,
                placement.is_highlighted,
                None,
                &theme,
                border,
                egui::Vec2::ZERO,
            );
        });
        self.window.setIsVisible(true);
    }

    pub(super) fn set_config(&mut self, config: &Config) {
        if self.config.theme != config.theme {
            self.renderer.apply_theme(config.theme);
        }
        if self.config.font != config.font {
            if self.config.font.family != config.font.family {
                self.renderer.reinstall_fonts(config.font.family.as_deref());
            }
            self.renderer.apply_font(&config.font);
        }
        self.config = config.clone();
        if let Some(placement) = self.placement {
            let config = &self.config;
            let border = BorderMetrics::from_thickness(Length::<Logical>::new(config.border_size));
            let theme = config.theme();
            self.renderer.render(self.scale as f32, Vec::new(), |ctx| {
                let painter = ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Middle,
                    egui::Id::new("border"),
                ));
                let clip = egui::Rect::from_min_size(
                    egui::pos2(0.0, 0.0),
                    egui::vec2(
                        placement.visible_frame.width.logical(),
                        placement.visible_frame.height.logical(),
                    ),
                );
                overlay::paint_window_border(
                    &painter.with_clip_rect(clip),
                    placement.frame,
                    placement.visible_frame,
                    placement.is_highlighted,
                    None,
                    &theme,
                    border,
                    egui::Vec2::ZERO,
                );
            });
        }
    }

    pub(super) fn apply_frame(&mut self, surface: &IOSurface) {
        if self.is_focused.get() {
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

impl Drop for FloatOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct TilingOverlay {
    window: Retained<KeyableWindow>,
    view: Retained<TilingOverlayView>,
}

impl TilingOverlay {
    pub(super) fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        config: Config,
        tab_bar_height: Length<Logical>,
        cocoa_frame: NSRect,
        scale: f64,
    ) -> Self {
        let flavor = config.theme;
        let font = config.font.clone();
        let window = KeyableWindow::new(mtm, cocoa_frame, NSWindowStyleMask::Borderless);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setOpaque(false);
        window.setLevel(NSNormalWindowLevel - 1);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };
        // Click-through so mouse events fall through to the application
        // window beneath. Tab clicks land on the per-container TabBarOverlay,
        // which is hosted as a sibling NSWindow at the same level.
        window.setIgnoresMouseEvents(true);

        let view =
            TilingOverlayView::new(mtm, backend, config, tab_bar_height, scale, flavor, &font);
        window.setContentView(Some(&view));
        window.setFrame_display(cocoa_frame, false);
        window.orderFront(None);

        Self { window, view }
    }

    pub(super) fn render(
        &self,
        cocoa_frame: NSRect,
        scale: f64,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        containers: &[ContainerShow],
    ) {
        self.window.setFrame_display(cocoa_frame, false);
        self.view.update(monitor, windows, containers, scale);
    }

    pub(super) fn set_tab_bar_height(&self, h: Length<Logical>) {
        self.view.ivars().tab_bar_height.set(h);
    }

    pub(super) fn clear(&self) {
        self.view.clear();
        self.view.render_now();
    }

    // macOS 14+ "cooperative activation" silently ignores NSApplication.activate() for
    // self-activation. The AX API bypasses this via the privileged accessibility subsystem.
    pub(super) fn focus(&self, _mtm: MainThreadMarker) {
        super::activate_self();
        self.window.makeKeyAndOrderFront(None);
    }

    pub(super) fn set_config(&self, config: &Config) {
        self.view.set_config(config);
    }
}

impl Drop for TilingOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct FloatOverlayViewIvars {
    root_layer: Retained<CALayer>,
    hub_sender: CalloopSender<HubEvent>,
    cg_id: Cell<CGWindowID>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = FloatOverlayViewIvars]
    pub(super) struct FloatOverlayView;

    unsafe impl NSObjectProtocol for FloatOverlayView {}

    impl FloatOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        #[unsafe(method(wantsLayer))]
        fn wants_layer(&self) -> bool {
            true
        }

        #[unsafe(method(makeBackingLayer))]
        fn make_backing_layer(&self) -> *mut objc2_quartz_core::CALayer {
            Retained::into_raw(self.ivars().root_layer.clone())
        }

        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, _event: &NSEvent) {
            self.ivars()
                .hub_sender
                .send(HubEvent::MirrorClicked(self.ivars().cg_id.get()))
                .ok();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl FloatOverlayView {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        root_layer: Retained<CALayer>,
        hub_sender: CalloopSender<HubEvent>,
        cg_id: CGWindowID,
    ) -> Retained<Self> {
        let ivars = FloatOverlayViewIvars {
            root_layer,
            hub_sender,
            cg_id: Cell::new(cg_id),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        unsafe { msg_send![super(this), initWithFrame: frame] }
    }
}

pub(super) struct TilingOverlayViewIvars {
    #[expect(dead_code, reason = "retains CAMetalLayer to prevent deallocation")]
    layer: Retained<CAMetalLayer>,
    renderer: RefCell<Renderer>,
    monitor: Cell<Dimension>,
    windows: RefCell<Vec<TilingWindowPlacement>>,
    containers: RefCell<Vec<ContainerShow>>,
    config: RefCell<Config>,
    tab_bar_height: Cell<Length<Logical>>,
    scale: Cell<f64>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TilingOverlayViewIvars]
    pub(super) struct TilingOverlayView;

    unsafe impl NSObjectProtocol for TilingOverlayView {}

    impl TilingOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }
    }
);

impl TilingOverlayView {
    fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        config: Config,
        tab_bar_height: Length<Logical>,
        scale: f64,
        flavor: Flavor,
        font: &FontConfig,
    ) -> Retained<Self> {
        let renderer = Renderer::new(backend, scale, 0.0, 0.0, false, flavor, font);
        let layer = renderer.layer();
        let ivars = TilingOverlayViewIvars {
            layer: layer.clone(),
            renderer: RefCell::new(renderer),
            monitor: Cell::new(Dimension::default()),
            windows: RefCell::new(Vec::new()),
            containers: RefCell::new(Vec::new()),
            config: RefCell::new(config),
            tab_bar_height: Cell::new(tab_bar_height),
            scale: Cell::new(scale),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    fn update(
        &self,
        monitor: Dimension,
        windows: &[TilingWindowPlacement],
        containers: &[ContainerShow],
        scale: f64,
    ) {
        let ivars = self.ivars();
        ivars.monitor.set(monitor);
        ivars.scale.set(scale);
        *ivars.windows.borrow_mut() = windows.to_vec();
        *ivars.containers.borrow_mut() = containers.to_vec();
        ivars.renderer.borrow().resize(
            monitor.width.logical() as f64,
            monitor.height.logical() as f64,
            scale,
        );
        self.render_now();
    }

    fn clear(&self) {
        let ivars = self.ivars();
        ivars.windows.borrow_mut().clear();
        ivars.containers.borrow_mut().clear();
    }

    fn set_config(&self, config: &Config) {
        let prev = self.ivars().config.borrow().clone();
        if prev.theme != config.theme {
            self.ivars().renderer.borrow().apply_theme(config.theme);
        }
        if prev.font != config.font {
            if prev.font.family != config.font.family {
                self.ivars()
                    .renderer
                    .borrow()
                    .reinstall_fonts(config.font.family.as_deref());
            }
            self.ivars().renderer.borrow().apply_font(&config.font);
        }
        *self.ivars().config.borrow_mut() = config.clone();
        self.render_now();
    }

    fn render_now(&self) {
        let ivars = self.ivars();
        let windows = ivars.windows.borrow();
        let containers = ivars.containers.borrow();
        let monitor = ivars.monitor.get();
        let config = ivars.config.borrow();
        let scale = ivars.scale.get();

        let monitor_logical = monitor;
        let windows_logical: Vec<LogicalTiledWindow> = windows
            .iter()
            .map(|wp| LogicalTiledWindow {
                id: wp.id,
                frame: wp.frame,
                visible_frame: wp.visible_frame,
                is_highlighted: wp.is_highlighted,
                spawn_indicator: wp.spawn_indicator,
            })
            .collect();
        let containers_logical: Vec<LogicalTiledContainer> = containers
            .iter()
            .map(|cs| LogicalTiledContainer {
                id: cs.placement.id,
                frame: cs.placement.frame,
                visible_frame: cs.placement.visible_frame,
                is_highlighted: cs.placement.is_highlighted,
                spawn_indicator: cs.placement.spawn_indicator,
                is_tabbed: cs.placement.is_tabbed,
                titles: cs.placement.titles.clone(),
            })
            .collect();
        let border = BorderMetrics::from_thickness(Length::<Logical>::new(config.border_size));
        let metrics = OverlayMetrics {
            border,
            tab_bar_height: ivars.tab_bar_height.get(),
        };
        let theme = config.theme();

        // Tab-bar painting and click collection live in per-container
        // TabBarOverlay windows. The per-monitor overlay paints only window
        // borders and container highlights.
        ivars
            .renderer
            .borrow_mut()
            .render(scale as f32, Vec::new(), |ctx| {
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

/// Per-tabbed-container overlay window. One instance per `ContainerId` while
/// the container is tabbed and on the active workspace, reconciled per-frame
/// in the UI thread's frame callback. Owns its own borderless window and
/// receives mouse events directly so a tab click never has to traverse the
/// per-monitor `TilingOverlay` (which is now click-through).
pub(super) struct TabBarOverlay {
    window: Retained<NSWindow>,
    view: Retained<TabBarOverlayView>,
}

impl TabBarOverlay {
    pub(super) fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        config: Config,
        container_id: ContainerId,
        cocoa_frame: NSRect,
        scale: f64,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Self {
        let flavor = config.theme;
        let font = config.font.clone();
        let window = unsafe {
            NSWindow::initWithContentRect_styleMask_backing_defer(
                NSWindow::alloc(mtm),
                cocoa_frame,
                NSWindowStyleMask::Borderless,
                NSBackingStoreType::Buffered,
                false,
            )
        };
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setOpaque(false);
        // Same level as the per-monitor tiling overlay. Stacking against
        // sibling same-level windows is fine because the tiling overlay is
        // mouse-transparent and visually empty in the strip the tab bar
        // covers.
        window.setLevel(NSNormalWindowLevel - 1);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Auxiliary
                | NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::Transient
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };
        // Inverse of the tiling overlay's setting: this window exists to
        // receive tab clicks.
        window.setIgnoresMouseEvents(false);

        let view = TabBarOverlayView::new(
            mtm,
            backend,
            config,
            container_id,
            scale,
            hub_sender,
            flavor,
            &font,
        );
        window.setContentView(Some(&view));
        window.setFrame_display(cocoa_frame, false);
        window.orderFront(None);

        Self { window, view }
    }

    pub(super) fn render(
        &self,
        cocoa_frame: NSRect,
        scale: f64,
        bar: Dimension<Logical>,
        titles: Vec<String>,
        active_tab_index: usize,
        is_highlighted: bool,
    ) {
        self.window.setFrame_display(cocoa_frame, false);
        self.view
            .update(scale, bar, titles, active_tab_index, is_highlighted);
        self.window.setIsVisible(true);
    }

    pub(super) fn set_config(&self, config: &Config) {
        self.view.set_config(config);
    }
}

impl Drop for TabBarOverlay {
    fn drop(&mut self) {
        self.window.close();
    }
}

pub(super) struct TabBarOverlayViewIvars {
    #[expect(dead_code, reason = "retains CAMetalLayer to prevent deallocation")]
    layer: Retained<CAMetalLayer>,
    events: RefCell<Vec<egui::Event>>,
    renderer: RefCell<Renderer>,
    bar: Cell<Dimension<Logical>>,
    titles: RefCell<Vec<String>>,
    active_tab_index: Cell<usize>,
    is_highlighted: Cell<bool>,
    scale: Cell<f64>,
    container_id: ContainerId,
    hub_sender: CalloopSender<HubEvent>,
    config: RefCell<Config>,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = TabBarOverlayViewIvars]
    pub(super) struct TabBarOverlayView;

    unsafe impl NSObjectProtocol for TabBarOverlayView {}

    impl TabBarOverlayView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

        // Both mouseDown: and mouseUp: are required: paint_tab_bar's
        // Sense::click() fires response.clicked() only on press-then-release,
        // so render_now must run on both edges to observe the second event.
        #[unsafe(method(mouseDown:))]
        fn mouse_down(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: true,
                modifiers: egui::Modifiers::NONE,
            });
            self.render_now();
        }

        #[unsafe(method(mouseUp:))]
        fn mouse_up(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars().events.borrow_mut().push(egui::Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: egui::Modifiers::NONE,
            });
            self.render_now();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl TabBarOverlayView {
    #[expect(
        clippy::too_many_arguments,
        reason = "all parameters are needed for overlay initialization"
    )]
    fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        config: Config,
        container_id: ContainerId,
        scale: f64,
        hub_sender: CalloopSender<HubEvent>,
        flavor: Flavor,
        font: &FontConfig,
    ) -> Retained<Self> {
        let renderer = Renderer::new(backend, scale, 0.0, 0.0, false, flavor, font);
        let layer = renderer.layer();
        let ivars = TabBarOverlayViewIvars {
            layer: layer.clone(),
            events: RefCell::new(Vec::new()),
            renderer: RefCell::new(renderer),
            bar: Cell::new(Dimension::default()),
            titles: RefCell::new(Vec::new()),
            active_tab_index: Cell::new(0),
            is_highlighted: Cell::new(false),
            scale: Cell::new(scale),
            container_id,
            hub_sender,
            config: RefCell::new(config),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(0.0, 0.0));
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    fn update(
        &self,
        scale: f64,
        bar: Dimension<Logical>,
        titles: Vec<String>,
        active_tab_index: usize,
        is_highlighted: bool,
    ) {
        let ivars = self.ivars();
        ivars.bar.set(bar);
        ivars.scale.set(scale);
        *ivars.titles.borrow_mut() = titles;
        ivars.active_tab_index.set(active_tab_index);
        ivars.is_highlighted.set(is_highlighted);
        ivars.renderer.borrow().resize(
            bar.width.logical() as f64,
            bar.height.logical() as f64,
            scale,
        );
        self.render_now();
    }

    fn set_config(&self, config: &Config) {
        let prev = self.ivars().config.borrow().clone();
        if prev.theme != config.theme {
            self.ivars().renderer.borrow().apply_theme(config.theme);
        }
        if prev.font != config.font {
            if prev.font.family != config.font.family {
                self.ivars()
                    .renderer
                    .borrow()
                    .reinstall_fonts(config.font.family.as_deref());
            }
            self.ivars().renderer.borrow().apply_font(&config.font);
        }
        *self.ivars().config.borrow_mut() = config.clone();
        self.render_now();
    }

    fn render_now(&self) {
        let ivars = self.ivars();
        let bar = ivars.bar.get();
        let titles = ivars.titles.borrow().clone();
        let active_tab_index = ivars.active_tab_index.get();
        let is_highlighted = ivars.is_highlighted.get();
        let config = ivars.config.borrow();
        let events = std::mem::take(&mut *ivars.events.borrow_mut());
        let scale = ivars.scale.get();
        let container_id = ivars.container_id;

        let border = BorderMetrics::from_thickness(Length::<Logical>::new(config.border_size));
        let metrics = OverlayMetrics {
            border,
            tab_bar_height: bar.height,
        };
        let theme = config.theme();

        // The tab-bar window's canvas is exactly the bar, so paint at the
        // canvas-local origin (0, 0) and let `paint_tab_bar` size its egui
        // Area to the bar's width and height.
        let canvas_local =
            Dimension::<Logical>::new(Length::ZERO, Length::ZERO, bar.width, bar.height);

        let clicked = ivars
            .renderer
            .borrow_mut()
            .render(scale as f32, events, |ctx| {
                overlay::paint_tab_bar(
                    ctx,
                    container_id,
                    canvas_local,
                    &titles,
                    active_tab_index,
                    is_highlighted,
                    metrics,
                    &theme,
                )
            });
        if let Some((cid, tab_idx)) = clicked {
            ivars
                .hub_sender
                .send(HubEvent::TabClicked(cid, tab_idx))
                .ok();
        }
    }

    fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
        let loc = event.locationInWindow();
        let view_loc = self.convertPoint_fromView(loc, None);
        egui::pos2(view_loc.x as f32, view_loc.y as f32)
    }
}
