//! Smoke tests and delta-debugging reducer for the Hub.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{setup_hub, setup_logger_with_level, validate_hub};
use crate::action::MonitorTarget;
use crate::core::hub::Hub;
use crate::core::node::{Dimension, Length, MonitorId, WindowId, WindowRestrictions};
use crate::core::strategy::TilingAction;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

#[test]
fn smoke_test() {
    setup_logger_with_level("info");

    let seed = 42u64;
    let runs = 200;
    let ops_per_run = 10000;
    let completed = AtomicUsize::new(0);
    let abort = AtomicBool::new(false);

    (0..runs).into_par_iter().for_each(|run| {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        run_smoke_iteration(
            seed.wrapping_add(run as u64),
            ops_per_run,
            setup_hub,
            &abort,
            "reproduce_smoke_failure",
            "reduce_smoke_failure",
        );
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        if done.is_multiple_of(10) {
            tracing::info!("Completed {done}/{runs}");
        }
    });
}

/// Confirms trait dispatch and placement collection work end-to-end:
/// insert a tiling window, get placements, verify the window appears
/// with correct dimensions.
#[test]
fn strategy_smoke_test() {
    use super::setup;
    use crate::core::hub::MonitorLayout;

    let mut hub = setup();
    let id = hub.insert_tiling(hub.current_workspace());
    let placements = hub.get_visible_placements();

    assert_eq!(placements.monitors.len(), 1);
    let mp = &placements.monitors[0];
    let MonitorLayout::Normal {
        tiling_windows,
        float_windows,
        containers,
    } = &mp.layout
    else {
        panic!("expected Normal layout, got Fullscreen");
    };

    assert_eq!(tiling_windows.len(), 1);
    assert!(float_windows.is_empty());
    assert!(containers.is_empty());

    let wp = &tiling_windows[0];
    assert_eq!(wp.id, id);
    assert!(wp.is_highlighted);
    // Single tiling window fills the 150x30 screen
    assert_eq!(wp.frame.width, Length::new(150.0));
    assert_eq!(wp.frame.height, Length::new(30.0));

    let ws = hub.current_workspace();
    assert_eq!(hub.focused_window(ws), Some(id));
}

