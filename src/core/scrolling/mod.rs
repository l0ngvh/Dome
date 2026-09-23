mod export;
mod placement;
mod preferred_layout;
mod scroll;
#[cfg(test)]
mod validate;

pub(crate) use preferred_layout::ColumnConfig;

use rustc_hash::FxHashMap;

use crate::core::TilingConfig;
use crate::core::hub::HubAccess;
use crate::core::node::{
    Child, Container, ContainerId, Dimension, Direction, Length, PixelRect, WindowId,
    WindowMetadata, WorkspaceId,
};
use crate::core::strategy::{TilingPlacements, TilingStrategy, WorkspaceExport, translate};
use crate::core::{PreferredWorkspace, SizeConstraint, SizeConstraints};

/// Scrolling tiling: a flat, ordered list of columns, one window per column. Each
/// column carries its own width and lays out left to right at a cumulative x. The
/// column total may exceed the screen width, so a window may extend past the screen
/// edge. A per-workspace horizontal offset and a per-column vertical offset reveal the
/// overflow.
#[derive(Debug)]
pub(crate) struct ScrollingStrategy {
    workspaces: FxHashMap<WorkspaceId, WorkspaceState>,
    /// Border-box dimension per window, in layout space.
    window_states: FxHashMap<WindowId, Dimension>,
    default_column_width: SizeConstraint,
    size_constraints: SizeConstraints,
}

#[derive(Debug, Default)]
struct WorkspaceState {
    /// This workspace's `layout.lua` columns, left to right.
    slots: Vec<ColumnConfig>,
    /// Left to right.
    columns: Vec<Column>,
    /// Windows from most to least recently focused.
    focus_history: Vec<WindowId>,
    /// Left edge of the viewport, in unscrolled workspace space.
    x_offset: Length,
}

#[derive(Debug)]
struct Column {
    /// A hub container holding exactly one `Child::Window`.
    container: ContainerId,
    width: SizeConstraint,
    /// Top edge of the viewport over this column.
    y_offset: Length,
    /// Index into `WorkspaceState::slots` of the layout column whose matcher placed
    /// this window.
    occupy: Option<usize>,
}

impl WorkspaceState {
    fn focused_window(&self) -> Option<WindowId> {
        self.focus_history.first().copied()
    }

    fn record_focus(&mut self, window_id: WindowId) {
        self.drop_from_history(window_id);
        self.focus_history.insert(0, window_id);
    }

    fn add_to_history(&mut self, window_id: WindowId) {
        if !self.focus_history.contains(&window_id) {
            self.focus_history.push(window_id);
        }
    }

    fn drop_from_history(&mut self, window_id: WindowId) {
        if let Some(pos) = self.focus_history.iter().position(|&w| w == window_id) {
            self.focus_history.remove(pos);
        }
    }
}

impl TilingStrategy for ScrollingStrategy {
    fn prepare_workspace(
        &mut self,
        _hub: &mut HubAccess,
        ws_id: WorkspaceId,
        preferred_layout: Option<&PreferredWorkspace>,
    ) {
        let slots = match preferred_layout {
            Some(PreferredWorkspace::Scrolling { columns, .. }) => columns.clone(),
            None => Vec::new(),
            Some(_) => panic!("Preparing a non-scrolling workspace in the scrolling strategy"),
        };
        self.workspaces.insert(
            ws_id,
            WorkspaceState {
                slots,
                ..WorkspaceState::default()
            },
        );
    }

    fn attach_window(&mut self, hub: &mut HubAccess, id: WindowId, ws_id: WorkspaceId) {
        hub.windows.get_mut(id).set_workspace(Some(ws_id));
        self.insert_column(hub, id, ws_id, self.default_column_width);
        self.compute_placement(hub, ws_id);
    }

    fn detach_window(&mut self, hub: &mut HubAccess, id: WindowId) -> PixelRect {
        let ws_id = hub
            .windows
            .get(id)
            .workspace()
            .expect("detaching tiling window has a workspace");
        let work_area = hub
            .monitors
            .get(hub.workspaces.get(ws_id).monitor)
            .work_area;

        let dim = self.window_states.remove(&id).unwrap_or_else(|| {
            panic!("scrolling: detach_window called for {id:?} but window_states has no entry")
        });
        let state = &self.workspaces[&ws_id];
        let x_offset = state.x_offset;
        let y_offset = self
            .column_index_of(hub, ws_id, id)
            .map_or(Length::ZERO, |i| state.columns[i].y_offset);
        self.remove_column(hub, ws_id, id);
        let result = translate(dim, x_offset, y_offset, work_area.x(), work_area.y());
        self.compute_placement(hub, ws_id);
        result
    }

