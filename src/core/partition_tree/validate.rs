use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Direction, Length, WindowId, WorkspaceId};
use crate::core::partition_tree::{Child, Parent};
use crate::core::strategy::{VALIDATION_TOLERANCE, ValidateStrategy};

use rustc_hash::FxHashSet;

use super::PartitionTreeStrategy;

impl ValidateStrategy for PartitionTreeStrategy {
    fn validate(&self, hub: &HubAccess) -> FxHashSet<ContainerId> {
        let mut reachable: FxHashSet<ContainerId> = FxHashSet::default();
        for workspace_id in hub.workspaces.sorted_ids() {
            let root = self.workspaces.get(&workspace_id).and_then(|s| s.root);
            let mut tree_windows: FxHashSet<WindowId> = FxHashSet::default();
            if let Some(root) = root {
                // Hand-rolled DFS kept because the walk threads expected_parent
                // derived from the traversal structure. Using children_dfs plus
                // parent would check the parent field against itself.
                let mut stack = vec![(root, Parent::Workspace(workspace_id))];
                for _ in crate::core::bounded_loop() {
                    let Some((child, expected_parent)) = stack.pop() else {
                        break;
                    };
                    match child {
                        Child::Window(wid) => {
                            tree_windows.insert(wid);
                            self.validate_window(hub, wid, expected_parent, workspace_id)
                        }
                        Child::Container(cid) => {
                            reachable.insert(cid);
                            self.validate_container(
                                hub,
                                cid,
                                expected_parent,
                                workspace_id,
                                &mut stack,
                            );
                        }
                    }
                }
            }
            self.validate_workspace_focus(hub, workspace_id, &tree_windows);
        }
        self.validate_container_arena(&reachable);
        reachable
    }
}

impl PartitionTreeStrategy {
    /// Validate workspace focus invariants:
    /// - `is_float_focused` is false when `float_windows` is empty
    /// - `focused_tiling` points to a tiling-mode window (not float/fullscreen)
    /// - `root.is_some()` implies `focused_tiling.is_some()`
    /// - `focus_history` is a permutation of the workspace's tiling windows
    /// - a focused window is the front of `focus_history`
    /// - every tabbed ancestor of `focused_tiling` has it as the active tab
    fn validate_workspace_focus(
        &self,
        hub: &HubAccess,
        workspace_id: WorkspaceId,
        tree_windows: &FxHashSet<WindowId>,
    ) {
        use crate::core::node::DisplayMode;

        let focused_tiling = self
            .workspaces
            .get(&workspace_id)
            .and_then(|s| s.focused_tiling);
        let root = self.workspaces.get(&workspace_id).and_then(|s| s.root);

        if let Some(Child::Window(wid)) = focused_tiling {
            assert_eq!(
                hub.windows.get(wid).mode,
                DisplayMode::Tiling,
                "Workspace {workspace_id}: focused_tiling points to non-tiling window {wid}"
            );
        }

        if let Some(child) = focused_tiling {
            assert!(
                root.is_some(),
                "Workspace {workspace_id}: focused_tiling is {child:?} but root is None"
            );
        }

        if root.is_some() {
            assert!(
                focused_tiling.is_some(),
                "Workspace {workspace_id}: root is Some but focused_tiling is None"
            );
        }

        self.validate_focus_history(workspace_id, tree_windows);

        if let Some(Child::Window(wid)) = focused_tiling {
            assert_eq!(
                self.workspaces
                    .get(&workspace_id)
                    .unwrap()
                    .focus_history
                    .first(),
                Some(&wid),
                "Workspace {workspace_id}: focused window {wid} is not the front of focus_history"
            );
        }

        if let Some(focused) = focused_tiling {
            for (child, parent_id) in self.ancestors_of(focused) {
                if self.tiling_containers.get(&parent_id).unwrap().is_tabbed() {
                    assert_eq!(
                        self.active_tab(hub, parent_id),
                        Some(child),
                        "Workspace {workspace_id}: tabbed container {parent_id} holds the focus \
                         path in a hidden tab, so the focused node is never drawn"
                    );
                }
            }
        }
    }

