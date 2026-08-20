//! Smoke tests and delta-debugging reducer for the Hub.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

use super::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect,
    setup_logger_with_level, titled, titled_matcher, validate_hub,
};
use crate::action::MonitorTarget;
use crate::config::{
    LayoutWorkspaceConfig, SizeConstraint, SplitMode, Strategy, TreeLayoutNode, WindowMatcher,
};
use crate::core::hub::{GlobalLayoutConfig, Hub};
use crate::core::node::{
    Length, LimitObservation, LimitUpdate, MonitorId, PixelRect, Pixels, WindowId,
    WindowRestrictions,
};
use crate::core::strategy::TilingAction;
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

const RUNS: usize = 200;
const OPS_PER_RUN: usize = 10000;
const SEED: u64 = 42u64;
const PREF_TREE_MAX_LEAVES: usize = 30;

/// The two titles the harness inserts under a fixed name. A generated matcher is
/// inert unless it is written against one of these or a `pref-N` preferred title,
/// so they are named rather than repeated at the four insert sites.
const DEFAULT_TILING_TITLE: &str = "w1";
const FULLSCREEN_TITLE: &str = "w3";
const CONTAINER_BASE: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
enum SmokeStrategy {
    PartitionTree,
    Master,
}

impl SmokeStrategy {
    fn all() -> &'static [SmokeStrategy] {
        &[SmokeStrategy::PartitionTree, SmokeStrategy::Master]
    }

    fn test_name(self) -> &'static str {
        match self {
            SmokeStrategy::PartitionTree => "partition-tree",
            SmokeStrategy::Master => "master",
        }
    }

    fn build_hub(self) -> Hub {
        TestHubBuilder::new()
            .with_layout(initial_layout(self))
            .build()
    }
}

/// `DEFAULT_TILING_TITLE` is the title of nearly every plain tiling insert, so a
/// matcher on it reroutes most of a run's inserts away from tiling. Kept rare so
/// that most runs still exercise ordinary tiling behaviour.
const DEFAULT_TITLE_MATCHER_PROBABILITY: f64 = 0.1;
const PREFERRED_TITLE_FLOAT_PROBABILITY: f64 = 0.15;
const PREFERRED_TITLE_FULLSCREEN_PROBABILITY: f64 = 0.1;

/// Insert titles are drawn from this fixed pool rather than from whatever the
/// generators happened to mint, so that a generated matcher is never inert once
/// preferred layouts arrive as ops instead of as construction artifacts. Sized to
/// cover every id the tree and pane generators can produce.
const PREF_TITLE_POOL_SIZE: usize = PREF_TREE_MAX_LEAVES;

fn pref_title(index: usize) -> String {
    format!("pref-{index}")
}

fn pref_title_pool() -> Vec<String> {
    (0..PREF_TITLE_POOL_SIZE).map(pref_title).collect()
}

fn initial_layout(strategy: SmokeStrategy) -> GlobalLayoutConfig {
    match strategy {
        SmokeStrategy::PartitionTree => LayoutConfigBuilder::new().build(),
        SmokeStrategy::Master => LayoutConfigBuilder::new()
            .with_strategy(Strategy::Master)
            .build(),
    }
}

/// Titles are drawn only from what the harness actually inserts, since a matcher
/// on anything else is inert. A title can land in both lists, which is what
/// reaches fullscreen-beats-float in `resolve_matcher`.
fn generate_matcher_titles(
    rng: &mut ChaCha8Rng,
    preferred_titles: &[String],
) -> (Vec<String>, Vec<String>) {
    let mut float = Vec::new();
    let mut fullscreen = Vec::new();
    if rng.random_bool(DEFAULT_TITLE_MATCHER_PROBABILITY) {
        float.push(DEFAULT_TILING_TITLE.to_string());
    }
    if rng.random_bool(DEFAULT_TITLE_MATCHER_PROBABILITY) {
        fullscreen.push(DEFAULT_TILING_TITLE.to_string());
    }
    for title in preferred_titles {
        if rng.random_bool(PREFERRED_TITLE_FLOAT_PROBABILITY) {
            float.push(title.clone());
        }
        if rng.random_bool(PREFERRED_TITLE_FULLSCREEN_PROBABILITY) {
            fullscreen.push(title.clone());
        }
    }
    (float, fullscreen)
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
    let (window_ops, signature) = record(seed, OPS_PER_RUN, strategy);

    tracing::info!(
        window_ops = window_ops.len(),
        ?signature,
        "captured failure"
    );
    let reduced = shrink_ops(window_ops, &signature, || strategy.build_hub());
    log_reduced(&reduced);
}

