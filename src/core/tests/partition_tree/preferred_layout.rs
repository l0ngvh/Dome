use insta::assert_snapshot;

use crate::config::{SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, snapshot, titled,
};

#[test]
fn insert_first_preferred_window_next_to_focused_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(
            LayoutConfigBuilder::new()
                .with_workspace(vec![
                    LayoutWorkspaceConfigBuilder::new("1")
                        .with_tree(TreeLayoutNode::Container {
                            split: Some(SplitMode::Tabbed),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("AAA".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("BBB".into()),
                                    ..Default::default()
                                }),
                            ],
                        })
                        .build(),
                ])
                .build(),
        )
        .build();
    hub.focus_workspace("1");

    hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.toggle_spawn_mode();
    hub.insert_tiling(hub.current_workspace(), titled("BBB"));
    hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=24.50, w=75.00, h=5.50, highlighted, spawn=top)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=7.50)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w1, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w2, Container])
        Container(id=ContainerId(3), x=75.00, y=22.50, w=75.00, h=7.50, tabbed, active_tab=1, titles=[BBB, AAA])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W1                                   |+-------------------------------------------------------------------------+
    |                                                                         ||                BBB                 |               [AAA]                |
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}