    fn validate_focus_history(
        &self,
        workspace_id: WorkspaceId,
        tree_windows: &FxHashSet<WindowId>,
    ) {
        let Some(state) = self.workspaces.get(&workspace_id) else {
            return;
        };

        assert_eq!(
            state.focus_history.len(),
            tree_windows.len(),
            "Workspace {workspace_id}: focus_history has {} entries for {} tiling windows, \
             so it holds a duplicate or a stale window",
            state.focus_history.len(),
            tree_windows.len()
        );
        let history_seen: FxHashSet<WindowId> = state.focus_history.iter().copied().collect();
        assert_eq!(
            &history_seen, tree_windows,
            "Workspace {workspace_id}: focus_history does not match the tiling windows in the tree"
        );
    }

    /// Every container that carries tiling state must be reachable from a workspace root, or
    /// that state leaked.
    fn validate_container_arena(&self, reachable: &FxHashSet<ContainerId>) {
        let with_state: FxHashSet<ContainerId> = self.tiling_containers.keys().copied().collect();
        assert_eq!(
            sorted_difference(&with_state, reachable),
            Vec::new(),
            "Containers holding tiling state but reachable from no workspace root, so their \
             state leaked"
        );
    }

    fn validate_container(
        &self,
        hub: &HubAccess,
        cid: ContainerId,
        expected_parent: Parent,
        workspace_id: WorkspaceId,
        stack: &mut Vec<(Child, Parent)>,
    ) {
        let container = hub.containers.get(cid);
        let data = self.tiling_containers.get(&cid).unwrap();
        assert_eq!(
            data.parent, expected_parent,
            "Container {cid} has wrong parent"
        );
        assert_eq!(
            data.workspace, workspace_id,
            "Container {cid} has wrong workspace"
        );
        assert!(
            container.children.len() >= 2,
            "Container {cid} has less than 2 children"
        );

        self.validate_container_tabbed(hub, cid);
        self.validate_container_direction(cid, expected_parent);
        self.validate_container_dimensions(hub, cid);

        for &c in container.children() {
            stack.push((c, Parent::Container(cid)));
        }
    }

    fn validate_container_tabbed(&self, hub: &HubAccess, cid: ContainerId) {
        let data = self.tiling_containers.get(&cid).unwrap();
        if !data.is_tabbed() {
            return;
        }
        assert!(
            data.active_tab_index() < hub.containers.get(cid).children().len(),
            "Container {cid} active_tab out of bounds"
        );
    }

    fn validate_container_direction(&self, cid: ContainerId, expected_parent: Parent) {
        if let Parent::Container(parent_cid) = expected_parent
            && let Some(parent_dir) = self.tiling_containers.get(&parent_cid).unwrap().direction()
            && let Some(child_dir) = self.tiling_containers.get(&cid).unwrap().direction()
        {
            assert_ne!(
                parent_dir, child_dir,
                "Container {cid} has same direction as parent {parent_cid}"
            );
        }
    }

