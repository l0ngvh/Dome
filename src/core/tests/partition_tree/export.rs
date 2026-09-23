use insta::assert_snapshot;

use crate::core::node::WindowRestrictions;
use crate::core::tests::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, TilingConfigBuilder, default_rect,
    partition_tree_entry, snapshot, titled, titled_matcher,
};
use crate::core::{SplitMode, TreeLayoutNode, WindowMatcher};

#[test]
fn export_empty_workspace_returns_empty_export() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    let result = hub.export_workspace(ws_id);
    assert_eq!(result, partition_tree_entry(None));
}

#[test]
fn export_single_foreign_window() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        partition_tree_entry(Some(TreeLayoutNode::Leaf(WindowMatcher {
            title: Some("w0".into()),
            ..Default::default()
        })))
    );
}

#[test]
fn export_held_window_slot_writes_own_matcher() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Leaf(titled_matcher("/preferred.*/")))
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    hub.insert_window(
        titled("preferred-title"),
        default_rect(),
        WindowRestrictions::None,
    );

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        partition_tree_entry(Some(TreeLayoutNode::Leaf(titled_matcher(
            "preferred-title"
        ))))
    );
}

#[test]
fn export_foreign_container_with_two_windows() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        partition_tree_entry(Some(TreeLayoutNode::Container {
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
        }))
    );
}

#[test]
fn export_tabbed_container() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        partition_tree_entry(Some(TreeLayoutNode::Container {
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
        }))
    );
}

#[test]
fn export_nested_containers() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();

    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        partition_tree_entry(Some(TreeLayoutNode::Container {
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
        }))
    );
}

#[test]
fn export_then_reopen_window_does_not_panic() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(titled_matcher("editor")),
                        TreeLayoutNode::Leaf(titled_matcher("term")),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);
    let ws_id = hub.current_workspace();
    let editor = hub
        .insert_window(titled("editor"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.export_workspace(ws_id);

    hub.delete_window(editor);
    hub.insert_window(titled("editor"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )

    ******************************************************************************************************************************************************
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                         W1                                                                         *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    *                                                                                                                                                    *
    ******************************************************************************************************************************************************
    ");
}
