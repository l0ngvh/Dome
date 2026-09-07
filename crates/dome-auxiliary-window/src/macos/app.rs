use std::cell::{OnceCell, RefCell};
use std::ffi::c_void;

use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationDidChangeScreenParametersNotification, NSControlStateValueOff,
    NSControlStateValueOn, NSImage, NSMenu, NSMenuDelegate, NSMenuItem, NSSquareStatusItemLength,
    NSStatusBar, NSStatusBarButton, NSStatusItem,
};
use objc2_core_foundation::{CFRetained, CFRunLoop, CFRunLoopSource, kCFRunLoopDefaultMode};
use objc2_foundation::{
    NSData, NSInteger, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSSize,
    NSString,
};

use super::run_loop::{LoopHandle, LoopWaker, create_wake_source};
use crate::{AppHandler, MenuEntry, MenuItem};

/// A decoded menu-bar icon. Built before the loop runs, applied to the status button at
/// launch.
pub(crate) struct Icon {
    image: Retained<NSImage>,
}

impl Icon {
    pub(crate) fn from_png(bytes: &[u8]) -> anyhow::Result<Self> {
        let data = NSData::with_bytes(bytes);
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .ok_or_else(|| anyhow::anyhow!("status bar icon must decode"))?;
        image.setSize(NSSize::new(22.0, 22.0));
        // Template mode lets AppKit tint the alpha-defined shape for dark and light menu
        // bars.
        image.setTemplate(true);
        Ok(Self { image })
    }
}

/// A handle to the status item's button. Owns a retain rather than a borrow, so the
/// public `Shell` carries no lifetime.
pub(crate) struct Shell {
    button: Retained<NSStatusBarButton>,
}

impl Shell {
    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        self.button.setToolTip(Some(&NSString::from_str(tooltip)));
    }
}

/// The status item and its button, created at launch and held for the app's lifetime.
struct ShellState {
    status_item: Retained<NSStatusItem>,
    button: Retained<NSStatusBarButton>,
}

pub(crate) struct App {
    app: Retained<NSApplication>,
    delegate: Retained<AppDelegate>,
    source: CFRetained<CFRunLoopSource>,
    run_loop: CFRetained<CFRunLoop>,
    mtm: MainThreadMarker,
}

impl App {
    pub(crate) fn new(icon: Icon, handler: Box<dyn AppHandler>) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("App::new must run on the main thread"))?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);

        let delegate = AppDelegate::new(mtm, icon, handler);
        let source = create_wake_source(Retained::as_ptr(&delegate) as *mut c_void, wake_callback);
        let run_loop = CFRunLoop::main().unwrap();
        run_loop.add_source(Some(&source), unsafe { kCFRunLoopDefaultMode });

        Ok(Self {
            app,
            delegate,
            source,
            run_loop,
            mtm,
        })
    }

    pub(crate) fn waker(&self) -> LoopWaker {
        LoopWaker::new(self.source.clone(), self.run_loop.clone())
    }

    pub(crate) fn handle(&self) -> LoopHandle {
        LoopHandle::new(self.mtm)
    }

    pub(crate) fn run(self) {
        self.app
            .setDelegate(Some(ProtocolObject::from_ref(&*self.delegate)));

        if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.app.run())).is_err() {
            self.delegate.ivars().handler.borrow_mut().on_stopping();
        }
    }
}

impl Drop for App {
    fn drop(&mut self) {
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(&self.delegate) };
        if let Some(state) = self.delegate.ivars().shell.get() {
            NSStatusBar::systemStatusBar().removeStatusItem(&state.status_item);
        }
    }
}

fn make_item(
    mtm: MainThreadMarker,
    target: &AppDelegate,
    item: &MenuItem,
    empty: &NSString,
) -> Retained<NSMenuItem> {
    let title = NSString::from_str(&item.label);
    let ns_item: Retained<NSMenuItem> = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
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

fn rebuild_menu(mtm: MainThreadMarker, menu: &NSMenu, target: &AppDelegate, entries: &[MenuEntry]) {
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

struct AppDelegateIvars {
    handler: RefCell<Box<dyn AppHandler>>,
    icon: Icon,
    shell: OnceCell<ShellState>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DomeAppDelegate"]
    #[ivars = AppDelegateIvars]
    struct AppDelegate;

    unsafe impl NSObjectProtocol for AppDelegate {}

    unsafe impl NSApplicationDelegate for AppDelegate {
        #[unsafe(method(applicationDidFinishLaunching:))]
        fn did_finish_launching(&self, _notification: &NSNotification) {
            let mtm = MainThreadMarker::new()
                .expect("applicationDidFinishLaunching runs on the main thread");
            self.install_status_item(mtm);
            let state = self.ivars().shell.get().expect("status item set at launch");
            let shell = crate::Shell::new(Shell {
                button: state.button.clone(),
            });
            self.ivars().handler.borrow_mut().on_started(&shell);
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.ivars().handler.borrow_mut().on_stopping();
        }
    }

    unsafe impl NSMenuDelegate for AppDelegate {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            let mtm = MainThreadMarker::new().expect("menuNeedsUpdate runs on the main thread");
            let entries = self.ivars().handler.borrow_mut().menu();
            rebuild_menu(mtm, menu, self, &entries);
        }
    }

    impl AppDelegate {
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

impl AppDelegate {
    fn new(mtm: MainThreadMarker, icon: Icon, handler: Box<dyn AppHandler>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars {
            handler: RefCell::new(handler),
            icon,
            shell: OnceCell::new(),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn install_status_item(&self, mtm: MainThreadMarker) {
        let status_item =
            NSStatusBar::systemStatusBar().statusItemWithLength(NSSquareStatusItemLength);
        let Some(button) = status_item.button(mtm) else {
            tracing::error!("NSStatusItem has no button");
            return;
        };
        button.setImage(Some(&self.ivars().icon.image));

        let menu = NSMenu::new(mtm);
        // AppKit auto-disables items whose action is unhandled, which would gray a
        // detached submenu's title and stop it expanding. Manual control keeps the
        // enabled state the handler asks for.
        menu.setAutoenablesItems(false);
        menu.setDelegate(Some(ProtocolObject::from_ref(self)));
        status_item.setMenu(Some(&menu));

        let center = NSNotificationCenter::defaultCenter();
        unsafe {
            center.addObserver_selector_name_object(
                self,
                sel!(screenParametersChanged:),
                Some(NSApplicationDidChangeScreenParametersNotification),
                None,
            );
        }

        if self
            .ivars()
            .shell
            .set(ShellState {
                status_item,
                button,
            })
            .is_err()
        {
            unreachable!("status item set once at launch");
        }
    }
}

// Keeps the `C-unwind` ABI so a panic unwinds through the CoreFoundation frame instead
// of aborting.
unsafe extern "C-unwind" fn wake_callback(info: *mut c_void) {
    let delegate: &AppDelegate = unsafe { &*(info as *const AppDelegate) };
    let ivars = delegate.ivars();
    let Some(state) = ivars.shell.get() else {
        return;
    };
    let shell = crate::Shell::new(Shell {
        button: state.button.clone(),
    });
    ivars.handler.borrow_mut().on_wake(&shell);
}
