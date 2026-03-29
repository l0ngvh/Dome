mod accessibility;
mod dome;
mod keyboard;
mod listeners;
mod objc2_wrapper;
mod running_application;
mod throttle;
mod ui;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::{Duration, Instant};

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::futures::Scheduler;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, RegistrationToken};
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::MainThreadMarker;
use objc2::rc::autoreleasepool;
use objc2_app_kit::{NSApplication, NSScreen};
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFDictionary, kCFBooleanTrue};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess,
};
use objc2_foundation::{NSNumber, NSString};

use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use crate::logging::Logger;
use dome::{
    Dome, HubEvent, WindowMove, compute_reconcile_all, compute_reconciliation,
    compute_window_positions,
};
use keyboard::KeyboardListener;
use listeners::EventListener;
use running_application::RunningApp;
use ui::Ui;

pub fn run_app(config_path: Option<String>) -> anyhow::Result<()> {
    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    let logger = Logger::init(&config);
    tracing::info!(%config_path, "Loaded config");

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    tracing::debug!("Accessibility: {}", unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    });

    if !CGPreflightScreenCaptureAccess() {
        tracing::info!("Screen recording permission not granted, requesting...");
        if !CGRequestScreenCaptureAccess() {
            return Err(anyhow::anyhow!(
                "Screen recording permission required. Please grant permission in System Settings > Privacy & Security > Screen Recording, then restart Dome."
            ));
        }
    }

    let mtm = MainThreadMarker::new().unwrap();

    let (event_tx, event_rx) = calloop::channel::channel();

    let hub_config = config.clone();
    let keymaps = Arc::new(RwLock::new(config.keymaps.clone()));

    let _config_watcher = start_config_watcher(&config_path, {
        let keymaps = keymaps.clone();
        let tx = event_tx.clone();
        move |cfg| {
            logger.set_level(cfg.log_level);
            *keymaps.write().unwrap() = cfg.keymaps.clone();
            tx.send(HubEvent::ConfigChanged(cfg)).ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    ipc::start_server({
        let tx = event_tx.clone();
        move |actions| {
            tx.send(HubEvent::Action(actions))
                .or(Err(anyhow::anyhow!("channel closed")))
        }
    })?;

    let screens = get_all_screens(mtm);
    if screens.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let is_suspended = Rc::new(Cell::new(false));
    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());
    let _keyboard_listener = KeyboardListener::new(keymaps, is_suspended, event_tx.clone())?;

    let (ui, sender) = Ui::new(mtm, event_tx, event_listener, config.clone());

    let hub_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dome = Dome::new(&screens, hub_config, Box::new(sender));
            run_dome(dome, event_rx);
        }))
        .ok();
    });

    ui.run();

    hub_thread.join().ok();
    Ok(())
}

fn send_hub_event(hub_sender: &calloop::channel::Sender<HubEvent>, event: HubEvent) {
    if hub_sender.send(event).is_err() {
        tracing::error!("Hub thread died, shutting down");
        let mtm = MainThreadMarker::new().unwrap();
        NSApplication::sharedApplication(mtm).terminate(None);
    }
}

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);

struct RunState {
    dome: Dome,
    dispatcher: GcdDispatcher,
    move_timers: HashMap<i32, RegistrationToken>,
    handle: LoopHandle<'static, RunState>,
}

fn run_dome(dome: Dome, channel: Channel<HubEvent>) {
    install_signal_handlers();
    let mut event_loop =
        EventLoop::<'static, RunState>::try_new().expect("Failed to create event loop");
    let handle = event_loop.handle();
    let signal = event_loop.get_signal();

    let (exec, scheduler) = calloop::futures::executor().expect("Failed to create executor");
    let dispatcher = GcdDispatcher::new(scheduler);

    let mut state = RunState {
        dome,
        dispatcher,
        move_timers: HashMap::new(),
        handle: handle.clone(),
    };

    handle
        .insert_source(exec, |callback, _, state: &mut RunState| {
            autoreleasepool(|_| callback(state));
        })
        .expect("Failed to insert executor source");

    let signal_clone = signal.clone();
    handle
        .insert_source(channel, move |event, _, state: &mut RunState| {
            match event {
                ChannelEvent::Msg(hub_event) => handle_event(state, hub_event),
                ChannelEvent::Closed => state.dome.stop(),
            }
            if state.dome.is_stopped() {
                signal_clone.stop();
            }
        })
        .expect("Failed to insert channel source");

    dispatch_reconcile_all(&mut state);
    event_loop
        .run(None, &mut state, |state| {
            if SIGNAL_RECEIVED.load(Ordering::Relaxed) {
                state.dome.stop();
            }
            if state.dome.is_stopped() {
                signal.stop();
            }
        })
        .expect("Event loop failed");
}

