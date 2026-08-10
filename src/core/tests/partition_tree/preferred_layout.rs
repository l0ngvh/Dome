use insta::assert_snapshot;

use crate::config::{SplitMode, TreeLayoutNode, WindowMatcher};
use crate::core::strategy::WorkspaceExport;
use crate::core::tests::{
    LayoutConfigBuilder, LayoutWorkspaceConfigBuilder, TestHubBuilder, default_dim,
    setup_logger_with_level, snapshot, titled,
};
use crate::core::{Dimension, Length, WindowRestrictions};

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

    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w1"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w2"), default_dim(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("BBB"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("AAA"), default_dim(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=25.00, w=75.00, h=5.00, highlighted, spawn=top)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=8.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w1, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w2, Container])
        Container(id=ContainerId(3), x=75.00, y=23.00, w=75.00, h=7.00, tabbed, active_tab=0, titles=[AAA, BBB])
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
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
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

    let _w0 = hub
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w1 = hub
        .insert_window(titled("w1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w2 = hub
        .insert_window(titled("w2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w3 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=113.00, y=23.00, w=37.00, h=7.00)
        Window(id=WindowId(4), x=75.00, y=23.00, w=38.00, h=7.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=8.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w1, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w2, Container])
        Container(id=ContainerId(3), x=75.00, y=23.00, w=75.00, h=7.00, titles=[AAA, DDD])
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
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                 W4                 *|                 W3                |
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

    let _w0 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
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

    let _w0 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("EEE"), default_dim(), WindowRestrictions::None)
        .unwrap();
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

    let _w0 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("EEE"), default_dim(), WindowRestrictions::None)
        .unwrap();

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

    let _w5 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();

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

    let _w0 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("EEE"), default_dim(), WindowRestrictions::None)
        .unwrap();

    hub.delete_window(w4);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=15.00, w=50.00, h=15.00, highlighted, spawn=bottom)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD])
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
    |                       W1                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    **************************************************|                       W2                       ||                       W0                       |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                       W3                       *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    **************************************************+------------------------------------------------++------------------------------------------------+
    ");

    let _w5 = hub
        .insert_window(titled("EEE"), default_dim(), WindowRestrictions::None)
        .unwrap();

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

    let _w0 = hub
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w1 = hub
        .insert_window(titled("w1"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w2 = hub
        .insert_window(titled("w2"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w3 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=25.00, w=75.00, h=5.00, highlighted, spawn=top)
        Window(id=WindowId(2), x=75.00, y=15.00, w=75.00, h=8.00)
        Window(id=WindowId(1), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, Container])
        Container(id=ContainerId(1), x=0.00, y=15.00, w=150.00, h=15.00, titles=[w1, Container])
        Container(id=ContainerId(2), x=75.00, y=15.00, w=75.00, h=15.00, titles=[w2, Container])
        Container(id=ContainerId(3), x=75.00, y=23.00, w=75.00, h=7.00, tabbed, active_tab=0, titles=[BBB, DDD])
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
    |                                                                         |*                                                                         *
    |                                                                         |*                                    W4                                   *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    hub.move_left();

    let _w5 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=23.00, w=50.00, h=7.00)
        Window(id=WindowId(2), x=100.00, y=15.00, w=50.00, h=8.00)
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
    |                                                |*                                                *|                                                |
    |                                                |*                                                *|                       W3                       |
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

    let _w0 = hub
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.focus_parent();
    hub.move_focused_to_workspace("10");
    let _w2 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
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

    let _w0 = hub
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("YYY"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("TTT"), default_dim(), WindowRestrictions::None)
        .unwrap();
    hub.focus_parent();
    hub.move_focused_to_workspace("10");
    let _w5 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
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

    let _w0 = hub
        .insert_window(titled("TTT"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("YYY"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();

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
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=113.00, y=23.00, w=37.00, h=7.00)
        Window(id=WindowId(0), x=113.00, y=15.00, w=37.00, h=8.00)
        Window(id=WindowId(1), x=75.00, y=15.00, w=38.00, h=15.00)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(4), x=0.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(4), x=0.00, y=0.00, w=150.00, h=30.00, titles=[DDD, Container])
        Container(id=ContainerId(2), x=75.00, y=0.00, w=75.00, h=30.00, titles=[YYY, Container])
        Container(id=ContainerId(5), x=75.00, y=15.00, w=75.00, h=15.00, titles=[AAA, Container])
        Container(id=ContainerId(3), x=113.00, y=15.00, w=37.00, h=15.00, titles=[TTT, CCC])
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
    *                                                                         *|                                    ||                 W0                |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    |+-----------------------------------+
    *                                                                         *|                 W1                 |+-----------------------------------+
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                                   |
    *                                                                         *|                                    ||                 W3                |
    *                                                                         *|                                    ||                                   |
    ***************************************************************************+------------------------------------++-----------------------------------+
    ");
}

#[test]
fn reset_to_empty_preferred_layout_dont_disturb_layout() {
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
    hub.insert_window(titled("w0"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("BBB"), default_dim(), WindowRestrictions::None);
    hub.insert_window(titled("AAA"), default_dim(), WindowRestrictions::None);

    let hub_snapshot = snapshot(&hub);
    hub.sync_preferred_layout(vec![]);
    assert_eq!(hub_snapshot, snapshot(&hub));

    let result = hub.export_workspace(ws_id);
    assert_eq!(
        result,
        WorkspaceExport {
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
        }
    );
}

#[test]
fn insert_preferred_window_to_non_focused_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("10")
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

    hub.insert_window(
        titled("BBB"),
        Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(800.0),
            Length::new(600.0),
        ),
        WindowRestrictions::None,
    );
    hub.insert_window(
        titled("AAA"),
        Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(800.0),
            Length::new(600.0),
        ),
        WindowRestrictions::None,
    );

    let prev_snapshot = snapshot(&hub);

    assert_snapshot!(prev_snapshot, @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=top)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=0, titles=[AAA, BBB])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                  [AAA]                                   |                                  BBB                                    |
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
    ******************************************************************************************************************************************************
    ");

    hub.focus_workspace("10");
    assert_eq!(prev_snapshot, snapshot(&hub));
}

