use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use calloop::channel::Sender as CalloopSender;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSEvent, NSFloatingWindowLevel, NSResponder, NSView, NSWindow,
    NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_foundation::{NSObject, NSObjectProtocol, NSPoint, NSRect, NSSize};
use objc2_quartz_core::CAMetalLayer;

use super::renderer::{MetalBackend, Renderer};
use crate::action::{Action, Actions};
use crate::core::Dimension;
use crate::picker::{PickerEntry, PickerResult};
use crate::platform::macos::dome::HubEvent;
use crate::theme::{Flavor, Theme};

/// Snapshot of the icon texture cache plus newly loaded images awaiting TextureHandle conversion.
type PendingIcons = (
    HashMap<String, Option<egui::TextureHandle>>,
    Vec<(String, egui::ColorImage)>,
);

struct PickerWindowIvars {
    hub_sender: CalloopSender<HubEvent>,
    view: RefCell<Option<Retained<PickerView>>>,
}

define_class!(
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PickerWindowIvars]
    struct PickerWindow;

    unsafe impl NSObjectProtocol for PickerWindow {}

    impl PickerWindow {
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }

        #[unsafe(method(resignKeyWindow))]
        fn resign_key_window(&self) {
            let _: () = unsafe { msg_send![super(self), resignKeyWindow] };
            // No event sent. orderOut is a no-op if already hidden (e.g. after
            // keyDown: Escape/Return already hid the window).
            self.orderOut(None);
        }

        #[unsafe(method(keyDown:))]
        fn key_down(&self, event: &NSEvent) {
            let keycode = event.keyCode();
            let ivars = self.ivars();
            let Some(view) = ivars.view.borrow().as_ref().map(|v| v.clone()) else {
                return;
            };
            let view_ivars = view.ivars();
            match keycode {
                // Up arrow
                0x7E => {
                    let idx = view_ivars.selected_index.get();
                    if idx > 0 {
                        view_ivars.selected_index.set(idx - 1);
                    }
                    view.render_now();
                }
                // Down arrow
                0x7D => {
                    let max = view_ivars.entries.borrow().len().saturating_sub(1);
                    let idx = view_ivars.selected_index.get();
                    if idx < max {
                        view_ivars.selected_index.set(idx + 1);
                    }
                    view.render_now();
                }
                // Return -- select
                0x24 => {
                    let entries = view_ivars.entries.borrow();
                    let idx = view_ivars.selected_index.get();
                    if let Some(entry) = entries.get(idx) {
                        let actions = Actions::new(vec![Action::UnminimizeWindow(entry.id)]);
                        ivars.hub_sender.send(HubEvent::Action(actions)).ok();
                    }
                    self.orderOut(None);
                }
                // Escape
                0x35 => {
                    self.orderOut(None);
                }
                _ => {}
            }
        }
    }
);