    fn focus_direction(&mut self, hub: &mut HubAccess, direction: Direction, forward: bool) {
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        if self.reveal_hidden_part(hub, ws_id, direction, forward) {
            return;
        }
        if direction == Direction::Vertical {
            return;
        }
        let Some(idx) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let state = self.workspaces.get(&ws_id).unwrap();
        let target_idx = match forward {
            true if idx + 1 < state.columns.len() => idx + 1,
            false if idx > 0 => idx - 1,
            _ => return,
        };
        let target = Self::column_window(hub, state.columns[target_idx].container);
        self.set_focus(hub, target);
    }

    fn move_direction(&mut self, hub: &mut HubAccess, direction: Direction, forward: bool) {
        if direction == Direction::Vertical {
            tracing::debug!("scrolling: vertical move waits on the join operation, no-op for now");
            return;
        }
        let ws_id = hub.monitors.get(hub.focused_monitor).active_workspace;
        let Some(idx) = self.focused_column_index(hub, ws_id) else {
            return;
        };
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let target_idx = match forward {
            true if idx + 1 < state.columns.len() => idx + 1,
            false if idx > 0 => idx - 1,
            _ => return,
        };
        state.columns.swap(idx, target_idx);
        let focus = Self::column_window(hub, state.columns[target_idx].container);
        self.compute_placement(hub, ws_id);
        self.set_focus(hub, focus);
    }

    fn toggle_container_layout(&mut self, _hub: &mut HubAccess) {
        tracing::debug!("scrolling: toggle_container_layout has no effect without tabbed columns");
    }

    fn focus_tab(&mut self, _hub: &mut HubAccess, _forward: bool) {
        tracing::debug!("scrolling: focus_tab has no effect without tabbed columns");
    }

    fn tab_clicked(&mut self, _hub: &mut HubAccess, _container_id: ContainerId, _index: usize) {
        tracing::debug!("scrolling: tab_clicked has no effect without tabbed columns");
    }

    fn compute_placement(&mut self, hub: &HubAccess, ws_id: WorkspaceId) {
        self.compute_placement(hub, ws_id);
    }

    fn set_focus(&mut self, hub: &mut HubAccess, window_id: WindowId) {
        let ws_id = hub
            .windows
            .get(window_id)
            .workspace()
            .expect("setting focus on tiling window requires a workspace");
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        state.record_focus(window_id);
        self.scroll_into_view(hub, ws_id);
    }

    fn collect_tiling_placements(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        highlighted: bool,
    ) -> TilingPlacements {
        self.collect_tiling_placements(hub, ws_id, highlighted)
    }

    fn focused_tiling_window(&self, ws_id: WorkspaceId) -> Option<WindowId> {
        self.workspaces
            .get(&ws_id)
            .and_then(WorkspaceState::focused_window)
    }

    fn detach_focused_child(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId) -> Option<Child> {
        let focus = self.workspaces.get(&ws_id)?.focused_window()?;
        self.window_states.remove(&focus);
        self.remove_column(hub, ws_id, focus);
        self.compute_placement(hub, ws_id);
        Some(Child::Window(focus))
    }

    fn tiling_window_count(&self, _hub: &HubAccess, ws_id: WorkspaceId) -> usize {
        self.workspaces.get(&ws_id).map_or(0, |s| s.columns.len())
    }

    fn matches_tiling(&self, ws_id: WorkspaceId, metadata: &dyn WindowMetadata) -> bool {
        self.workspaces.get(&ws_id).is_some_and(|state| {
            state
                .slots
                .iter()
                .filter_map(|slot| slot.children.first())
                .any(|m| metadata.matches_window_matcher(m))
        })
    }

    fn reattach_child(&mut self, hub: &mut HubAccess, child: Child, ws_id: WorkspaceId) {
        let arrivals = hub.take_windows(child);
        for &id in &arrivals {
            self.attach_window(hub, id, ws_id);
        }
        if let Some(&focus) = arrivals.first() {
            self.set_focus(hub, focus);
        }
    }

    fn migrate(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
    ) -> (Vec<WindowId>, Option<WindowId>) {
        let focused = self.focused_tiling_window(ws_id);
        let mut tiling = Vec::new();
        if let Some(state) = self.workspaces.remove(&ws_id) {
            for col in state.columns {
                tiling.push(Self::column_window(hub, col.container));
                hub.free_container(col.container);
            }
            for &wid in &tiling {
                self.window_states.remove(&wid);
            }
        }
        (tiling, focused)
    }

