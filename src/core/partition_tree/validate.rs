use crate::core::hub::HubAccess;
use crate::core::node::{ContainerId, Dimension, Direction, WorkspaceId};
use crate::core::partition_tree::{Child, Container, Parent};

use super::PartitionTreeStrategy;

impl PartitionTreeStrategy {
    pub(super) fn validate_workspace_focus(
        &self,
        hub: &HubAccess,
        workspace_id: WorkspaceId,
        workspace: &crate::core::node::Workspace,
    ) {
        use crate::core::node::DisplayMode;

        if workspace.is_float_focused() {
            assert!(
                !workspace.float_windows().is_empty(),
                "Workspace {workspace_id}: is_float_focused is true but float_windows is empty"
            );
        }

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
            let root = root.unwrap_or_else(|| {
                panic!("Workspace {workspace_id}: focused_tiling is {child:?} but root is None")
            });
            // Walk the focus chain from root to a leaf to check reachability
            let mut current = root;
            for _ in crate::core::bounded_loop() {
                if current == child {
                    break;
                }
                match current {
                    Child::Window(_) => {
                        panic!(
                            "Workspace {workspace_id}: focused_tiling ({child:?}) not reachable from root ({root:?})"
                        );
                    }
                    Child::Container(cid) => current = self.containers.get(cid).focused,
                }
            }
        }

