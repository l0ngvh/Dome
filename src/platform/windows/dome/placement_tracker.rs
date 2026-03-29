use std::collections::HashMap;

use crate::platform::windows::external::HwndId;

enum MoveKind {
    UserDrag,
    Programmatic,
}

/// Tracks which windows are currently being moved, either by user drag or
/// programmatic repositioning. Pure state — no timers or time awareness.
/// The run loop is responsible for scheduling debounce/timeout timers and
/// calling `Dome::placement_timeout` when they fire.
pub(super) struct PlacementTracker {
    windows: HashMap<HwndId, MoveKind>,
}

impl PlacementTracker {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
        }
    }

    /// Mark a window as being dragged by the user.
    pub(super) fn drag_started(&mut self, id: HwndId) {
        self.windows.insert(id, MoveKind::UserDrag);
    }

    /// Remove a window from the moving set after a drag ends.
    pub(super) fn drag_ended(&mut self, id: HwndId) {
        self.windows.remove(&id);
    }

    /// Record a programmatic move for a window. Returns `true` if a new
    /// debounce timer should be scheduled. Returns `false` if the window
    /// is being dragged by the user (no debounce during drag).
    pub(super) fn location_changed(&mut self, id: HwndId) -> bool {
        if matches!(self.windows.get(&id), Some(MoveKind::UserDrag)) {
            return false;
        }
        self.windows.insert(id, MoveKind::Programmatic);
        true
    }

    /// Remove a window from the moving set. Called by `Dome::placement_timeout`
    /// when a timer fires.
    pub(super) fn clear(&mut self, id: HwndId) {
        self.windows.remove(&id);
    }

    /// Returns true if the window is currently being moved (drag or
    /// programmatic). Used by `apply_layout` to skip repositioning
    /// windows mid-move.
    pub(super) fn is_moving(&self, id: HwndId) -> bool {
        self.windows.contains_key(&id)
    }
}