fn handle_event(state: &mut RunState, event: HubEvent) {
    autoreleasepool(|_| match event {
        HubEvent::WindowMovedOrResized { pid, observed_at } => {
            start_move_timer(state, pid, observed_at);
        }
        HubEvent::VisibleWindowsChanged { pid } => {
            dispatch_refresh_windows(state, pid);
        }
        HubEvent::AppTerminated { pid } => {
            tracing::debug!(pid, "App terminated");
            cancel_move_timer(state, pid);
            state.dome.app_terminated(pid);
        }
        HubEvent::Sync => {
            dispatch_reconcile_all(state);
        }
        HubEvent::Shutdown => {
            tracing::info!("Shutdown requested");
            state.dome.stop();
        }
        HubEvent::ConfigChanged(new_config) => {
            state.dome.config_changed(new_config);
        }
        HubEvent::SyncFocus { pid } => {
            state.dome.sync_focus(pid);
        }
        HubEvent::TitleChanged(cg_id) => {
            state.dome.title_changed(cg_id);
        }
        HubEvent::Action(actions) => {
            tracing::debug!(%actions, "Executing actions");
            state.dome.run_actions(&actions);
        }
        HubEvent::ScreensChanged(screens) => {
            tracing::info!(count = screens.len(), "Screens changed");
            state.dome.screens_changed(screens);
        }
        HubEvent::MirrorClicked(window_id) => {
            state.dome.mirror_clicked(window_id);
        }
        HubEvent::TabClicked(container_id, tab_idx) => {
            state.dome.tab_clicked(container_id, tab_idx);
        }
        HubEvent::SpaceChanged => {
            state.dome.space_changed();
        }
    });
}

fn start_move_timer(state: &mut RunState, pid: i32, observed_at: Instant) {
    cancel_move_timer(state, pid);
    state.dome.set_pid_moving(pid, true);
    let token = state
        .handle
        .insert_source(
            Timer::from_duration(DEBOUNCE_INTERVAL),
            move |_, _, state: &mut RunState| {
                state.move_timers.remove(&pid);
                state.dome.set_pid_moving(pid, false);
                dispatch_check_positions(state, pid, observed_at);
                TimeoutAction::Drop
            },
        )
        .expect("Failed to insert timer");
    state.move_timers.insert(pid, token);
}

fn cancel_move_timer(state: &mut RunState, pid: i32) {
    if let Some(token) = state.move_timers.remove(&pid) {
        state.handle.remove(token);
    }
}

fn dispatch_refresh_windows(state: &mut RunState, pid: i32) {
    let tracked = state.dome.tracked_for_pid(pid);
    let ignore_rules = state.dome.ignore_rules();
    state.dispatcher.dispatch(
        move || {
            let app = RunningApp::new(pid)?;
            Some(compute_reconciliation(&app, &tracked, &ignore_rules))
        },
        |result, state| {
            if let Some((to_remove, to_add)) = result {
                state.dome.reconcile_windows(&to_remove, to_add);
            }
        },
    );
}

fn dispatch_check_positions(state: &mut RunState, pid: i32, observed_at: Instant) {
    let tracked = state.dome.tracked_for_pid(pid);
    state.dispatcher.dispatch(
        move || {
            let app = RunningApp::new(pid)?;
            Some(compute_window_positions(&app, &tracked))
        },
        move |result, state| {
            if let Some(existing) = result {
                let moves = existing
                    .into_iter()
                    .map(|e| WindowMove {
                        window_id: e.id,
                        x: e.x,
                        y: e.y,
                        w: e.w,
                        h: e.h,
                        observed_at,
                        is_native_fullscreen: e.is_native_fullscreen,
                    })
                    .collect();
                state.dome.windows_moved(moves);
            }
        },
    );
}

