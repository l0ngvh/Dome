use super::*;

pub(super) fn run_smoke_iteration(
    seed: u64,
    ops_per_run: usize,
    strategy: SmokeStrategy,
    abort: &AtomicBool,
) {
    if abort.load(Ordering::Relaxed) {
        return;
    }
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let workload = Workload::for_seed(seed, ops_per_run);
    let mut dome = strategy.build_dome();
    let mut current_tiling = initial_tiling(strategy);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_iteration(
            &mut dome,
            abort,
            |_| {},
            &mut rng,
            &workload,
            &mut current_tiling,
        );
    }));

    if let Err(e) = result {
        abort.store(true, Ordering::Relaxed);
        let name = strategy.test_name();
        tracing::error!("Workload: {workload}");
        tracing::error!("Strategy: {name}");
        tracing::error!(
            "To reproduce: DOME_SMOKE_SEED={seed} cargo test -p dome-core --lib \
             reproduce_smoke_failure -- --ignored --nocapture",
        );
        tracing::error!(
            "To reduce:    DOME_SMOKE_SEED={seed} cargo test -p dome-core --lib \
             reduce_smoke_failure -- --ignored --nocapture",
        );
        std::panic::resume_unwind(e);
    }
}

pub(super) fn run_iteration<F>(
    dome: &mut TestDome,
    abort: &AtomicBool,
    mut observer: F,
    rng: &mut ChaCha8Rng,
    workload: &Workload,
    current_tiling: &mut TilingConfig,
) where
    F: FnMut(&RecordedOp),
{
    let mut windows: Vec<LiveWindow> = Vec::new();
    let mut monitors = vec![LiveMonitor {
        id: dome.primary_monitor(),
        producer_id: usize::MAX,
    }];
    let mut next_op_index: usize = 0;
    let mut workspace_names: Vec<String> = vec!["0".to_string()];

    for pattern in workload.op_patterns() {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let kind = workload.draw_kind(pattern, rng);
        let Some(op) = build_op(
            kind,
            rng,
            &windows,
            &monitors,
            next_op_index,
            current_tiling,
            &workspace_names,
        ) else {
            continue;
        };
        observer(&op);
        assert!(
            apply_op(dome, &op, &mut windows, &mut monitors,),
            "generated operation references a live producer"
        );
        if let RecordedOp::ConfigReload { tiling } = &op {
            *current_tiling = tiling.clone();
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
    }

    while !windows.is_empty() {
        if abort.load(Ordering::Relaxed) {
            return;
        }
        let window = windows.remove(0);
        let producer_id = window.producer_id;
        let op = RecordedOp::DeleteWindow {
            window: RecordedWindow(producer_id),
        };
        observer(&op);
        dome.close(window.id);
    }
}

pub(super) struct LiveWindow {
    pub(super) id: WindowId,
    pub(super) producer_id: usize,
    pub(super) minimized: bool,
}

pub(super) struct LiveMonitor {
    pub(super) id: MonitorId,
    pub(super) producer_id: usize,
}

pub(super) fn apply_op(
    dome: &mut TestDome,
    op: &RecordedOp,
    windows: &mut Vec<LiveWindow>,
    monitors: &mut Vec<LiveMonitor>,
) -> bool {
    match op {
        RecordedOp::InsertTiling { producer_id, title } => {
            let window_title = title.as_deref().unwrap_or(DEFAULT_TILING_TITLE);
            let id = dome.open(
                window(window_title)
                    .label(&producer_id.to_string())
                    .frame([0, 0, 100, 100]),
            );
            windows.push(LiveWindow {
                id,
                producer_id: *producer_id,
                minimized: false,
            });
        }
        RecordedOp::InsertFullscreen {
            producer_id,
            restrictions,
        } => {
            let id = dome.open(
                window(FULLSCREEN_TITLE)
                    .label(&producer_id.to_string())
                    .frame([0, 0, 100, 100])
                    .restricted(*restrictions),
            );
            windows.push(LiveWindow {
                id,
                producer_id: *producer_id,
                minimized: false,
            });
        }
        RecordedOp::AddMonitor {
            producer_id,
            name,
            rect,
            scale,
        } => {
            let id = dome.add_monitor(
                monitor(
                    name,
                    [
                        rect.x().value(),
                        rect.y().value(),
                        rect.width().value().try_into().unwrap(),
                        rect.height().value().try_into().unwrap(),
                    ],
                )
                .scale(*scale),
            );
            monitors.push(LiveMonitor {
                id,
                producer_id: *producer_id,
            });
        }
        RecordedOp::DeleteWindow { window } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            let id = windows.remove(pos).id;
            dome.close(id);
        }
        RecordedOp::RemoveMonitor { monitor } => {
            let Some(pos) = monitors
                .iter()
                .position(|entry| entry.producer_id == monitor.0)
            else {
                return false;
            };
            let id = monitors.remove(pos).id;
            dome.remove_monitor(id);
        }
        RecordedOp::SetFullscreen {
            window,
            restrictions,
        } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            dome.set_fullscreen(windows[pos].id, *restrictions);
        }
        RecordedOp::UnsetFullscreen { window } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            dome.unset_fullscreen(windows[pos].id);
        }
        RecordedOp::SetFocus { window } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            dome.focus(windows[pos].id);
        }
        RecordedOp::SetWindowConstraint {
            window,
            min_w,
            min_h,
            max_w,
            max_h,
        } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            dome.set_constraint(
                windows[pos].id,
                LimitObservation {
                    min_width: *min_w,
                    min_height: *min_h,
                    max_width: *max_w,
                    max_height: *max_h,
                },
            );
        }
        RecordedOp::SetWindowTitle { window, title } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            dome.set_title(windows[pos].id, title);
        }
        RecordedOp::MinimizeWindow { window } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            windows[pos].minimized = true;
            dome.minimize(windows[pos].id);
        }
        RecordedOp::UnminimizeWindow { window } => {
            let Some(pos) = windows
                .iter()
                .position(|entry| entry.producer_id == window.0)
            else {
                return false;
            };
            windows[pos].minimized = false;
            dome.unminimize(windows[pos].id);
        }
        RecordedOp::MoveToWorkspace { name } => {
            dome.run_action(Action::Move {
                target: MoveTarget::Workspace {
                    name: name.clone(),
                    monitor: None,
                },
            });
        }
        RecordedOp::FocusWorkspace { name } => {
            dome.run_action(Action::Focus {
                target: FocusTarget::Workspace {
                    name: name.clone(),
                    monitor: None,
                },
            });
        }
        RecordedOp::FocusMonitor { target } => {
            dome.run_action(Action::Focus {
                target: FocusTarget::Monitor {
                    target: target.clone(),
                },
            });
        }
        RecordedOp::MoveToMonitor { target } => {
            dome.run_action(Action::Move {
                target: MoveTarget::Monitor {
                    target: target.clone(),
                },
            });
        }
        RecordedOp::FocusLeft => dome.run("focus left"),
        RecordedOp::FocusRight => dome.run("focus right"),
        RecordedOp::FocusUp => dome.run("focus up"),
        RecordedOp::FocusDown => dome.run("focus down"),
        RecordedOp::MoveLeft => dome.run("move left"),
        RecordedOp::MoveRight => dome.run("move right"),
        RecordedOp::MoveUp => dome.run("move up"),
        RecordedOp::MoveDown => dome.run("move down"),
        RecordedOp::ToggleSpawnMode => dome.run("toggle spawn"),
        RecordedOp::ToggleDirection => dome.run("toggle direction"),
        RecordedOp::FocusParent => dome.run("focus parent"),
        RecordedOp::ToggleContainerLayout => dome.run("toggle layout"),
        RecordedOp::FocusNextTab => dome.run("focus tab next"),
        RecordedOp::FocusPrevTab => dome.run("focus tab prev"),
        RecordedOp::ToggleFloat => dome.run("toggle float"),
        RecordedOp::ToggleFullscreen => dome.run("toggle fullscreen"),
        RecordedOp::Grow => {
            dome.run("grow");
        }
        RecordedOp::Shrink => {
            dome.run("shrink");
        }
        RecordedOp::IncrementMasterCount => {
            dome.run("master more");
        }
        RecordedOp::DecrementMasterCount => {
            dome.run("master fewer");
        }
        RecordedOp::QueryWorkspaces => {
            dome.query_workspaces();
        }
        RecordedOp::ConfigReload { tiling } => {
            dome.reload(&tiling_source(tiling))
                .expect("recorded config should reload");
        }
        RecordedOp::ApplyPreferredLayouts { .. } => {
            let layouts = preferred_layout_everywhere(dome, op);
            dome.apply_preferred_layouts(layouts);
        }
        RecordedOp::ExportLayout => {
            export_then_apply(dome);
        }
    }
    true
}

/// Records the op sequence a seed generates, ignoring whether it panics.
pub(super) fn record_ops(
    seed: u64,
    ops_per_run: usize,
    strategy: SmokeStrategy,
) -> Vec<RecordedOp> {
    let abort = AtomicBool::new(false);
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let workload = Workload::for_seed(seed, ops_per_run);
    let mut dome = strategy.build_dome();
    let mut ops: Vec<RecordedOp> = Vec::new();
    let mut current_tiling = initial_tiling(strategy);
    let _ = capture_panic(|| {
        run_iteration(
            &mut dome,
            &abort,
            |op| ops.push(op.clone()),
            &mut rng,
            &workload,
            &mut current_tiling,
        );
    });
    ops
}

pub(super) fn replay_without_capture(ops: &[RecordedOp], make_dome: impl FnOnce() -> TestDome) {
    let mut dome = make_dome();
    let mut windows = Vec::new();
    let mut monitors = vec![LiveMonitor {
        id: dome.primary_monitor(),
        producer_id: usize::MAX,
    }];
    for op in ops {
        // Reduction can remove a producer while retaining its consumers.
        let _ = apply_op(&mut dome, op, &mut windows, &mut monitors);
    }
}
