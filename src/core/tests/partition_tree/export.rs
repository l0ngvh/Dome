use crate::config::{SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, titled,
};

#[test]
fn export_empty_workspace_returns_none() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();

    let result = hub.export_workspace(ws_id);
    assert!(result.is_none());
}

#[test]
fn export_single_foreign_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled("w0"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Leaf(WindowMatcher {
                title: Some("w0".into()),
                ..Default::default()
            })),
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_occupied_window_slot_uses_slot_matcher() {
    let slot_matcher = WindowMatcher {
        title: Some("preferred-title".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Leaf(slot_matcher.clone()))
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();

    hub.insert_tiling(ws_id, titled("preferred-title"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Leaf(slot_matcher)),
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_foreign_container_with_two_windows() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled("w0"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(ws_id, titled("w1"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Vertical),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("w0".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("w1".into()),
                        ..Default::default()
                    }),
                ],
            }),
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_tabbed_container() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled("w0"));
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling(ws_id, titled("w1"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Tabbed),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("w0".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("w1".into()),
                        ..Default::default()
                    }),
                ],
            }),
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_nested_containers() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();

    hub.insert_tiling(ws_id, titled("w0"));
    hub.toggle_spawn_mode();
    hub.toggle_spawn_mode();
    hub.insert_tiling(ws_id, titled("w1"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(ws_id, titled("w2"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Tabbed),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("w0".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Horizontal),
                        children: vec![
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("w1".into()),
                                ..Default::default()
                            }),
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("w2".into()),
                                ..Default::default()
                            }),
                        ],
                    },
                ],
            }),
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_mixed_occupied_and_foreign() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Tabbed),
                    children: vec![
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("AAA".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("/B.*/".into()),
                            ..Default::default()
                        }),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled("w0"));
    hub.insert_tiling(ws_id, titled("BBB"));
    hub.insert_tiling(ws_id, titled("AAA"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Horizontal),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("w0".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Tabbed),
                        children: vec![
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("AAA".into()),
                                ..Default::default()
                            }),
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("/B.*/".into()),
                                ..Default::default()
                            }),
                        ],
                    },
                ],
            }),
            ..WorkspaceExport::default()
        })
    );
}
