//! Smoke tests and delta-debugging reducer for the Hub.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, setup_hub,
    setup_logger_with_level, titled, validate_hub,
};
use crate::action::MonitorTarget;
use crate::config::{SizeConstraint, SplitMode, Strategy, TreeLayoutNode, WindowMatcher};
use crate::core::hub::{GlobalLayoutConfig, Hub};
use crate::core::node::{Dimension, Length, MonitorId, WindowId, WindowRestrictions};
use crate::core::strategy::TilingAction;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

const RUNS: usize = 200;
const OPS_PER_RUN: usize = 10000;
const SEED: u64 = 42u64;
const PREF_TREE_MAX_LEAVES: usize = 30;
const CONTAINER_BASE: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SmokeStrategy {
    PartitionTree,
    Master,
    PreferredTree,
}

impl SmokeStrategy {
    fn all() -> &'static [SmokeStrategy] {
        &[
            SmokeStrategy::PartitionTree,
            SmokeStrategy::Master,
            SmokeStrategy::PreferredTree,
        ]
    }

    fn test_name(self) -> &'static str {
        match self {
            SmokeStrategy::PartitionTree => "partition-tree",
            SmokeStrategy::Master => "master",
            SmokeStrategy::PreferredTree => "pref-tree",
        }
    }

    /// Build a hub from this strategy. Returns the hub, the list of preferred
    /// titles (empty for non-preferred-tree strategies), and the tree ops used
    /// (empty for non-preferred-tree strategies; needed by the reducer).
    fn build_hub(
        self,
        rng: &mut ChaCha8Rng,
        abort: &AtomicBool,
    ) -> (Hub, Vec<String>, Vec<PrefTreeBuildOp>) {
        match self {
            SmokeStrategy::PartitionTree => (setup_hub(), Vec::new(), Vec::new()),
            SmokeStrategy::Master => (
                TestHubBuilder::new()
                    .with_layout(
                        LayoutConfigBuilder::new()
                            .with_strategy(Strategy::Master)
                            .build(),
                    )
                    .build(),
                Vec::new(),
                Vec::new(),
            ),
            SmokeStrategy::PreferredTree => {
                let (tree_ops, preferred_titles) =
                    generate_tree_ops(rng, abort, PREF_TREE_MAX_LEAVES);
                let tree_node = reconstruct_tree(&tree_ops);
                let hub = make_pref_tree_hub(tree_node);
                (hub, preferred_titles, tree_ops)
            }
        }
    }

    /// Return a plain `fn() -> Hub` for this strategy. Only valid for
    /// non-preferred-tree strategies (used by the reducer's `ddmin` loop).
    fn make_simple_hub(self) -> fn() -> Hub {
        match self {
            SmokeStrategy::PartitionTree => setup_hub,
            SmokeStrategy::Master => || {
                TestHubBuilder::new()
                    .with_layout(
                        LayoutConfigBuilder::new()
                            .with_strategy(Strategy::Master)
                            .build(),
                    )
                    .build()
            },
            SmokeStrategy::PreferredTree => {
                panic!("PreferredTree reduce uses pref_tree_shrink, not make_simple_hub")
            }
        }
    }
}

fn strategy_for_seed(seed: u64) -> SmokeStrategy {
    let all = SmokeStrategy::all();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    all[rng.random_range(0..all.len())]
}

