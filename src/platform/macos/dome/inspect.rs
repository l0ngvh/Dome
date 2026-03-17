use std::collections::{HashMap, HashSet};
use std::time::Instant;

use calloop::channel::Sender as CalloopSender;
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::rc::autoreleasepool;
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFString, CFType};
use objc2_core_graphics::{CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption};

use crate::config::MacosWindow;
use crate::platform::macos::accessibility::AXWindow;
use crate::platform::macos::dome::window::RoundedDimension;
use crate::platform::macos::objc2_wrapper::kCGWindowNumber;

use super::super::running_application::RunningApp;
use super::AsyncResult;
use super::window::FullscreenState;

pub(super) struct VisibleWindowsReconciled {
    pub(super) pid: i32,
    pub(super) is_hidden: bool,
    pub(super) to_remove: Vec<CGWindowID>,
    pub(super) to_add: Vec<NewAxWindow>,
    pub(super) existing: Vec<ExistingWindow>,
    pub(super) observed_at: Instant,
}

pub(super) struct NewAxWindow {
    pub(super) ax: AXWindow,
    pub(super) dimension: RoundedDimension,
}

pub(super) struct ExistingWindow {
    pub(super) cg_id: CGWindowID,
    pub(super) dimension: RoundedDimension,
    pub(super) is_native_fullscreen: bool,
}

pub(super) fn dispatch_refresh_app_windows(
    pid: i32,
    tracked: HashMap<CGWindowID, (AXWindow, FullscreenState)>,
    ignore_rules: Vec<MacosWindow>,
    async_tx: CalloopSender<AsyncResult>,
) {
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let Some(app) = RunningApp::new(pid) else {
                return;
            };
            let result = compute_app_visible_windows(&app, &tracked, &ignore_rules);
            async_tx.send(AsyncResult::AppVisibleWindows(result)).ok();
        });
    });
}

pub(super) fn dispatch_reconcile_all_windows(
    observed_pids: HashSet<i32>,
    tracked: HashMap<CGWindowID, (AXWindow, FullscreenState)>,
    ignore_rules: Vec<MacosWindow>,
    async_tx: CalloopSender<AsyncResult>,
) {
    let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
        DispatchQoS::UserInitiated,
    ));
    queue.exec_async(move || {
        autoreleasepool(|_| {
            let running: Vec<_> = RunningApp::all().collect();
            let running_pids: HashSet<_> = running.iter().map(|app| app.pid()).collect();

            let terminated_pids: Vec<_> = observed_pids
                .iter()
                .filter(|pid| !running_pids.contains(pid))
                .copied()
                .collect();

            let new_apps: Vec<_> = running
                .iter()
                .filter(|app| !observed_pids.contains(&app.pid()))
                .cloned()
                .collect();

            let apps: Vec<_> = running
                .iter()
                .map(|app| compute_app_visible_windows(app, &tracked, &ignore_rules))
                .collect();

            async_tx
                .send(AsyncResult::AllVisibleWindows {
                    terminated_pids,
                    new_apps,
                    apps,
                })
                .ok();
        });
    });
}

fn compute_app_visible_windows(
    app: &RunningApp,
    tracked: &HashMap<CGWindowID, (AXWindow, FullscreenState)>,
    ignore_rules: &[MacosWindow],
) -> VisibleWindowsReconciled {
    let pid = app.pid();
    let is_hidden = app.is_hidden();

    if is_hidden {
        return VisibleWindowsReconciled {
            pid,
            is_hidden,
            to_remove: Vec::new(),
            to_add: Vec::new(),
            existing: Vec::new(),
            observed_at: Instant::now(),
        };
    }

    let cg_window_ids = list_cg_window_ids();

    let mut to_remove = Vec::new();
    let mut existing = Vec::new();
    for (&cg_id, (ax, fs)) in tracked.iter().filter(|(_, (ax, _))| ax.pid() == pid) {
        if !cg_window_ids.contains(&cg_id) || !ax.is_valid() {
            to_remove.push(cg_id);
            continue;
        }
        // Skip minimized check for mock fullscreen - we minimize them ourselves
        if *fs != FullscreenState::Borderless && ax.is_minimized() {
            to_remove.push(cg_id);
            continue;
        }
        let Ok((x, y)) = ax.get_position() else {
            continue;
        };
        let Ok((w, h)) = ax.get_size() else {
            continue;
        };
        existing.push(ExistingWindow {
            cg_id,
            dimension: RoundedDimension {
                width: w,
                height: h,
                x,
                y,
            },
            is_native_fullscreen: ax.is_native_fullscreen(),
        });
    }

    let mut to_add = Vec::new();
    for ax in app.ax_windows() {
        if tracked.contains_key(&ax.cg_id()) {
            continue;
        }
        if !ax.is_manageable() {
            continue;
        }
        if should_ignore(&ax, ignore_rules) {
            continue;
        }
        let Ok((x, y)) = ax.get_position() else {
            continue;
        };
        let Ok((w, h)) = ax.get_size() else {
            continue;
        };
        to_add.push(NewAxWindow {
            ax,
            dimension: RoundedDimension {
                width: w,
                height: h,
                x,
                y,
            },
        });
    }

    VisibleWindowsReconciled {
        pid,
        is_hidden,
        to_remove,
        to_add,
        existing,
        observed_at: Instant::now(),
    }
}

fn should_ignore(ax_window: &AXWindow, rules: &[MacosWindow]) -> bool {
    let matched = rules
        .iter()
        .find(|r| r.matches(ax_window.title(), ax_window.bundle_id(), ax_window.title()));
    if let Some(rule) = matched {
        tracing::debug!(
            %ax_window,
            ?rule,
            "Window ignored by rule"
        );
        return true;
    }
    false
}

fn list_cg_window_ids() -> HashSet<CGWindowID> {
    let Some(window_list) = CGWindowListCopyWindowInfo(CGWindowListOption::OptionAll, 0) else {
        tracing::warn!("CGWindowListCopyWindowInfo returned None");
        return HashSet::new();
    };
    let window_list: &CFArray<CFDictionary<CFString, CFType>> =
        unsafe { window_list.cast_unchecked() };

    let mut ids = HashSet::new();
    let key = kCGWindowNumber();
    for dict in window_list {
        // window id is a required attribute
        // https://developer.apple.com/documentation/coregraphics/kcgwindownumber?language=objc
        let id = dict
            .get(&key)
            .unwrap()
            .downcast::<CFNumber>()
            .unwrap()
            .as_i64()
            .unwrap();
        ids.insert(id as CGWindowID);
    }
    ids
}
