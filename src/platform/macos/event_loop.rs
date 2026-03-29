use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::futures::Scheduler;
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, RegistrationToken};
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::rc::autoreleasepool;

use crate::platform::macos::dome::{
    Dome, HubEvent, WindowMove, compute_reconcile_all, compute_reconciliation,
    compute_window_positions,
};
use crate::platform::macos::running_application::RunningApp;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);

struct DomeRunner {
    dome: Dome,
    dispatcher: GcdDispatcher,
    move_timers: HashMap<i32, RegistrationToken>,
    handle: LoopHandle<'static, DomeRunner>,
}

pub(super) fn run_dome(dome: Dome, channel: Channel<HubEvent>) {
    install_signal_handlers();
    let mut event_loop =
        EventLoop::<'static, DomeRunner>::try_new().expect("Failed to create event loop");
    let handle = event_loop.handle();
    let signal = event_loop.get_signal();

    let (exec, scheduler) = calloop::futures::executor().expect("Failed to create executor");
    let dispatcher = GcdDispatcher::new(scheduler);

    let mut runner = DomeRunner {
        dome,
        dispatcher,
        move_timers: HashMap::new(),
        handle: handle.clone(),
    };

    handle
        .insert_source(exec, |callback, _, runner: &mut DomeRunner| {
            autoreleasepool(|_| callback(runner));
        })
        .expect("Failed to insert executor source");

    let signal_clone = signal.clone();
    handle
        .insert_source(channel, move |event, _, runner: &mut DomeRunner| {
            match event {
                ChannelEvent::Msg(hub_event) => handle_event(runner, hub_event),
                ChannelEvent::Closed => runner.dome.stop(),
            }
            if runner.dome.is_stopped() {
                signal_clone.stop();
            }
        })
        .expect("Failed to insert channel source");

    dispatch_reconcile_all(&mut runner);
    event_loop
        .run(None, &mut runner, |runner| {
            if SIGNAL_RECEIVED.load(Ordering::Relaxed) {
                runner.dome.stop();
            }
            if runner.dome.is_stopped() {
                signal.stop();
            }
        })
        .expect("Event loop failed");
}

fn handle_event(runner: &mut DomeRunner, event: HubEvent) {
    autoreleasepool(|_| match event {
        HubEvent::WindowMovedOrResized { pid, observed_at } => {
            start_move_timer(runner, pid, observed_at);
        }
        HubEvent::VisibleWindowsChanged { pid } => {
            dispatch_refresh_windows(runner, pid);
        }
        HubEvent::AppTerminated { pid } => {
            tracing::debug!(pid, "App terminated");
            cancel_move_timer(runner, pid);
            runner.dome.app_terminated(pid);
        }
        HubEvent::Sync => {
            dispatch_reconcile_all(runner);
        }
        HubEvent::Shutdown => {
            tracing::info!("Shutdown requested");
            runner.dome.stop();
        }
        HubEvent::ConfigChanged(new_config) => {
            runner.dome.config_changed(new_config);
        }
        HubEvent::SyncFocus { pid } => {
            runner.dome.sync_focus(pid);
        }
        HubEvent::TitleChanged(cg_id) => {
            runner.dome.title_changed(cg_id);
        }
        HubEvent::Action(actions) => {
            tracing::debug!(%actions, "Executing actions");
            runner.dome.run_actions(&actions);
        }
        HubEvent::ScreensChanged(screens) => {
            tracing::info!(count = screens.len(), "Screens changed");
            runner.dome.screens_changed(screens);
        }
        HubEvent::MirrorClicked(window_id) => {
            runner.dome.mirror_clicked(window_id);
        }
        HubEvent::TabClicked(container_id, tab_idx) => {
            runner.dome.tab_clicked(container_id, tab_idx);
        }
        HubEvent::SpaceChanged => {
            runner.dome.space_changed();
        }
    });
}

fn start_move_timer(runner: &mut DomeRunner, pid: i32, observed_at: Instant) {
    cancel_move_timer(runner, pid);
    runner.dome.set_pid_moving(pid, true);
    let token = runner
        .handle
        .insert_source(
            Timer::from_duration(DEBOUNCE_INTERVAL),
            move |_, _, runner: &mut DomeRunner| {
                runner.move_timers.remove(&pid);
                runner.dome.set_pid_moving(pid, false);
                dispatch_check_positions(runner, pid, observed_at);
                TimeoutAction::Drop
            },
        )
        .expect("Failed to insert timer");
    runner.move_timers.insert(pid, token);
}

fn cancel_move_timer(runner: &mut DomeRunner, pid: i32) {
    if let Some(token) = runner.move_timers.remove(&pid) {
        runner.handle.remove(token);
    }
}

fn dispatch_refresh_windows(runner: &mut DomeRunner, pid: i32) {
    let tracked = runner.dome.tracked_for_pid(pid);
    let ignore_rules = runner.dome.ignore_rules();
    runner.dispatcher.dispatch(
        move || {
            let app = RunningApp::new(pid)?;
            Some(compute_reconciliation(&app, &tracked, &ignore_rules))
        },
        |result, runner| {
            if let Some((to_remove, to_add)) = result {
                runner.dome.reconcile_windows(&to_remove, to_add);
            }
        },
    );
}

fn dispatch_check_positions(runner: &mut DomeRunner, pid: i32, observed_at: Instant) {
    let tracked = runner.dome.tracked_for_pid(pid);
    runner.dispatcher.dispatch(
        move || {
            let app = RunningApp::new(pid)?;
            Some(compute_window_positions(&app, &tracked))
        },
        move |result, runner| {
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
                runner.dome.windows_moved(moves);
            }
        },
    );
}

fn dispatch_reconcile_all(runner: &mut DomeRunner) {
    let observed_pids = runner.dome.observed_pids();
    let tracked = runner.dome.all_tracked();
    let ignore_rules = runner.dome.ignore_rules();
    runner.dispatcher.dispatch(
        move || compute_reconcile_all(observed_pids, tracked, ignore_rules),
        |result, runner| {
            for pid in result.terminated_pids {
                // FIXME: cleanup observer for terminated apps
                cancel_move_timer(runner, pid);
                runner.dome.unmark_pid_observed(pid);
                runner.dome.remove_untracked_app(pid);
            }
            for pid in result.hidden_pids.clone() {
                cancel_move_timer(runner, pid);
                runner.dome.remove_untracked_app(pid);
            }
            runner
                .dome
                .reconcile_windows(&result.to_remove, result.to_add);
            if !result.new_apps.is_empty() {
                for app in &result.new_apps {
                    runner.dome.mark_pid_observed(app.pid());
                }
                runner.dome.register_observers(result.new_apps);
            }
            // Windows moved/resized events aren't fired from time to time, like when windows
            // are brought into view after new monitors are plugged in, or when windows moved
            // from fullscreen.
            let pids_to_check: Vec<_> = runner
                .dome
                .observed_pids()
                .iter()
                .copied()
                .filter(|pid| {
                    !result.hidden_pids.contains(pid) && !runner.move_timers.contains_key(pid)
                })
                .collect();
            for pid in pids_to_check {
                start_move_timer(runner, pid, Instant::now());
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

type ApplyFn = Box<dyn FnOnce(&mut DomeRunner)>;

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
        A: FnOnce(R, &mut DomeRunner) + 'static,
    {
        self.scheduler
            .schedule(async move {
                let result = gcd_spawn(work).await;
                Box::new(move |runner: &mut DomeRunner| apply(result, runner)) as ApplyFn
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