        if root.is_some() {
            assert!(
                focused_tiling.is_some(),
                "Workspace {workspace_id}: root is Some but focused_tiling is None"
            );
        }
    }

    pub(super) fn validate_container(
        &self,
        hub: &HubAccess,
        cid: ContainerId,
        expected_parent: Parent,
        workspace_id: WorkspaceId,
        stack: &mut Vec<(Child, Parent)>,
    ) {
        let container = self.containers.get(cid);
        assert_eq!(
            container.parent, expected_parent,
            "Container {cid} has wrong parent"
        );
        assert_eq!(
            container.workspace, workspace_id,
            "Container {cid} has wrong workspace"
        );
        assert!(
            container.children.len() >= 2,
            "Container {cid} has less than 2 children"
        );

        if let Child::Window(wid) = container.focused {
            assert!(
                !hub.windows.get(wid).is_float(),
                "Container {cid} focused on float {wid}"
            );
        }

        self.validate_container_tabbed(cid, container);
        self.validate_container_direction(cid, container, expected_parent);
        self.validate_container_dimensions(hub, cid, container);
        self.validate_container_focus(hub, cid, container);

        for &c in container.children() {
            stack.push((c, Parent::Container(cid)));
        }
    }

    fn validate_container_tabbed(&self, cid: ContainerId, container: &Container) {
        if !container.is_tabbed() {
            return;
        }
        assert!(
            container.active_tab_index() < container.children().len(),
            "Container {cid} active_tab out of bounds"
        );
        let active_tab = container.children()[container.active_tab_index()];
        let expected_focus = match active_tab {
            Child::Window(_) => active_tab,
            Child::Container(child_cid) => self.containers.get(child_cid).focused,
        };
        assert!(
            container.focused == expected_focus || container.focused == active_tab,
            "Container {cid} focused {:?} doesn't match active_tab {:?} or its focused {:?}",
            container.focused,
            active_tab,
            expected_focus
        );
    }

    fn validate_container_direction(
        &self,
        cid: ContainerId,
        container: &Container,
        expected_parent: Parent,
    ) {
        if let Parent::Container(parent_cid) = expected_parent
            && let Some(parent_dir) = self.containers.get(parent_cid).direction()
            && let Some(child_dir) = container.direction()
        {
            assert_ne!(
                parent_dir, child_dir,
                "Container {cid} has same direction as parent {parent_cid}"
            );
        }
    }

    fn child_constraints(
        &self,
        hub: &HubAccess,
        child: Child,
    ) -> (Dimension, (f32, f32), (f32, f32)) {
        match child {
            Child::Window(wid) => {
                let dim = self.tiling_data(wid).dimension;
                let w = hub.windows.get(wid);
                (dim, w.min_size(), w.max_size())
            }
            Child::Container(cid) => {
                let c = self.containers.get(cid);
                (c.dimension, c.min_size(), (0.0, 0.0))
            }
        }
    }

    fn validate_container_dimensions(
        &self,
        hub: &HubAccess,
        cid: ContainerId,
        container: &Container,
    ) {
        let dim = container.dimension;
        let children = container.children();
        let constraints: Vec<_> = children
            .iter()
            .map(|&c| self.child_constraints(hub, c))
            .collect();

        match container.direction() {
            Some(dir) => {
                let (split_label, split_limit) = match dir {
                    Direction::Horizontal => ("width", dim.width),
                    Direction::Vertical => ("height", dim.height),
                };
                let split_sum: f32 = match dir {
                    Direction::Horizontal => constraints.iter().map(|(d, _, _)| d.width).sum(),
                    Direction::Vertical => constraints.iter().map(|(d, _, _)| d.height).sum(),
                };
                assert!(
                    split_sum <= split_limit + 0.01,
                    "Container {cid} children total {split_label} {split_sum:.2} > container {split_label} {split_limit:.2}",
                );

                for (i, (child_dim, child_min, child_max)) in constraints.iter().enumerate() {
                    let (cross_child, cross_container, cross_min, cross_max, label) = match dir {
                        Direction::Horizontal => (
                            child_dim.height,
                            dim.height,
                            child_min.1,
                            child_max.1,
                            "height",
                        ),
                        Direction::Vertical => (
                            child_dim.width,
                            dim.width,
                            child_min.0,
                            child_max.0,
                            "width",
                        ),
                    };
                    let allows_smaller = cross_max > 0.0 && cross_max < cross_container;
                    assert!(
                        cross_child >= cross_container - 0.01
                            || cross_child >= cross_min - 0.01
                            || allows_smaller,
                        "Container {cid} child {i} {label} {cross_child:.2} < container {label} {cross_container:.2} and < min_{label} {cross_min:.2}",
                    );
                }
            }
            None => {
                let expected_height = dim.height - hub.config.tab_bar_height;
                for (i, (child_dim, _, child_max)) in constraints.iter().enumerate() {
                    let allows_smaller_w = child_max.0 > 0.0 && child_max.0 < dim.width;
                    let allows_smaller_h = child_max.1 > 0.0 && child_max.1 < expected_height;
                    assert!(
                        (child_dim.width - dim.width).abs() < 0.01 || allows_smaller_w,
                        "Container {cid} tabbed child {i} width {:.2} != container width {:.2}",
                        child_dim.width,
                        dim.width
                    );
                    assert!(
                        (child_dim.height - expected_height).abs() < 0.01 || allows_smaller_h,
                        "Container {cid} tabbed child {i} height {:.2} != expected {:.2}",
                        child_dim.height,
                        expected_height
                    );
                }
            }
        }

        let (min_w, min_h) = container.min_size();
        assert!(
            dim.width >= min_w - 0.01,
            "Container {cid} width {:.2} < min_width {:.2}",
            dim.width,
            min_w
        );
        assert!(
            dim.height >= min_h - 0.01,
            "Container {cid} height {:.2} < min_height {:.2}",
            dim.height,
            min_h
        );
    }

    fn validate_container_focus(&self, hub: &HubAccess, cid: ContainerId, container: &Container) {
        let focused = container.focused;
        let is_direct_child = container.children().contains(&focused);
        let matches_child_focus = container.children().iter().any(|&c| {
            matches!(c, Child::Container(child_cid) if self.containers.get(child_cid).focused == focused)
        });
        assert!(
            is_direct_child || matches_child_focus,
            "Container {cid} focus {focused:?} is neither a direct child nor matches a child's focus"
        );
        self.validate_child_exists(hub, focused);
    }

    fn validate_child_exists(&self, hub: &HubAccess, child: Child) {
        match child {
            Child::Window(wid) => {
                hub.windows.get(wid);
            }
            Child::Container(cid) => {
                self.containers.get(cid);
            }
        }
    }

    pub(super) fn validate_window(
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

        let td = self.tiling_data(wid);
        assert_eq!(td.parent, expected_parent, "Window {wid} has wrong parent");
        assert_eq!(
            window.workspace, workspace_id,
            "Window {wid} has wrong workspace"
        );

        let dim = td.dimension;
        let (min_w, min_h) = window.min_size();
        let (max_w, max_h) = window.max_size();

        assert!(
            dim.width >= min_w - 0.01,
            "Window {wid} width {:.2} < min_width {:.2}",
            dim.width,
            min_w
        );
        assert!(
            dim.height >= min_h - 0.01,
            "Window {wid} height {:.2} < min_height {:.2}",
            dim.height,
            min_h
        );

        if max_w > 0.0 {
            assert!(
                dim.width <= max_w + 0.01,
                "Window {wid} width {:.2} > max_width {:.2}",
                dim.width,
                max_w
            );
            assert!(
                max_w >= min_w,
                "Window {wid} max_width {:.2} < min_width {:.2}",
                max_w,
                min_w
            );
        }
        if max_h > 0.0 {
            assert!(
                dim.height <= max_h + 0.01,
                "Window {wid} height {:.2} > max_height {:.2}",
                dim.height,
                max_h
            );
            assert!(
                max_h >= min_h,
                "Window {wid} max_height {:.2} < min_height {:.2}",
                max_h,
                min_h
            );
        }
    }
}