impl PickerWindow {
    fn new(
        mtm: MainThreadMarker,
        frame: NSRect,
        style: NSWindowStyleMask,
        hub_sender: CalloopSender<HubEvent>,
    ) -> Retained<Self> {
        let ivars = PickerWindowIvars {
            hub_sender,
            view: RefCell::new(None),
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
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

const PICKER_WIDTH: f64 = 400.0;
const PICKER_HEIGHT: f64 = 300.0;

struct PickerViewIvars {
    // Retained to keep the CAMetalLayer alive for the lifetime of the view;
    // also read to set corner radius / masksToBounds.
    layer: Retained<CAMetalLayer>,
    events: RefCell<Vec<egui::Event>>,
    renderer: RefCell<Renderer>,
    entries: RefCell<Vec<PickerEntry>>,
    selected_index: Cell<usize>,
    scale: Cell<f64>,
    hub_sender: CalloopSender<HubEvent>,
    icon_textures: RefCell<HashMap<String, Option<egui::TextureHandle>>>,
    flavor: Flavor,
}

define_class!(
    #[unsafe(super(NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[ivars = PickerViewIvars]
    struct PickerView;

    unsafe impl NSObjectProtocol for PickerView {}

    impl PickerView {
        #[unsafe(method(isFlipped))]
        fn is_flipped(&self) -> bool {
            true
        }

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

        #[unsafe(method(mouseMoved:))]
        fn mouse_moved(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars()
                .events
                .borrow_mut()
                .push(egui::Event::PointerMoved(pos));
            self.render_now();
        }

        #[unsafe(method(mouseDragged:))]
        fn mouse_dragged(&self, event: &NSEvent) {
            let pos = self.event_pos(event);
            self.ivars()
                .events
                .borrow_mut()
                .push(egui::Event::PointerMoved(pos));
            self.render_now();
        }

        #[unsafe(method(acceptsFirstMouse:))]
        fn accepts_first_mouse(&self, _event: Option<&NSEvent>) -> bool {
            true
        }
    }
);

impl PickerView {
    fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        entries: Vec<PickerEntry>,
        render_info: (f64, f64, f64),
        hub_sender: CalloopSender<HubEvent>,
        flavor: Flavor,
    ) -> Retained<Self> {
        let (scale, width, height) = render_info;
        let renderer = Renderer::new(backend, scale, width, height, false, flavor);
        let theme = Theme::from_flavor(flavor);
        // Renderer::new called set_theme with this flavor, which wrote catppuccin
        // values into egui Visuals. The set_visuals call below fully overwrites
        // those Visuals with picker_visuals(&theme), so at runtime the earlier
        // set_theme is redundant for the picker. We keep it so every Renderer in
        // the process is constructed uniformly, and so the picker stays themed if
        // this picker-specific set_visuals is ever removed.
        renderer.set_visuals(crate::picker::picker_visuals(&theme));
        let layer = renderer.layer();
        let ivars = PickerViewIvars {
            layer: layer.clone(),
            events: RefCell::new(Vec::new()),
            renderer: RefCell::new(renderer),
            entries: RefCell::new(entries),
            selected_index: Cell::new(0),
            scale: Cell::new(scale),
            hub_sender,
            icon_textures: RefCell::new(HashMap::new()),
            flavor,
        };
        let this = Self::alloc(mtm).set_ivars(ivars);
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        let view: Retained<Self> = unsafe { msg_send![super(this), initWithFrame: frame] };
        view.setLayer(Some(&layer));
        view.setWantsLayer(true);
        view
    }

    fn render_now(&self) {
        let ivars = self.ivars();

        let entries = ivars.entries.borrow();
        let selected_index = ivars.selected_index.get();
        let events = std::mem::take(&mut *ivars.events.borrow_mut());
        let scale = ivars.scale.get();

        let (icon_snapshot, mut new_icons) = self.load_pending_icons(&entries);
        let flavor = ivars.flavor;

        // Convert new ColorImages to TextureHandles inside the render closure
        // (requires egui Context), then call paint_picker with the merged map.
        let (result, new_textures) =
            ivars
                .renderer
                .borrow_mut()
                .render(scale as f32, events, None, |ctx| {
                    let mut all_icons = icon_snapshot.clone();
                    let mut created = Vec::new();
                    for (app_id, image) in new_icons.drain(..) {
                        let handle = ctx.load_texture(
                            &app_id,
                            image,
                            Default::default(), // TextureOptions default is fine for icon textures
                        );
                        all_icons.insert(app_id.clone(), Some(handle.clone()));
                        created.push((app_id, handle));
                    }
                    let theme = Theme::from_flavor(flavor);
                    let picker_result = crate::picker::paint_picker(
                        ctx,
                        &entries,
                        selected_index,
                        &all_icons,
                        &theme,
                    );
                    (picker_result, created)
                });

        self.commit_icons(new_textures);

        if let PickerResult::Selected(id) = result {
            let actions = Actions::new(vec![Action::UnminimizeWindow(id)]);
            ivars.hub_sender.send(HubEvent::Action(actions)).ok();
            self.window().unwrap().orderOut(None);
        }
    }

    /// Loads icons for entries not yet in the cache. Inserts None sentinels for
    /// entries being attempted so failed loads are not retried. Returns a snapshot
    /// of the current icon_textures cache and any newly loaded ColorImages that
    /// still need TextureHandle conversion (which requires the egui Context).
    fn load_pending_icons(&self, entries: &[PickerEntry]) -> PendingIcons {
        let ivars = self.ivars();
        let mut new_icons: Vec<(String, egui::ColorImage)> = Vec::new();
        for entry in entries.iter() {
            if let Some(app_id) = &entry.app_id
                && !ivars.icon_textures.borrow().contains_key(app_id)
            {
                // Sentinel prevents re-attempting failed loads within a session.
                ivars
                    .icon_textures
                    .borrow_mut()
                    .insert(app_id.clone(), None);
                if let Some(image) = super::icon::load_app_icon(app_id) {
                    new_icons.push((app_id.clone(), image));
                }
            }
        }
        let snapshot = ivars.icon_textures.borrow().clone();
        (snapshot, new_icons)
    }

    /// Inserts newly created TextureHandles into the persistent icon cache.
    fn commit_icons(&self, new_textures: Vec<(String, egui::TextureHandle)>) {
        let mut textures = self.ivars().icon_textures.borrow_mut();
        for (app_id, handle) in new_textures {
            textures.insert(app_id, Some(handle));
        }
    }

    fn event_pos(&self, event: &NSEvent) -> egui::Pos2 {
        let loc = event.locationInWindow();
        let view_loc = self.convertPoint_fromView(loc, None);
        egui::pos2(view_loc.x as f32, view_loc.y as f32)
    }

    fn update(
        &self,
        _mtm: MainThreadMarker,
        entries: Vec<PickerEntry>,
        scale: f64,
        width: f64,
        height: f64,
    ) {
        let ivars = self.ivars();
        *ivars.entries.borrow_mut() = entries;
        ivars.selected_index.set(0);
        ivars.scale.set(scale);
        // Clear failed-load sentinels so relaunched apps can be retried.
        // Loaded Some(_) entries are preserved as valid cache.
        ivars.icon_textures.borrow_mut().retain(|_, v| v.is_some());
        let frame = NSRect::new(NSPoint::new(0.0, 0.0), NSSize::new(width, height));
        self.setFrame(frame);
        ivars.renderer.borrow().resize(width, height, scale);
    }
}

/// Opaque picker popup window for browsing and restoring minimized windows.
pub(super) struct PickerPopup {
    window: Retained<PickerWindow>,
}

impl PickerPopup {
    pub(super) fn new(
        mtm: MainThreadMarker,
        backend: Rc<MetalBackend>,
        entries: Vec<PickerEntry>,
        monitor: (Dimension, NSRect, f64),
        hub_sender: CalloopSender<HubEvent>,
        flavor: Flavor,
    ) -> Self {
        let (monitor_dim, cocoa_frame, scale) = monitor;
        let pw = PICKER_WIDTH.min(monitor_dim.width as f64);
        let ph = PICKER_HEIGHT.min(monitor_dim.height as f64);
        // Center on the monitor's Cocoa frame
        let x = cocoa_frame.origin.x + (cocoa_frame.size.width - pw) / 2.0;
        let y = cocoa_frame.origin.y + (cocoa_frame.size.height - ph) / 2.0;
        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(pw, ph));

        let window = PickerWindow::new(
            mtm,
            frame,
            NSWindowStyleMask::Borderless,
            hub_sender.clone(),
        );
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setHasShadow(true);
        window.setLevel(NSFloatingWindowLevel);
        window.setCollectionBehavior(
            NSWindowCollectionBehavior::Default
                | NSWindowCollectionBehavior::FullScreenNone
                | NSWindowCollectionBehavior::FullScreenDisallowsTiling
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        unsafe { window.setReleasedWhenClosed(false) };
        window.setAcceptsMouseMovedEvents(true);

        let view = PickerView::new(mtm, backend, entries, (scale, pw, ph), hub_sender, flavor);
        view.ivars().layer.setCornerRadius(12.0);
        view.ivars().layer.setMasksToBounds(true);
        window.setContentView(Some(&view));
        *window.ivars().view.borrow_mut() = Some(view.clone());
        window.makeKeyAndOrderFront(None);
        view.render_now();

        Self { window }
    }

    pub(super) fn is_visible(&self) -> bool {
        self.window.isVisible()
    }

    pub(super) fn hide(&self) {
        self.window.orderOut(None);
    }

    pub(super) fn update_and_show(
        &self,
        mtm: MainThreadMarker,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        cocoa_frame: NSRect,
        scale: f64,
    ) {
        let pw = PICKER_WIDTH.min(monitor_dim.width as f64);
        let ph = PICKER_HEIGHT.min(monitor_dim.height as f64);
        let x = cocoa_frame.origin.x + (cocoa_frame.size.width - pw) / 2.0;
        let y = cocoa_frame.origin.y + (cocoa_frame.size.height - ph) / 2.0;
        let frame = NSRect::new(NSPoint::new(x, y), NSSize::new(pw, ph));
        self.window.setFrame_display(frame, false);

        let view = self.window.ivars().view.borrow();
        let view = view.as_ref().expect("view set during new()");
        view.update(mtm, entries, scale, pw, ph);

        self.window.makeKeyAndOrderFront(None);
        view.render_now();
    }
}
