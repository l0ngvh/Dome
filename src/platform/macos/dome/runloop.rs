use std::collections::HashMap;
use std::time::Duration;

use calloop::channel::{Channel, Event as ChannelEvent};
use calloop::timer::{TimeoutAction, Timer};
use calloop::{EventLoop, LoopHandle, RegistrationToken};

use objc2::rc::autoreleasepool;

use crate::config::Config;
use crate::platform::macos::running_application::RunningApp;
use crate::platform::macos::ui::MessageSender;

use super::super::MonitorInfo;
use super::dispatcher::GcdDispatcher;
use super::dome::{Dome, WindowMove};
use super::events::{HubEvent, HubMessage};
use super::inspect::{compute_reconcile_all, compute_reconciliation, compute_window_positions};
use super::recovery;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);

pub(super) struct State {
    dome: Dome,
    dispatcher: GcdDispatcher,
    move_timers: HashMap<i32, RegistrationToken>,
    handle: LoopHandle<'static, State>,
}

pub(in crate::platform::macos) fn start(
    config: Config,
    screens: Vec<MonitorInfo>,
    sender: MessageSender,
    channel: Channel<HubEvent>,
) {
    recovery::install_handlers();
    let mut event_loop =
        EventLoop::<'static, State>::try_new().expect("Failed to create event loop");
    let handle = event_loop.handle();
    let signal = event_loop.get_signal();

    let (exec, scheduler) = calloop::futures::executor().expect("Failed to create executor");
    let dispatcher = GcdDispatcher::new(scheduler);

    let primary = screens.iter().find(|s| s.is_primary).unwrap_or(&screens[0]);
    tracing::info!(%primary, "Primary monitor");

    let dome = Dome::new(&screens, config, Box::new(sender), signal);

    let mut state = State {
        dome,
        dispatcher,
        move_timers: HashMap::new(),
        handle: handle.clone(),
    };

    handle
        .insert_source(exec, |callback, _, state: &mut State| {
            autoreleasepool(|_| callback(state));
        })
        .expect("Failed to insert executor source");

    handle
        .insert_source(channel, |event, _, state: &mut State| match event {
            ChannelEvent::Msg(hub_event) => handle_event(state, hub_event),
            ChannelEvent::Closed => state.dome.stop(),
        })
        .expect("Failed to insert channel source");

    dispatch_reconcile_all(&mut state);
    event_loop
        .run(None, &mut state, |_| {})
        .expect("Event loop failed");
}

fn handle_event(state: &mut State, event: HubEvent) {
    autoreleasepool(|_| match event {
        HubEvent::WindowMovedOrResized { pid } => {
            start_move_timer(state, pid);
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

fn start_move_timer(state: &mut State, pid: i32) {
    cancel_move_timer(state, pid);
    state.dome.set_pid_moving(pid, true);
    let token = state
        .handle
        .insert_source(
            Timer::from_duration(DEBOUNCE_INTERVAL),
            move |_, _, state: &mut State| {
                state.move_timers.remove(&pid);
                state.dome.set_pid_moving(pid, false);
                dispatch_check_positions(state, pid);
                TimeoutAction::Drop
            },
        )
        .expect("Failed to insert timer");
    state.move_timers.insert(pid, token);
}

fn cancel_move_timer(state: &mut State, pid: i32) {
    if let Some(token) = state.move_timers.remove(&pid) {
        state.handle.remove(token);
    }
}

fn dispatch_refresh_windows(state: &mut State, pid: i32) {
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

fn dispatch_check_positions(state: &mut State, pid: i32) {
    let tracked = state.dome.tracked_for_pid(pid);
    state.dispatcher.dispatch(
        move || {
            let app = RunningApp::new(pid)?;
            Some(compute_window_positions(&app, &tracked))
        },
        |result, state| {
            if let Some(positions) = result {
                let observed_at = positions.observed_at;
                let moves = positions
                    .existing
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

fn dispatch_reconcile_all(state: &mut State) {
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
                start_move_timer(state, pid);
            }
        },
    );
}
