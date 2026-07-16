use crate::config::{Strategy, WindowMatcher};
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, titled,
};

#[test]
fn export_master_empty_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "master".into(),
            master_ratio: Some(0.5),
            master_count: Some(1),
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_master_single_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled("w0"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "master".into(),
            master_ratio: Some(0.5),
            master_count: Some(1),
            master: vec![WindowMatcher {
                title: Some("w0".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_master_matched_preserves_slot_matcher() {
    let slot_matcher = WindowMatcher {
        title: Some("AAA".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master(vec![slot_matcher.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled("AAA"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "master".into(),
            master_ratio: Some(0.5),
            master_count: Some(1),
            master: vec![slot_matcher],
            secondary: vec![],
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_master_mixed_matched_and_unmatched() {
    let slot_matcher = WindowMatcher {
        title: Some("AAA".into()),
        ..Default::default()
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master(vec![slot_matcher.clone()])
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();

    hub.insert_tiling(ws_id, titled("AAA"));
    hub.insert_tiling(ws_id, titled("foreign"));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        Some(WorkspaceExport {
            strategy: "master".into(),
            master_ratio: Some(0.5),
            master_count: Some(1),
            master: vec![slot_matcher],
            secondary: vec![WindowMatcher {
                title: Some("foreign".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        })
    );
}
