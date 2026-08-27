use crate::config::{PaneConfig, Strategy, WindowMatcher};
use crate::core::PaneDisplay;
use crate::core::WindowRestrictions;
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, titled,
    titled_process,
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
        WorkspaceExport {
            strategy: "master".into(),
            ..WorkspaceExport::default()
        }
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
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "master".into(),
            master: PaneConfig::tiled(vec![WindowMatcher {
                title: Some("w0".into()),
                ..Default::default()
            }]),
            ..WorkspaceExport::default()
        }
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
    hub.insert_window(titled("AAA"), default_rect(), WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "master".into(),
            master: PaneConfig::tiled(vec![slot_matcher]),
            secondary: PaneConfig::tiled(vec![]),
            ..WorkspaceExport::default()
        }
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

    hub.insert_window(titled("AAA"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("foreign"), default_rect(), WindowRestrictions::None);

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
            strategy: "master".into(),
            master: PaneConfig::tiled(vec![slot_matcher]),
            secondary: PaneConfig::tiled(vec![WindowMatcher {
                title: Some("foreign".into()),
                ..Default::default()
            }]),
            ..WorkspaceExport::default()
        }
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
    hub.insert_window(
        titled_process("Browser A", "browser.exe"),
        default_rect(),
        WindowRestrictions::None,
    );
    hub.insert_window(
        titled_process("Browser B", "browser.exe"),
        default_rect(),
        WindowRestrictions::None,
    );

    let result = hub.export_workspace(ws_id);
    // Two windows share one slot, so the slot's matcher must be emitted once.
    assert_eq!(result.master.children.len(), 1);
    assert_eq!(
        result.master.children,
        vec![WindowMatcher {
            process: Some("browser.exe".into()),
            ..Default::default()
        }]
    );
}

#[test]
fn export_reload_restores_tabbed_pane() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .with_master_count(2)
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws = hub.current_workspace();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    // Both windows land in the master pane.
    hub.toggle_container_layout();
    let export = hub.export_workspace(ws);
    assert_eq!(export.master.display, PaneDisplay::Tabbed);

    let config = export.to_layout_workspace_config("1");
    let mut reloaded = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_strategy(Strategy::Master)
                .build(),
        )
        .with_preferred_layout(vec![config])
        .build();
    reloaded.focus_workspace("1");
    let rws = reloaded.current_workspace();
    reloaded.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    reloaded.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

    assert_eq!(
        reloaded.export_workspace(rws).master.display,
        PaneDisplay::Tabbed
    );
}