#[test]
fn master_smoke_test() {
    setup_logger_with_level("info");

    let seed = 42u64;
    let runs = 200;
    let ops_per_run = 10000;
    let completed = AtomicUsize::new(0);
    let abort = AtomicBool::new(false);

    (0..runs).into_par_iter().for_each(|run| {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        run_smoke_iteration(
            seed.wrapping_add(run as u64),
            ops_per_run,
            super::master::setup_master,
            &abort,
            "reproduce_master_smoke_failure",
            "reduce_master_smoke_failure",
        );
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        if done.is_multiple_of(10) {
            tracing::info!("Completed master-stack {done}/{runs}");
        }
    });
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reproduce_smoke_failure() {
    setup_logger_with_level("info");
    let seed = smoke_seed_from_env("reproduce_smoke_failure");
    let abort = AtomicBool::new(false);
    run_smoke_iteration(
        seed,
        10000,
        setup_hub,
        &abort,
        "reproduce_smoke_failure",
        "reduce_smoke_failure",
    );
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reproduce_master_smoke_failure() {
    setup_logger_with_level("info");
    let seed = smoke_seed_from_env("reproduce_master_smoke_failure");
    let abort = AtomicBool::new(false);
    run_smoke_iteration(
        seed,
        10000,
        super::master::setup_master,
        &abort,
        "reproduce_master_smoke_failure",
        "reduce_master_smoke_failure",
    );
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reduce_smoke_failure() {
    setup_logger_with_level("info");
    let seed = smoke_seed_from_env("reduce_smoke_failure");
    let (recorded, signature) = record(seed, 10000, setup_hub);
    tracing::info!(recorded = recorded.len(), ?signature, "captured failure");
    let reduced = ddmin(recorded, |c| reproduces_signature(c, &signature, setup_hub));
    tracing::error!("=== REDUCED OPERATIONS ({}) ===", reduced.len());
    for (i, op) in reduced.iter().enumerate() {
        tracing::error!("  {i}: {op:?}");
    }
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reduce_master_smoke_failure() {
    setup_logger_with_level("info");
    let seed = smoke_seed_from_env("reduce_master_smoke_failure");
    let (recorded, signature) = record(seed, 10000, super::master::setup_master);
    tracing::info!(recorded = recorded.len(), ?signature, "captured failure");
    let reduced = ddmin(recorded, |c| {
        reproduces_signature(c, &signature, super::master::setup_master)
    });
    tracing::error!("=== REDUCED OPERATIONS ({}) ===", reduced.len());
    for (i, op) in reduced.iter().enumerate() {
        tracing::error!("  {i}: {op:?}");
    }
}

#[derive(Debug, Clone, Copy)]
enum OpKind {
    InsertTiling,
    InsertFloat,
    InsertFullscreen,
    DeleteWindow,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    ToggleSpawnMode,
    ToggleDirection,
    FocusParent,
    ToggleContainerLayout,
    FocusNextTab,
    FocusPrevTab,
    ToggleFloat,
    ToggleFullscreen,
    SetFullscreen,
    UnsetFullscreen,
    MoveToWorkspace,
    FocusWorkspace,
    AddMonitor,
    RemoveMonitor,
    FocusMonitor,
    MoveToMonitor,
    SetFocus,
    SetWindowConstraint,
    SetWindowTitle,
    IncreaseMasterRatio,
    DecreaseMasterRatio,
    IncrementMasterCount,
    DecrementMasterCount,
    QueryWorkspaces,
    MinimizeWindow,
    UnminimizeWindow,
}

const ALL_OP_KINDS: &[OpKind] = &[
    OpKind::InsertTiling,
    OpKind::InsertFloat,
    OpKind::InsertFullscreen,
    OpKind::DeleteWindow,
    OpKind::FocusLeft,
    OpKind::FocusRight,
    OpKind::FocusUp,
    OpKind::FocusDown,
    OpKind::MoveLeft,
    OpKind::MoveRight,
    OpKind::MoveUp,
    OpKind::MoveDown,
    OpKind::ToggleSpawnMode,
    OpKind::ToggleDirection,
    OpKind::FocusParent,
    OpKind::ToggleContainerLayout,
    OpKind::FocusNextTab,
    OpKind::FocusPrevTab,
    OpKind::ToggleFloat,
    OpKind::ToggleFullscreen,
    OpKind::SetFullscreen,
    OpKind::UnsetFullscreen,
    OpKind::MoveToWorkspace,
    OpKind::FocusWorkspace,
    OpKind::AddMonitor,
    OpKind::RemoveMonitor,
    OpKind::FocusMonitor,
    OpKind::MoveToMonitor,
    OpKind::SetFocus,
    OpKind::SetWindowConstraint,
    OpKind::SetWindowTitle,
    OpKind::IncreaseMasterRatio,
    OpKind::DecreaseMasterRatio,
    OpKind::IncrementMasterCount,
    OpKind::DecrementMasterCount,
    OpKind::QueryWorkspaces,
    OpKind::MinimizeWindow,
    OpKind::UnminimizeWindow,
];

#[derive(Debug, Clone)]
enum RecordedOp {
    InsertTiling {
        producer_id: usize,
    },
    InsertFloat {
        producer_id: usize,
        dim: Dimension,
    },
    InsertFullscreen {
        producer_id: usize,
        restrictions: WindowRestrictions,
    },
    AddMonitor {
        producer_id: usize,
        name: String,
        dim: Dimension,
        scale: f32,
    },
    DeleteWindow {
        window: RecordedWindow,
    },
    RemoveMonitor {
        monitor: RecordedMonitor,
        fallback: RecordedMonitor,
    },
    SetFullscreen {
        window: RecordedWindow,
        restrictions: WindowRestrictions,
    },
    UnsetFullscreen {
        window: RecordedWindow,
    },
    SetFocus {
        window: RecordedWindow,
    },
    SetWindowConstraint {
        window: RecordedWindow,
        min_w: Option<f32>,
        min_h: Option<f32>,
        max_w: Option<f32>,
        max_h: Option<f32>,
    },
    SetWindowTitle {
        window: RecordedWindow,
        title: String,
    },
    MinimizeWindow {
        window: RecordedWindow,
    },
    UnminimizeWindow {
        window: RecordedWindow,
    },
    MoveToWorkspace {
        name: String,
    },
    FocusWorkspace {
        name: String,
    },
    FocusMonitor {
        target: MonitorTarget,
    },
    MoveToMonitor {
        target: MonitorTarget,
    },
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    ToggleSpawnMode,
    ToggleDirection,
    FocusParent,
    ToggleContainerLayout,
    FocusNextTab,
    FocusPrevTab,
    ToggleFloat,
    ToggleFullscreen,
    IncreaseMasterRatio,
    DecreaseMasterRatio,
    IncrementMasterCount,
    DecrementMasterCount,
    QueryWorkspaces,
}

#[derive(Debug, Clone, Copy)]
struct RecordedWindow(usize);

#[derive(Debug, Clone, Copy)]
struct RecordedMonitor(usize);

fn run_smoke_iteration(
    seed: u64,
    ops_per_run: usize,
    make_hub: fn() -> Hub,
    abort: &AtomicBool,
    reproduce_test_name: &'static str,
    reduce_test_name: &'static str,
) {
    if abort.load(Ordering::Relaxed) {
        return;
    }
    let mut hub = make_hub();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_iteration(seed, ops_per_run, &mut hub, abort, |_| {});
    }));

    if let Err(e) = result {
        abort.store(true, Ordering::Relaxed);
        tracing::error!(
            "To reproduce: DOME_SMOKE_SEED={seed} cargo test --lib \
             {reproduce_test_name} -- --ignored --nocapture",
        );
        tracing::error!(
            "To reduce:    DOME_SMOKE_SEED={seed} cargo test --lib \
             {reduce_test_name} -- --ignored --nocapture",
        );
        std::panic::resume_unwind(e);
    }
}

fn pick_non_minimized(rng: &mut ChaCha8Rng, minimized: &[bool]) -> Option<usize> {
    let eligible: Vec<usize> = (0..minimized.len()).filter(|&i| !minimized[i]).collect();
    if eligible.is_empty() {
        return None;
    }
    Some(eligible[rng.random_range(0..eligible.len())])
}

fn run_iteration<F>(
    seed: u64,
    ops_per_run: usize,
    hub: &mut Hub,
    abort: &AtomicBool,
    mut observer: F,
) where
    F: FnMut(&RecordedOp),
{
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut windows: Vec<WindowId> = Vec::new();
    let mut window_origin: Vec<usize> = Vec::new();
    let mut window_minimized: Vec<bool> = Vec::new();
    let mut monitors: Vec<MonitorId> = vec![hub.focused_monitor()];
    let mut monitor_origin: Vec<usize> = vec![usize::MAX];
    let mut next_op_index: usize = 0;

    for _ in 0..ops_per_run {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let kind = ALL_OP_KINDS[rng.random_range(0..ALL_OP_KINDS.len())];
        let Some(op) = build_op(
            kind,
            &mut rng,
            &windows,
            &window_origin,
            &window_minimized,
            &monitors,
            &monitor_origin,
            next_op_index,
        ) else {
            continue;
        };
        observer(&op);
        apply_op(
            hub,
            &op,
            &mut windows,
            &mut window_origin,
            &mut window_minimized,
            &mut monitors,
            &mut monitor_origin,
        );
        next_op_index += 1;
        validate_hub(hub);
    }

    while !windows.is_empty() {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let producer_id = window_origin.remove(0);
        let op = RecordedOp::DeleteWindow {
            window: RecordedWindow(producer_id),
        };
        observer(&op);
        let id = windows.remove(0);
        hub.delete_window(id);
        validate_hub(hub);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "single-call-site test helper threading full smoke state"
)]
fn build_op(
    kind: OpKind,
    rng: &mut ChaCha8Rng,
    windows: &[WindowId],
    window_origin: &[usize],
    window_minimized: &[bool],
    monitors: &[MonitorId],
    monitor_origin: &[usize],
    next_op_index: usize,
) -> Option<RecordedOp> {
    match kind {
        OpKind::InsertTiling => Some(RecordedOp::InsertTiling {
            producer_id: next_op_index,
        }),
        OpKind::InsertFloat => {
            let dim = Dimension::new(
                Length::new(rng.random_range(0.0f32..100.0)),
                Length::new(rng.random_range(0.0f32..20.0)),
                Length::new(rng.random_range(10.0f32..50.0)),
                Length::new(rng.random_range(5.0f32..15.0)),
            );
            Some(RecordedOp::InsertFloat {
                producer_id: next_op_index,
                dim,
            })
        }
        OpKind::InsertFullscreen => {
            let restrictions = match rng.random_range(0..3u8) {
                0 => WindowRestrictions::None,
                1 => WindowRestrictions::BlockAll,
                _ => WindowRestrictions::ProtectFullscreen,
            };
            Some(RecordedOp::InsertFullscreen {
                producer_id: next_op_index,
                restrictions,
            })
        }
        OpKind::DeleteWindow => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            Some(RecordedOp::DeleteWindow {
                window: RecordedWindow(window_origin[idx]),
            })
        }
        OpKind::FocusLeft => Some(RecordedOp::FocusLeft),
        OpKind::FocusRight => Some(RecordedOp::FocusRight),
        OpKind::FocusUp => Some(RecordedOp::FocusUp),
        OpKind::FocusDown => Some(RecordedOp::FocusDown),
        OpKind::MoveLeft => Some(RecordedOp::MoveLeft),
        OpKind::MoveRight => Some(RecordedOp::MoveRight),
        OpKind::MoveUp => Some(RecordedOp::MoveUp),
        OpKind::MoveDown => Some(RecordedOp::MoveDown),
        OpKind::ToggleSpawnMode => Some(RecordedOp::ToggleSpawnMode),
        OpKind::ToggleDirection => Some(RecordedOp::ToggleDirection),
        OpKind::FocusParent => Some(RecordedOp::FocusParent),
        OpKind::ToggleContainerLayout => Some(RecordedOp::ToggleContainerLayout),
        OpKind::FocusNextTab => Some(RecordedOp::FocusNextTab),
        OpKind::FocusPrevTab => Some(RecordedOp::FocusPrevTab),
        OpKind::ToggleFloat => Some(RecordedOp::ToggleFloat),
        OpKind::ToggleFullscreen => Some(RecordedOp::ToggleFullscreen),
        OpKind::SetFullscreen => {
            let idx = pick_non_minimized(rng, window_minimized)?;
            let restrictions = match rng.random_range(0..3u8) {
                0 => WindowRestrictions::None,
                1 => WindowRestrictions::BlockAll,
                _ => WindowRestrictions::ProtectFullscreen,
            };
            Some(RecordedOp::SetFullscreen {
                window: RecordedWindow(window_origin[idx]),
                restrictions,
            })
        }
        OpKind::UnsetFullscreen => {
            let idx = pick_non_minimized(rng, window_minimized)?;
            Some(RecordedOp::UnsetFullscreen {
                window: RecordedWindow(window_origin[idx]),
            })
        }
        OpKind::MoveToWorkspace => {
            let ws = rng.random_range(0..5);
            Some(RecordedOp::MoveToWorkspace {
                name: ws.to_string(),
            })
        }
        OpKind::FocusWorkspace => {
            let ws = rng.random_range(0..5);
            Some(RecordedOp::FocusWorkspace {
                name: ws.to_string(),
            })
        }
        OpKind::AddMonitor => {
            let x = monitors.len() as f32 * 150.0;
            let name = format!("monitor-{}", monitors.len());
            let dim = Dimension::new(
                Length::new(x),
                Length::new(0.0),
                Length::new(150.0),
                Length::new(30.0),
            );
            Some(RecordedOp::AddMonitor {
                producer_id: next_op_index,
                name,
                dim,
                scale: 1.0,
            })
        }
        OpKind::RemoveMonitor => {
            if monitors.len() <= 1 {
                return None;
            }
            let idx = rng.random_range(0..monitors.len());
            let fallback_idx = if idx == 0 { 1 } else { 0 };
            Some(RecordedOp::RemoveMonitor {
                monitor: RecordedMonitor(monitor_origin[idx]),
                fallback: RecordedMonitor(monitor_origin[fallback_idx]),
            })
        }
        OpKind::FocusMonitor => {
            let targets = [
                MonitorTarget::Up,
                MonitorTarget::Down,
                MonitorTarget::Left,
                MonitorTarget::Right,
            ];
            let target = targets[rng.random_range(0..targets.len())].clone();
            Some(RecordedOp::FocusMonitor { target })
        }
        OpKind::MoveToMonitor => {
            let targets = [
                MonitorTarget::Up,
                MonitorTarget::Down,
                MonitorTarget::Left,
                MonitorTarget::Right,
            ];
            let target = targets[rng.random_range(0..targets.len())].clone();
            Some(RecordedOp::MoveToMonitor { target })
        }
        OpKind::SetFocus => {
            let idx = pick_non_minimized(rng, window_minimized)?;
            Some(RecordedOp::SetFocus {
                window: RecordedWindow(window_origin[idx]),
            })
        }
        OpKind::SetWindowConstraint => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            let min_w = match rng.random_range(0..3) {
                0 => None,
                1 => Some(0.0),
                _ => Some(rng.random_range(1.0f32..50.0)),
            };
            let min_h = match rng.random_range(0..3) {
                0 => None,
                1 => Some(0.0),
                _ => Some(rng.random_range(1.0f32..10.0)),
            };
            let max_w = match rng.random_range(0..3) {
                0 => None,
                1 => Some(0.0),
                _ => Some(rng.random_range(1.0f32..100.0)),
            };
            let max_h = match rng.random_range(0..3) {
                0 => None,
                1 => Some(0.0),
                _ => Some(rng.random_range(1.0f32..20.0)),
            };
            Some(RecordedOp::SetWindowConstraint {
                window: RecordedWindow(window_origin[idx]),
                min_w,
                min_h,
                max_w,
                max_h,
            })
        }
        OpKind::SetWindowTitle => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            let title = format!("title-{}", rng.random_range(0..100u32));
            Some(RecordedOp::SetWindowTitle {
                window: RecordedWindow(window_origin[idx]),
                title,
            })
        }
        OpKind::IncreaseMasterRatio => Some(RecordedOp::IncreaseMasterRatio),
        OpKind::DecreaseMasterRatio => Some(RecordedOp::DecreaseMasterRatio),
        OpKind::IncrementMasterCount => Some(RecordedOp::IncrementMasterCount),
        OpKind::DecrementMasterCount => Some(RecordedOp::DecrementMasterCount),
        OpKind::QueryWorkspaces => Some(RecordedOp::QueryWorkspaces),
        OpKind::MinimizeWindow => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            Some(RecordedOp::MinimizeWindow {
                window: RecordedWindow(window_origin[idx]),
            })
        }
        OpKind::UnminimizeWindow => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            Some(RecordedOp::UnminimizeWindow {
                window: RecordedWindow(window_origin[idx]),
            })
        }
    }
}

