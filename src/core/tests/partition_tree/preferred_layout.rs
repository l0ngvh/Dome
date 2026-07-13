use insta::assert_snapshot;

use crate::config::{SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, setup_logger_with_level,
    snapshot, titled,
};

#[test]
fn insert_first_preferred_window_next_to_focused_window() {
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
                            title: Some("BBB".into()),
                            ..Default::default()
                        }),
                    ],
                })
                .build(),
        ])
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
        Container(id=ContainerId(3), x=75.00, y=22.50, w=75.00, h=7.50, tabbed, active_tab=0, titles=[AAA, BBB])
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
    |                                                                         ||               [AAA]                |                BBB                 |
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn insert_second_preferred_window_forming_lowest_common_ancestor() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
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
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("DDD".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("EEE".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.toggle_spawn_mode();
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.toggle_spawn_mode();
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.toggle_spawn_mode();
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=112.50, y=22.50, w=37.50, h=7.50)
        Window(id=WindowId(4), x=75.00, y=22.50, w=37.50, h=7.50, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=7.50)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w1, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w2, Container])
        Container(id=ContainerId(3), x=75.00, y=22.50, w=75.00, h=7.50, titles=[AAA, DDD])
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
    |                                    W1                                   |**************************************+-----------------------------------+
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                 W4                 *|                W3                 |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    +-------------------------------------------------------------------------+**************************************+-----------------------------------+
    ");
}

#[test]
fn insert_three_preferred_window_to_lowest_common_ancestor() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
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
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("DDD".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("EEE".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[AAA, CCC, DDD])
      )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                       W1                       |*                       W2                       *|                       W0                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn insert_nested_preferred_layout_tree() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
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
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("DDD".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("EEE".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("BBB"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("EEE"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=15.00, w=50.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, Container])
        Container(id=ContainerId(2), x=100.00, y=0.00, w=50.00, h=30.00, titles=[DDD, EEE])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[AAA, BBB])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W1                       ||                                                ||                       W0                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                |+------------------------------------------------+
    +------------------------------------------------+|                       W2                       |**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W3                       ||                                                |*                       W4                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

#[test]
fn delete_and_reinsert_the_same_matching_window() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
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
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("DDD".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("EEE".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    let w2 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("BBB"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("EEE"));

    hub.delete_window(w2);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(3), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, titles=[DDD, EEE])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=75.00, h=30.00, titles=[AAA, BBB])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W1                                   ||                                    W0                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W3                                   |*                                    W4                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    let _w5 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(5), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, Container])
        Container(id=ContainerId(2), x=100.00, y=0.00, w=50.00, h=30.00, titles=[DDD, EEE])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[AAA, BBB])
      )

    +------------------------------------------------+**************************************************+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                       W1                       |*                                                *|                       W0                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+*                                                *+------------------------------------------------+
    +------------------------------------------------+*                       W5                       *+------------------------------------------------+
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                       W3                       |*                                                *|                       W4                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn clean_up_and_reforming_preferred_contaner() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
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
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Vertical),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("DDD".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("EEE".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("BBB"));
    let w4 = hub.insert_tiling(hub.current_workspace(), titled("EEE"));

    hub.delete_window(w4);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=bottom)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[AAA, BBB])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W1                       ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------+|                                                |*                                                *
    +------------------------------------------------+|                       W2                       |*                       W0                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W3                       ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");

    let _w5 = hub.insert_tiling(hub.current_workspace(), titled("EEE"));

    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=100.00, y=15.00, w=50.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=15.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, Container])
        Container(id=ContainerId(3), x=100.00, y=0.00, w=50.00, h=30.00, titles=[DDD, EEE])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[AAA, BBB])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W1                       ||                                                ||                       W0                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                |+------------------------------------------------+
    +------------------------------------------------+|                       W2                       |**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W3                       ||                                                |*                       W5                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");
}

