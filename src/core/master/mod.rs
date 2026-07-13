use std::collections::{HashMap, HashSet};

use crate::config::{LayoutWorkspaceConfig, WindowMatcher};
use crate::core::GlobalLayoutConfig;
use crate::core::WindowMetadata;
use crate::core::hub::{HubAccess, TilingWindowPlacement};
use crate::core::node::{Child, Dimension, Direction, Length, WindowId, WorkspaceId};
use crate::core::strategy::{
    TilingAction, TilingPlacements, TilingStrategy, clip, distribute_space, translate,
};

/// XMonad-style tiling: a master area on the left and a stack on the right.
/// No containers, no tabs. Each pane scrolls vertically and independently when
/// per-window min heights push the pane's total content past the screen height.
/// Horizontal scroll does not exist in master.
#[derive(Debug)]
pub(crate) struct MasterStrategy {
    workspaces: HashMap<WorkspaceId, MasterState>,
    window_dimensions: HashMap<WindowId, Dimension>,
}
impl TilingStrategy for MasterStrategy {
    fn prepare_workspace(
        &mut self,
        ws_id: WorkspaceId,
        ws_name: &str,
        layout: &GlobalLayoutConfig,
        preferred_layouts: &[LayoutWorkspaceConfig],
    ) {
        let preferred_layout = preferred_layouts.iter().find_map(|w| match w {
            LayoutWorkspaceConfig::Master {
                name,
                master_ratio,
                master_count,
                master,
                secondary,
                ..
            } if *name == ws_name => Some((*master_ratio, *master_count, master, secondary)),
            _ => None,
        });
        let initial_master_count = preferred_layout
            .as_ref()
            .and_then(|(_, count, _, _)| *count)
            .unwrap_or(layout.master.master_count);
        let initial_master_ratio = preferred_layout
            .as_ref()
            .and_then(|(ratio, _, _, _)| *ratio)
            .unwrap_or(layout.master.master_ratio);
        self.workspaces.entry(ws_id).or_insert_with(|| MasterState {
            master: Vec::new(),
            stack: Vec::new(),
            matchers: build_matchers_from(preferred_layouts, ws_name),
            focus: None,
            master_y_offset: Length::ZERO,
            stack_y_offset: Length::ZERO,
            master_count: initial_master_count,
            master_ratio: initial_master_ratio,
        });
    }

    fn attach_window(&mut self, hub: &mut HubAccess, id: WindowId, ws_id: WorkspaceId) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        let state = self.workspaces.get_mut(&ws_id).unwrap_or_else(|| {
            panic!("MasterStrategy: attach_window called for unprepared workspace {ws_id}")
        });
        let metadata = hub.windows.get(id).metadata.as_ref();
        let (pane, ins, tag) = Self::place(state, metadata);
        match pane {
            Pane::Master => state.master.insert(ins, (id, tag)),
            Pane::Secondary => state.stack.insert(ins, (id, tag)),
        }
        state.focus = Some(Focus { pane, index: ins });
        // Zero placeholder -- layout_workspace computes the real rect.
        self.window_dimensions.insert(id, Dimension::default());
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.layout_workspace(hub, ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, id: WindowId) -> Dimension {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;

        let state = self.workspaces.get_mut(&ws_id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but workspace {ws_id} has no state")
        });

        let (pane, idx) = state
            .master
            .iter()
            .position(|&(w, _)| w == id)
            .map(|i| (Pane::Master, i))
            .or_else(|| {
                state
                    .stack
                    .iter()
                    .position(|&(w, _)| w == id)
                    .map(|i| (Pane::Secondary, i))
            })
            .unwrap_or_else(|| {
                panic!(
                    "master: detach_window called for {id:?} but window is not in workspace {ws_id}"
                )
            });

        let y_offset = match pane {
            Pane::Master => state.master_y_offset,
            Pane::Secondary => state.stack_y_offset,
        };

        match pane {
            Pane::Master => state.master.remove(idx),
            Pane::Secondary => state.stack.remove(idx),
        };
        Self::adjust_focus_after_removal(state, pane, idx);

        if state.master.is_empty() && state.stack.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        let dim = self.window_dimensions.remove(&id).unwrap_or_else(|| {
            panic!("master: detach_window called for {id:?} but window_dimensions has no entry")
        });
        let result = Dimension::new(
            dim.x + screen.x,
            dim.y - y_offset + screen.y,
            dim.width,
            dim.height,
        );