fn apply_op(
    hub: &mut Hub,
    op: &RecordedOp,
    windows: &mut Vec<WindowId>,
    window_origin: &mut Vec<usize>,
    window_minimized: &mut Vec<bool>,
    monitors: &mut Vec<MonitorId>,
    monitor_origin: &mut Vec<usize>,
) {
    match op {
        RecordedOp::InsertTiling { producer_id } => {
            let id = hub.insert_tiling(hub.current_workspace());
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::InsertFloat { producer_id, dim } => {
            let id = hub.insert_float(hub.current_workspace(), *dim);
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::InsertFullscreen {
            producer_id,
            restrictions,
        } => {
            let id = hub.insert_fullscreen(hub.current_workspace(), *restrictions);
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::AddMonitor {
            producer_id,
            name,
            dim,
            scale,
        } => {
            let id = hub.add_monitor(name.clone(), *dim, *scale);
            monitors.push(id);
            monitor_origin.push(*producer_id);
        }
        RecordedOp::DeleteWindow { window } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            let id = windows.remove(pos);
            window_origin.remove(pos);
            window_minimized.remove(pos);
            hub.delete_window(id);
        }
        RecordedOp::RemoveMonitor { monitor, fallback } => {
            let pos = monitor_origin
                .iter()
                .position(|&o| o == monitor.0)
                .expect("apply_op: monitor producer_id not found");
            let id = monitors.remove(pos);
            monitor_origin.remove(pos);
            let fb_pos = monitor_origin
                .iter()
                .position(|&o| o == fallback.0)
                .expect("apply_op: fallback producer_id not found");
            let fb_id = monitors[fb_pos];
            hub.remove_monitor(id, fb_id);
        }
        RecordedOp::SetFullscreen {
            window,
            restrictions,
        } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            hub.set_fullscreen(windows[pos], *restrictions);
        }
        RecordedOp::UnsetFullscreen { window } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            hub.unset_fullscreen(windows[pos]);
        }
        RecordedOp::SetFocus { window } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            hub.set_focus(windows[pos]);
        }
        RecordedOp::SetWindowConstraint {
            window,
            min_w,
            min_h,
            max_w,
            max_h,
        } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            hub.set_window_constraint(windows[pos], *min_w, *min_h, *max_w, *max_h);
        }
        RecordedOp::SetWindowTitle { window, title } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            hub.set_window_title(windows[pos], title.clone());
        }
        RecordedOp::MinimizeWindow { window } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            window_minimized[pos] = true;
            hub.minimize_window(windows[pos]);
        }
        RecordedOp::UnminimizeWindow { window } => {
            let pos = window_origin
                .iter()
                .position(|&o| o == window.0)
                .expect("apply_op: window producer_id not found");
            window_minimized[pos] = false;
            hub.unminimize_window(windows[pos]);
        }
        RecordedOp::MoveToWorkspace { name } => {
            hub.move_focused_to_workspace(name);
        }
        RecordedOp::FocusWorkspace { name } => {
            hub.focus_workspace(name);
        }
        RecordedOp::FocusMonitor { target } => {
            hub.focus_monitor(target);
        }
        RecordedOp::MoveToMonitor { target } => {
            hub.move_focused_to_monitor(target);
        }
        RecordedOp::FocusLeft => hub.focus_left(),
        RecordedOp::FocusRight => hub.focus_right(),
        RecordedOp::FocusUp => hub.focus_up(),
        RecordedOp::FocusDown => hub.focus_down(),
        RecordedOp::MoveLeft => hub.move_left(),
        RecordedOp::MoveRight => hub.move_right(),
        RecordedOp::MoveUp => hub.move_up(),
        RecordedOp::MoveDown => hub.move_down(),
        RecordedOp::ToggleSpawnMode => hub.toggle_spawn_mode(),
        RecordedOp::ToggleDirection => hub.toggle_direction(),
        RecordedOp::FocusParent => hub.focus_parent(),
        RecordedOp::ToggleContainerLayout => hub.toggle_container_layout(),
        RecordedOp::FocusNextTab => hub.focus_next_tab(),
        RecordedOp::FocusPrevTab => hub.focus_prev_tab(),
        RecordedOp::ToggleFloat => hub.toggle_float(),
        RecordedOp::ToggleFullscreen => hub.toggle_fullscreen(),
        RecordedOp::IncreaseMasterRatio => {
            hub.handle_tiling_action(TilingAction::GrowMaster);
        }
        RecordedOp::DecreaseMasterRatio => {
            hub.handle_tiling_action(TilingAction::ShrinkMaster);
        }
        RecordedOp::IncrementMasterCount => {
            hub.handle_tiling_action(TilingAction::MoreMaster);
        }
        RecordedOp::DecrementMasterCount => {
            hub.handle_tiling_action(TilingAction::FewerMaster);
        }
        RecordedOp::QueryWorkspaces => {
            hub.query_workspaces();
        }
    }
}