fn dispatch_reconcile_all(state: &mut RunState) {
    let observed_pids = state.dome.observed_pids();
    let tracked = state.dome.all_tracked();
    let ignore_rules = state.dome.ignore_rules();
    state.dispatcher.dispatch(
        move || compute_reconcile_all(observed_pids, tracked, ignore_rules),
        |result, state| {
            for pid in result.terminated_pids {
                // FIXME: cleanup observer for terminated apps
                cancel_move_timer(state, pid);
                state.dome.unmark_pid_observed(pid);
                state.dome.remove_untracked_app(pid);
            }
            for pid in result.hidden_pids.clone() {
                cancel_move_timer(state, pid);
                state.dome.remove_untracked_app(pid);
            }
            state
                .dome
                .reconcile_windows(&result.to_remove, result.to_add);
            if !result.new_apps.is_empty() {
                for app in &result.new_apps {
                    state.dome.mark_pid_observed(app.pid());
                }
                state.dome.register_observers(result.new_apps);
            }
            // Windows moved/resized events aren't fired from time to time, like when windows
            // are brought into view after new monitors are plugged in, or when windows moved
            // from fullscreen.
            let pids_to_check: Vec<_> = state
                .dome
                .observed_pids()
                .iter()
                .copied()
                .filter(|pid| {
                    !result.hidden_pids.contains(pid) && !state.move_timers.contains_key(pid)
                })
                .collect();
            for pid in pids_to_check {
                start_move_timer(state, pid, Instant::now());
            }
        },
    );
}

static SIGNAL_RECEIVED: AtomicBool = AtomicBool::new(false);

fn install_signal_handlers() {
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as usize);
        libc::signal(libc::SIGTERM, signal_handler as usize);
        libc::signal(libc::SIGHUP, signal_handler as usize);
    }
}

extern "C" fn signal_handler(_sig: libc::c_int) {
    SIGNAL_RECEIVED.store(true, Ordering::Relaxed);
}

type ApplyFn = Box<dyn FnOnce(&mut RunState)>;

struct GcdDispatcher {
    scheduler: Scheduler<ApplyFn>,
}

impl GcdDispatcher {
    fn new(scheduler: Scheduler<ApplyFn>) -> Self {
        Self { scheduler }
    }

    fn dispatch<W, R, A>(&self, work: W, apply: A)
    where
        W: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
        A: FnOnce(R, &mut RunState) + 'static,
    {
        self.scheduler
            .schedule(async move {
                let result = gcd_spawn(work).await;
                Box::new(move |state: &mut RunState| apply(result, state)) as ApplyFn
            })
            .ok();
    }
}

async fn gcd_spawn<R: Send + 'static>(work: impl FnOnce() -> R + Send + 'static) -> R {
    let (tx, rx) = futures_channel::oneshot::channel();
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let _ = tx.send(work());
        });
    });
    rx.await.expect("GCD task was cancelled")
}

#[derive(Clone, Debug)]
pub(in crate::platform::macos) struct MonitorInfo {
    pub(in crate::platform::macos) display_id: CGDirectDisplayID,
    pub(in crate::platform::macos) name: String,
    pub(in crate::platform::macos) dimension: Dimension,
    pub(in crate::platform::macos) full_height: f32,
    pub(in crate::platform::macos) is_primary: bool,
    pub(in crate::platform::macos) scale: f64,
}

impl std::fmt::Display for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, dim={:?}, scale={})",
            self.name, self.display_id, self.dimension, self.scale
        )
    }
}

fn get_display_id(screen: &NSScreen) -> CGDirectDisplayID {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    desc.objectForKey(&key)
        .and_then(|obj| {
            let num: Option<&NSNumber> = obj.downcast_ref();
            num.map(|n| n.unsignedIntValue())
        })
        .unwrap_or(0)
}

fn get_all_screens(mtm: MainThreadMarker) -> Vec<MonitorInfo> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen);
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            MonitorInfo {
                display_id,
                name,
                dimension: Dimension {
                    x: bounds.origin.x as f32,
                    y: (bounds.origin.y + top_inset) as f32,
                    width: bounds.size.width as f32,
                    height: (bounds.size.height - top_inset - bottom_inset) as f32,
                },
                full_height: bounds.size.height as f32,
                is_primary: display_id == primary_id,
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}