#[test]
fn insert_same_slot_windows_as_sibling() {
    setup_logger_with_level("trace");
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
                                    title: Some("/A.*/".into()),
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

    let w0 = hub
        .insert_window(titled("ABC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("DDD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let w5 = hub
        .insert_window(titled("ACD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=23.00, w=50.00, h=7.00)
        Window(id=WindowId(5), x=0.00, y=15.00, w=50.00, h=8.00, highlighted, spawn=bottom)
        Window(id=WindowId(3), x=0.00, y=8.00, w=50.00, h=7.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=8.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[ABC, AAA, ACD, BBB])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W3                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    **************************************************|                       W1                       ||                       W4                       |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                       W5                       *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    **************************************************|                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W2                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ");

    hub.delete_window(w0);
    let w6 = hub
        .insert_window(titled("ADE"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(6))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=23.00, w=50.00, h=7.00)
        Window(id=WindowId(6), x=0.00, y=15.00, w=50.00, h=8.00, highlighted, spawn=bottom)
        Window(id=WindowId(5), x=0.00, y=8.00, w=50.00, h=7.00)
        Window(id=WindowId(3), x=0.00, y=0.00, w=50.00, h=8.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[AAA, ACD, ADE, BBB])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W3                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W5                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    **************************************************|                       W1                       ||                       W4                       |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                       W6                       *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    **************************************************|                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W2                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ");
    hub.delete_window(w3);
    hub.delete_window(w5);
    hub.delete_window(w6);
    let _w7 = hub
        .insert_window(titled("AEF"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w8 = hub
        .insert_window(titled("AFG"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(8))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=20.00, w=50.00, h=10.00)
        Window(id=WindowId(8), x=0.00, y=10.00, w=50.00, h=10.00, highlighted, spawn=bottom)
        Window(id=WindowId(7), x=0.00, y=0.00, w=50.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD])
        Container(id=ContainerId(2), x=0.00, y=0.00, w=50.00, h=30.00, titles=[AEF, AFG, BBB])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W7                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    **************************************************|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                       W8                       *|                       W1                       ||                       W4                       |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    **************************************************|                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W2                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ");
}

#[test]
fn same_slot_windows_share_container_with_other_window_slot_under_same_preferred_container() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("/A.*/".into()),
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

    let _w0 = hub
        .insert_window(titled("BCD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("ABC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("BEF"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w5 = hub
        .insert_window(titled("ACD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00)
        Window(id=WindowId(4), x=0.00, y=24.00, w=75.00, h=6.00)
        Window(id=WindowId(2), x=0.00, y=18.00, w=75.00, h=6.00)
        Window(id=WindowId(0), x=0.00, y=12.00, w=75.00, h=6.00)
        Window(id=WindowId(5), x=0.00, y=6.00, w=75.00, h=6.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=0.00, y=0.00, w=75.00, h=6.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=75.00, h=30.00, titles=[ABC, ACD, BCD, BBB, BEF])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W3                                   ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    ***************************************************************************|                                                                         |
    *                                                                         *|                                                                         |
    *                                                                         *|                                                                         |
    *                                    W5                                   *|                                                                         |
    *                                                                         *|                                                                         |
    ***************************************************************************|                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W0                                   ||                                    W1                                   |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W2                                   ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    +-------------------------------------------------------------------------+|                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                    W4                                   ||                                                                         |
    |                                                                         ||                                                                         |
    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    ");

    let export = hub.export_workspace(hub.current_workspace());
    assert_eq!(
        export,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Horizontal),
                children: vec![
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Vertical),
                        children: vec![
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("/A.*/".into()),
                                ..Default::default()
                            }),
                            TreeLayoutNode::Leaf(WindowMatcher {
                                title: Some("/B.*/".into()),
                                ..Default::default()
                            })
                        ],
                    },
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("CCC".into()),
                        ..Default::default()
                    }),
                ],
            }),
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn single_window_slot_in_container_slot() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("/A.*/".into()),
                        ..Default::default()
                    })],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub
        .insert_window(titled("ABC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("ACD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(4), x=0.00, y=20.00, w=50.00, h=10.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=0.00, y=10.00, w=50.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, BBB])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[ABC, AAA, ACD])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W3                       ||                       W1                       ||                       W2                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------+|                                                ||                                                |
    **************************************************|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                       W4                       *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    **************************************************+------------------------------------------------++------------------------------------------------+
    ");

    let export = hub.export_workspace(hub.current_workspace());
    assert_eq!(
        export,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Horizontal),
                children: vec![
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Vertical),
                        children: vec![TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("/A.*/".into()),
                            ..Default::default()
                        })],
                    },
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("CCC".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("BBB".into()),
                        ..Default::default()
                    })
                ],
            }),
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn bare_window_slot() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("/A.*/".into()),
                    ..Default::default()
                }))
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub
        .insert_window(titled("ABC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("ACD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=113.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=38.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=38.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=38.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[ABC, AAA, ACD, BBB])
      )

    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                 W0                 ||                 W1                |*                 W3                 *|                 W2                |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    |                                    ||                                   |*                                    *|                                   |
    +------------------------------------++-----------------------------------+**************************************+-----------------------------------+
    ");

    let export = hub.export_workspace(hub.current_workspace());
    assert_eq!(
        export,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Horizontal),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("/A.*/".into()),
                        ..Default::default()
                    }),
                    TreeLayoutNode::Leaf(WindowMatcher {
                        title: Some("BBB".into()),
                        ..Default::default()
                    })
                ],
            }),
            ..WorkspaceExport::default()
        }
    );
}