fn smoke_seed_from_env(test_name: &'static str) -> u64 {
    match std::env::var("DOME_SMOKE_SEED") {
        Ok(value) => match value.parse::<u64>() {
            Ok(seed) => seed,
            Err(_) => panic!(
                "DOME_SMOKE_SEED='{value}' is not a valid u64.\n\
                 example: DOME_SMOKE_SEED=167 cargo test --lib {test_name} \
                 -- --ignored --nocapture"
            ),
        },
        Err(_) => panic!(
            "DOME_SMOKE_SEED not set.\n\
             example: DOME_SMOKE_SEED=167 cargo test --lib {test_name} \
             -- --ignored --nocapture"
        ),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FailureSignature {
    file: String,
    line: u32,
    normalized_payload: String,
}

fn capture_panic<F: FnOnce()>(f: F) -> Option<FailureSignature> {
    use std::cell::RefCell;

    thread_local! {
        static SIG: RefCell<Option<FailureSignature>> = const { RefCell::new(None) };
    }

    SIG.with(|cell| *cell.borrow_mut() = None);

    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(|info| {
        // Empty string is a meaningful "no location data" representation.
        let file = info
            .location()
            .map(|loc| loc.file().to_owned())
            .unwrap_or_default();
        let line = info.location().map(|loc| loc.line()).unwrap_or(0);

        // Payload is typically &str or String. unwrap_or_default is acceptable
        // here: an empty string is a meaningful "no payload" representation.
        let raw_payload = info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| info.payload().downcast_ref::<String>().map(|s| s.as_str()))
            .unwrap_or_default();

        let sig = FailureSignature {
            file,
            line,
            normalized_payload: normalize_digits(raw_payload),
        };
        SIG.with(|cell| *cell.borrow_mut() = Some(sig));
    }));

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    std::panic::set_hook(prev_hook);

    match result {
        Err(_) => SIG.with(|cell| cell.borrow().clone()),
        Ok(()) => None,
    }
}

