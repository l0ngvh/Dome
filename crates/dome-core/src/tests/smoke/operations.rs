use super::*;

pub(super) const DEFAULT_TILING_TITLE: &str = "w1";
pub(super) const FULLSCREEN_TITLE: &str = "w3";
pub(super) const CONTAINER_BASE: usize = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum SmokeStrategy {
    PartitionTree,
    Master,
    Scrolling,
}

impl SmokeStrategy {
    pub(super) fn all() -> &'static [SmokeStrategy] {
        &[
            SmokeStrategy::PartitionTree,
            SmokeStrategy::Master,
            SmokeStrategy::Scrolling,
        ]
    }

    pub(super) fn test_name(self) -> &'static str {
        match self {
            SmokeStrategy::PartitionTree => "partition-tree",
            SmokeStrategy::Master => "master",
            SmokeStrategy::Scrolling => "scrolling",
        }
    }

    pub(super) fn build_dome(self) -> TestDome {
        TestDome::with_primary_work_area(
            &tiling_source(&initial_tiling(self)),
            PixelRect::new(0, 0, 150, 30),
        )
    }
}

/// A matcher on `DEFAULT_TILING_TITLE` reroutes inserts with that title away from
/// tiling. Keep these matchers rare to preserve ordinary tiling coverage.
pub(super) const DEFAULT_TITLE_MATCHER_PROBABILITY: f64 = 0.1;
pub(super) const PREFERRED_TITLE_FLOAT_PROBABILITY: f64 = 0.15;
pub(super) const PREFERRED_TITLE_FULLSCREEN_PROBABILITY: f64 = 0.1;

/// Inserts and generated matchers share this title pool so each matcher has an
/// eligible insert title. Sized to cover every identifier the tree and pane
/// generators can produce.
pub(super) const PREF_TITLE_POOL_SIZE: usize = PREF_TREE_MAX_LEAVES;

pub(super) fn pref_title(index: usize) -> String {
    format!("pref-{index}")
}

pub(super) fn pref_title_pool() -> Vec<String> {
    (0..PREF_TITLE_POOL_SIZE).map(pref_title).collect()
}

pub(super) fn initial_tiling(strategy: SmokeStrategy) -> TilingConfig {
    TilingConfig {
        layout: match strategy {
            SmokeStrategy::PartitionTree => Strategy::PartitionTree,
            SmokeStrategy::Master => Strategy::Master,
            SmokeStrategy::Scrolling => Strategy::Scrolling,
        },
        border_size: Length::new(1),
        partition_tree: PartitionTreeConfig {
            tab_bar_height: Length::new(2),
            automatic_tiling: false,
        },
        master: MasterConfig {
            master_ratio: 0.5,
            master_count: 1,
        },
        scrolling: ScrollingConfig {
            column_width: SizeConstraint::Percent(50.0),
        },
        size_constraints: SizeConstraints {
            minimum_width: SizeConstraint::Pixels(Length::new(1)),
            minimum_height: SizeConstraint::Pixels(Length::new(1)),
            maximum_width: SizeConstraint::Pixels(Length::new(0)),
            maximum_height: SizeConstraint::Pixels(Length::new(0)),
        },
        ignore: Vec::new(),
    }
}