    fn sync_preferred_layout(
        &mut self,
        hub: &mut HubAccess,
        ws_id: WorkspaceId,
        incoming: Option<&PreferredWorkspace>,
    ) {
        self.sync_preferred_layout(hub, ws_id, incoming);
    }

    fn apply_config(&mut self, hub: &mut HubAccess, tiling: TilingConfig) {
        self.default_column_width = tiling.scrolling.default_column_width;
        self.size_constraints = tiling.size_constraints;
        for ws_id in self.workspaces.keys().copied().collect::<Vec<_>>() {
            self.compute_placement(hub, ws_id);
        }
    }

    fn export_workspace(&mut self, hub: &HubAccess, ws_id: WorkspaceId) -> WorkspaceExport {
        self.export_workspace(hub, ws_id)
    }
}

impl ScrollingStrategy {
    pub(crate) fn new(
        default_column_width: SizeConstraint,
        size_constraints: SizeConstraints,
    ) -> Self {
        Self {
            workspaces: FxHashMap::default(),
            window_states: FxHashMap::default(),
            default_column_width,
            size_constraints,
        }
    }

    fn column_window(hub: &HubAccess, cid: ContainerId) -> WindowId {
        match hub.containers.get(cid).children() {
            [Child::Window(wid)] => *wid,
            children => {
                panic!("scrolling column {cid:?} holds {children:?}, expected one window")
            }
        }
    }

    fn focused_column_index(&self, hub: &HubAccess, ws_id: WorkspaceId) -> Option<usize> {
        let focus = self.workspaces.get(&ws_id)?.focused_window()?;
        self.column_index_of(hub, ws_id, focus)
    }

    fn column_index_of(
        &self,
        hub: &HubAccess,
        ws_id: WorkspaceId,
        window_id: WindowId,
    ) -> Option<usize> {
        self.workspaces
            .get(&ws_id)?
            .columns
            .iter()
            .position(|c| Self::column_window(hub, c.container) == window_id)
    }

    /// Inserts `id` as a new column and leaves the relayout to the caller. A window that
    /// matches a layout column takes that column's place and width. Any other window
    /// opens right of the focused column at `unmatched_width`.
    fn insert_column(
        &mut self,
        hub: &mut HubAccess,
        id: WindowId,
        ws_id: WorkspaceId,
        unmatched_width: SizeConstraint,
    ) {
        let container = hub.allocate_container(Container {
            children: vec![Child::Window(id)],
        });
        let default = self.default_column_width;
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let metadata = hub.windows.get(id).metadata.as_ref();
        let occupy = state.slots.iter().position(|slot| {
            slot.children
                .first()
                .is_some_and(|m| metadata.matches_window_matcher(m))
        });
        let (insert_at, width) = match occupy {
            Some(slot) => (
                state
                    .columns
                    .iter()
                    .position(|c| c.occupy.is_some_and(|other| other > slot))
                    .unwrap_or(state.columns.len()),
                state.slots[slot].width.unwrap_or(default),
            ),
            None => {
                let right_of_focus = state
                    .focused_window()
                    .and_then(|f| {
                        state
                            .columns
                            .iter()
                            .position(|c| Self::column_window(hub, c.container) == f)
                    })
                    .map_or(state.columns.len(), |i| i + 1);
                (right_of_focus, unmatched_width)
            }
        };
        state.columns.insert(
            insert_at,
            Column {
                container,
                width,
                y_offset: Length::ZERO,
                occupy,
            },
        );
        state.add_to_history(id);
        self.window_states.insert(id, Dimension::default());
    }

    /// The caller must drop `id` from `window_states` before calling this.
    fn remove_column(&mut self, hub: &mut HubAccess, ws_id: WorkspaceId, id: WindowId) {
        let state = self.workspaces.get_mut(&ws_id).unwrap();
        let pos = state
            .columns
            .iter()
            .position(|c| Self::column_window(hub, c.container) == id);
        let Some(pos) = pos else { return };
        let was_focused = state.focused_window() == Some(id);
        let container = state.columns.remove(pos).container;
        state.drop_from_history(id);
        if was_focused && !state.columns.is_empty() {
            let target_idx = pos.min(state.columns.len() - 1);
            let target = Self::column_window(hub, state.columns[target_idx].container);
            state.record_focus(target);
        }
        hub.free_container(container);
    }
}