fn normalize_digits(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut in_digit_run = false;
    for ch in s.chars() {
        if ch.is_ascii_digit() {
            if !in_digit_run {
                out.push('#');
                in_digit_run = true;
            }
        } else {
            in_digit_run = false;
            out.push(ch);
        }
    }
    out
}

#[expect(
    clippy::mut_range_bound,
    reason = "granularity is modified then we break immediately, so the loop bound is unaffected"
)]
fn ddmin<P>(mut ops: Vec<RecordedOp>, mut reproduces: P) -> Vec<RecordedOp>
where
    P: FnMut(&[RecordedOp]) -> bool,
{
    let mut granularity: usize = 2;
    let max_outer_iterations = ops.len().saturating_mul(2).saturating_add(8);
    for _ in 0..max_outer_iterations {
        if granularity > ops.len() {
            break;
        }
        let chunk = ops.len() / granularity;
        if chunk == 0 {
            break;
        }
        let mut reduced = false;
        for c in 0..granularity {
            let start = c * chunk;
            let end = if c + 1 == granularity {
                ops.len()
            } else {
                start + chunk
            };
            let candidate: Vec<RecordedOp> = ops[..start]
                .iter()
                .chain(ops[end..].iter())
                .cloned()
                .collect();
            if reproduces(&candidate) {
                ops = candidate;
                granularity = (granularity - 1).max(2);
                reduced = true;
                break;
            }
        }
        if !reduced {
            granularity = granularity.saturating_mul(2);
        }
    }
    ops
}

