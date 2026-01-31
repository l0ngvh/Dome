use super::{setup_hub, setup_logger_with_level, snapshot_text, validate_hub};
use crate::action::MonitorTarget;
use crate::core::node::{Dimension, MonitorId, WindowId};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use rayon::prelude::*;

#[derive(Debug, Clone, Copy)]
enum Op {
    InsertTiling,
    InsertFloat,
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
    MoveToWorkspace,
    FocusWorkspace,
    AddMonitor,
    RemoveMonitor,
    FocusMonitor,
    MoveToMonitor,
    SetFocus,
    SetWindowConstraint,
    // Note: Exec is not included because it's a platform-specific action
    // that spawns external processes, not a core hub operation.
}

const ALL_OPS: &[Op] = &[
    Op::InsertTiling,
    Op::InsertFloat,
    Op::DeleteWindow,
    Op::FocusLeft,
    Op::FocusRight,
    Op::FocusUp,
    Op::FocusDown,
    Op::MoveLeft,
    Op::MoveRight,
    Op::MoveUp,
    Op::MoveDown,
    Op::ToggleSpawnMode,
    Op::ToggleDirection,
    Op::FocusParent,
    Op::ToggleContainerLayout,
    Op::FocusNextTab,
    Op::FocusPrevTab,
    Op::ToggleFloat,
    Op::MoveToWorkspace,
    Op::FocusWorkspace,
    Op::AddMonitor,
    Op::RemoveMonitor,
    Op::FocusMonitor,
    Op::MoveToMonitor,
    Op::SetFocus,
    Op::SetWindowConstraint,
];