        self.layout_workspace(hub, ws_id);
        result
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("setting focus on tiling window requires a workspace");
        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return;
        };
        let pane = if let Some(idx) = state.master.iter().position(|&(w, _)| w == window_id) {
            state.focus = Some(Focus {
                pane: Pane::Master,
                index: idx,
            });
            Pane::Master
        } else if let Some(idx) = state.stack.iter().position(|&(w, _)| w == window_id) {
            state.focus = Some(Focus {
                pane: Pane::Secondary,
                index: idx,
            });
            Pane::Secondary
        } else {
            return;
        };
        hub.workspaces.get_mut(ws_id).is_float_focused = false;
        self.scroll_into_view(hub, ws_id, pane);
    }

    fn focused_tiling_window(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> Option<WindowId> {
        let state = self.workspaces.get(&ws_id)?;
        let f = state.focus?;
        match f.pane {
            Pane::Master => state.master.get(f.index).map(|&(id, _)| id),
            Pane::Secondary => state.stack.get(f.index).map(|&(id, _)| id),
        }
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> TilingPlacements {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return TilingPlacements {
                windows: Vec::new(),
                containers: Vec::new(),
            };
        };

        let ws = hub.workspaces.get(ws_id);
        let screen = hub.monitors.get(ws.monitor).dimension;

        let mut windows = Vec::with_capacity(state.master.len() + state.stack.len());

        let focused_idx = if highlighted && !ws.is_float_focused {
            state.focus
        } else {
            None
        };

        let mut push_pane = |pane: Pane, vec: &[(WindowId, PlacementTag)], y_offset: Length| {
            for (i, &(wid, _)) in vec.iter().enumerate() {
                let dim = self
                    .window_dimensions
                    .get(&wid)
                    .expect("master: window in state but missing from window_dimensions");
                let frame = translate(*dim, Length::ZERO, y_offset, screen);
                if let Some(visible_frame) = clip(frame, screen) {
                    let is_highlighted = focused_idx == Some(Focus { pane, index: i });
                    windows.push(TilingWindowPlacement {
                        id: wid,
                        frame,
                        visible_frame,
                        is_highlighted,
                        spawn_indicator: None,
                    });
                }
            }
        };

        push_pane(Pane::Master, &state.master, state.master_y_offset);
        push_pane(Pane::Secondary, &state.stack, state.stack_y_offset);

        TilingPlacements {
            windows,
            containers: Vec::new(),
        }
    }

    fn handle_action(&mut self, hub: &mut HubAccess, action: TilingAction) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;

        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(f) = state.focus else {
            return;
        };
        let master_len = state.master.len();
        let stack_len = state.stack.len();

        match action {
            TilingAction::FocusDirection { direction, forward } => {
                if master_len + stack_len <= 1 {
                    return;
                }
                match (direction, forward) {
                    // Left: from stack -> focus first master
                    (Direction::Horizontal, false) => {
                        if f.pane == Pane::Secondary && master_len > 0 {
                            self.workspaces.get_mut(&ws_id).unwrap().focus = Some(Focus {
                                pane: Pane::Master,
                                index: 0,
                            });
                        }
                    }
                    // Right: from master -> focus first stack
                    (Direction::Horizontal, true) => {
                        if f.pane == Pane::Master && stack_len > 0 {
                            self.workspaces.get_mut(&ws_id).unwrap().focus = Some(Focus {
                                pane: Pane::Secondary,
                                index: 0,
                            });
                        }
                    }
                    // Up: prev within pane, wrapping
                    (Direction::Vertical, false) => {
                        let len = match f.pane {
                            Pane::Master => master_len,
                            Pane::Secondary => stack_len,
                        };
                        if len <= 1 {
                            return;
                        }
                        let new = if f.index == 0 { len - 1 } else { f.index - 1 };
                        self.workspaces.get_mut(&ws_id).unwrap().focus = Some(Focus {
                            pane: f.pane,
                            index: new,
                        });
                    }
                    // Down: next within pane, wrapping
                    (Direction::Vertical, true) => {
                        let len = match f.pane {
                            Pane::Master => master_len,
                            Pane::Secondary => stack_len,
                        };
                        if len <= 1 {
                            return;
                        }
                        let new = if f.index == len - 1 { 0 } else { f.index + 1 };
                        self.workspaces.get_mut(&ws_id).unwrap().focus = Some(Focus {
                            pane: f.pane,
                            index: new,
                        });
                    }
                }
                let state = self.workspaces.get(&ws_id).unwrap();
                if let Some(f) = state.focus {
                    self.scroll_into_view(hub, ws_id, f.pane);
                }
            }
            TilingAction::MoveDirection { direction, forward } => {
                if master_len + stack_len <= 1 {
                    return;
                }
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                match (direction, forward) {
                    // Left from stack: move focused stack window to master.
                    // If master is at capacity, swap with the last master.
                    (Direction::Horizontal, false) => {
                        if f.pane == Pane::Secondary {
                            let moved = state.stack.remove(f.index);
                            if state.master.len() >= state.master_count && master_len > 0 {
                                let swapped = state.master.pop().unwrap();
                                state.master.push(moved);
                                state.stack.push(swapped);
                                state.focus = Some(Focus {
                                    pane: Pane::Master,
                                    index: state.master.len() - 1,
                                });
                            } else if state.master.len() < state.master_count {
                                state.master.push(moved);
                                state.focus = Some(Focus {
                                    pane: Pane::Master,
                                    index: state.master.len() - 1,
                                });
                            }
                        }
                    }
                    // Right from master: swap focused master window with first stack
                    (Direction::Horizontal, true) => {
                        if f.pane == Pane::Master && stack_len > 0 {
                            let moved = state.master.remove(f.index);
                            let swapped = state.stack.remove(0);
                            state.master.push(swapped);
                            state.stack.push(moved);
                            state.focus = Some(Focus {
                                pane: Pane::Secondary,
                                index: state.stack.len() - 1,
                            });
                        }
                    }
                    // Up: swap with prev within pane, wrapping
                    (Direction::Vertical, false) => {
                        let len = match f.pane {
                            Pane::Master => state.master.len(),
                            Pane::Secondary => state.stack.len(),
                        };
                        if len <= 1 {
                            return;
                        }
                        let target = if f.index == 0 { len - 1 } else { f.index - 1 };
                        let vec = match f.pane {
                            Pane::Master => &mut state.master,
                            Pane::Secondary => &mut state.stack,
                        };
                        vec.swap(f.index, target);
                        state.focus = Some(Focus {
                            pane: f.pane,
                            index: target,
                        });
                    }
                    // Down: swap with next within pane, wrapping
                    (Direction::Vertical, true) => {
                        let len = match f.pane {
                            Pane::Master => state.master.len(),
                            Pane::Secondary => state.stack.len(),
                        };
                        if len <= 1 {
                            return;
                        }
                        let target = if f.index == len - 1 { 0 } else { f.index + 1 };
                        let vec = match f.pane {
                            Pane::Master => &mut state.master,
                            Pane::Secondary => &mut state.stack,
                        };
                        vec.swap(f.index, target);
                        state.focus = Some(Focus {
                            pane: f.pane,
                            index: target,
                        });
                    }
                }
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::GrowMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_ratio = (state.master_ratio + 0.05).clamp(0.1, 0.9);
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::ShrinkMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                state.master_ratio = (state.master_ratio - 0.05).clamp(0.1, 0.9);
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::MoreMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                let focused_wid = state.focus.and_then(|f| match f.pane {
                    Pane::Master => state.master.get(f.index).map(|&(id, _)| id),
                    Pane::Secondary => state.stack.get(f.index).map(|&(id, _)| id),
                });
                state.master_count += 1;
                if state.master.len() < state.master_count && !state.stack.is_empty() {
                    let (wid, tag) = state.stack.remove(0);
                    state.master.push((wid, tag));
                }
                state.focus = focused_wid.and_then(|wid| {
                    state
                        .master
                        .iter()
                        .position(|&(id, _)| id == wid)
                        .map(|i| Focus {
                            pane: Pane::Master,
                            index: i,
                        })
                        .or_else(|| {
                            state
                                .stack
                                .iter()
                                .position(|&(id, _)| id == wid)
                                .map(|i| Focus {
                                    pane: Pane::Secondary,
                                    index: i,
                                })
                        })
                });
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::FewerMaster => {
                let state = self.workspaces.get_mut(&ws_id).unwrap();
                if state.master_count <= 1 {
                    return;
                }
                let focused_wid = state.focus.and_then(|f| match f.pane {
                    Pane::Master => state.master.get(f.index).map(|&(id, _)| id),
                    Pane::Secondary => state.stack.get(f.index).map(|&(id, _)| id),
                });
                state.master_count -= 1;
                if state.master.len() > state.master_count {
                    let (wid, tag) = state.master.pop().unwrap();
                    state.stack.insert(0, (wid, tag));
                }
                state.focus = focused_wid.and_then(|wid| {
                    state
                        .master
                        .iter()
                        .position(|&(id, _)| id == wid)
                        .map(|i| Focus {
                            pane: Pane::Master,
                            index: i,
                        })
                        .or_else(|| {
                            state
                                .stack
                                .iter()
                                .position(|&(id, _)| id == wid)
                                .map(|i| Focus {
                                    pane: Pane::Secondary,
                                    index: i,
                                })
                        })
                });
                self.layout_workspace(hub, ws_id);
            }
            TilingAction::ToggleSpawnMode
            | TilingAction::ToggleDirection
            | TilingAction::ToggleContainerLayout
            | TilingAction::FocusParent
            | TilingAction::FocusTab { .. }
            | TilingAction::TabClicked { .. } => {}
        }
    }

    fn layout_workspace(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        self.do_layout(hub, ws_id);
    }

    fn has_tiling_windows(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> bool {
        self.workspaces
            .get(&ws_id)
            .is_some_and(|s| !(s.master.is_empty() && s.stack.is_empty()))
    }

    fn tiling_window_count(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        self.workspaces
            .get(&ws_id)
            .map_or(0, |ws| ws.master.len() + ws.stack.len())
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let state = self.workspaces.get(&ws_id)?;
        let f = state.focus?;
        let id = match f.pane {
            Pane::Master => state.master.get(f.index)?.0,
            Pane::Secondary => state.stack.get(f.index)?.0,
        };

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        match f.pane {
            Pane::Master => state.master.remove(f.index),
            Pane::Secondary => state.stack.remove(f.index),
        };
        Self::adjust_focus_after_removal(state, f.pane, f.index);

        if state.master.is_empty() && state.stack.is_empty() {
            let ws = hub.workspaces.get_mut(ws_id);
            ws.is_float_focused = !ws.float_windows.is_empty();
        }

        self.window_dimensions.remove(&id);
        self.layout_workspace(hub, ws_id);

        Some(Child::Window(id))
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        let Child::Window(id) = child else {
            panic!("MasterStrategy does not support Container children");
        };
        self.attach_window(hub, id, ws_id);
        self.set_focus(hub, id);
    }

    fn prune_workspace(&mut self, ws_id: WorkspaceId) {
        if let Some(state) = self.workspaces.remove(&ws_id) {
            for (wid, _) in &state.master {
                self.window_dimensions.remove(wid);
            }
            for (wid, _) in &state.stack {
                self.window_dimensions.remove(wid);
            }
        }
    }

    fn apply_config(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) {
        // Refresh matchers from config.
        let new_matchers: Vec<PaneMatcher> =
            build_matchers_from(&hub.preferred_layouts, &hub.workspaces.get(ws_id).name);

        let Some(state) = self.workspaces.get_mut(&ws_id) else {
            return;
        };
        state.matchers = new_matchers;

        // Snapshot focused window id.
        let focused_wid = state.focus.and_then(|f| match f.pane {
            Pane::Master => state.master.get(f.index).map(|&(id, _)| id),
            Pane::Secondary => state.stack.get(f.index).map(|&(id, _)| id),
        });

        // Pull all windows out in order (master first, then stack).
        let all_windows: Vec<WindowId> = state
            .master
            .iter()
            .chain(state.stack.iter())
            .map(|&(id, _)| id)
            .collect();

        state.master.clear();
        state.stack.clear();

        // Re-place each window into the empty (growing) state.
        for &wid in &all_windows {
            let metadata = hub.windows.get(wid).metadata.as_ref();
            let (pane, ins, tag) = Self::place(state, metadata);
            match pane {
                Pane::Master => state.master.insert(ins, (wid, tag)),
                Pane::Secondary => state.stack.insert(ins, (wid, tag)),
            }
        }

        // Restore focus.
        state.focus = focused_wid.and_then(|wid| {
            state
                .master
                .iter()
                .position(|&(id, _)| id == wid)
                .map(|i| Focus {
                    pane: Pane::Master,
                    index: i,
                })
                .or_else(|| {
                    state
                        .stack
                        .iter()
                        .position(|&(id, _)| id == wid)
                        .map(|i| Focus {
                            pane: Pane::Secondary,
                            index: i,
                        })
                })
                .or({
                    if !state.master.is_empty() {
                        Some(Focus {
                            pane: Pane::Master,
                            index: 0,
                        })
                    } else if !state.stack.is_empty() {
                        Some(Focus {
                            pane: Pane::Secondary,
                            index: 0,
                        })
                    } else {
                        None
                    }
                })
        });

        self.layout_workspace(hub, ws_id);
    }

    #[cfg(test)]
    fn validate_tree(&self, hub: &HubAccess) {
        for (&ws_id, state) in &self.workspaces {
            let mut seen = HashSet::new();
            for &(wid, _) in state.master.iter().chain(state.stack.iter()) {
                hub.windows.get(wid);
                assert!(
                    seen.insert(wid),
                    "master-stack workspace {ws_id}: duplicate window {wid:?}"
                );
            }
            assert!(
                state.master.len() <= state.master_count,
                "master-stack workspace {ws_id}: master.len() {} > master_count {}",
                state.master.len(),
                state.master_count
            );

            match state.focus {
                Some(f) => {
                    let len = match f.pane {
                        Pane::Master => state.master.len(),
                        Pane::Secondary => state.stack.len(),
                    };
                    assert!(
                        f.index < len,
                        "master-stack workspace {ws_id}: focus {f:?} index {} out of bounds ({} {})",
                        f.index,
                        if f.pane == Pane::Master {
                            "master"
                        } else {
                            "stack"
                        },
                        len
                    );
                }
                None => {
                    assert!(
                        state.master.is_empty() && state.stack.is_empty(),
                        "master-stack workspace {ws_id}: focus is None but windows exist"
                    );
                }
            }

            if state.master.is_empty() && state.stack.is_empty() {
                continue;
            }

            let pane_height = hub
                .monitors
                .get(hub.workspaces.get(ws_id).monitor)
                .dimension
                .height;

            for &(wid, _) in &state.master {
                let dim = self.window_dimensions.get(&wid).unwrap_or_else(|| {
                    panic!(
                        "master-stack workspace {ws_id}: window {wid:?} missing from window_dimensions"
                    )
                });
                assert!(
                    dim.width > Length::ZERO,
                    "master-stack workspace {ws_id}: window {wid:?} has non-positive width {}",
                    dim.width
                );
                assert!(
                    dim.height > Length::ZERO,
                    "master-stack workspace {ws_id}: window {wid:?} has non-positive height {}",
                    dim.height
                );
                let (min_w, min_h, _, _) = effective_constraints(hub, wid);
                assert!(
                    dim.width >= min_w,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_w {}",
                    dim.width,
                    min_w
                );
                assert!(
                    dim.height >= min_h,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_h {}",
                    dim.height,
                    min_h
                );
            }

            // Same for stack windows.
            for &(wid, _) in &state.stack {
                let dim = self.window_dimensions.get(&wid).unwrap_or_else(|| {
                    panic!(
                        "master-stack workspace {ws_id}: window {wid:?} missing from window_dimensions"
                    )
                });
                assert!(
                    dim.width > Length::ZERO,
                    "master-stack workspace {ws_id}: window {wid:?} has non-positive width {}",
                    dim.width
                );
                assert!(
                    dim.height > Length::ZERO,
                    "master-stack workspace {ws_id}: window {wid:?} has non-positive height {}",
                    dim.height
                );
                let (min_w, min_h, _, _) = effective_constraints(hub, wid);
                assert!(
                    dim.width >= min_w,
                    "master-stack workspace {ws_id}: window {wid:?} width {} < effective min_w {}",
                    dim.width,
                    min_w
                );
                assert!(
                    dim.height >= min_h,
                    "master-stack workspace {ws_id}: window {wid:?} height {} < effective min_h {}",
                    dim.height,
                    min_h
                );
            }

            // Master pane scroll bounds.
            let master_ids: Vec<WindowId> = state.master.iter().map(|&(id, _)| id).collect();
            if !master_ids.is_empty() {
                let master_content_h = self.pane_content_h(hub, &master_ids, pane_height);
                let master_max_offset = (master_content_h - pane_height).max(Length::ZERO);
                assert!(
                    state.master_y_offset >= Length::ZERO
                        && state.master_y_offset <= master_max_offset,
                    "master-stack workspace {ws_id}: master_y_offset {} out of bounds [0, {}]",
                    state.master_y_offset,
                    master_max_offset
                );
            } else {
                assert!(
                    state.master_y_offset == Length::ZERO,
                    "master-stack workspace {ws_id}: master_y_offset should be zero (no master windows)"
                );
            }

            // Stack pane scroll bounds.
            let stack_ids: Vec<WindowId> = state.stack.iter().map(|&(id, _)| id).collect();
            if !stack_ids.is_empty() {
                let stack_content_h = self.pane_content_h(hub, &stack_ids, pane_height);
                let stack_max_offset = (stack_content_h - pane_height).max(Length::ZERO);
                assert!(
                    state.stack_y_offset >= Length::ZERO
                        && state.stack_y_offset <= stack_max_offset,
                    "master-stack workspace {ws_id}: stack_y_offset {} out of bounds [0, {}]",
                    state.stack_y_offset,
                    stack_max_offset
                );
            } else {
                assert!(
                    state.stack_y_offset == Length::ZERO,
                    "master-stack workspace {ws_id}: stack_y_offset should be zero (no stack windows)"
                );
            }
        }
    }
}

impl MasterStrategy {
    pub(crate) fn new() -> Self {
        Self {
            workspaces: HashMap::new(),
            window_dimensions: HashMap::new(),
        }
    }

    /// Extract the per-pane slot from a tag. `Some(slot)` for `Matched`,
    /// `None` for `Unmatched`. Used for insertion ordering: matched entries
    /// sort before unmatched; among matched, lower slot sorts earlier.
    fn slot_of(tag: &PlacementTag) -> Option<usize> {
        match tag {
            PlacementTag::Matched { slot, .. } => Some(*slot),
            PlacementTag::Unmatched => None,
        }
    }

    /// Decide where a new window should go. Pure function, no mutation.
    /// Returns `(target_pane, insertion_index, tag)`.
    fn place(state: &MasterState, metadata: &dyn WindowMetadata) -> (Pane, usize, PlacementTag) {
        let occupied_master: HashSet<usize> = state
            .master
            .iter()
            .filter_map(|(_, tag)| match tag {
                PlacementTag::Matched {
                    pane: Pane::Master,
                    slot,
                } => Some(*slot),
                _ => None,
            })
            .collect();

        let occupied_secondary: HashSet<usize> = state
            .stack
            .iter()
            .filter_map(|(_, tag)| match tag {
                PlacementTag::Matched {
                    pane: Pane::Secondary,
                    slot,
                } => Some(*slot),
                _ => None,
            })
            .collect();

        let mut master_slot = 0usize;
        let mut secondary_slot = 0usize;
        for m in &state.matchers {
            let (slot, occupied) = match m.pane {
                Pane::Master => {
                    let s = master_slot;
                    master_slot += 1;
                    (s, &occupied_master)
                }
                Pane::Secondary => {
                    let s = secondary_slot;
                    secondary_slot += 1;
                    (s, &occupied_secondary)
                }
            };
            if occupied.contains(&slot) {
                continue;
            }
            if metadata.matches_window_matcher(&m.matcher) {
                return Self::insert_for_match(state, m.pane, slot);
            }
        }

        // No matcher matched: master if room, else stack.
        if state.master.len() < state.master_count {
            (Pane::Master, state.master.len(), PlacementTag::Unmatched)
        } else {
            (Pane::Secondary, state.stack.len(), PlacementTag::Unmatched)
        }
    }

    /// Insert a matched window. When master-targeted and master is full,
    /// cascades to stack.
    fn insert_for_match(
        state: &MasterState,
        pane: Pane,
        slot: usize,
    ) -> (Pane, usize, PlacementTag) {
        let tag = PlacementTag::Matched { pane, slot };

        // Master-targeted but master is full: cascade to stack.
        if pane == Pane::Master && state.master.len() >= state.master_count {
            let ins = state
                .stack
                .iter()
                .position(|(_, t)| Self::slot_of(t).is_some_and(|k| k > slot))
                .unwrap_or(state.stack.len());
            return (Pane::Secondary, ins, tag);
        }

        let vec = match pane {
            Pane::Master => &state.master,
            Pane::Secondary => &state.stack,
        };
        let ins = vec
            .iter()
            .position(|(_, t)| Self::slot_of(t).is_some_and(|k| k > slot))
            .unwrap_or(vec.len());
        (pane, ins, tag)
    }

    /// Compute layout dimensions for all windows. Master pane on the left,
    /// stack on the right (two-pane) or a single pane fills the full width
    /// when the other pane is empty.
    fn do_layout(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let master_n = state.master.len();
        let stack_n = state.stack.len();
        if master_n == 0 && stack_n == 0 {
            return;
        }

        let screen = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension;
        let h = screen.height;

        let master_ids: Vec<WindowId> = state.master.iter().map(|&(id, _)| id).collect();
        let stack_ids: Vec<WindowId> = state.stack.iter().map(|&(id, _)| id).collect();

        match (master_n, stack_n) {
            (_, 0) => {
                // Only master: fills full screen width.
                self.do_pane_layout(hub, &master_ids, screen.width, Length::ZERO, h);
            }
            (0, _) => {
                // Only stack: fills full screen width.
                self.do_pane_layout(hub, &stack_ids, screen.width, Length::ZERO, h);
            }
            (_, _) => {
                // Two-pane: master left, stack right.
                let master_min_w = master_ids
                    .iter()
                    .map(|&id| effective_constraints(hub, id).0)
                    .fold(Length::ZERO, Length::max);
                let stack_min_w = stack_ids
                    .iter()
                    .map(|&id| effective_constraints(hub, id).0)
                    .fold(Length::ZERO, Length::max);

                let desired_master_w =
                    Length::new((screen.width.value() * state.master_ratio).floor());
                let total_min = master_min_w + stack_min_w;

                let (master_w, stack_w) = if total_min >= screen.width {
                    (master_min_w, stack_min_w)
                } else if desired_master_w < master_min_w {
                    (master_min_w, screen.width - master_min_w)
                } else if screen.width - desired_master_w < stack_min_w {
                    (screen.width - stack_min_w, stack_min_w)
                } else {
                    (desired_master_w, screen.width - desired_master_w)
                };

                self.do_pane_layout(hub, &master_ids, master_w, Length::ZERO, h);
                self.do_pane_layout(hub, &stack_ids, stack_w, master_w, h);
            }
        }

        self.clamp_scroll(hub, ws_id);
        let state = self.workspaces.get(&ws_id).unwrap();
        if let Some(f) = state.focus {
            self.scroll_into_view(hub, ws_id, f.pane);
        }
    }

    /// Layout a single pane's windows vertically within `pane_width`,
    /// starting at x offset `x_start`, within screen height `h`.
    fn do_pane_layout(
        &mut self,
        hub: &HubAccess,
        ids: &[WindowId],
        pane_width: Length,
        x_start: Length,
        h: Length,
    ) {
        if ids.is_empty() {
            return;
        }
        let pane_min_w = ids
            .iter()
            .map(|&id| effective_constraints(hub, id).0)
            .fold(Length::ZERO, Length::max);
        let adjusted_w = pane_min_w.max(pane_width);

        let constraints: Vec<(Length, Length)> = ids
            .iter()
            .map(|&id| {
                let (_, min_h, _, max_h) = effective_constraints(hub, id);
                (min_h, max_h)
            })
            .collect();
        let heights = distribute_space(&constraints, h);
        let sum_h: Length = heights.iter().copied().sum();
        let mut y = if sum_h < h {
            (h - sum_h) / 2.0
        } else {
            Length::ZERO
        };
        for (i, &id) in ids.iter().enumerate() {
            let (_, _, max_w, max_h) = effective_constraints(hub, id);
            let (w, x_off) = apply_max_constraint(max_w, adjusted_w);
            let (slot_h, y_off) = apply_max_constraint(max_h, heights[i]);
            self.window_dimensions
                .insert(id, Dimension::new(x_start + x_off, y + y_off, w, slot_h));
            y += heights[i];
        }
    }

    /// Update focus after removing the window at `(pane, idx)`.
    fn adjust_focus_after_removal(state: &mut MasterState, pane: Pane, idx: usize) {
        let Some(f) = state.focus else {
            return;
        };

        let pane_now_empty = match pane {
            Pane::Master => state.master.is_empty(),
            Pane::Secondary => state.stack.is_empty(),
        };

        if pane_now_empty {
            // Focus moves to the other pane, or None if both empty.
            let other_pane_empty = match pane {
                Pane::Master => state.stack.is_empty(),
                Pane::Secondary => state.master.is_empty(),
            };
            if other_pane_empty {
                state.focus = None;
            } else {
                state.focus = Some(Focus {
                    pane: match pane {
                        Pane::Master => Pane::Secondary,
                        Pane::Secondary => Pane::Master,
                    },
                    index: 0,
                });
            }
            return;
        }

        if f.pane == pane {
            if idx == f.index {
                // Removed the focused window: clamp to pane bounds.
                let len = match pane {
                    Pane::Master => state.master.len(),
                    Pane::Secondary => state.stack.len(),
                };
                state.focus = Some(Focus {
                    pane,
                    index: idx.min(len.saturating_sub(1)),
                });
            } else if idx < f.index {
                state.focus = Some(Focus {
                    pane,
                    index: f.index - 1,
                });
            }
        }
        // If focus.pane != pane, no adjustment needed.
    }

    fn clamp_scroll(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;

        let master_ids: Vec<WindowId> = state.master.iter().map(|&(id, _)| id).collect();
        let master_max = if !master_ids.is_empty() {
            let content_h = self.pane_content_h(hub, &master_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let stack_ids: Vec<WindowId> = state.stack.iter().map(|&(id, _)| id).collect();
        let stack_max = if !stack_ids.is_empty() {
            let content_h = self.pane_content_h(hub, &stack_ids, pane_height);
            (content_h - pane_height).max(Length::ZERO)
        } else {
            Length::ZERO
        };

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.master_y_offset = state.master_y_offset.clamp(Length::ZERO, master_max);
        state.stack_y_offset = state.stack_y_offset.clamp(Length::ZERO, stack_max);
    }

    fn scroll_into_view(&mut self, hub: &HubAccess, ws_id: WorkspaceId, pane: Pane) {
        let Some(state) = self.workspaces.get(&ws_id) else {
            return;
        };
        let Some(f) = state.focus else {
            return;
        };
        if f.pane != pane {
            return;
        }
        let pane_height = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .dimension
            .height;

        let (pane_windows, offset): (Vec<WindowId>, Length) = match pane {
            Pane::Master => (
                state.master.iter().map(|&(id, _)| id).collect(),
                state.master_y_offset,
            ),
            Pane::Secondary => (
                state.stack.iter().map(|&(id, _)| id).collect(),
                state.stack_y_offset,
            ),
        };

        let slot_heights = self.pane_slot_heights(hub, &pane_windows, pane_height);
        let content_h: Length = slot_heights.iter().copied().sum();
        let max_offset = (content_h - pane_height).max(Length::ZERO);

        let focused_in_pane = f.index;
        let content_start = if content_h < pane_height {
            (pane_height - content_h) / 2.0
        } else {
            Length::ZERO
        };
        let slot_y: Length = content_start
            + slot_heights[..focused_in_pane]
                .iter()
                .copied()
                .sum::<Length>();
        let slot_height = slot_heights[focused_in_pane];

        let mut new_offset = offset;
        if slot_y + slot_height - new_offset > pane_height {
            new_offset = slot_y + slot_height - pane_height;
        }
        if slot_y - new_offset < Length::ZERO {
            new_offset = slot_y;
        }
        new_offset = new_offset.clamp(Length::ZERO, max_offset);

        let state = self.workspaces.get_mut(&ws_id).unwrap();
        match pane {
            Pane::Master => state.master_y_offset = new_offset,
            Pane::Secondary => state.stack_y_offset = new_offset,
        }
    }

    fn pane_content_h(
        &self,
        hub: &HubAccess,
        pane_windows: &[WindowId],
        pane_height: Length,
    ) -> Length {
        let heights = self.pane_slot_heights(hub, pane_windows, pane_height);
        heights.iter().copied().sum()
    }

    fn pane_slot_heights(
        &self,
        hub: &HubAccess,
        pane_windows: &[WindowId],
        pane_height: Length,
    ) -> Vec<Length> {
        if pane_windows.is_empty() {
            return Vec::new();
        }
        let constraints: Vec<(Length, Length)> = pane_windows
            .iter()
            .map(|&id| {
                let (_, min_h, _, max_h) = effective_constraints(hub, id);
                (min_h, max_h)
            })
            .collect();
        distribute_space(&constraints, pane_height)
    }
}

/// Per-workspace state for master-stack layout.
///
/// `master` and `stack` are independent vecs. `master.len()` never exceeds
/// `master_count`; overflow windows live in the stack vec.
///
/// Invariants:
///   master: Matched { pane: Master, slot } in ascending slot order, then Unmatched
///   stack:  Matched { pane: Secondary, slot } asc, then Matched { pane: Master, slot } overflow asc, then Unmatched
///   focus is None iff both vecs are empty
#[derive(Debug)]
struct MasterState {
    master: Vec<(WindowId, PlacementTag)>,
    stack: Vec<(WindowId, PlacementTag)>,
    matchers: Vec<PaneMatcher>,
    focus: Option<Focus>,
    master_y_offset: Length,
    stack_y_offset: Length,
    master_count: usize,
    master_ratio: f32,
}

/// Which side of the master-stack split a window lives in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pane {
    Master,
    Secondary,
}

/// Why a window is in its pane, and the slot it should hold there.
/// `slot` is the window's index among matchers of the same pane in config order.
/// `Unmatched` windows have no slot and sort after all matched ones.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlacementTag {
    Matched { pane: Pane, slot: usize },
    Unmatched,
}

/// Pane-aware focus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Focus {
    pane: Pane,
    index: usize,
}