fn max_producer_id(ops: &[RecordedOp]) -> Option<usize> {
    ops.iter()
        .filter_map(|op| match op {
            RecordedOp::InsertTiling { producer_id }
            | RecordedOp::InsertFloat { producer_id, .. }
            | RecordedOp::InsertFullscreen { producer_id, .. }
            | RecordedOp::AddMonitor { producer_id, .. } => Some(*producer_id),
            _ => None,
        })
        .max()
}

fn record(
    seed: u64,
    ops_per_run: usize,
    make_hub: fn() -> Hub,
) -> (Vec<RecordedOp>, FailureSignature) {
    let mut hub = make_hub();
    let mut ops: Vec<RecordedOp> = Vec::new();
    let abort = AtomicBool::new(false);
    let signature = capture_panic(|| {
        run_iteration(seed, ops_per_run, &mut hub, &abort, |op| {
            ops.push(op.clone());
        });
    });
    (
        ops,
        signature.expect("seed did not panic, nothing to reduce"),
    )
}

fn replay(ops: &[RecordedOp], make_hub: fn() -> Hub) -> Option<FailureSignature> {
    capture_panic(|| {
        let mut hub = make_hub();
        let table_size = max_producer_id(ops).map(|m| m + 1).unwrap_or(0);
        let mut live_window: Vec<Option<WindowId>> = vec![None; table_size];
        let mut live_monitor: Vec<Option<MonitorId>> = vec![None; table_size];
        let primary = hub.focused_monitor();

        for op in ops {
            match op {
                RecordedOp::InsertTiling { producer_id } => {
                    let id = hub.insert_tiling(hub.current_workspace());
                    live_window[*producer_id] = Some(id);
                }
                RecordedOp::InsertFloat { producer_id, dim } => {
                    let id = hub.insert_float(hub.current_workspace(), *dim);
                    live_window[*producer_id] = Some(id);
                }
                RecordedOp::InsertFullscreen {
                    producer_id,
                    restrictions,
                } => {
                    let id = hub.insert_fullscreen(hub.current_workspace(), *restrictions);
                    live_window[*producer_id] = Some(id);
                }
                RecordedOp::AddMonitor {
                    producer_id,
                    name,
                    dim,
                    scale,
                } => {
                    let id = hub.add_monitor(name.clone(), *dim, *scale);
                    live_monitor[*producer_id] = Some(id);
                }
                RecordedOp::DeleteWindow { window } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.delete_window(id);
                    live_window[window.0] = None;
                }
                RecordedOp::RemoveMonitor { monitor, fallback } => {
                    let Some(mon_id) = resolve_monitor(monitor, &live_monitor, primary) else {
                        continue;
                    };
                    let Some(fb_id) = resolve_monitor(fallback, &live_monitor, primary) else {
                        continue;
                    };
                    if let Some(pos) = live_monitor.iter().position(|m| *m == Some(mon_id)) {
                        live_monitor[pos] = None;
                    }
                    hub.remove_monitor(mon_id, fb_id);
                }
                RecordedOp::SetFullscreen {
                    window,
                    restrictions,
                } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.set_fullscreen(id, *restrictions);
                }
                RecordedOp::UnsetFullscreen { window } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.unset_fullscreen(id);
                }
                RecordedOp::SetFocus { window } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.set_focus(id);
                }
                RecordedOp::SetWindowConstraint {
                    window,
                    min_w,
                    min_h,
                    max_w,
                    max_h,
                } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.set_window_constraint(id, *min_w, *min_h, *max_w, *max_h);
                }
                RecordedOp::SetWindowTitle { window, title } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.set_window_title(id, title.clone());
                }
                RecordedOp::MinimizeWindow { window } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.minimize_window(id);
                }
                RecordedOp::UnminimizeWindow { window } => {
                    let Some(id) = live_window.get(window.0).copied().flatten() else {
                        continue;
                    };
                    hub.unminimize_window(id);
                }
                RecordedOp::MoveToWorkspace { name } => {
                    hub.move_focused_to_workspace(name);
                }
                RecordedOp::FocusWorkspace { name } => {
                    hub.focus_workspace(name);
                }
                RecordedOp::FocusMonitor { target } => {
                    hub.focus_monitor(target);
                }
                RecordedOp::MoveToMonitor { target } => {
                    hub.move_focused_to_monitor(target);
                }
                RecordedOp::FocusLeft => hub.focus_left(),
                RecordedOp::FocusRight => hub.focus_right(),
                RecordedOp::FocusUp => hub.focus_up(),
                RecordedOp::FocusDown => hub.focus_down(),
                RecordedOp::MoveLeft => hub.move_left(),
                RecordedOp::MoveRight => hub.move_right(),
                RecordedOp::MoveUp => hub.move_up(),
                RecordedOp::MoveDown => hub.move_down(),
                RecordedOp::ToggleSpawnMode => hub.toggle_spawn_mode(),
                RecordedOp::ToggleDirection => hub.toggle_direction(),
                RecordedOp::FocusParent => hub.focus_parent(),
                RecordedOp::ToggleContainerLayout => hub.toggle_container_layout(),
                RecordedOp::FocusNextTab => hub.focus_next_tab(),
                RecordedOp::FocusPrevTab => hub.focus_prev_tab(),
                RecordedOp::ToggleFloat => hub.toggle_float(),
                RecordedOp::ToggleFullscreen => hub.toggle_fullscreen(),
                RecordedOp::IncreaseMasterRatio => {
                    hub.handle_tiling_action(TilingAction::GrowMaster);
                }
                RecordedOp::DecreaseMasterRatio => {
                    hub.handle_tiling_action(TilingAction::ShrinkMaster);
                }
                RecordedOp::IncrementMasterCount => {
                    hub.handle_tiling_action(TilingAction::MoreMaster);
                }
                RecordedOp::DecrementMasterCount => {
                    hub.handle_tiling_action(TilingAction::FewerMaster);
                }
                RecordedOp::QueryWorkspaces => {
                    hub.query_workspaces();
                }
            }
            validate_hub(&hub);
        }
    })
}