fn run_smoke_iteration(seed: u64, ops_per_run: usize) {
    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let mut hub = setup_hub();
    let mut windows: Vec<WindowId> = Vec::new();
    let mut monitors: Vec<MonitorId> = vec![hub.focused_monitor()];
    let mut history: Vec<String> = Vec::new();

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        for _ in 0..ops_per_run {
            let op = ALL_OPS[rng.random_range(0..ALL_OPS.len())];

            let op_str = match op {
                Op::InsertTiling => {
                    let id = hub.insert_tiling();
                    windows.push(id);
                    format!("InsertTiling -> {id}")
                }
                Op::InsertFloat => {
                    let dim = Dimension {
                        x: rng.random_range(0.0..100.0),
                        y: rng.random_range(0.0..20.0),
                        width: rng.random_range(10.0..50.0),
                        height: rng.random_range(5.0..15.0),
                    };
                    let id = hub.insert_float(dim);
                    windows.push(id);
                    format!("InsertFloat -> {id}")
                }
                Op::DeleteWindow => {
                    if windows.is_empty() {
                        continue;
                    }
                    let idx = rng.random_range(0..windows.len());
                    let id = windows.remove(idx);
                    hub.delete_window(id);
                    format!("DeleteWindow({id})")
                }
                Op::FocusLeft => {
                    hub.focus_left();
                    "FocusLeft".into()
                }
                Op::FocusRight => {
                    hub.focus_right();
                    "FocusRight".into()
                }
                Op::FocusUp => {
                    hub.focus_up();
                    "FocusUp".into()
                }
                Op::FocusDown => {
                    hub.focus_down();
                    "FocusDown".into()
                }
                Op::MoveLeft => {
                    hub.move_left();
                    "MoveLeft".into()
                }
                Op::MoveRight => {
                    hub.move_right();
                    "MoveRight".into()
                }
                Op::MoveUp => {
                    hub.move_up();
                    "MoveUp".into()
                }
                Op::MoveDown => {
                    hub.move_down();
                    "MoveDown".into()
                }
                Op::ToggleSpawnMode => {
                    hub.toggle_spawn_mode();
                    "ToggleSpawnMode".into()
                }
                Op::ToggleDirection => {
                    hub.toggle_direction();
                    "ToggleDirection".into()
                }
                Op::FocusParent => {
                    hub.focus_parent();
                    "FocusParent".into()
                }
                Op::ToggleContainerLayout => {
                    hub.toggle_container_layout();
                    "ToggleContainerLayout".into()
                }
                Op::FocusNextTab => {
                    hub.focus_next_tab();
                    "FocusNextTab".into()
                }
                Op::FocusPrevTab => {
                    hub.focus_prev_tab();
                    "FocusPrevTab".into()
                }
                Op::ToggleFloat => {
                    hub.toggle_float();
                    "ToggleFloat".into()
                }
                Op::MoveToWorkspace => {
                    let ws = rng.random_range(0..5);
                    hub.move_focused_to_workspace(&ws.to_string());
                    format!("MoveToWorkspace({ws})")
                }
                Op::FocusWorkspace => {
                    let ws = rng.random_range(0..5);
                    hub.focus_workspace(&ws.to_string());
                    format!("FocusWorkspace({ws})")
                }
                Op::AddMonitor => {
                    let x = monitors.len() as f32 * 150.0;
                    let id = hub.add_monitor(
                        format!("monitor-{}", monitors.len()),
                        Dimension {
                            x,
                            y: 0.0,
                            width: 150.0,
                            height: 30.0,
                        },
                    );
                    monitors.push(id);
                    format!("AddMonitor({id})")
                }
                Op::RemoveMonitor => {
                    if monitors.len() <= 1 {
                        continue;
                    }
                    let idx = rng.random_range(0..monitors.len());
                    let id = monitors.remove(idx);
                    let fallback = monitors[0];
                    hub.remove_monitor(id, fallback);
                    format!("RemoveMonitor({id})")
                }
                Op::FocusMonitor => {
                    let targets = [
                        MonitorTarget::Up,
                        MonitorTarget::Down,
                        MonitorTarget::Left,
                        MonitorTarget::Right,
                    ];
                    let target = &targets[rng.random_range(0..targets.len())];
                    hub.focus_monitor(target);
                    format!("FocusMonitor({target:?})")
                }
                Op::MoveToMonitor => {
                    let targets = [
                        MonitorTarget::Up,
                        MonitorTarget::Down,
                        MonitorTarget::Left,
                        MonitorTarget::Right,
                    ];
                    let target = &targets[rng.random_range(0..targets.len())];
                    hub.move_focused_to_monitor(target);
                    format!("MoveToMonitor({target:?})")
                }
                Op::SetFocus => {
                    if windows.is_empty() {
                        continue;
                    }
                    let idx = rng.random_range(0..windows.len());
                    let id = windows[idx];
                    hub.set_focus(id);
                    format!("SetFocus({id})")
                }
                Op::SetWindowConstraint => {
                    if windows.is_empty() {
                        continue;
                    }
                    let idx = rng.random_range(0..windows.len());
                    let id = windows[idx];
                    let mut rand_or_clear = |lo: f32, hi: f32| -> Option<f32> {
                        match rng.random_range(0..3) {
                            0 => None,
                            1 => Some(0.0),
                            _ => Some(rng.random_range(lo..hi)),
                        }
                    };
                    let min_w = rand_or_clear(1.0, 50.0);
                    let min_h = rand_or_clear(1.0, 10.0);
                    let max_w = rand_or_clear(1.0, 100.0);
                    let max_h = rand_or_clear(1.0, 20.0);
                    hub.set_window_constraint(id, min_w, min_h, max_w, max_h);
                    format!(
                        "SetWindowConstraint({id}, min=({min_w:?}, {min_h:?}), max=({max_w:?}, {max_h:?}))"
                    )
                }
            };

            history.push(op_str);

            validate_hub(&hub);
        }

        // Exhaust all windows to ensure none are in a dangling state
        for id in windows.drain(..) {
            history.push(format!("Cleanup: DeleteWindow({id})"));
            hub.delete_window(id);
            validate_hub(&hub);
        }
    }));

    if let Err(e) = result {
        tracing::error!("=== SMOKE TEST FAILURE ===");
        tracing::error!("Seed: {seed}");
        tracing::error!("Operations:");
        for (i, op) in history.iter().enumerate() {
            tracing::error!("  {i}: {op}");
        }
        tracing::error!("\nHub state:\n{}", snapshot_text(&hub));
        std::panic::resume_unwind(e);
    }
}

#[test]
fn smoke_test() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    setup_logger_with_level("info");

    let seed = 42u64;
    let runs = 200;
    let ops_per_run = 10000;
    let completed = AtomicUsize::new(0);

    (0..runs).into_par_iter().for_each(|run| {
        run_smoke_iteration(seed.wrapping_add(run as u64), ops_per_run);
        let done = completed.fetch_add(1, Ordering::Relaxed) + 1;
        if done % 10 == 0 {
            tracing::info!("Completed {done}/{runs}");
        }
    });
}

#[test]
#[ignore]
fn reproduce_smoke_failure() {
    setup_logger_with_level("info");
    let seed = 0; // paste failing seed here
    run_smoke_iteration(seed, 10000);
}