/// One config matcher plus which pane it targets.
#[derive(Debug)]
struct PaneMatcher {
    pane: Pane,
    matcher: WindowMatcher,
}

/// Build the matcher list from config for a given workspace.
fn build_matchers_from(
    preferred_layouts: &[LayoutWorkspaceConfig],
    ws_name: &str,
) -> Vec<PaneMatcher> {
    preferred_layouts
        .iter()
        .find_map(|w| match w {
            LayoutWorkspaceConfig::Master {
                name,
                master,
                secondary,
                ..
            } if *name == ws_name => Some(
                master
                    .iter()
                    .map(|m| PaneMatcher {
                        pane: Pane::Master,
                        matcher: m.clone(),
                    })
                    .chain(secondary.iter().map(|m| PaneMatcher {
                        pane: Pane::Secondary,
                        matcher: m.clone(),
                    }))
                    .collect(),
            ),
            _ => None,
        })
        .unwrap_or_default()
}

/// Returns (size, offset) for centering a max-constrained child inside its slot.
/// When max is zero or >= slot_extent, the child fills the slot with no offset.
fn apply_max_constraint(max: Length, slot_extent: Length) -> (Length, Length) {
    let size = if max > Length::ZERO && max < slot_extent {
        max
    } else {
        slot_extent
    };
    let offset = (slot_extent - size) / 2.0;
    (size, offset.max(Length::ZERO))
}

