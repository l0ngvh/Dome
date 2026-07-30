use crate::config::{Strategy, WindowMatcher};
use crate::core::WindowMetadata;
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, TestMetadata, titled,
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
            master: vec![slot_matcher],
            secondary: vec![WindowMatcher {
                title: Some("foreign".into()),
                ..Default::default()
            }],
            ..WorkspaceExport::default()
        })
    );
}

#[test]
fn export_two_windows_one_slot_emits_single_matcher() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master(vec![WindowMatcher {
                    process: Some("browser.exe".into()),
                    ..Default::default()
                }])
                .with_master_count(2)
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws_id = hub.current_workspace();
    hub.insert_tiling(ws_id, titled_process("Browser A", "browser.exe"));
    hub.insert_tiling(ws_id, titled_process("Browser B", "browser.exe"));

    let result = hub.export_workspace(ws_id).unwrap();
    // Two windows share one slot, so the slot's matcher must be emitted once.
    assert_eq!(result.master.len(), 1);
    assert_eq!(
        result.master,
        vec![WindowMatcher {
            process: Some("browser.exe".into()),
            ..Default::default()
        }]
    );
}

fn titled_process(title: &str, process: &str) -> Box<dyn WindowMetadata> {
    Box::new(TestMetadata {
        title: Some(title.into()),
        process: Some(process.into()),
    })
}