fn resolve_monitor(
    recorded: &RecordedMonitor,
    live_monitor: &[Option<MonitorId>],
    primary: MonitorId,
) -> Option<MonitorId> {
    if recorded.0 == usize::MAX {
        return Some(primary);
    }
    live_monitor.get(recorded.0).copied().flatten()
}

fn reproduces_signature(
    candidate: &[RecordedOp],
    target: &FailureSignature,
    make_hub: fn() -> Hub,
) -> bool {
    matches!(replay(candidate, make_hub), Some(ref sig) if sig == target)
}
mod tests {
    use super::*;

    #[test]
    fn normalize_digits_replaces_runs() {
        assert_eq!(normalize_digits(""), "");
        assert_eq!(normalize_digits("abc"), "abc");
        assert_eq!(normalize_digits("7"), "#");
        assert_eq!(normalize_digits("123"), "#");
        assert_eq!(normalize_digits("a1b"), "a#b");
        assert_eq!(normalize_digits("12abc34"), "#abc#");
        assert_eq!(
            normalize_digits("window 42 at pos 100"),
            "window # at pos #"
        );
        assert_eq!(normalize_digits("99x00y11"), "#x#y#");
    }

    #[test]
    fn ddmin_strips_context_to_sentinel() {
        let sentinel_title = "SENTINEL";
        let sentinel = RecordedOp::SetWindowTitle {
            window: RecordedWindow(0),
            title: sentinel_title.into(),
        };
        let ops = vec![
            RecordedOp::FocusParent,
            RecordedOp::QueryWorkspaces,
            RecordedOp::ToggleSpawnMode,
            sentinel.clone(),
            RecordedOp::FocusParent,
            RecordedOp::QueryWorkspaces,
            RecordedOp::ToggleSpawnMode,
        ];
        let predicate = |candidate: &[RecordedOp]| {
            candidate.iter().any(|op| {
                matches!(
                    op,
                    RecordedOp::SetWindowTitle { title, .. } if title == sentinel_title
                )
            })
        };
        let reduced = ddmin(ops, predicate);
        assert_eq!(reduced.len(), 1);
        assert!(matches!(
            &reduced[0],
            RecordedOp::SetWindowTitle { title, .. } if title == sentinel_title
        ));
    }
}
