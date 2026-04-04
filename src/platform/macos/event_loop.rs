use std::collections::HashMap;
use std::os::unix::process::CommandExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, LoopSignal, RegistrationToken};
use objc2::rc::autoreleasepool;
use objc2_app_kit::NSWorkspace;
use objc2_core_graphics::CGWindowID;

use crate::action::{Action, Actions};
use crate::platform::macos::accessibility::AXWindowApi;
use crate::platform::macos::dispatcher::GcdDispatcher;
use crate::platform::macos::dome::{
    Dome, HubEvent, WindowMove, compute_reconcile_all, compute_reconciliation,
    compute_window_positions,
};
use crate::platform::macos::running_application::RunningApp;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct DomeRunner {
    dome: Dome,
    dispatcher: GcdDispatcher,
    move_timers: HashMap<i32, RegistrationToken>,
    handle: LoopHandle<'static, DomeRunner>,
    signal: LoopSignal,
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
        signal,
    };

    handle
        .insert_source(exec, |callback, _, runner: &mut DomeRunner| {
            autoreleasepool(|_| callback(runner));
        })
        .expect("Failed to insert executor source");

    handle
        .insert_source(
            channel,
            move |event, _, runner: &mut DomeRunner| match event {
                ChannelEvent::Msg(hub_event) => handle_event(runner, hub_event),
                ChannelEvent::Closed => runner.signal.stop(),
            },
        )
        .expect("Failed to insert channel source");

    dispatch_reconcile_all(&mut runner);
    event_loop
        .run(None, &mut runner, |runner| {
            if SIGNAL_RECEIVED.load(Ordering::Relaxed) {
                runner.signal.stop();
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
            runner.signal.stop();
        }
        HubEvent::ConfigChanged(new_config) => {
            runner.dome.config_changed(*new_config);
        }
        HubEvent::SyncFocus { pid } => {
            dispatch_sync_focus(runner, pid);
        }
        HubEvent::TitleChanged(cg_id) => {
            dispatch_title_read(runner, cg_id);
        }
        HubEvent::Action(actions) => {
            tracing::debug!(%actions, "Executing actions");
            runner.dome.run_hub_actions(&actions);
            handle_system_actions(runner, &actions);
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
            dispatch_space_changed(runner);
        }
    });
}

fn handle_system_actions(runner: &mut DomeRunner, actions: &Actions) {
    for action in actions {
        if let Action::Exec { command } = action {
            if let Err(e) = std::process::Command::new("sh")
                .arg("-c")
                .arg(command)
                .process_group(0)
                .spawn()
            {
                tracing::warn!(%command, "Failed to exec: {e}");
            }
        } else if let Action::Exit = action {
            tracing::debug!("Exit action received");
            runner.signal.stop();
        }
    }
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
        move |marker| {
            let app = RunningApp::new(pid)?;
            Some(compute_reconciliation(
                &app,
                &tracked,
                &ignore_rules,
                marker,
            ))
        },
        |result, runner| {
            if let Some((to_remove, to_add)) = result {
                let on_open = runner.dome.reconcile_windows(&to_remove, to_add);
                for actions in on_open {
                    runner.dome.run_hub_actions(&actions);
                    handle_system_actions(runner, &actions);
                }
            }
        },
    );
}

fn dispatch_check_positions(runner: &mut DomeRunner, pid: i32, observed_at: Instant) {
    let tracked = runner.dome.tracked_for_pid(pid);
    runner.dispatcher.dispatch(
        move |marker| {
            let app = RunningApp::new(pid)?;
            Some(compute_window_positions(&app, &tracked, marker))
        },
        move |result, runner| {
            if let Some(existing) = result {
                let moves = existing
                    .into_iter()
                    .map(|e| WindowMove {
                        cg_id: e.cg_id,
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

fn dispatch_sync_focus(runner: &mut DomeRunner, pid: i32) {
    runner.dispatcher.dispatch(
        move |marker| {
            let app = RunningApp::new(pid)?;
            if !app.is_active() {
                return None;
            }
            Some(app.focused_window(marker)?.cg_id())
        },
        |result, runner| {
            if let Some(cg_id) = result {
                runner.dome.focus_window_by_cg(cg_id);
            }
        },
    );
}

fn dispatch_title_read(runner: &mut DomeRunner, cg_id: CGWindowID) {
    let Some(entry) = runner.dome.tracked_window(cg_id) else {
        return;
    };
    runner.dispatcher.dispatch(
        move |marker| entry.ax.read_title(marker),
        move |title, runner| {
            runner.dome.update_title(cg_id, title);
        },
    );
}

fn dispatch_space_changed(runner: &mut DomeRunner) {
    runner.dispatcher.dispatch(
        move |marker| {
            let app = NSWorkspace::sharedWorkspace().frontmostApplication()?;
            let app = RunningApp::from(app);
            let ax = app.focused_window(marker)?;
            let cg_id = ax.cg_id();
            let is_native_fs = ax.is_native_fullscreen();
            let pos = ax.get_position().ok();
            let size = ax.get_size().ok();
            let app_name = ax.app_name().map(str::to_owned);
            let bundle_id = ax.bundle_id().map(str::to_owned);
            let title = ax.title().map(str::to_owned);
            Some((
                cg_id,
                is_native_fs,
                pos,
                size,
                Arc::new(ax) as Arc<dyn AXWindowApi>,
                app_name,
                bundle_id,
                title,
            ))
        },
        |result, runner| {
            let Some((cg_id, is_native_fs, pos, size, ax, app_name, bundle_id, title)) = result
            else {
                return;
            };
            if is_native_fs {
                runner
                    .dome
                    .enter_native_fullscreen(cg_id, ax, app_name, bundle_id, title);
            } else if let (Some(pos), Some(size)) = (pos, size) {
                runner.dome.exit_native_fullscreen(cg_id, pos, size);
            }
        },
    );
}

fn dispatch_reconcile_all(runner: &mut DomeRunner) {
    let observed_pids = runner.dome.observed_pids();
    let tracked = runner.dome.all_tracked();
    let ignore_rules = runner.dome.ignore_rules();
    runner.dispatcher.dispatch(
        move |marker| compute_reconcile_all(observed_pids, tracked, ignore_rules, marker),
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
            let on_open = runner
                .dome
                .reconcile_windows(&result.to_remove, result.to_add);
            for actions in on_open {
                runner.dome.run_hub_actions(&actions);
                handle_system_actions(runner, &actions);
            }
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