/// This is not really an expected behavior, more like to show that we don't guarrantee that the
/// tree will be formed when there are manual modifications to it.
#[test]
fn attach_window_after_moving_preferred_window_out_of_preferred_container_reforming_container_with_the_first_child()
 {
    setup_logger_with_level("trace");
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
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Tabbed),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("BBB".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("CCC".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("DDD".into()),
                            ..Default::default()
                        }),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    hub.toggle_spawn_mode();
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.toggle_spawn_mode();
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    hub.toggle_spawn_mode();
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("BBB"));
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
        Container(id=ContainerId(3), x=75.00, y=22.50, w=75.00, h=7.50, tabbed, active_tab=0, titles=[BBB, DDD])
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
    |                                                                         ||               [BBB]                |                DDD                 |
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    hub.move_left();

    let _w5 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=22.50, w=50.00, h=7.50)
        Window(id=WindowId(2), x=100.00, y=15.00, w=50.00, h=7.50)
        Window(id=WindowId(5), x=50.00, y=17.00, w=50.00, h=13.00, highlighted, spawn=top)
        Window(id=WindowId(1), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w1, Container, Container])
        Container(id=ContainerId(2), x=100.00, y=15.00, w=50.00, h=15.00, titles=[w2, DDD])
        Container(id=ContainerId(4), x=50.00, y=15.00, w=50.00, h=15.00, tabbed, active_tab=0, titles=[AAA, BBB])
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
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||         [AAA]          |         BBB           ||                                                |
    |                                                |**************************************************|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W2                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *+------------------------------------------------+
    |                       W1                       |*                                                *+------------------------------------------------+
    |                                                |*                       W5                       *|                                                |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W3                       |
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                                                |
    +------------------------------------------------+**************************************************+------------------------------------------------+
    ");
}

#[test]
fn move_preferred_root_to_another_workspace() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("AAA".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("DDD".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        }),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    hub.focus_parent();
    hub.move_focused_to_workspace("10");
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[DDD, CCC])
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W3                                   *|                                    W2                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************+-------------------------------------------------------------------------+
    ");
}

#[test]
fn move_preferred_container_to_another_workspace() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("AAA".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("DDD".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Horizontal),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("TTT".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("YYY".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("CCC".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("YYY"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("TTT"));
    hub.focus_parent();
    hub.move_focused_to_workspace("10");
    let _w5 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=75.00, y=20.00, w=75.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=10.00, w=75.00, h=10.00)
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=75.00, y=0.00, w=75.00, h=30.00, titles=[AAA, DDD, CCC])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W1                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                                                         |***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W5                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn reloading_preferred_layout_puts_matched_windows_to_place() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("AAA".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("DDD".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Horizontal),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("TTT".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("CCC".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("YYY".into()),
                            ..Default::default()
                        }),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("TTT"));
    let _w1 = hub.insert_tiling(hub.current_workspace(), titled("AAA"));
    let _w2 = hub.insert_tiling(hub.current_workspace(), titled("YYY"));
    let _w3 = hub.insert_tiling(hub.current_workspace(), titled("CCC"));
    let _w4 = hub.insert_tiling(hub.current_workspace(), titled("DDD"));

    hub.sync_preferred_layout(vec![
        LayoutWorkspaceConfigBuilder::new("1")
            .with_tree(TreeLayoutNode::Container {
                split: Some(SplitMode::Horizontal),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("DDD".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Horizontal),
                        children: vec![
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("YYY".into()),
                                ..Default::default()
                            }),
                            TreeLayoutNode::Container {
                                split: Some(SplitMode::Horizontal),
                                children: vec![
                                    TreeLayoutNode::Leaf(WindowMatcher {
                                        title: Some("AAA".into()),
                                        ..Default::default()
                                    }),
                                    TreeLayoutNode::Container {
                                        split: Some(SplitMode::Horizontal),
                                        children: vec![
                                            TreeLayoutNode::Leaf(WindowMatcher {
                                                title: Some("TTT".into()),
                                                ..Default::default()
                                            }),
                                            TreeLayoutNode::Leaf(WindowMatcher {
                                                title: Some("CCC".into()),
                                                ..Default::default()
                                            }),
                                        ],
                                    },
                                ],
                            },
                        ],
                    },
                ],
            })
            .build(),
    ]);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=112.50, y=22.50, w=37.50, h=7.50)
        Window(id=WindowId(0), x=112.50, y=15.00, w=37.50, h=7.50)
        Window(id=WindowId(1), x=75.00, y=15.00, w=37.50, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(4), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(4), x=0.00, y=0.00, w=150.00, h=30.00, titles=[DDD, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, titles=[YYY, Container])
        Container(id=ContainerId(5), x=75.00, y=15.00, w=75.00, h=15.00, titles=[AAA, Container])
        Container(id=ContainerId(3), x=112.50, y=15.00, w=37.50, h=15.00, titles=[TTT, CCC])
      )

    ***************************************************************************+-------------------------------------------------------------------------+
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                    W2                                   |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *+-------------------------------------------------------------------------+
    *                                    W4                                   *+------------------------------------++-----------------------------------+
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                W0                 |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    |+-----------------------------------+
    *                                                                         *|                 W1                 |+-----------------------------------+
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                W3                 |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    ***************************************************************************+------------------------------------++-----------------------------------+
    ");
}