fn log_reduced(window: &[RecordedOp]) {
    tracing::error!("=== REDUCED WINDOW OPS ({}) ===", window.len());
    for (i, op) in window.iter().enumerate() {
        tracing::error!("  {i}: {op:?}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum OpKind {
    InsertTiling,
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
    InsertFullscreen {
        producer_id: usize,
        restrictions: WindowRestrictions,
    },
    AddMonitor {
        producer_id: usize,
        name: String,
        rect: PixelRect,
        scale: f32,
    },
    DeleteWindow {
        window: RecordedWindow,
    },
    RemoveMonitor {
        monitor: RecordedMonitor,
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
        min_w: LimitUpdate,
        min_h: LimitUpdate,
        max_w: LimitUpdate,
        max_h: LimitUpdate,
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
        strategy: Strategy,
        tree_ops: Vec<PrefTreeBuildOp>,
        master: Vec<String>,
        secondary: Vec<String>,
        float: Vec<String>,
        fullscreen: Vec<String>,
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
    let mut hub = strategy.build_hub();
    let mut current_layout = initial_layout(strategy);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_iteration(
            &mut hub,
            abort,
            |_| {},
            &mut rng,
            ops_per_run,
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
    current_layout: &mut GlobalLayoutConfig,
) where
    F: FnMut(&RecordedOp),
{
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
            RecordedOp::MoveToWorkspace { name } | RecordedOp::FocusWorkspace { name }
                if !workspace_names.iter().any(|n| n == name) =>
            {
                workspace_names.push(name.clone());
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
    current_layout: &mut GlobalLayoutConfig,
    workspace_names: &[String],
) -> Option<RecordedOp> {
    match kind {
        OpKind::InsertTiling => {
            if rng.random_bool(0.5) {
                let pool = pref_title_pool();
                let title = pool[rng.random_range(0..pool.len())].clone();
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
        OpKind::InsertFullscreen => {
            // No matcher is configured here, so `None` restrictions would take the
            // default-tiling branch and silently tile under an op named `InsertFullscreen`.
            // `SetFullscreen` still draws all three values.
            let restrictions = match rng.random_range(0..2u8) {
                0 => WindowRestrictions::BlockAll,
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
            let x = monitors.len() as i32 * 150;
            let name = format!("monitor-{}", monitors.len());
            let rect = PixelRect::new(x, 0, 150, 30);
            Some(RecordedOp::AddMonitor {
                producer_id: next_op_index,
                name,
                rect,
                scale: 1.0,
            })
        }
        OpKind::RemoveMonitor => {
            if monitors.len() <= 1 {
                return None;
            }
            // Index 0 is the primary, which the real system re-keys onto a new
            // display rather than removing, so never generate its removal.
            let idx = rng.random_range(1..monitors.len());
            Some(RecordedOp::RemoveMonitor {
                monitor: RecordedMonitor(monitor_origin[idx]),
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
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(Length::new(rng.random_range(1.0f32..50.0))),
            };
            let min_h = match rng.random_range(0..3) {
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(Length::new(rng.random_range(1.0f32..10.0))),
            };
            let max_w = match rng.random_range(0..3) {
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(Length::new(rng.random_range(1.0f32..100.0))),
            };
            let max_h = match rng.random_range(0..3) {
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(Length::new(rng.random_range(1.0f32..20.0))),
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
            match rng.random_range(0..8u8) {
                0 => {
                    layout.partition_tree.automatic_tiling =
                        !layout.partition_tree.automatic_tiling;
                }
                1 => {
                    let h = rng.random_range(10i32..50);
                    layout.partition_tree.tab_bar_height = Pixels::new(h);
                }
                2 => {
                    layout.master.master_ratio = rng.random_range(0.2f32..0.8);
                }
                3 => {
                    layout.master.master_count = rng.random_range(1..=4);
                }
                4 => {
                    let v = rng.random_range(10..200);
                    layout.size_constraints.minimum_width = SizeConstraint::Pixels(Pixels::new(v));
                }
                5 => {
                    layout.strategy = match layout.strategy {
                        Strategy::PartitionTree => Strategy::Master,
                        Strategy::Master => Strategy::PartitionTree,
                    };
                }
                6 => {
                    let (float, _) = generate_matcher_titles(rng, &pref_title_pool());
                    layout.float = float.iter().map(|t| titled_matcher(t)).collect();
                }
                _ => {
                    let (_, fullscreen) = generate_matcher_titles(rng, &pref_title_pool());
                    layout.fullscreen = fullscreen.iter().map(|t| titled_matcher(t)).collect();
                }
            }
            Some(RecordedOp::ConfigReload { layout })
        }
        OpKind::SyncPreferredLayout => {
            if workspace_names.is_empty() {
                return None;
            }
            let workspace_name =
                workspace_names[rng.random_range(0..workspace_names.len())].clone();
            let strategy = current_layout.strategy;
            let (float, fullscreen) = generate_matcher_titles(rng, &pref_title_pool());
            let mut tree_ops = Vec::new();
            let mut master = Vec::new();
            let mut secondary = Vec::new();
            match strategy {
                Strategy::PartitionTree => {
                    let max_leaves = sync_tree_max_leaves(rng);
                    let (ops, _titles) =
                        generate_tree_ops_small(rng, &AtomicBool::new(false), max_leaves);
                    if ops.is_empty() {
                        return None;
                    }
                    tree_ops = ops;
                }
                Strategy::Master => {
                    for title in pref_title_pool() {
                        match rng.random_range(0..4u8) {
                            0 => master.push(title),
                            1 => secondary.push(title),
                            _ => {}
                        }
                    }
                }
            }
            Some(RecordedOp::SyncPreferredLayout {
                workspace_name,
                strategy,
                tree_ops,
                master,
                secondary,
                float,
                fullscreen,
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
            let window_title = title.as_deref().unwrap_or(DEFAULT_TILING_TITLE);
            let id = hub
                .insert_window(
                    titled(window_title),
                    default_rect(),
                    WindowRestrictions::None,
                )
                .expect("test ignore list is empty");
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::InsertFullscreen {
            producer_id,
            restrictions,
        } => {
            let id = hub
                .insert_window(titled(FULLSCREEN_TITLE), default_rect(), *restrictions)
                .expect("test ignore list is empty");
            windows.push(id);
            window_origin.push(*producer_id);
            window_minimized.push(false);
        }
        RecordedOp::AddMonitor {
            producer_id,
            name,
            rect,
            scale,
        } => {
            let id = hub.add_monitor(name.clone(), *rect, *scale);
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
        RecordedOp::RemoveMonitor { monitor } => {
            let pos = monitor_origin
                .iter()
                .position(|&o| o == monitor.0)
                .expect("apply_op: monitor producer_id not found");
            let id = monitors.remove(pos);
            monitor_origin.remove(pos);
            hub.remove_monitor(id);
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
            hub.set_window_constraint(
                windows[pos],
                LimitObservation {
                    min_width: *min_w,
                    min_height: *min_h,
                    max_width: *max_w,
                    max_height: *max_h,
                },
            );
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
            hub.move_focused_to_workspace(name, None);
        }
        RecordedOp::FocusWorkspace { name } => {
            hub.focus_workspace(name, None);
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
            strategy,
            tree_ops,
            master,
            secondary,
            float,
            fullscreen,
        } => {
            hub.sync_preferred_layout(vec![preferred_workspace_config(
                workspace_name,
                *strategy,
                tree_ops,
                master,
                secondary,
                float,
                fullscreen,
            )]);
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
) -> (Vec<RecordedOp>, FailureSignature) {
    let abort = AtomicBool::new(false);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut hub = strategy.build_hub();
    let mut ops: Vec<RecordedOp> = Vec::new();
    let mut current_layout = initial_layout(strategy);
    let signature = capture_panic(|| {
        run_iteration(
            &mut hub,
            &abort,
            |op| ops.push(op.clone()),
            &mut rng,
            ops_per_run,
            &mut current_layout,
        );
    });
    (
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
                let window_title = title.as_deref().unwrap_or(DEFAULT_TILING_TITLE);
                let id = hub
                    .insert_window(
                        titled(window_title),
                        default_rect(),
                        WindowRestrictions::None,
                    )
                    .expect("test ignore list is empty");
                live_window[*producer_id] = Some(id);
            }
            RecordedOp::InsertFullscreen {
                producer_id,
                restrictions,
            } => {
                let id = hub
                    .insert_window(titled(FULLSCREEN_TITLE), default_rect(), *restrictions)
                    .expect("test ignore list is empty");
                live_window[*producer_id] = Some(id);
            }
            RecordedOp::AddMonitor {
                producer_id,
                name,
                rect,
                scale,
            } => {
                let id = hub.add_monitor(name.clone(), *rect, *scale);
                live_monitor[*producer_id] = Some(id);
            }
            RecordedOp::DeleteWindow { window } => {
                let Some(id) = live_window.get(window.0).copied().flatten() else {
                    continue;
                };
                hub.delete_window(id);
                live_window[window.0] = None;
            }
            RecordedOp::RemoveMonitor { monitor } => {
                let Some(mon_id) = resolve_monitor(monitor, &live_monitor, primary) else {
                    continue;
                };
                if let Some(pos) = live_monitor.iter().position(|m| *m == Some(mon_id)) {
                    live_monitor[pos] = None;
                }
                hub.remove_monitor(mon_id);
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
                hub.set_window_constraint(
                    id,
                    LimitObservation {
                        min_width: *min_w,
                        min_height: *min_h,
                        max_width: *max_w,
                        max_height: *max_h,
                    },
                );
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
                hub.move_focused_to_workspace(name, None);
            }
            RecordedOp::FocusWorkspace { name } => {
                hub.focus_workspace(name, None);
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
                strategy,
                tree_ops,
                master,
                secondary,
                float,
                fullscreen,
            } => {
                hub.sync_preferred_layout(vec![preferred_workspace_config(
                    workspace_name,
                    *strategy,
                    tree_ops,
                    master,
                    secondary,
                    float,
                    fullscreen,
                )]);
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
    let title = pref_title(root_id);
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
        let title = pref_title(leaf_id);
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

/// Only the fields the chosen strategy actually consumes are set, because
/// `LayoutWorkspaceConfigBuilder::build` silently discards a tree under
/// `Strategy::Master`, and generated input a builder throws away is fake coverage.
fn preferred_workspace_config(
    workspace_name: &str,
    strategy: Strategy,
    tree_ops: &[PrefTreeBuildOp],
    master: &[String],
    secondary: &[String],
    float: &[String],
    fullscreen: &[String],
) -> LayoutWorkspaceConfig {
    let mut builder = LayoutWorkspaceConfigBuilder::new(workspace_name)
        .with_strategy(strategy)
        .with_float(float.iter().map(|t| titled_matcher(t)).collect())
        .with_fullscreen(fullscreen.iter().map(|t| titled_matcher(t)).collect());
    match strategy {
        Strategy::PartitionTree => {
            if let Some(tree) = reconstruct_tree(tree_ops) {
                builder = builder.with_tree(tree);
            }
        }
        Strategy::Master => {
            builder = builder
                .with_master(master.iter().map(|t| titled_matcher(t)).collect())
                .with_secondary(secondary.iter().map(|t| titled_matcher(t)).collect());
        }
    }
    builder.build()
}

/// Small trees stay common so a `SyncPreferredLayout` does not crowd out the rest of
/// the op mix, while the range still reaches `PREF_TREE_MAX_LEAVES` so that moving
/// tree generation off the construction path does not cut depth.
fn sync_tree_max_leaves(rng: &mut ChaCha8Rng) -> usize {
    if rng.random_bool(0.8) {
        rng.random_range(2..=5)
    } else {
        rng.random_range(6..=PREF_TREE_MAX_LEAVES)
    }
}

fn sync_tree_ops(op: &RecordedOp) -> Option<Vec<PrefTreeBuildOp>> {
    match op {
        RecordedOp::SyncPreferredLayout { tree_ops, .. } => Some(tree_ops.clone()),
        _ => None,
    }
}

fn set_sync_tree_ops(op: &mut RecordedOp, ops: Vec<PrefTreeBuildOp>) {
    if let RecordedOp::SyncPreferredLayout { tree_ops, .. } = op {
        *tree_ops = ops;
    }
}

/// Shrink a generated build artifact and the recorded window ops together,
/// keeping only what still reproduces `target`. `rebuild` turns a candidate
/// artifact back into the hub the run started from, which is what lets a
/// failure be reduced through generated input rather than only through ops.
fn shrink_ops(
    window_ops: Vec<RecordedOp>,
    target: &FailureSignature,
    make_hub: impl Fn() -> Hub + Copy,
) -> Vec<RecordedOp> {
    let mut window = window_ops;
    let target = target.clone();
    let reproduces =
        |w: &[RecordedOp]| matches!(replay(w, make_hub), Some(ref sig) if *sig == target);
    loop {
        let new_window = ddmin(window.clone(), |w| reproduces(w));
        let payload_shrunk = shrink_op_payloads(&new_window, |w| reproduces(w));

        if new_window.len() == window.len() && payload_shrunk.is_none() {
            return new_window;
        }
        window = payload_shrunk.unwrap_or(new_window);
        tracing::info!(window = window.len(), "shrink iteration");
    }
}

/// Shrinks the payload of the first op whose payload is reducible, returning
/// `None` when no payload could be reduced. Op-level `ddmin` can only delete a
/// whole op, so an op the failure needs keeps whatever payload it was recorded
/// with unless it is shrunk here.
fn shrink_op_payloads(
    window: &[RecordedOp],
    mut reproduces: impl FnMut(&[RecordedOp]) -> bool,
) -> Option<Vec<RecordedOp>> {
    for i in 0..window.len() {
        let Some(tree_ops) = sync_tree_ops(&window[i]) else {
            continue;
        };
        let reduced_tree = ddmin(tree_ops.clone(), |t| {
            let mut candidate = window.to_vec();
            set_sync_tree_ops(&mut candidate[i], t.to_vec());
            reproduces(&candidate)
        });
        if reduced_tree.len() < tree_ops.len() {
            let mut shrunk = window.to_vec();
            set_sync_tree_ops(&mut shrunk[i], reduced_tree);
            return Some(shrunk);
        }
    }
    None
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

    fn master_hub() -> Hub {
        TestHubBuilder::new()
            .with_layout(initial_layout(SmokeStrategy::Master))
            .build()
    }

    /// `resolve_monitor` maps `usize::MAX` to the primary, and
    /// `Hub::remove_monitor` asserts the removed monitor is not the primary, so
    /// this fires on every replay. That gives the reducer tests a panic that does
    /// not depend on a real bug.
    fn panicking_op() -> RecordedOp {
        RecordedOp::RemoveMonitor {
            monitor: RecordedMonitor(usize::MAX),
        }
    }

    fn padded_with(op: RecordedOp) -> Vec<RecordedOp> {
        vec![
            RecordedOp::QueryWorkspaces,
            RecordedOp::FocusParent,
            RecordedOp::ToggleSpawnMode,
            op,
            RecordedOp::QueryWorkspaces,
            RecordedOp::FocusParent,
            RecordedOp::ToggleSpawnMode,
        ]
    }

    #[test]
    fn shrink_reduces_window_ops_to_the_panicking_op() {
        let ops = padded_with(panicking_op());
        let signature = replay(&ops, master_hub).expect("fixture op must panic");

        let reduced = shrink_ops(ops, &signature, master_hub);

        assert_eq!(reduced.len(), 1);
        assert!(matches!(reduced[0], RecordedOp::RemoveMonitor { .. }));
    }

    #[test]
    fn shrink_reduces_a_sync_preferred_layout_payload() {
        let window = vec![RecordedOp::SyncPreferredLayout {
            workspace_name: "1".into(),
            strategy: Strategy::PartitionTree,
            tree_ops: vec![
                PrefTreeBuildOp::InsertLeaf {
                    leaf_id: 0,
                    title: pref_title(0),
                    anchor: None,
                    split: SplitMode::Horizontal,
                },
                PrefTreeBuildOp::InsertLeaf {
                    leaf_id: 1,
                    title: pref_title(1),
                    anchor: Some(0),
                    split: SplitMode::Vertical,
                },
                PrefTreeBuildOp::InsertLeaf {
                    leaf_id: 2,
                    title: pref_title(2),
                    anchor: Some(1),
                    split: SplitMode::Tabbed,
                },
            ],
            master: Vec::new(),
            secondary: Vec::new(),
            float: Vec::new(),
            fullscreen: Vec::new(),
        }];

        let reduced = shrink_op_payloads(&window, |candidate| {
            candidate
                .iter()
                .any(|op| matches!(op, RecordedOp::SyncPreferredLayout { .. }))
        })
        .expect("a three-leaf payload must shrink");

        // The predicate ignores payload size, so every leaf is individually
        // removable. `ddmin` still bottoms out at one element: granularity floors
        // at 2, so its `granularity > ops.len()` guard exits once a single element
        // is left.
        assert_eq!(
            sync_tree_ops(&reduced[0]).expect("still a sync op").len(),
            1
        );
    }
}
