//! Four managed windows that move at the same time while the focus moves
//! between them, all through AX writes.

use std::f64::consts::{FRAC_PI_2, PI, TAU};
use std::time::{Duration, Instant};

use spike::geometry::Rect;
use spike::now_ms;

use crate::dome::{Dome, Layout};
use crate::stage::{self, Owner};

pub const LABELS: [&str; 4] = ["a1", "a2", "b1", "b2"];
/// Alternates the two fixture apps, so each focus change also activates an app.
const FOCUS_ORDER: [&str; 4] = ["a1", "b1", "a2", "b2"];
const FOCUS_PERIOD: Duration = Duration::from_millis(150);
const JUMP_PERIOD: Duration = Duration::from_millis(300);
const WRITE_PERIOD: Duration = Duration::from_micros(16_667);
/// One lap of a smooth path. With `RADIUS`, a window moves about 180 to
/// 260 pt/s.
const LAP_SECS: f64 = 2.4;
const RADIUS: (f64, f64) = (100.0, 70.0);
/// The start angle and the direction of each window's lap, in `LABELS` order.
const PATHS: [(f64, f64); 4] = [(PI, 1.0), (FRAC_PI_2, 1.0), (0.0, -1.0), (3.0 * FRAC_PI_2, -1.0)];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Motion {
    /// A new layout every `JUMP_PERIOD`.
    Jumps,
    /// A position write per window every `WRITE_PERIOD`.
    Smooth,
}

impl Motion {
    pub const ALL: [Motion; 2] = [Motion::Jumps, Motion::Smooth];

    pub fn name(self) -> &'static str {
        match self {
            Motion::Jumps => "layout jumps",
            Motion::Smooth => "smooth moves",
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Motion::Jumps => "layout-jumps",
            Motion::Smooth => "smooth-moves",
        }
    }
}

/// Each window overlaps at least one other in both layouts, so the z-order
/// decides which ring parts are cut.
pub fn layouts() -> [Layout; 2] {
    [
        vec![
            ("a1", Rect::new(170.0, 140.0, 520.0, 360.0)),
            ("a2", Rect::new(470.0, 340.0, 520.0, 360.0)),
            ("b1", Rect::new(830.0, 160.0, 480.0, 340.0)),
            ("b2", Rect::new(660.0, 480.0, 520.0, 320.0)),
        ],
        vec![
            ("a1", Rect::new(720.0, 420.0, 540.0, 380.0)),
            ("a2", Rect::new(260.0, 470.0, 520.0, 400.0)),
            ("b1", Rect::new(200.0, 110.0, 520.0, 420.0)),
            ("b2", Rect::new(840.0, 120.0, 500.0, 360.0)),
        ],
    ]
}

/// The first layout, with a distinct content color per window so that the
/// per-frame check can tell the windows apart.
pub fn stage_layout() -> Vec<(Owner, &'static str, Rect, &'static str)> {
    let owners = [Owner::A, Owner::A, Owner::B, Owner::B];
    let colors = [stage::BLUE, stage::GREEN, stage::ORANGE, stage::CYAN];
    layouts()[0]
        .iter()
        .zip(owners)
        .zip(colors)
        .map(|(((label, rect), owner), color)| (owner, *label, *rect, color))
        .collect()
}

/// The top-left corner of window `i` of `LABELS` after `t` seconds. Each path
/// starts at the window's place in the first layout.
pub fn smooth_origin(i: usize, t: f64) -> (f64, f64) {
    let base = layouts()[0][i].1;
    let (start, direction) = PATHS[i];
    let angle = start + direction * t * TAU / LAP_SECS;
    (
        base.x + RADIUS.0 * (angle.cos() - start.cos()),
        base.y + RADIUS.1 * (angle.sin() - start.sin()),
    )
}

#[derive(Clone, Debug, Default)]
pub struct Outcome {
    pub writes: u64,
    pub failed_writes: u64,
    pub focuses: u64,
    pub failed_focuses: u64,
    /// When the last write returned, in Unix-epoch milliseconds.
    pub end_ms: u64,
}

fn due(t: Duration, period: Duration) -> u32 {
    (t.as_micros() / period.as_micros()) as u32
}

/// Moves the windows until `done` returns true for the time since the start.
/// A step that runs late skips the steps it missed rather than replay them.
pub fn run(motion: Motion, dome: &Dome, mut done: impl FnMut(Duration) -> bool) -> Outcome {
    let start = Instant::now();
    let mut outcome = Outcome::default();
    let mut next_focus = 0;
    let mut next_jump = 0;
    loop {
        let t = start.elapsed();
        if done(t) {
            break;
        }
        let wake = match motion {
            Motion::Jumps => {
                let jump = due(t, JUMP_PERIOD);
                if jump >= next_jump {
                    // The stage starts in the first layout, so the first jump
                    // goes to the second.
                    let layout = &layouts()[(jump as usize + 1) % 2];
                    let failed = dome.apply(layout);
                    outcome.writes += layout.len() as u64;
                    outcome.failed_writes += failed.len() as u64;
                    next_jump = jump + 1;
                }
                (JUMP_PERIOD * next_jump).min(FOCUS_PERIOD * (due(t, FOCUS_PERIOD) + 1))
            }
            Motion::Smooth => {
                for (i, label) in LABELS.iter().enumerate() {
                    let ok = dome.move_window(label, smooth_origin(i, t.as_secs_f64()));
                    outcome.writes += 1;
                    outcome.failed_writes += u64::from(!ok);
                }
                WRITE_PERIOD * (due(start.elapsed(), WRITE_PERIOD) + 1)
            }
        };
        let focus = due(t, FOCUS_PERIOD);
        if focus >= next_focus {
            let ok = dome.focus(FOCUS_ORDER[focus as usize % FOCUS_ORDER.len()]);
            outcome.focuses += 1;
            outcome.failed_focuses += u64::from(!ok);
            next_focus = focus + 1;
        }
        outcome.end_ms = now_ms();
        if let Some(wait) = (start + wake).checked_duration_since(Instant::now()) {
            std::thread::sleep(wait);
        }
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_window_overlaps_another_in_both_layouts() {
        for layout in layouts() {
            for (label, rect) in &layout {
                let overlaps = layout
                    .iter()
                    .any(|(other, r)| other != label && r.overlaps(rect));
                assert!(overlaps, "{label} overlaps no other window in {layout:?}");
            }
        }
    }

    #[test]
    fn smooth_paths_start_at_the_first_layout_and_stay_on_the_stage() {
        let first = &layouts()[0];
        for (i, (label, rect)) in first.iter().enumerate() {
            let (x, y) = smooth_origin(i, 0.0);
            assert!((x - rect.x).abs() < 1e-9 && (y - rect.y).abs() < 1e-9, "{label}");
            for step in 0..=240 {
                let (x, y) = smooth_origin(i, f64::from(step) * LAP_SECS / 240.0);
                assert!(x >= 10.0 && x + rect.w <= 1500.0, "{label} x {x}");
                assert!(y >= 45.0 && y + rect.h <= 970.0, "{label} y {y}");
            }
        }
    }
}
