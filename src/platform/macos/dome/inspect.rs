use std::collections::{HashMap, HashSet};
use std::time::Instant;

use calloop::channel::Sender as CalloopSender;
use dispatch2::{DispatchQoS, DispatchQueue, GlobalQueueIdentifier};
use objc2::rc::autoreleasepool;
use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFString, CFType};
use objc2_core_graphics::{CGWindowID, CGWindowListCopyWindowInfo, CGWindowListOption};

use crate::config::MacosWindow;
use crate::core::WindowId;
use crate::platform::macos::Dome;
use crate::platform::macos::accessibility::AXWindow;
use crate::platform::macos::dome::window::RoundedDimension;
use crate::platform::macos::objc2_wrapper::kCGWindowNumber;

use super::super::running_application::RunningApp;
use super::mirror::WindowCapture;
use super::registry::WindowEntry;
use super::window::WindowState;

pub(super) enum AsyncResult {
    AppWindowsReconciled {
        to_remove: Vec<CGWindowID>,
        to_add: Vec<NewAxWindow>,
    },
    AppWindowPositions(WindowPositions),
    AllWindowsReconciled {
        terminated_pids: Vec<i32>,
        new_apps: Vec<RunningApp>,
        hidden_pids: Vec<i32>,
        to_remove: Vec<CGWindowID>,
        to_add: Vec<NewAxWindow>,
    },
    CaptureReady {
        window_id: WindowId,
        capture: WindowCapture,
    },
}

pub(super) struct GcdDispatcher {
    tx: CalloopSender<AsyncResult>,
}

impl GcdDispatcher {
    pub(super) fn new(tx: CalloopSender<AsyncResult>) -> Self {
        Self { tx }
    }

    pub(super) fn sender(&self) -> CalloopSender<AsyncResult> {
        self.tx.clone()
    }

    pub(super) fn refresh_windows(
        &self,
        pid: i32,
        tracked: HashMap<CGWindowID, WindowEntry>,
        ignore_rules: Vec<MacosWindow>,
    ) {
        let tx = self.tx.clone();
        Self::dispatch(move || {
            let Some(app) = RunningApp::new(pid) else {
                return;
            };
            let (to_remove, to_add) = compute_reconciliation(&app, &tracked, &ignore_rules);
            tx.send(AsyncResult::AppWindowsReconciled { to_remove, to_add })
                .ok();
        });
    }

    pub(super) fn check_positions(&self, pid: i32, tracked: HashMap<CGWindowID, WindowEntry>) {
        let tx = self.tx.clone();
        Self::dispatch(move || {
            let Some(app) = RunningApp::new(pid) else {
                return;
            };
            let result = compute_window_positions(&app, &tracked);
            tx.send(AsyncResult::AppWindowPositions(result)).ok();
        });
    }

    pub(super) fn reconcile_all(
        &self,
        observed_pids: HashSet<i32>,
        tracked: HashMap<CGWindowID, WindowEntry>,
        ignore_rules: Vec<MacosWindow>,
    ) {
        let tx = self.tx.clone();
        Self::dispatch(move || {
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

            let mut hidden_pids = Vec::new();
            let mut to_remove = Vec::new();
            let mut to_add = Vec::new();
            for app in &running {
                if app.is_hidden() {
                    hidden_pids.push(app.pid());
                } else {
                    let (removed, added) = compute_reconciliation(app, &tracked, &ignore_rules);
                    to_remove.extend(removed);
                    to_add.extend(added);
                }
            }

            tx.send(AsyncResult::AllWindowsReconciled {
                terminated_pids,
                new_apps,
                hidden_pids,
                to_remove,
                to_add,
            })
            .ok();
        });
    }

    fn dispatch(work: impl FnOnce() + Send + 'static) {
        let queue = DispatchQueue::global_queue(GlobalQueueIdentifier::QualityOfService(
            DispatchQoS::UserInitiated,
        ));
        queue.exec_async(move || autoreleasepool(|_| work()));
    }
}

impl Dome {
    pub(super) fn dispatch_refresh_windows(&self, pid: i32) {
        self.dispatcher.refresh_windows(
            pid,
            self.registry
                .for_pid(pid)
                .map(|(id, e)| (id, e.clone()))
                .collect(),
            self.config.macos.ignore.clone(),
        );
    }

    pub(super) fn dispatch_check_positions(&self, pid: i32) {
        self.dispatcher.check_positions(
            pid,
            self.registry
                .for_pid(pid)
                .map(|(id, e)| (id, e.clone()))
                .collect(),
        );
    }

    pub(super) fn apply_window_positions(&mut self, positions: WindowPositions) {
        let observed_at = positions.observed_at;
        for existing in positions.existing {
            if existing.is_native_fullscreen {
                self.window_entered_native_fullscreen(existing.id);
            } else {
                self.window_moved(existing.id, existing.dimension, observed_at);
            }
        }
    }
}

/// Can't only return a list of windows and reconcile at Dome as Dome won't have any method to know
/// whether a window moved to other space, or minimized without querying ax api
pub(super) struct WindowsReconciled {
    pub(super) to_remove: Vec<CGWindowID>,
    pub(super) to_add: Vec<NewAxWindow>,
}

pub(super) struct WindowPositions {
    pub(super) existing: Vec<ExistingWindow>,
    pub(super) observed_at: Instant,
}

pub(super) struct NewAxWindow {
    pub(super) ax: AXWindow,
    pub(super) dimension: RoundedDimension,
    pub(super) is_native_fullscreen: bool,
}

/// A still in display window (unminimized, in current space, returned by AXWindowsAttribute)
pub(super) struct ExistingWindow {
    pub(super) id: WindowId,
    pub(super) dimension: RoundedDimension,
    pub(super) is_native_fullscreen: bool,
}

fn compute_reconciliation(
    app: &RunningApp,
    tracked: &HashMap<CGWindowID, WindowEntry>,
    ignore_rules: &[MacosWindow],
) -> (Vec<CGWindowID>, Vec<NewAxWindow>) {
    let pid = app.pid();
    let cg_window_ids = list_cg_window_ids();

    let mut to_remove = Vec::new();
    for (&cg_id, entry) in tracked.iter().filter(|(_, e)| e.ax.pid() == pid) {
        if !cg_window_ids.contains(&cg_id)
            || !entry.ax.is_valid()
        // Skip minimized check for mock fullscreen - we minimize them ourselves
            || (!matches!(entry.state, WindowState::BorderlessFullscreen) && entry.ax.is_minimized())
        {
            to_remove.push(cg_id);
        }
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
            dimension: RoundedDimension {
                width: w,
                height: h,
                x,
                y,
            },
            is_native_fullscreen: ax.is_native_fullscreen(),
            ax,
        });
    }

    (to_remove, to_add)
}

fn compute_window_positions(
    app: &RunningApp,
    tracked: &HashMap<CGWindowID, WindowEntry>,
) -> WindowPositions {
    let mut existing = Vec::new();
    for ax in app.ax_windows() {
        let Some(window) = tracked.get(&ax.cg_id()) else {
            continue;
        };
        let Ok((x, y)) = window.ax.get_position() else {
            continue;
        };
        let Ok((w, h)) = window.ax.get_size() else {
            continue;
        };
        existing.push(ExistingWindow {
            id: window.window_id,
            dimension: RoundedDimension {
                width: w,
                height: h,
                x,
                y,
            },
            is_native_fullscreen: window.ax.is_native_fullscreen(),
        });
    }
    WindowPositions {
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