#[test]
fn smoke_test() {
    setup_logger_with_level("warn");

    let completed = AtomicUsize::new(0);
    let abort = AtomicBool::new(false);

    (0..RUNS).into_par_iter().for_each(|run| {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let strategy_seed = SEED.wrapping_add(run as u64);
        let strategy = strategy_for_seed(strategy_seed);
        run_smoke_iteration(strategy_seed, OPS_PER_RUN, strategy, &abort);
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        if done.is_multiple_of(10) {
            tracing::info!("Completed {done}/{RUNS}");
        }
    });
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reproduce_smoke_failure() {
    setup_logger_with_level("info");
    let seed = smoke_seed_from_env();
    let strategy = strategy_for_seed(seed);
    let abort = AtomicBool::new(false);
    run_smoke_iteration(seed, OPS_PER_RUN, strategy, &abort);
}

#[test]
#[ignore = "manual: set DOME_SMOKE_SEED to a failing seed and run with --ignored"]
fn reduce_smoke_failure() {
    setup_logger_with_level("info");
    let seed = smoke_seed_from_env();
    let strategy = strategy_for_seed(seed);
    let (tree_ops, window_ops, signature) = record(seed, OPS_PER_RUN, strategy);

    match strategy {
        SmokeStrategy::PreferredTree => {
            tracing::info!(
                tree_ops = tree_ops.len(),
                window_ops = window_ops.len(),
                ?signature,
                "captured failure",
            );
            let (reduced_tree, reduced_window) = pref_tree_shrink(tree_ops, window_ops, &signature);
            tracing::error!("=== REDUCED TREE OPS ({}) ===", reduced_tree.len());
            for (i, op) in reduced_tree.iter().enumerate() {
                tracing::error!("  {i}: {op:?}");
            }
            tracing::error!("=== REDUCED WINDOW OPS ({}) ===", reduced_window.len());
            for (i, op) in reduced_window.iter().enumerate() {
                tracing::error!("  {i}: {op:?}");
            }
        }
        _ => {
            tracing::info!(recorded = window_ops.len(), ?signature, "captured failure");
            let make_hub = strategy.make_simple_hub();
            let reduced = config_op_shrink(window_ops, &signature, make_hub);
            tracing::error!("=== REDUCED OPERATIONS ({}) ===", reduced.len());
            for (i, op) in reduced.iter().enumerate() {
                tracing::error!("  {i}: {op:?}");
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
    ConfigReload,
    SyncPreferredLayout,
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
    OpKind::ConfigReload,
    OpKind::SyncPreferredLayout,
];

#[derive(Debug, Clone)]
enum RecordedOp {
    InsertTiling {
        producer_id: usize,
        title: Option<String>,
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
    ConfigReload {
        layout: GlobalLayoutConfig,
    },
    SyncPreferredLayout {
        workspace_name: String,
        tree_ops: Vec<PrefTreeBuildOp>,
    },
}

#[derive(Debug, Clone, Copy)]
struct RecordedWindow(usize);

#[derive(Debug, Clone, Copy)]
struct RecordedMonitor(usize);

fn run_smoke_iteration(seed: u64, ops_per_run: usize, strategy: SmokeStrategy, abort: &AtomicBool) {
    if abort.load(Ordering::Relaxed) {
        return;
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let (mut hub, free_titles, _tree_ops) = strategy.build_hub(&mut rng, abort);
    if abort.load(Ordering::Relaxed) {
        return;
    }

    let mut current_layout = match strategy {
        SmokeStrategy::PartitionTree | SmokeStrategy::PreferredTree => {
            LayoutConfigBuilder::new().build()
        }
        SmokeStrategy::Master => LayoutConfigBuilder::new()
            .with_strategy(Strategy::Master)
            .build(),
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_iteration(
            &mut hub,
            abort,
            |_| {},
            &mut rng,
            ops_per_run,
            &free_titles,
            &mut current_layout,
        );
    }));

    if let Err(e) = result {
        abort.store(true, Ordering::Relaxed);
        let name = strategy.test_name();
        tracing::error!(
            "To reproduce: DOME_SMOKE_STRATEGY={name} DOME_SMOKE_SEED={seed} cargo test --lib \
             reproduce_smoke_failure -- --ignored --nocapture",
        );
        tracing::error!(
            "To reduce:    DOME_SMOKE_STRATEGY={name} DOME_SMOKE_SEED={seed} cargo test --lib \
             reduce_smoke_failure -- --ignored --nocapture",
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
    hub: &mut Hub,
    abort: &AtomicBool,
    mut observer: F,
    rng: &mut ChaCha8Rng,
    ops_per_run: usize,
    free_titles: &[String],
    current_layout: &mut GlobalLayoutConfig,
) where
    F: FnMut(&RecordedOp),
{
    let mut free_titles: Vec<String> = free_titles.to_vec();
    let mut windows: Vec<WindowId> = Vec::new();
    let mut window_origin: Vec<usize> = Vec::new();
    let mut window_minimized: Vec<bool> = Vec::new();
    let mut monitors: Vec<MonitorId> = vec![hub.focused_monitor()];
    let mut monitor_origin: Vec<usize> = vec![usize::MAX];
    let mut next_op_index: usize = 0;
    let mut workspace_names: Vec<String> = vec!["0".to_string()];

    for _ in 0..ops_per_run {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let kind = ALL_OP_KINDS[rng.random_range(0..ALL_OP_KINDS.len())];
        let Some(op) = build_op(
            kind,
            rng,
            &windows,
            &window_origin,
            &window_minimized,
            &monitors,
            &monitor_origin,
            next_op_index,
            &mut free_titles,
            current_layout,
            &workspace_names,
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
        if let RecordedOp::ConfigReload { layout } = &op {
            *current_layout = layout.clone();
        }
        match &op {
            RecordedOp::MoveToWorkspace { name } | RecordedOp::FocusWorkspace { name } => {
                if !workspace_names.iter().any(|n| n == name) {
                    workspace_names.push(name.clone());
                }
            }
            _ => {}
        }
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
    free_titles: &mut Vec<String>,
    current_layout: &mut GlobalLayoutConfig,
    workspace_names: &[String],
) -> Option<RecordedOp> {
    match kind {
        OpKind::InsertTiling => {
            if rng.random_bool(0.5)
                && let Some(title) = free_titles.pop()
            {
                return Some(RecordedOp::InsertTiling {
                    producer_id: next_op_index,
                    title: Some(title),
                });
            }
            Some(RecordedOp::InsertTiling {
                producer_id: next_op_index,
                title: None,
            })
        }
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
        OpKind::ConfigReload => {
            let mut layout = current_layout.clone();
            match rng.random_range(0..5u8) {
                0 => {
                    layout.partition_tree.automatic_tiling =
                        !layout.partition_tree.automatic_tiling;
                }
                1 => {
                    let h = rng.random_range(10.0f32..50.0);
                    layout.partition_tree.tab_bar_height = Length::new(h);
                }
                2 => {
                    layout.master.master_ratio = rng.random_range(0.2f32..0.8);
                }
                3 => {
                    layout.master.master_count = rng.random_range(1..=4);
                }
                _ => {
                    let v = rng.random_range(10.0f32..200.0);
                    layout.min_width = SizeConstraint::Pixels(Length::new(v));
                }
            }
            Some(RecordedOp::ConfigReload { layout })
        }
        OpKind::SyncPreferredLayout => {
            if workspace_names.is_empty() {
                return None;
            }
            if current_layout.strategy != Strategy::PartitionTree {
                return None;
            }
            let workspace_name =
                workspace_names[rng.random_range(0..workspace_names.len())].clone();
            let max_leaves = rng.random_range(2..=5);
            let (tree_ops, _titles) =
                generate_tree_ops_small(rng, &AtomicBool::new(false), max_leaves);
            if tree_ops.is_empty() {
                return None;
            }
            Some(RecordedOp::SyncPreferredLayout {
                workspace_name,
                tree_ops,
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
        RecordedOp::InsertTiling { producer_id, title } => {
            let window_title = title.as_deref().unwrap_or("w1");
            let id = hub.insert_tiling(hub.current_workspace(), titled(window_title));
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::InsertFloat { producer_id, dim } => {
            let id = hub.insert_float(hub.current_workspace(), *dim, titled("w2"));
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::InsertFullscreen {
            producer_id,
            restrictions,
        } => {
            let id = hub.insert_fullscreen(hub.current_workspace(), *restrictions, titled("w3"));
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
        RecordedOp::ConfigReload { layout } => {
            hub.sync_configuration(layout.clone());
        }
        RecordedOp::SyncPreferredLayout {
            workspace_name,
            tree_ops,
        } => {
            let tree = reconstruct_tree(tree_ops);
            let mut ws_builder = LayoutWorkspaceConfigBuilder::new(workspace_name)
                .with_strategy(Strategy::PartitionTree);
            if let Some(t) = tree {
                ws_builder = ws_builder.with_tree(t);
            }
            hub.sync_preferred_layout(vec![ws_builder.build()]);
        }
    }
}

fn smoke_seed_from_env() -> u64 {
    match std::env::var("DOME_SMOKE_SEED") {
        Ok(value) => match value.parse::<u64>() {
            Ok(seed) => seed,
            Err(_) => panic!(
                "DOME_SMOKE_SEED='{value}' is not a valid u64.\n\
                 example: DOME_SMOKE_SEED=167 cargo test --lib reproduce_smoke_failure \
                 -- --ignored --nocapture"
            ),
        },
        Err(_) => panic!(
            "DOME_SMOKE_SEED not set.\n\
             example: DOME_SMOKE_SEED=167 cargo test --lib reproduce_smoke_failure \
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
fn ddmin<T: Clone, P>(mut ops: Vec<T>, mut reproduces: P) -> Vec<T>
where
    P: FnMut(&[T]) -> bool,
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
            let candidate: Vec<T> = ops[..start]
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
            RecordedOp::InsertTiling { producer_id, .. }
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
    strategy: SmokeStrategy,
) -> (Vec<PrefTreeBuildOp>, Vec<RecordedOp>, FailureSignature) {
    let abort = AtomicBool::new(false);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let (mut hub, free_titles, tree_ops) = strategy.build_hub(&mut rng, &abort);
    let mut ops: Vec<RecordedOp> = Vec::new();
    let mut current_layout = match strategy {
        SmokeStrategy::PartitionTree | SmokeStrategy::PreferredTree => {
            LayoutConfigBuilder::new().build()
        }
        SmokeStrategy::Master => LayoutConfigBuilder::new()
            .with_strategy(Strategy::Master)
            .build(),
    };
    let signature = capture_panic(|| {
        run_iteration(
            &mut hub,
            &abort,
            |op| ops.push(op.clone()),
            &mut rng,
            ops_per_run,
            &free_titles,
            &mut current_layout,
        );
    });
    (
        tree_ops,
        ops,
        signature.expect("seed did not panic, nothing to reduce"),
    )
}

fn replay_without_capture(ops: &[RecordedOp], make_hub: impl FnOnce() -> Hub) {
    let mut hub = make_hub();
    let table_size = max_producer_id(ops).map(|m| m + 1).unwrap_or(0);
    let mut live_window: Vec<Option<WindowId>> = vec![None; table_size];
    let mut live_monitor: Vec<Option<MonitorId>> = vec![None; table_size];
    let primary = hub.focused_monitor();

    for op in ops {
        match op {
            RecordedOp::InsertTiling { producer_id, title } => {
                let window_title = title.as_deref().unwrap_or("w4");
                let id = hub.insert_tiling(hub.current_workspace(), titled(window_title));
                live_window[*producer_id] = Some(id);
            }
            RecordedOp::InsertFloat { producer_id, dim } => {
                let id = hub.insert_float(hub.current_workspace(), *dim, titled("w5"));
                live_window[*producer_id] = Some(id);
            }
            RecordedOp::InsertFullscreen {
                producer_id,
                restrictions,
            } => {
                let id =
                    hub.insert_fullscreen(hub.current_workspace(), *restrictions, titled("w6"));
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
            RecordedOp::ConfigReload { layout } => {
                hub.sync_configuration(layout.clone());
            }
            RecordedOp::SyncPreferredLayout {
                workspace_name,
                tree_ops,
            } => {
                let tree = reconstruct_tree(tree_ops);
                let mut ws_builder = LayoutWorkspaceConfigBuilder::new(workspace_name)
                    .with_strategy(Strategy::PartitionTree);
                if let Some(t) = tree {
                    ws_builder = ws_builder.with_tree(t);
                }
                hub.sync_preferred_layout(vec![ws_builder.build()]);
            }
        }
        validate_hub(&hub);
    }
}

fn replay(ops: &[RecordedOp], make_hub: impl FnOnce() -> Hub) -> Option<FailureSignature> {
    capture_panic(|| replay_without_capture(ops, make_hub))
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

#[derive(Debug, Clone)]
enum PrefTreeBuildOp {
    InsertLeaf {
        leaf_id: usize,
        title: String,
        anchor: Option<usize>,
        split: SplitMode,
    },
}

fn generate_tree_ops_small(
    rng: &mut ChaCha8Rng,
    abort: &AtomicBool,
    max_leaves: usize,
) -> (Vec<PrefTreeBuildOp>, Vec<String>) {
    generate_tree_ops(rng, abort, max_leaves)
}

fn random_split(rng: &mut ChaCha8Rng) -> SplitMode {
    match rng.random_range(0..3u8) {
        0 => SplitMode::Horizontal,
        1 => SplitMode::Vertical,
        _ => SplitMode::Tabbed,
    }
}

fn generate_tree_ops(
    rng: &mut ChaCha8Rng,
    abort: &AtomicBool,
    max_leaves: usize,
) -> (Vec<PrefTreeBuildOp>, Vec<String>) {
    let mut ops = Vec::new();
    let mut titles = Vec::new();
    let leaf_target = rng.random_range(2..=max_leaves);

    if abort.load(Ordering::Relaxed) {
        return (ops, titles);
    }

    // First leaf is the implicit root.
    let root_id = 0usize;
    let title = format!("pref-{}", root_id);
    titles.push(title.clone());
    ops.push(PrefTreeBuildOp::InsertLeaf {
        leaf_id: root_id,
        title,
        anchor: None,
        split: random_split(rng),
    });

    let mut next_leaf_id = 1usize;
    let mut container_ids: HashSet<usize> = HashSet::new();
    let mut container_counter: usize = 0;
    let mut leaves_created = 1usize;

    while leaves_created < leaf_target {
        if abort.load(Ordering::Relaxed) {
            break;
        }

        // Pick an anchor from among all existing tree nodes.
        let leaf_count = next_leaf_id;
        let total_nodes = leaf_count + container_ids.len();
        let pick = rng.random_range(0..total_nodes);
        let anchor = if pick < leaf_count {
            pick
        } else {
            let ci = pick - leaf_count;
            *container_ids.iter().nth(ci).unwrap()
        };

        let leaf_id = next_leaf_id;
        next_leaf_id += 1;
        let title = format!("pref-{}", leaf_id);
        titles.push(title.clone());

        let split = random_split(rng);
        ops.push(PrefTreeBuildOp::InsertLeaf {
            leaf_id,
            title,
            anchor: Some(anchor),
            split,
        });

        if container_ids.contains(&anchor) {
            // Anchor is a container: new leaf becomes a child of that container.
        } else {
            // Anchor is a leaf: wrapping creates a new container.
            let container_id = CONTAINER_BASE + container_counter;
            container_counter += 1;
            container_ids.insert(container_id);
        }

        leaves_created += 1;
    }

    (ops, titles)
}

struct ReconContainer {
    split: SplitMode,
    children: Vec<usize>,
}

fn build_node_recursive(
    id: usize,
    leaves: &HashMap<usize, String>,
    containers: &HashMap<usize, ReconContainer>,
) -> Option<TreeLayoutNode> {
    if let Some(title) = leaves.get(&id) {
        return Some(TreeLayoutNode::Leaf(WindowMatcher {
            title: Some(title.clone()),
            ..Default::default()
        }));
    }
    if let Some(c) = containers.get(&id) {
        let children = c
            .children
            .iter()
            .filter_map(|cid| build_node_recursive(*cid, leaves, containers))
            .collect::<Vec<_>>();
        if children.is_empty() {
            return None;
        }
        return Some(TreeLayoutNode::Container {
            split: Some(c.split),
            children,
        });
    }
    None
}

fn reconstruct_tree(ops: &[PrefTreeBuildOp]) -> Option<TreeLayoutNode> {
    let mut leaves: HashMap<usize, String> = HashMap::new();
    let mut containers: HashMap<usize, ReconContainer> = HashMap::new();
    let mut parent: HashMap<usize, Option<usize>> = HashMap::new();
    let mut root: Option<usize> = None;
    let mut container_counter: usize = 0;

    for op in ops {
        let PrefTreeBuildOp::InsertLeaf {
            leaf_id,
            title,
            anchor,
            split,
        } = op;

        match anchor {
            None => {
                if root.is_none() {
                    leaves.insert(*leaf_id, title.clone());
                    parent.insert(*leaf_id, None);
                    root = Some(*leaf_id);
                }
            }
            Some(anchor_id) => {
                let aid = *anchor_id;
                if containers.contains_key(&aid) {
                    leaves.insert(*leaf_id, title.clone());
                    containers.get_mut(&aid).unwrap().children.push(*leaf_id);
                    parent.insert(*leaf_id, Some(aid));
                } else if leaves.contains_key(&aid) {
                    let container_id = CONTAINER_BASE + container_counter;
                    container_counter += 1;
                    containers.insert(
                        container_id,
                        ReconContainer {
                            split: *split,
                            children: vec![aid, *leaf_id],
                        },
                    );
                    leaves.insert(*leaf_id, title.clone());
                    let anchor_parent = parent.get(&aid).copied().unwrap();
                    parent.insert(container_id, anchor_parent);
                    match anchor_parent {
                        None => root = Some(container_id),
                        Some(pid) => {
                            if let Some(p) = containers.get_mut(&pid)
                                && let Some(pos) = p.children.iter().position(|c| *c == aid)
                            {
                                p.children[pos] = container_id;
                            }
                        }
                    }
                    parent.insert(aid, Some(container_id));
                    parent.insert(*leaf_id, Some(container_id));
                }
            }
        }
    }

    let root_id = root?;
    build_node_recursive(root_id, &leaves, &containers)
}

fn make_pref_tree_hub(tree: Option<TreeLayoutNode>) -> Hub {
    let mut ws_builder =
        LayoutWorkspaceConfigBuilder::new("1").with_strategy(Strategy::PartitionTree);
    if let Some(t) = tree {
        ws_builder = ws_builder.with_tree(t);
    }
    TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![ws_builder.build()])
        .build()
}

fn pref_tree_reproduces_signature(
    tree_ops: &[PrefTreeBuildOp],
    window_ops: &[RecordedOp],
    target: &FailureSignature,
) -> bool {
    let tree = reconstruct_tree(tree_ops);
    matches!(
        replay(window_ops, || make_pref_tree_hub(tree)),
        Some(ref sig) if sig == target
    )
}

fn pref_tree_shrink(
    tree_ops: Vec<PrefTreeBuildOp>,
    window_ops: Vec<RecordedOp>,
    target: &FailureSignature,
) -> (Vec<PrefTreeBuildOp>, Vec<RecordedOp>) {
    let mut tree = tree_ops;
    let mut window = window_ops;
    let target = target.clone();
    loop {
        let new_tree = ddmin(tree.clone(), |t| {
            pref_tree_reproduces_signature(t, &window, &target)
        });
        let new_window = ddmin(window.clone(), |w| {
            pref_tree_reproduces_signature(&new_tree, w, &target)
        });

        let mut payload_shrunk = false;
        let mut current_window = new_window.clone();
        for i in 0..current_window.len() {
            if let RecordedOp::SyncPreferredLayout {
                ref workspace_name,
                ref tree_ops,
            } = current_window[i]
            {
                let reduced_tree = ddmin(tree_ops.clone(), |t| {
                    let mut candidate = current_window.clone();
                    candidate[i] = RecordedOp::SyncPreferredLayout {
                        workspace_name: workspace_name.clone(),
                        tree_ops: t.to_vec(),
                    };
                    pref_tree_reproduces_signature(&new_tree, &candidate, &target)
                });
                if reduced_tree.len() < tree_ops.len() {
                    current_window[i] = RecordedOp::SyncPreferredLayout {
                        workspace_name: workspace_name.clone(),
                        tree_ops: reduced_tree,
                    };
                    payload_shrunk = true;
                    break;
                }
            }
        }

        if new_tree.len() == tree.len() && new_window.len() == window.len() && !payload_shrunk {
            return (tree, new_window);
        }
        tree = new_tree;
        window = current_window;
        tracing::info!(tree = tree.len(), window = window.len(), "shrink iteration");
    }
}

fn config_op_shrink(
    ops: Vec<RecordedOp>,
    target: &FailureSignature,
    make_hub: fn() -> Hub,
) -> Vec<RecordedOp> {
    let mut ops = ops;
    let target = target.clone();
    loop {
        ops = ddmin(ops.clone(), |c| reproduces_signature(c, &target, make_hub));

        let mut shrunk = false;
        for i in 0..ops.len() {
            if let RecordedOp::SyncPreferredLayout {
                ref workspace_name,
                ref tree_ops,
            } = ops[i]
            {
                let reduced_tree = ddmin(tree_ops.clone(), |t| {
                    let mut candidate = ops.clone();
                    candidate[i] = RecordedOp::SyncPreferredLayout {
                        workspace_name: workspace_name.clone(),
                        tree_ops: t.to_vec(),
                    };
                    reproduces_signature(&candidate, &target, make_hub)
                });
                if reduced_tree.len() < tree_ops.len() {
                    ops[i] = RecordedOp::SyncPreferredLayout {
                        workspace_name: workspace_name.clone(),
                        tree_ops: reduced_tree,
                    };
                    shrunk = true;
                    break;
                }
            }
        }

        if !shrunk {
            break;
        }
    }
    ops
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
    fn ddmin_strips_padding_to_sentinel() {
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