#[test]
fn sync_preferred_layout_preserves_siblings_order() {
    setup_logger_with_level("trace");
    let layout = vec![
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
                                title: Some("/D.*/".into()),
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
    ];
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(layout.clone())
        .build();
    hub.focus_workspace("1");

    let _w0 = hub
        .insert_window(titled("EEE"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("DEF"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("DGH"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w5 = hub
        .insert_window(titled("DHJ"), default_dim(), WindowRestrictions::None)
        .unwrap();

    let hub_snapshot = snapshot(&hub);
    assert_snapshot!(hub_snapshot, @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=100.00, y=23.00, w=50.00, h=7.00)
        Window(id=WindowId(5), x=100.00, y=15.00, w=50.00, h=8.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=100.00, y=8.00, w=50.00, h=7.00)
        Window(id=WindowId(1), x=100.00, y=0.00, w=50.00, h=8.00)
        Window(id=WindowId(4), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[AAA, CCC, Container])
        Container(id=ContainerId(0), x=100.00, y=0.00, w=50.00, h=30.00, titles=[DEF, DGH, DHJ, EEE])
      )

    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W1                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W3                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                |+------------------------------------------------+
    |                       W2                       ||                       W4                       |**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                       W5                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |**************************************************
    |                                                ||                                                |+------------------------------------------------+
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                       W0                       |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ");
    hub.sync_preferred_layout(layout);
    let new_snapshot = snapshot(&hub);
    assert_eq!(hub_snapshot, new_snapshot);
}

#[test]
fn export_container_with_single_multi_matched_slot() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Tabbed),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("/A.*/".into()),
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
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    let _w0 = hub
        .insert_window(titled("w0"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("ABC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("ACD"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let export = hub.export_workspace(hub.current_workspace());

    assert_eq!(
        export,
        WorkspaceExport {
            strategy: "partition_tree".into(),
            tree: Some(TreeLayoutNode::Container {
                split: Some(SplitMode::Horizontal),
                children: vec![
                    TreeLayoutNode::Leaf(WindowMatcher {
                        app: None,
                        bundle_id: None,
                        title: Some("w0".into()),
                        process: None,
                        class: None,
                        aumid: None
                    }),
                    TreeLayoutNode::Container {
                        split: Some(SplitMode::Vertical),
                        children: vec![
                            TreeLayoutNode::Container {
                                split: Some(SplitMode::Tabbed),
                                children: vec![TreeLayoutNode::Leaf(WindowMatcher {
                                    app: None,
                                    bundle_id: None,
                                    title: Some("/A.*/".into()),
                                    process: None,
                                    class: None,
                                    aumid: None
                                })]
                            },
                            TreeLayoutNode::Leaf(WindowMatcher {
                                app: None,
                                bundle_id: None,
                                title: Some("CCC".into()),
                                process: None,
                                class: None,
                                aumid: None
                            })
                        ]
                    }
                ]
            }),
            ..Default::default()
        }
    );
}

#[test]
fn matches_tiling_leaf_matcher() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("editor".into()),
                    ..Default::default()
                }))
                .build(),
        ])
        .build();
    hub.focus_workspace("1");
    let ws = hub.current_workspace();
    let strategy = hub.strategies.for_workspace(ws);
    assert!(strategy.matches_tiling(ws, titled("editor").as_ref()));
    assert!(!strategy.matches_tiling(ws, titled("other").as_ref()));
}

