use std::ptr::NonNull;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use crate::platform::macos::event_loop::EventSender;

use crate::platform::macos::timer::Timer;
use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::ProtocolObject;
use objc2_app_kit::{
    NSWorkspace, NSWorkspaceActiveSpaceDidChangeNotification,
    NSWorkspaceDidActivateApplicationNotification, NSWorkspaceDidHideApplicationNotification,
    NSWorkspaceDidLaunchApplicationNotification, NSWorkspaceDidTerminateApplicationNotification,
    NSWorkspaceDidUnhideApplicationNotification, NSWorkspaceScreensDidSleepNotification,
    NSWorkspaceWillSleepNotification,
};
use objc2_core_graphics::CGWindowID;
use objc2_foundation::{
    NSDistributedNotificationCenter, NSNotification, NSObjectProtocol, NSOperationQueue, NSString,
};

use crate::platform::macos::running_application::RunningApp;
use crate::platform::macos::throttle::Throttle;
use crate::platform::macos::window_server::NativeEvent;
use crate::platform::macos::window_server::send_native_event;

const FRAME_THROTTLE: Duration = Duration::from_millis(16);
const SYNC_INTERVAL: Duration = Duration::from_secs(5);

pub(super) struct EventListener {
    ctx: Box<ListenerContext>,
    sync_timer: Option<Timer>,
    workspace_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
    distributed_observers: Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>,
}

impl EventListener {
    pub(super) fn focus_observed(&self, pid: i32) {
        self.ctx.focus_throttle.submit(pid);
    }

    pub(super) fn title_observed(&self, id: CGWindowID) {
        self.ctx.title_throttle.submit(id);
    }

    pub(super) fn new(sender: EventSender<NativeEvent>, is_suspended: Arc<AtomicBool>) -> Self {
        let (focus_throttle, title_throttle) = setup_throttles(sender.clone());

        let mut ctx = Box::new(ListenerContext {
            is_suspended,
            focus_throttle,
            title_throttle,
            sender,
        });

        let (workspace_observers, distributed_observers) = setup_app_observers(&mut ctx);
        let sender = ctx.sender.clone();
        let suspended = ctx.is_suspended.clone();
        let sync_timer = Timer::repeating(SYNC_INTERVAL, move || {
            if !suspended.load(Ordering::Relaxed) {
                send_native_event(&sender, NativeEvent::RefreshTimerElapsed);
            }
        });

        Self {
            ctx,
            sync_timer: Some(sync_timer),
            workspace_observers,
            distributed_observers,
        }
    }
}

struct ListenerContext {
    is_suspended: Arc<AtomicBool>,
    // Prevent feedback loop when Mac queue a focus event, but by the time the event is processed
    // the focus have been given to another window. This window then tries to take focus and
    // succeeds, but the focus event for the other window is already queued. The other window will
    // then proceed to take focus when the event is processed, which tries to take focus and
    // succeeds after the focus event for the original window is queued but before it's executed
    // and forms the feedback loop. By throttling the focus events, we can be confident that as
    // long as the processing of each focus event is shorter than the throttle duration, the
    // feedback loop can't be formed.
    focus_throttle: FocusThrottle,
    title_throttle: TitleThrottle,
    sender: EventSender<NativeEvent>,
}

type FocusThrottle = Throttle<i32>;

type TitleThrottle = Throttle<CGWindowID>;

impl Drop for EventListener {
    fn drop(&mut self) {
        self.sync_timer.take();

        let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
        for observer in &self.workspace_observers {
            unsafe { notification_center.removeObserver(ProtocolObject::as_ref(observer)) };
        }

        let distributed_center = NSDistributedNotificationCenter::defaultCenter();
        for observer in &self.distributed_observers {
            unsafe { distributed_center.removeObserver(ProtocolObject::as_ref(observer)) };
        }
    }
}

type WorkspaceObservers = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;
type DistributedObservers = Vec<Retained<ProtocolObject<dyn NSObjectProtocol>>>;

