use super::{setup, snapshot_text, validate_hub};
use crate::core::node::{Dimension, FloatWindowId, WindowId};
use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;

#[derive(Debug, Clone, Copy)]
enum Op {
    InsertTiling,
    DeleteWindow,
    InsertFloat,
    DeleteFloat,
    FocusLeft,
    FocusRight,
    FocusUp,
    FocusDown,
    MoveLeft,
    MoveRight,
    MoveUp,
    MoveDown,
    ToggleSpawnDirection,
    ToggleDirection,
    FocusParent,
    ToggleContainerLayout,
    FocusNextTab,
    FocusPrevTab,
    ToggleFloat,
    MoveToWorkspace,
    FocusWorkspace,
    SetFocus,
    SetFloatFocus,
}

const ALL_OPS: &[Op] = &[
    Op::InsertTiling,
    Op::DeleteWindow,
    Op::InsertFloat,
    Op::DeleteFloat,
    Op::FocusLeft,
    Op::FocusRight,
    Op::FocusUp,
    Op::FocusDown,
    Op::MoveLeft,
    Op::MoveRight,
    Op::MoveUp,
    Op::MoveDown,
    Op::ToggleSpawnDirection,
    Op::ToggleDirection,
    Op::FocusParent,
    Op::ToggleContainerLayout,
    Op::FocusNextTab,
    Op::FocusPrevTab,
    Op::ToggleFloat,
    Op::MoveToWorkspace,
    Op::FocusWorkspace,
    Op::SetFocus,
    Op::SetFloatFocus,
];

fn run_smoke_iteration(rng: &mut ChaCha8Rng, ops_per_run: usize) {
    let mut hub = setup();
    let mut windows: Vec<WindowId> = Vec::new();
    let mut floats: Vec<FloatWindowId> = Vec::new();
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
                Op::DeleteWindow => {
                    if windows.is_empty() {
                        continue;
                    }
                    let idx = rng.random_range(0..windows.len());
                    let id = windows.remove(idx);
                    hub.delete_window(id);
                    format!("DeleteWindow({id})")
                }
                Op::InsertFloat => {
                    let dim = Dimension {
                        x: rng.random_range(0.0..100.0),
                        y: rng.random_range(0.0..20.0),
                        width: rng.random_range(10.0..50.0),
                        height: rng.random_range(5.0..15.0),
                    };
                    let id = hub.insert_float(dim);
                    floats.push(id);
                    format!("InsertFloat -> {id}")
                }
                Op::DeleteFloat => {
                    if floats.is_empty() {
                        continue;
                    }
                    let idx = rng.random_range(0..floats.len());
                    let id = floats.remove(idx);
                    hub.delete_float(id);
                    format!("DeleteFloat({id})")
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
                Op::ToggleSpawnDirection => {
                    hub.toggle_spawn_direction();
                    "ToggleSpawnDirection".into()
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
                    if let Some((win_id, float_id)) = hub.toggle_float() {
                        if windows.contains(&win_id) {
                            // Tiling -> Float
                            windows.retain(|&w| w != win_id);
                            floats.push(float_id);
                            format!("ToggleFloat({win_id} -> {float_id})")
                        } else {
                            // Float -> Tiling
                            floats.retain(|&f| f != float_id);
                            windows.push(win_id);
                            format!("ToggleFloat({float_id} -> {win_id})")
                        }
                    } else {
                        continue;
                    }
                }
                Op::MoveToWorkspace => {
                    let ws = rng.random_range(0..5);
                    hub.move_focused_to_workspace(ws);
                    format!("MoveToWorkspace({ws})")
                }
                Op::FocusWorkspace => {
                    let ws = rng.random_range(0..5);
                    hub.focus_workspace(ws);
                    format!("FocusWorkspace({ws})")
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
                Op::SetFloatFocus => {
                    if floats.is_empty() {
                        continue;
                    }
                    let idx = rng.random_range(0..floats.len());
                    let id = floats[idx];
                    hub.set_float_focus(id);
                    format!("SetFloatFocus({id})")
                }
            };

            history.push(op_str);

            validate_hub(&hub);
        }

        // Exhaust all windows and floats to ensure none are in a dangling state
        for id in floats.drain(..) {
            history.push(format!("Cleanup: DeleteFloat({id})"));
            hub.delete_float(id);
            validate_hub(&hub);
        }
        for id in windows.drain(..) {
            history.push(format!("Cleanup: DeleteWindow({id})"));
            hub.delete_window(id);
            validate_hub(&hub);
        }
    }));

    if let Err(e) = result {
        eprintln!("=== SMOKE TEST FAILURE ===");
        eprintln!("Operations:");
        for (i, op) in history.iter().enumerate() {
            eprintln!("  {i}: {op}");
        }
        eprintln!("\nHub state:\n{}", snapshot_text(&hub));
        std::panic::resume_unwind(e);
    }
}

#[test]
fn smoke_test() {
    let seed = 42u64;
    let runs = 100;
    let ops_per_run = 1000;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    for run in 0..runs {
        run_smoke_iteration(&mut rng, ops_per_run);
        if run % 10 == 0 {
            eprintln!("Completed run {run}/{runs}");
        }
    }
}
