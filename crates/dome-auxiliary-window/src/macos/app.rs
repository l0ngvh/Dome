use std::cell::OnceCell;
use std::hash::Hash;
use std::rc::Rc;
use std::sync::atomic::Ordering;
use std::sync::{Arc, mpsc};

use dispatch2::DispatchQueue;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, Message, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDelegate,
    NSApplicationDidChangeScreenParametersNotification, NSApplicationTerminateReply,
    NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuDelegate, NSMenuItem,
    NSSquareStatusItemLength, NSStatusBar, NSStatusBarButton, NSStatusItem,
};
use objc2_core_foundation::{CFRunLoop, kCFRunLoopCommonModes};
use objc2_foundation::{
    NSData, NSInteger, NSNotification, NSNotificationCenter, NSObject, NSObjectProtocol, NSSize,
    NSString,
};

use super::run_loop::{
    CallbackContext, DispatchBridge, LoopHandle, LoopState, NativeDispatch, WakeTarget,
    create_wake_source,
};
use crate::delivery::{Callbacks, WakeDelivery};
use crate::{AppHandler, LoopWaker, MenuEntry, MenuItem};

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

pub(crate) enum Shell {
    Button(Retained<NSStatusBarButton>),
    #[cfg(test)]
    Test,
}

impl Shell {
    pub(crate) fn set_tooltip(&self, tooltip: &str) {
        match self {
            Self::Button(button) => button.setToolTip(Some(&NSString::from_str(tooltip))),
            #[cfg(test)]
            Self::Test => panic!("test shell has no native tooltip"),
        }
    }
}

/// The status item and its button, created at launch and held for the app's lifetime.
struct ShellState {
    status_item: Retained<NSStatusItem>,
    button: Retained<NSStatusBarButton>,
}

pub(crate) struct App<W: Clone + Eq + Hash + Send + 'static> {
    app: Retained<NSApplication>,
    delegate: Retained<AppDelegate>,
    state: Rc<LoopState<W>>,
    waker: LoopWaker<W>,
    context: Arc<CallbackContext>,
    _bridge: Box<DispatchBridge>,
    mtm: MainThreadMarker,
}

impl<W: Clone + Eq + Hash + Send + 'static> App<W> {
    pub(crate) fn new(icon: Icon, handler: Box<dyn AppHandler<Wake = W>>) -> anyhow::Result<Self> {
        let mtm = MainThreadMarker::new()
            .ok_or_else(|| anyhow::anyhow!("App::new must run on the main thread"))?;
        let app = NSApplication::sharedApplication(mtm);
        app.setActivationPolicy(NSApplicationActivationPolicy::Regular);
        let context = Arc::new(CallbackContext::new());
        let source = create_wake_source(&context);
        let run_loop = CFRunLoop::main().expect("main run loop");
        let (sender, receiver) = mpsc::channel();
        let waker = LoopWaker {
            sender,
            target: WakeTarget {
                source,
                run_loop,
                #[cfg(test)]
                recorder: None,
            },
        };
        let state = Rc::new(LoopState::new(
            WakeDelivery::new(receiver, handler),
            context.clone(),
            waker.clone(),
        ));
        let mut bridge = Box::new(DispatchBridge {
            dispatch: state.clone(),
        });
        let delegate = AppDelegate::new(mtm, icon, state.clone(), context.clone());
        context.dispatch.store(&mut *bridge, Ordering::Release);
        waker
            .target
            .run_loop
            .add_source(Some(&waker.target.source), unsafe { kCFRunLoopCommonModes });
        Ok(Self {
            app,
            delegate,
            state,
            waker,
            context,
            _bridge: bridge,
            mtm,
        })
    }

    pub(crate) fn waker(&self) -> LoopWaker<W> {
        self.waker.clone()
    }

    pub(crate) fn handle(&self) -> LoopHandle<W> {
        LoopHandle::new(&self.state, self.mtm)
    }

    pub(crate) fn run(self) {
        self.app
            .setDelegate(Some(ProtocolObject::from_ref(&*self.delegate)));
        let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| self.app.run()));
        self.state.stopping();
    }
}

