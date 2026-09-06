use std::cell::RefCell;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSApplicationDidChangeScreenParametersNotification, NSControlStateValueOff,
    NSControlStateValueOn, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSSquareStatusItemLength,
    NSStatusBar, NSStatusBarButton, NSStatusItem,
};
use objc2_foundation::{
    NSData, NSInteger, NSNotification, NSNotificationCenter, NSObjectProtocol, NSSize, NSString,
};

use crate::{AppShellHandler, MenuEntry, MenuItem};

pub(crate) struct AppShell {
    status_item: Retained<NSStatusItem>,
    button: Retained<NSStatusBarButton>,
    // Kept alive because AppKit holds the menu delegate weakly. Dropping it would leave
    // the menu with no delegate and no click target.
    target: Retained<AppShellTarget>,
}

impl AppShell {
    pub(crate) fn new(icon_png: &[u8], handler: Box<dyn AppShellHandler>) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("AppShell::new must run on the main thread"))?;

        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSSquareStatusItemLength);
        let button = status_item
            .button(mtm)
            .ok_or_else(|| anyhow::anyhow!("NSStatusItem has no button"))?;

        let data = NSData::with_bytes(icon_png);
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .ok_or_else(|| anyhow::anyhow!("status bar icon must decode"))?;
        image.setSize(NSSize::new(22.0, 22.0));
        image.setTemplate(true);
        button.setImage(Some(&image));

        let target = AppShellTarget::new(mtm, handler);

        let menu = NSMenu::new(mtm);
        // AppKit auto-disables items whose action is unhandled, which would gray a
        // detached submenu's title and stop it expanding. Manual control keeps the
        // enabled state the handler asks for.
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(&*target)));
        status_item.setMenu(Some(&menu));

        let center = NSNotificationCenter::defaultCenter();
        unsafe {
            center.addObserver_selector_name_object(
                &target,
                sel!(screenParametersChanged:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
            );
        }

        Ok(Self {
            status_item,
            button,
            target,
        })
    }

    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        let ns = NSString::from_str(tooltip);
        self.button.setToolTip(Some(&ns));
    }
}

impl Drop for AppShell {
    fn drop(&mut self) {
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(&self.target) };
        NSStatusBar::systemStatusBar().removeStatusItem(&self.status_item);
    }
}

fn make_item(
    mtm: MainThreadMarker,
    target: &AppShellTarget,
    item: &MenuItem,
    empty: &NSString,
) -> Retained<NSMenuItem> {
    let title = NSString::from_str(&item.label);
    let alloc = NSMenuItem::alloc(mtm);
    let ns_item: Retained<NSMenuItem> = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            alloc,
            &title,
            Some(sel!(menuClicked:)),
            empty,
        )
    };
    ns_item.setTag(item.id as NSInteger);
    unsafe { ns_item.setTarget(Some(target)) };
    ns_item.setState(if item.checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    ns_item
}

fn rebuild_menu(
    mtm: MainThreadMarker,
    menu: &NSMenu,
    target: &AppShellTarget,
    entries: &[MenuEntry],
) {
    menu.removeAllItems();
    let empty = NSString::from_str("");
    for entry in entries {
        match entry {
            MenuEntry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            MenuEntry::Item(item) => menu.addItem(&make_item(mtm, target, item, &empty)),
            MenuEntry::Submenu {
                label,
                items,
                enabled,
            } => {
                let ns_title = NSString::from_str(label);
                let parent: Retained<NSMenuItem> = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &ns_title,
                        None,
                        &empty,
                    )
                };
                let submenu = NSMenu::new(mtm);
                submenu.setAutoenablesItems(false);
                for item in items {
                    submenu.addItem(&make_item(mtm, target, item, &empty));
                }
                parent.setSubmenu(Some(&submenu));
                parent.setEnabled(*enabled);
                menu.addItem(&parent);
            }
        }
    }
}

struct AppShellTargetIvars {
    handler: RefCell<Box<dyn AppShellHandler>>,
}

define_class!(
    #[unsafe(super(objc2_foundation::NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DomeAppShellTarget"]
    #[ivars = AppShellTargetIvars]
    struct AppShellTarget;

    unsafe impl NSObjectProtocol for AppShellTarget {}

    unsafe impl NSMenuDelegate for AppShellTarget {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            let mtm = MainThreadMarker::new().expect("menuNeedsUpdate runs on the main thread");
            let entries = self.ivars().handler.borrow_mut().menu();
            rebuild_menu(mtm, menu, self, &entries);
        }
    }

    impl AppShellTarget {
        #[unsafe(method(menuClicked:))]
        fn menu_clicked(&self, sender: &NSMenuItem) {
            let id = sender.tag() as u32;
            self.ivars().handler.borrow_mut().on_menu_selected(id);
        }

        #[unsafe(method(screenParametersChanged:))]
        fn screen_parameters_changed(&self, _notification: &NSNotification) {
            self.ivars().handler.borrow_mut().on_display_changed();
        }
    }
);

impl AppShellTarget {
    fn new(mtm: MainThreadMarker, handler: Box<dyn AppShellHandler>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppShellTargetIvars {
            handler: RefCell::new(handler),
        });
        unsafe { msg_send![super(this), init] }
    }
}