#[test]
fn matches_tiling_no_preferred_root() {
    let hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    let ws = hub.current_workspace();
    let strategy = hub.strategies.for_workspace(ws);
    assert!(!strategy.matches_tiling(ws, titled("editor").as_ref()));
}

#[test]
fn sync_preferred_layout_keeps_focus_history() {
    let leaves = || {
        vec![
            TreeLayoutNode::Leaf(WindowMatcher {
                title: Some("AAA".into()),
                ..Default::default()
            }),
            TreeLayoutNode::Leaf(WindowMatcher {
                title: Some("BBB".into()),
                ..Default::default()
            }),
            TreeLayoutNode::Leaf(WindowMatcher {
                title: Some("CCC".into()),
                ..Default::default()
            }),
        ]
    };
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: leaves(),
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1");

    // Out of tree order on purpose: children_dfs yields siblings in reverse, so an
    // insert order of AAA, BBB, CCC rebuilds the recency order by accident.
    let bbb = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let aaa = hub
        .insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let ccc = hub
        .insert_window(titled("CCC"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    // Without the saved history, ccc's predecessor comes back as bbb.
    hub.sync_preferred_layout(vec![
        LayoutWorkspaceConfigBuilder::new("1")
            .with_tree(TreeLayoutNode::Container {
                split: Some(SplitMode::Vertical),
                children: leaves(),
            })
            .build(),
    ]);
    assert_eq!(hub.focused_window(ws), Some(ccc));

    hub.delete_window(ccc);
    assert_eq!(hub.focused_window(ws), Some(aaa));
    assert_ne!(hub.focused_window(ws), Some(bbb));
}

#[test]
fn sync_preferred_layout_focuses_window_inside_previously_highlighted_container() {
    let mut hub = TestHubBuilder::new()
        .with_layout(LayoutConfigBuilder::new().build())
        .build();
    hub.focus_workspace("3");
    hub.insert_window(titled("AAA"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let bbb = hub
        .insert_window(titled("BBB"), default_dim(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    // focus_parent highlights the container, so no window is the focused node.
    hub.focus_parent();
    assert_eq!(hub.focused_window(ws), None);

    hub.sync_preferred_layout(vec![
        LayoutWorkspaceConfigBuilder::new("3")
            .with_tree(TreeLayoutNode::Leaf(WindowMatcher {
                title: Some("pref-0".into()),
                ..Default::default()
            }))
            .build(),
    ]);

    // The rebuild deletes the container, so the highlight cannot survive it.
    assert_eq!(hub.focused_window(ws), Some(bbb));
}