impl<W: Clone + Eq + Hash + Send + 'static> Drop for App<W> {
    fn drop(&mut self) {
        self.context.clear();
        self.state.close();
        self.waker.target.source.invalidate();
        self.waker
            .target
            .run_loop
            .remove_source(Some(&self.waker.target.source), unsafe {
                kCFRunLoopCommonModes
            });
        self.app.setDelegate(None);
        unsafe { NSNotificationCenter::defaultCenter().removeObserver(&self.delegate) };
        if let Some(state) = self.delegate.ivars().shell.get() {
            if let Some(menu) = state.status_item.menu(self.mtm) {
                detach_menu(&menu);
            }
            state.status_item.setMenu(None);
            NSStatusBar::systemStatusBar().removeStatusItem(&state.status_item);
        }
    }
}

fn detach_menu(menu: &NSMenu) {
    menu.setDelegate(None);
    detach_items(menu);
}

fn detach_items(menu: &NSMenu) {
    for item in menu.itemArray() {
        unsafe { item.setTarget(None) };
        if let Some(submenu) = item.submenu() {
            detach_menu(&submenu);
        }
    }
    menu.removeAllItems();
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
    detach_items(menu);
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
    callbacks: Rc<dyn NativeDispatch>,
    context: Arc<CallbackContext>,
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
            let shell = crate::Shell::new(Shell::Button(state.button.clone()));
            self.ivars().callbacks.started(shell);
        }

        // A terminate from inside a handler callback would reach will_terminate while the
        // callback still borrows the handler. Cancel it and retry on the next main
        // run-loop turn, after the borrow is released. TerminateLater is not safe here,
        // because it waits in a nested run loop that can still be inside the callback.
        #[unsafe(method(applicationShouldTerminate:))]
        fn should_terminate(&self, _sender: &NSApplication) -> NSApplicationTerminateReply {
            if !self.ivars().callbacks.active() {
                return NSApplicationTerminateReply::TerminateNow;
            }
            let context = self.ivars().context.clone();
            DispatchQueue::main().exec_async(move || {
                if context.dispatch().is_some() {
                    let mtm = MainThreadMarker::new()
                        .expect("main dispatch queue runs on the main thread");
                    NSApplication::sharedApplication(mtm).terminate(None);
                }
            });
            NSApplicationTerminateReply::TerminateCancel
        }

        #[unsafe(method(applicationWillTerminate:))]
        fn will_terminate(&self, _notification: &NSNotification) {
            self.ivars().callbacks.stopping();
        }
    }

    unsafe impl NSMenuDelegate for AppDelegate {
        #[unsafe(method(menuNeedsUpdate:))]
        fn menu_needs_update(&self, menu: &NSMenu) {
            let mtm = MainThreadMarker::new().expect("menuNeedsUpdate runs on the main thread");
            let menu = menu.retain();
            let target = self.retain();
            self.ivars().callbacks.menu(Box::new(move |entries| {
                rebuild_menu(mtm, &menu, &target, &entries);
            }));
        }
    }

    impl AppDelegate {
        #[unsafe(method(menuClicked:))]
        fn menu_clicked(&self, sender: &NSMenuItem) {
            let id = sender.tag() as u32;
            self.ivars().callbacks.selected(id);
        }

        #[unsafe(method(screenParametersChanged:))]
        fn screen_parameters_changed(&self, _notification: &NSNotification) {
            self.ivars().callbacks.display_changed();
        }
    }
);

impl AppDelegate {
    fn new(
        mtm: MainThreadMarker,
        icon: Icon,
        callbacks: Rc<dyn NativeDispatch>,
        context: Arc<CallbackContext>,
    ) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(AppDelegateIvars {
            callbacks,
            context,
            icon,
            shell: OnceCell::new(),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn install_status_item(&self, mtm: MainThreadMarker) {
        let status_item =
            NSStatusBar::systemStatusBar().statusItemWithLength(NSSquareStatusItemLength);
        let Some(button) = status_item.button(mtm) else {
            NSStatusBar::systemStatusBar().removeStatusItem(&status_item);
            panic!("NSStatusItem has no button");
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