fn effective_constraints(hub: &HubAccess, wid: WindowId) -> (Length, Length, Length, Length) {
    let ws_id = hub
        .windows
        .get(wid)
        .workspace()
        .expect("tiling window has a workspace");
    let monitor = hub.monitors.get(hub.workspaces.get(ws_id).monitor);
    let scale = monitor.scale;
    let screen = monitor.dimension;

    let global_min_w = hub.layout.min_width.resolve(screen.width, scale);
    let global_min_h = hub.layout.min_height.resolve(screen.height, scale);
    let global_max_w = hub.layout.max_width.resolve(screen.width, scale);
    let global_max_h = hub.layout.max_height.resolve(screen.height, scale);

    let window = hub.windows.get(wid);
    let (raw_min_w, raw_min_h) = window.min_size();
    let (raw_max_w, raw_max_h) = window.max_size();
    let win_min_w = Length::new(raw_min_w);
    let win_min_h = Length::new(raw_min_h);
    let win_max_w = Length::new(raw_max_w);
    let win_max_h = Length::new(raw_max_h);

    let max_w = if win_max_w > Length::ZERO {
        win_max_w
    } else {
        global_max_w
    };
    let max_h = if win_max_h > Length::ZERO {
        win_max_h
    } else {
        global_max_h
    };

    let min_w = if max_w > Length::ZERO {
        win_min_w.max(global_min_w).min(max_w)
    } else {
        win_min_w.max(global_min_w)
    };
    let min_h = if max_h > Length::ZERO {
        win_min_h.max(global_min_h).min(max_h)
    } else {
        win_min_h.max(global_min_h)
    };

    (min_w, min_h, max_w, max_h)
}