/// Titles are drawn only from what the harness actually inserts, since a matcher
/// on anything else is inert. A title can land in both lists, so one window can
/// match a fullscreen slot and a float slot of the same workspace.
pub(super) fn generate_matcher_titles(
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

pub(super) fn strategy_for_seed(seed: u64) -> SmokeStrategy {
    let all = SmokeStrategy::all();
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    all[rng.random_range(0..all.len())]
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum OpKind {
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
    Grow,
    Shrink,
    IncrementMasterCount,
    DecrementMasterCount,
    QueryWorkspaces,
    MinimizeWindow,
    UnminimizeWindow,
    ConfigReload,
    ApplyPreferredLayouts,
    ExportLayout,
}

pub(super) const ALL_OP_KINDS: &[OpKind] = &[
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
    OpKind::Grow,
    OpKind::Shrink,
    OpKind::IncrementMasterCount,
    OpKind::DecrementMasterCount,
    OpKind::QueryWorkspaces,
    OpKind::MinimizeWindow,
    OpKind::UnminimizeWindow,
    OpKind::ConfigReload,
    OpKind::ApplyPreferredLayouts,
    OpKind::ExportLayout,
];

#[derive(Debug, Clone)]
pub(super) enum RecordedOp {
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
        scale: f64,
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
    Grow,
    Shrink,
    IncrementMasterCount,
    DecrementMasterCount,
    QueryWorkspaces,
    ConfigReload {
        tiling: TilingConfig,
    },
    ApplyPreferredLayouts {
        workspace_name: String,
        strategy: Strategy,
        tree_ops: Vec<PrefTreeBuildOp>,
        master: Vec<String>,
        secondary: Vec<String>,
        /// Each column lists the titles of its matchers, top to bottom.
        columns: Vec<Vec<String>>,
        float: Vec<String>,
        fullscreen: Vec<String>,
    },
    ExportLayout,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct RecordedWindow(pub(super) usize);

#[derive(Debug, Clone, Copy)]
pub(super) struct RecordedMonitor(pub(super) usize);

pub(super) fn pick_non_minimized(rng: &mut ChaCha8Rng, windows: &[LiveWindow]) -> Option<usize> {
    let eligible: Vec<usize> = (0..windows.len())
        .filter(|&i| !windows[i].minimized)
        .collect();
    if eligible.is_empty() {
        return None;
    }
    Some(eligible[rng.random_range(0..eligible.len())])
}

pub(super) fn build_op(
    kind: OpKind,
    rng: &mut ChaCha8Rng,
    windows: &[LiveWindow],
    monitors: &[LiveMonitor],
    next_op_index: usize,
    current_tiling: &mut TilingConfig,
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
                window: RecordedWindow(windows[idx].producer_id),
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
            let idx = pick_non_minimized(rng, windows)?;
            let restrictions = match rng.random_range(0..3u8) {
                0 => WindowRestrictions::None,
                1 => WindowRestrictions::BlockAll,
                _ => WindowRestrictions::ProtectFullscreen,
            };
            Some(RecordedOp::SetFullscreen {
                window: RecordedWindow(windows[idx].producer_id),
                restrictions,
            })
        }
        OpKind::UnsetFullscreen => {
            let idx = pick_non_minimized(rng, windows)?;
            Some(RecordedOp::UnsetFullscreen {
                window: RecordedWindow(windows[idx].producer_id),
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
            if monitors.len() >= MAX_MONITORS {
                return None;
            }
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
                monitor: RecordedMonitor(monitors[idx].producer_id),
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
            let idx = pick_non_minimized(rng, windows)?;
            Some(RecordedOp::SetFocus {
                window: RecordedWindow(windows[idx].producer_id),
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
                _ => LimitUpdate::Set(FractionalLength::new(rng.random_range(1.0f64..50.0))),
            };
            let min_h = match rng.random_range(0..3) {
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(FractionalLength::new(rng.random_range(1.0f64..10.0))),
            };
            let max_w = match rng.random_range(0..3) {
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(FractionalLength::new(rng.random_range(1.0f64..100.0))),
            };
            let max_h = match rng.random_range(0..3) {
                0 => LimitUpdate::Unchanged,
                1 => LimitUpdate::Cleared,
                _ => LimitUpdate::Set(FractionalLength::new(rng.random_range(1.0f64..20.0))),
            };
            Some(RecordedOp::SetWindowConstraint {
                window: RecordedWindow(windows[idx].producer_id),
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
                window: RecordedWindow(windows[idx].producer_id),
                title,
            })
        }
        OpKind::Grow => Some(RecordedOp::Grow),
        OpKind::Shrink => Some(RecordedOp::Shrink),
        OpKind::IncrementMasterCount => Some(RecordedOp::IncrementMasterCount),
        OpKind::DecrementMasterCount => Some(RecordedOp::DecrementMasterCount),
        OpKind::QueryWorkspaces => Some(RecordedOp::QueryWorkspaces),
        OpKind::MinimizeWindow => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            Some(RecordedOp::MinimizeWindow {
                window: RecordedWindow(windows[idx].producer_id),
            })
        }
        OpKind::UnminimizeWindow => {
            if windows.is_empty() {
                return None;
            }
            let idx = rng.random_range(0..windows.len());
            Some(RecordedOp::UnminimizeWindow {
                window: RecordedWindow(windows[idx].producer_id),
            })
        }
        OpKind::ConfigReload => {
            let mut tiling = current_tiling.clone();
            match rng.random_range(0..6u8) {
                0 => {
                    tiling.partition_tree.automatic_tiling =
                        !tiling.partition_tree.automatic_tiling;
                }
                1 => {
                    let h = rng.random_range(10u32..50);
                    tiling.partition_tree.tab_bar_height = Length::new(h);
                }
                2 => {
                    tiling.master.master_ratio = rng.random_range(0.2f64..0.8);
                }
                3 => {
                    tiling.master.master_count = rng.random_range(1..=4);
                }
                4 => {
                    let v = rng.random_range(10..200);
                    tiling.size_constraints.minimum_width = SizeConstraint::Pixels(Length::new(v));
                }
                _ => {
                    tiling.layout = match tiling.layout {
                        Strategy::PartitionTree => Strategy::Master,
                        Strategy::Master => Strategy::Scrolling,
                        Strategy::Scrolling => Strategy::PartitionTree,
                    };
                }
            }
            Some(RecordedOp::ConfigReload { tiling })
        }
        OpKind::ApplyPreferredLayouts => {
            if workspace_names.is_empty() {
                return None;
            }
            let workspace_name =
                workspace_names[rng.random_range(0..workspace_names.len())].clone();
            // Drawn independently of `current_tiling.layout`, so the named workspace often runs
            // another strategy than the rest, and a move to another workspace or monitor then
            // crosses between strategies.
            let strategy = match rng.random_range(0..3u8) {
                0 => Strategy::PartitionTree,
                1 => Strategy::Master,
                _ => Strategy::Scrolling,
            };
            let (float, fullscreen) = generate_matcher_titles(rng, &pref_title_pool());
            let mut tree_ops = Vec::new();
            let mut master = Vec::new();
            let mut secondary = Vec::new();
            let mut columns: Vec<Vec<String>> = Vec::new();
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
                Strategy::Scrolling => {
                    for title in pref_title_pool() {
                        match (rng.random_range(0..4u8), columns.last_mut()) {
                            (0, _) | (1, None) => columns.push(vec![title]),
                            (1, Some(column)) => column.push(title),
                            _ => {}
                        }
                    }
                }
            }
            Some(RecordedOp::ApplyPreferredLayouts {
                workspace_name,
                strategy,
                tree_ops,
                master,
                secondary,
                columns,
                float,
                fullscreen,
            })
        }
        OpKind::ExportLayout => Some(RecordedOp::ExportLayout),
    }
}