fn setup_app_observers(ctx: &mut ListenerContext) -> (WorkspaceObservers, DistributedObservers) {
    // To bypass FnMut and lifetime requirement of block2. ctx will outlive these callbacks as
    // these callbacks are removed on EventListener drop
    let ctx_ptr = ctx as *mut ListenerContext;

    let notification_center = NSWorkspace::sharedWorkspace().notificationCenter();
    let mut workspace_observers = Vec::new();

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidLaunchApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_launched(&*ctx_ptr, notification.as_ref());
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidTerminateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_terminated(&*ctx_ptr, notification.as_ref());
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceDidActivateApplicationNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |notification: NonNull<NSNotification>| {
                handle_app_activated(&mut *ctx_ptr, notification.as_ref());
            }),
        )
    });

    for (name, hidden) in unsafe {
        [
            (NSWorkspaceDidHideApplicationNotification, true),
            (NSWorkspaceDidUnhideApplicationNotification, false),
        ]
    } {
        workspace_observers.push(unsafe {
            notification_center.addObserverForName_object_queue_usingBlock(
                Some(name),
                None,
                Some(&NSOperationQueue::mainQueue()),
                &RcBlock::new(move |notification: NonNull<NSNotification>| {
                    let Some(app) = RunningApp::from_notification(notification.as_ref()) else {
                        return;
                    };
                    let event = if hidden {
                        NativeEvent::AppHidden { pid: app.pid() }
                    } else {
                        NativeEvent::AppShown { pid: app.pid() }
                    };
                    send_native_event(&(*ctx_ptr).sender, event);
                }),
            )
        });
    }

    // Suspend on system sleep, screen sleep, or lock, as AX APIs are unusable while under these
    // conditions
    // Resume ONLY on unlock, as screen can wake while locked, AX APIs are still unusable while
    // locked
    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceWillSleepNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("System will sleep, suspending window management");
                (*ctx_ptr).is_suspended.store(true, Ordering::Relaxed);
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceScreensDidSleepNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen did sleep, suspending window management");
                (*ctx_ptr).is_suspended.store(true, Ordering::Relaxed);
            }),
        )
    });

    workspace_observers.push(unsafe {
        notification_center.addObserverForName_object_queue_usingBlock(
            Some(NSWorkspaceActiveSpaceDidChangeNotification),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                send_native_event(&(*ctx_ptr).sender, NativeEvent::SpaceChanged);
            }),
        )
    });

    let distributed_center = NSDistributedNotificationCenter::defaultCenter();
    let lock_name = NSString::from_str("com.apple.screenIsLocked");
    let unlock_name = NSString::from_str("com.apple.screenIsUnlocked");
    let mut distributed_observers = Vec::new();

    distributed_observers.push(unsafe {
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(lock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen locked, suspending window management");
                (*ctx_ptr).is_suspended.store(true, Ordering::Relaxed);
            }),
        )
    });

    distributed_observers.push(unsafe {
        distributed_center.addObserverForName_object_queue_usingBlock(
            Some(unlock_name.as_ref()),
            None,
            Some(&NSOperationQueue::mainQueue()),
            &RcBlock::new(move |_: NonNull<NSNotification>| {
                tracing::info!("Screen unlocked, resuming window management");
                (*ctx_ptr).is_suspended.store(false, Ordering::Relaxed);
                send_native_event(&(*ctx_ptr).sender, NativeEvent::ScreenUnlocked);
            }),
        )
    });

    (workspace_observers, distributed_observers)
}

fn setup_throttles(sender: EventSender<NativeEvent>) -> (FocusThrottle, TitleThrottle) {
    let sender2 = sender.clone();

    let focus_throttle = Throttle::new(Duration::from_millis(500), move |pid: i32| {
        send_native_event(&sender, NativeEvent::FocusChanged { pid });
    });

    let title_throttle = Throttle::new(FRAME_THROTTLE, move |cg_id: CGWindowID| {
        send_native_event(&sender2, NativeEvent::TitleChanged(cg_id));
    });

    (focus_throttle, title_throttle)
}

fn handle_app_launched(ctx: &ListenerContext, notification: &NSNotification) {
    let Some(app) = RunningApp::from_notification(notification) else {
        return;
    };
    tracing::debug!(%app, "RunLoop launched");
    send_native_event(
        &ctx.sender,
        NativeEvent::AppWindowsChanged { pid: app.pid() },
    );
}

fn handle_app_terminated(ctx: &ListenerContext, notification: &NSNotification) {
    let Some(app) = RunningApp::from_notification(notification) else {
        return;
    };
    let pid = app.pid();
    tracing::debug!(%app, "RunLoop terminated");
    send_native_event(&ctx.sender, NativeEvent::AppTerminated { pid });
}

fn handle_app_activated(ctx: &mut ListenerContext, notification: &NSNotification) {
    if ctx.is_suspended.load(Ordering::Relaxed) {
        return;
    }
    let Some(app) = RunningApp::from_notification(notification) else {
        return;
    };

    // This can happen when Mac queue an event for an activated application, but by the time this
    // callback is run the focus have been given to another app. See focus_throttle for more detail
    if !app.is_active() {
        return;
    }

    tracing::debug!(%app, "RunLoop activated");
    ctx.focus_throttle.submit(app.pid());
}