    fn validate_container_dimensions(&self, hub: &HubAccess, cid: ContainerId) {
        let data = self.tiling_containers.get(&cid).unwrap();
        let dim = data.dimension;
        let children = hub.containers.get(cid).children();
        let child_dims: Vec<Dimension> =
            children.iter().map(|&c| self.child_dimension(c)).collect();

        match data.direction() {
            Some(dir) => {
                let (split_label, split_extent) = match dir {
                    Direction::Horizontal => ("width", dim.width.value()),
                    Direction::Vertical => ("height", dim.height.value()),
                };
                let split_sum: f32 = match dir {
                    Direction::Horizontal => child_dims.iter().map(|d| d.width.value()).sum(),
                    Direction::Vertical => child_dims.iter().map(|d| d.height.value()).sum(),
                };
                assert!(
                    (split_sum - split_extent).abs() <= VALIDATION_TOLERANCE.value(),
                    "Container {cid} children total {split_label} {split_sum:.2} != container {split_label} {split_extent:.2}",
                );

                for (i, child_dim) in child_dims.iter().enumerate() {
                    let (cross_child, cross_container, label) = match dir {
                        Direction::Horizontal => {
                            (child_dim.height.value(), dim.height.value(), "height")
                        }
                        Direction::Vertical => {
                            (child_dim.width.value(), dim.width.value(), "width")
                        }
                    };
                    assert!(
                        (cross_child - cross_container).abs() <= VALIDATION_TOLERANCE.value(),
                        "Container {cid} child {i} {label} {cross_child:.2} != container {label} {cross_container:.2}",
                    );
                }
            }
            None => {
                let scale = hub
                    .monitors
                    .get(hub.workspaces.get(data.workspace).monitor)
                    .scale;
                let expected_height = (dim.height - self.tab_bar_length(scale)).max(Length::ZERO);
                for (i, child_dim) in child_dims.iter().enumerate() {
                    assert!(
                        (child_dim.width - dim.width).abs() < VALIDATION_TOLERANCE,
                        "Container {cid} tabbed child {i} width {:.2} != container width {:.2}",
                        child_dim.width.value(),
                        dim.width.value()
                    );
                    assert!(
                        (child_dim.height - expected_height).abs() < VALIDATION_TOLERANCE,
                        "Container {cid} tabbed child {i} height {:.2} != expected {:.2}",
                        child_dim.height.value(),
                        expected_height.value()
                    );
                }
            }
        }
    }

    fn validate_window(
        &self,
        hub: &HubAccess,
        wid: crate::core::node::WindowId,
        expected_parent: Parent,
        workspace_id: WorkspaceId,
    ) {
        let window = hub.windows.get(wid);
        assert!(!window.is_float(), "Window {wid} in tree but mode is Float");
        assert!(
            !window.is_fullscreen(),
            "Window {wid} in tree but mode is Fullscreen"
        );
        assert!(
            !window.is_minimized(),
            "Window {wid} in tree but mode is Minimized"
        );

        assert_eq!(
            self.tiling_windows.get(&wid).unwrap().parent,
            expected_parent,
            "Window {wid} has wrong parent"
        );
        assert_eq!(
            window.workspace(),
            Some(workspace_id),
            "Window {wid} has wrong workspace"
        );

        let dim = self.tiling_windows.get(&wid).unwrap().dimension;
        let usable = hub
            .monitors
            .get(hub.workspaces.get(workspace_id).monitor)
            .work_area
            .to_dimension();
        let tol = VALIDATION_TOLERANCE;
        assert!(
            dim.x >= usable.x - tol,
            "Window {wid} x {:.2} left of usable area {:.2}",
            dim.x.value(),
            usable.x.value()
        );
        assert!(
            dim.y >= usable.y - tol,
            "Window {wid} y {:.2} above usable area {:.2}",
            dim.y.value(),
            usable.y.value()
        );
        assert!(
            dim.x + dim.width <= usable.x + usable.width + tol,
            "Window {wid} right {:.2} past usable area right {:.2}",
            (dim.x + dim.width).value(),
            (usable.x + usable.width).value()
        );
        assert!(
            dim.y + dim.height <= usable.y + usable.height + tol,
            "Window {wid} bottom {:.2} past usable area bottom {:.2}",
            (dim.y + dim.height).value(),
            (usable.y + usable.height).value()
        );

        // window_constraints caps min by max, so an inverted stored pair is only
        // observable before that cap runs.
        let stored = window.limits();
        if let (Some(min), Some(max)) = (stored.min_width, stored.max_width) {
            assert!(
                max >= min,
                "Window {wid} stored max_width {max} < min_width {min}"
            );
        }
        if let (Some(min), Some(max)) = (stored.min_height, stored.max_height) {
            assert!(
                max >= min,
                "Window {wid} stored max_height {max} < min_height {min}"
            );
        }
    }
}

fn sorted_difference(
    from: &FxHashSet<ContainerId>,
    minus: &FxHashSet<ContainerId>,
) -> Vec<ContainerId> {
    let mut extra: Vec<ContainerId> = from.difference(minus).copied().collect();
    extra.sort_unstable();
    extra
}
