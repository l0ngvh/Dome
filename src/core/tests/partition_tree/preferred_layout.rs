use insta::assert_snapshot;

use crate::core::hub::Hub;
use crate::core::node::WindowId;
use crate::core::tests::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, TilingConfigBuilder,
    assert_one_window_per_workspace, default_rect, partition_tree_entry, preferred_layout,
    setup_logger_with_level, snapshot, titled, titled_matcher,
};
use crate::core::{PixelRect, PreferredLayouts, PreferredWorkspace, WindowRestrictions};
use crate::core::{SplitMode, TreeLayoutNode, WindowMatcher};

#[test]
fn insert_first_preferred_window_next_to_focused_window() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("BBB"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("AAA"), default_rect(), WindowRestrictions::None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=25.00, w=75.00, h=5.00, highlighted, spawn=right)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w2 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w3 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("EEE"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w2 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("EEE"), default_rect(), WindowRestrictions::None)
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
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w4 = hub
        .insert_window(titled("EEE"), default_rect(), WindowRestrictions::None)
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
        .insert_window(titled("EEE"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w2 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.toggle_spawn_mode();
    let _w3 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=75.00, y=25.00, w=75.00, h=5.00, highlighted, spawn=right)
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
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=100.00, y=23.00, w=50.00, h=7.00)
        Window(id=WindowId(2), x=100.00, y=15.00, w=50.00, h=8.00)
        Window(id=WindowId(5), x=50.00, y=17.00, w=50.00, h=13.00, highlighted, spawn=right)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.focus_parent();
    hub.move_focused_to_workspace("10", None);
    let _w2 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("YYY"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("TTT"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.focus_parent();
    hub.move_focused_to_workspace("10", None);
    let _w5 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
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
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("TTT"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("YYY"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("1")
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
        .build()]));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=113.00, y=23.00, w=37.00, h=7.00)
        Window(id=WindowId(0), x=113.00, y=15.00, w=37.00, h=8.00)
        Window(id=WindowId(1), x=75.00, y=15.00, w=38.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=15.00)
        Window(id=WindowId(4), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(2), x=0.00, y=0.00, w=150.00, h=30.00, titles=[DDD, Container])
        Container(id=ContainerId(5), x=75.00, y=0.00, w=75.00, h=30.00, titles=[YYY, Container])
        Container(id=ContainerId(3), x=75.00, y=15.00, w=75.00, h=15.00, titles=[AAA, Container])
        Container(id=ContainerId(4), x=113.00, y=15.00, w=37.00, h=15.00, titles=[TTT, CCC])
      )

    +-------------------------------------------------------------------------++-------------------------------------------------------------------------+
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                    W2                                   |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         ||                                                                         |
    |                                                                         |+-------------------------------------------------------------------------+
    |                                    W4                                   |**************************************+-----------------------------------+
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                 W0                |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *+-----------------------------------+
    |                                                                         |*                 W1                 *+-----------------------------------+
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                                   |
    |                                                                         |*                                    *|                 W3                |
    |                                                                         |*                                    *|                                   |
    +-------------------------------------------------------------------------+**************************************+-----------------------------------+
    ");
}

#[test]
fn insert_preferred_window_to_non_focused_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
        PixelRect::new(0, 0, 800, 600),
        WindowRestrictions::None,
    );
    hub.insert_window(
        titled("AAA"),
        PixelRect::new(0, 0, 800, 600),
        WindowRestrictions::None,
    );

    let prev_snapshot = snapshot(&hub);

    assert_snapshot!(prev_snapshot, @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=2.00, w=150.00, h=28.00, highlighted, spawn=right)
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

    hub.focus_workspace("10", None);
    assert_eq!(prev_snapshot, snapshot(&hub));
}

#[test]
fn held_leaf_on_current_workspace_sends_window_to_spawn_direction() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let w0 = hub
        .insert_window(titled("ABC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w3 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w5 = hub
        .insert_window(titled("ACD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=113.00, y=0.00, w=37.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=75.00, y=0.00, w=38.00, h=30.00)
        Window(id=WindowId(1), x=38.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=20.00, w=38.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=38.00, h=10.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=38.00, h=10.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD, ACD])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=38.00, h=30.00, titles=[ABC, BBB, AAA])
      )

    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                 W0                 ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    +------------------------------------+|                                   ||                                    |*                                   *
    +------------------------------------+|                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                 W2                 ||                 W1                ||                 W4                 |*                 W5                *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    +------------------------------------+|                                   ||                                    |*                                   *
    +------------------------------------+|                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                 W3                 ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    ");

    hub.delete_window(w0);
    let w6 = hub
        .insert_window(titled("ADE"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(6))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=113.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(4), x=75.00, y=0.00, w=38.00, h=30.00)
        Window(id=WindowId(1), x=38.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(3), x=0.00, y=20.00, w=38.00, h=10.00)
        Window(id=WindowId(2), x=0.00, y=10.00, w=38.00, h=10.00)
        Window(id=WindowId(6), x=0.00, y=0.00, w=38.00, h=10.00, highlighted, spawn=bottom)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, DDD, ACD])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=38.00, h=30.00, titles=[ADE, BBB, AAA])
      )

    **************************************+-----------------------------------++------------------------------------++-----------------------------------+
    *                                    *|                                   ||                                    ||                                   |
    *                                    *|                                   ||                                    ||                                   |
    *                                    *|                                   ||                                    ||                                   |
    *                                    *|                                   ||                                    ||                                   |
    *                 W6                 *|                                   ||                                    ||                                   |
    *                                    *|                                   ||                                    ||                                   |
    *                                    *|                                   ||                                    ||                                   |
    *                                    *|                                   ||                                    ||                                   |
    **************************************|                                   ||                                    ||                                   |
    +------------------------------------+|                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                 W2                 ||                 W1                ||                 W4                 ||                 W5                |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    +------------------------------------+|                                   ||                                    ||                                   |
    +------------------------------------+|                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                 W3                 ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    |                                    ||                                   ||                                    ||                                   |
    +------------------------------------++-----------------------------------++------------------------------------++-----------------------------------+
    ");
    hub.delete_window(w3);
    hub.delete_window(w5);
    hub.delete_window(w6);
    let _w7 = hub
        .insert_window(titled("AEF"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w8 = hub
        .insert_window(titled("AFG"), default_rect(), WindowRestrictions::None)
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
fn export_writes_title_for_each_window() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("BCD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("ABC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("BEF"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w5 = hub
        .insert_window(titled("ACD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=15.00, w=50.00, h=15.00)
        Window(id=WindowId(5), x=33.00, y=0.00, w=17.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=17.00, y=0.00, w=16.00, h=15.00)
        Window(id=WindowId(3), x=0.00, y=0.00, w=17.00, h=15.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC, BBB])
        Container(id=ContainerId(1), x=0.00, y=0.00, w=50.00, h=30.00, titles=[Container, BCD])
        Container(id=ContainerId(2), x=0.00, y=0.00, w=50.00, h=15.00, titles=[ABC, BEF, ACD])
      )

    +---------------++--------------+*****************+------------------------------------------------++------------------------------------------------+
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |       W3      ||      W4      |*       W5      *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    |               ||              |*               *|                                                ||                                                |
    +---------------++--------------+*****************|                                                ||                                                |
    +------------------------------------------------+|                       W1                       ||                       W2                       |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                       W0                       ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    |                                                ||                                                ||                                                |
    +------------------------------------------------++------------------------------------------------++------------------------------------------------+
    ");

    let export = hub.export_workspace(hub.current_workspace());
    assert_eq!(
        export,
        partition_tree_entry(Some(TreeLayoutNode::Container {
            split: Some(SplitMode::Horizontal),
            children: vec![
                TreeLayoutNode::Container {
                    split: Some(SplitMode::Vertical),
                    children: vec![
                        TreeLayoutNode::Container {
                            split: Some(SplitMode::Horizontal),
                            children: vec![
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("ABC".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("BEF".into()),
                                    ..Default::default()
                                }),
                                TreeLayoutNode::Leaf(WindowMatcher {
                                    title: Some("ACD".into()),
                                    ..Default::default()
                                }),
                            ],
                        },
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("BCD".into()),
                            ..Default::default()
                        })
                    ],
                },
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("CCC".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("BBB".into()),
                    ..Default::default()
                }),
            ],
        }))
    );
}

#[test]
fn single_window_slot_in_container_slot() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("ABC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("ACD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(4), x=120.00, y=0.00, w=30.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=90.00, y=0.00, w=30.00, h=30.00)
        Window(id=WindowId(2), x=60.00, y=0.00, w=30.00, h=30.00)
        Window(id=WindowId(1), x=30.00, y=0.00, w=30.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=30.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[ABC, CCC, BBB, AAA, ACD])
      )

    +----------------------------++----------------------------++----------------------------++----------------------------+******************************
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |             W0             ||             W1             ||             W2             ||             W3             |*             W4             *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    |                            ||                            ||                            ||                            |*                            *
    +----------------------------++----------------------------++----------------------------++----------------------------+******************************
    ");

    let export = hub.export_workspace(hub.current_workspace());
    assert_eq!(
        export,
        partition_tree_entry(Some(TreeLayoutNode::Container {
            split: Some(SplitMode::Horizontal),
            children: vec![
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("ABC".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("CCC".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("BBB".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("AAA".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("ACD".into()),
                    ..Default::default()
                })
            ],
        }))
    );
}

#[test]
fn bare_window_slot() {
    setup_logger_with_level("trace");
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("/A.*/".into()),
                    ..Default::default()
                }))
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("ABC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("ACD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=113.00, y=0.00, w=37.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=75.00, y=0.00, w=38.00, h=30.00)
        Window(id=WindowId(1), x=38.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=38.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[ABC, AAA, BBB, ACD])
      )

    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                 W0                 ||                 W1                ||                 W2                 |*                 W3                *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    |                                    ||                                   ||                                    |*                                   *
    +------------------------------------++-----------------------------------++------------------------------------+*************************************
    ");

    let export = hub.export_workspace(hub.current_workspace());
    assert_eq!(
        export,
        partition_tree_entry(Some(TreeLayoutNode::Container {
            split: Some(SplitMode::Horizontal),
            children: vec![
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("ABC".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("AAA".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("BBB".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("ACD".into()),
                    ..Default::default()
                })
            ],
        }))
    );
}

#[test]
fn export_omits_tabbed_slot_with_one_held_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
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
    hub.focus_workspace("1", None);

    let _w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w2 = hub
        .insert_window(titled("ABC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w3 = hub
        .insert_window(titled("ACD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w4 = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let export = hub.export_workspace(hub.current_workspace());

    assert_eq!(
        export,
        partition_tree_entry(Some(TreeLayoutNode::Container {
            split: Some(SplitMode::Horizontal),
            children: vec![
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("w0".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Container {
                    split: Some(SplitMode::Vertical),
                    children: vec![
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("AAA".into()),
                            ..Default::default()
                        }),
                        TreeLayoutNode::Leaf(WindowMatcher {
                            title: Some("CCC".into()),
                            ..Default::default()
                        })
                    ]
                },
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("ABC".into()),
                    ..Default::default()
                }),
                TreeLayoutNode::Leaf(WindowMatcher {
                    title: Some("ACD".into()),
                    ..Default::default()
                })
            ]
        }))
    );
}

#[test]
fn apply_preferred_layouts_focuses_window_inside_previously_highlighted_container() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    hub.focus_workspace("3", None);
    let aaa = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let ws = hub.current_workspace();

    hub.focus_parent();
    assert_eq!(hub.focused_window(ws), None, "the container is highlighted");

    hub.apply_preferred_layouts(preferred_layout([LayoutWorkspaceConfigBuilder::new("3")
        .with_tree(TreeLayoutNode::Leaf(WindowMatcher {
            title: Some("pref-0".into()),
            ..Default::default()
        }))
        .build()]));

    assert_eq!(
        hub.focused_window(ws),
        Some(aaa),
        "the highlight goes with the container, and focus goes to the first window attached again"
    );
}

fn term_leaf_on(name: &str) -> (String, PreferredWorkspace) {
    LayoutWorkspaceConfigBuilder::new(name)
        .with_tree(TreeLayoutNode::Leaf(titled_matcher("term")))
        .build()
}

fn insert_term(hub: &mut Hub) -> WindowId {
    hub.insert_window(titled("term"), default_rect(), WindowRestrictions::None)
        .expect("term window inserted")
}

#[test]
fn held_leaf_sends_window_to_current_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();

    insert_term(&mut hub);
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
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

#[test]
fn held_leaf_sends_window_to_free_leaf_on_another_workspace() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("a"), term_leaf_on("b")])
        .build();

    let windows = [insert_term(&mut hub), insert_term(&mut hub)];

    assert_one_window_per_workspace(&hub, &windows, &["a", "b"]);
    hub.validate();
}

#[test]
fn closed_window_frees_its_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();

    let a = insert_term(&mut hub);
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
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

    hub.delete_window(a);
    insert_term(&mut hub);
    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
    *                                                                         W2                                                                         *
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

#[test]
fn floating_window_keeps_its_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();
    insert_term(&mut hub);
    hub.toggle_float();
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
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

    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, float, highlighted)
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
    *                                                                         F0                                                                         *
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

#[test]
fn fullscreen_window_keeps_its_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();
    insert_term(&mut hub);
    hub.toggle_fullscreen();
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
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

    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(0))
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
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
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
}

#[test]
fn minimized_window_keeps_its_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();
    let a = insert_term(&mut hub);
    hub.minimize_window(a);
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Minimized: [WindowId(0)]
    ");
}

#[test]
fn window_moved_to_another_workspace_keeps_its_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();
    insert_term(&mut hub);
    hub.move_focused_to_workspace("2", None);
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
    hub.focus_workspace("dev", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");
}

#[test]
fn closing_a_minimized_window_frees_its_leaf() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![term_leaf_on("dev")])
        .build();
    let a = insert_term(&mut hub);
    hub.minimize_window(a);
    hub.delete_window(a);
    hub.focus_workspace("0", None);
    insert_term(&mut hub);
    hub.focus_workspace("dev", None);
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

#[test]
fn floating_holder_is_skipped_when_a_later_window_is_placed() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("dev")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(titled_matcher("term")),
                        TreeLayoutNode::Leaf(titled_matcher("term")),
                    ],
                })
                .build(),
        ])
        .build();
    let a = insert_term(&mut hub);
    hub.toggle_float();
    insert_term(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, float)
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         F0                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");

    hub.set_focus(a);
    hub.toggle_float();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[term, term])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W1                                   |*                                    W0                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn window_holding_another_workspace_leaf_is_skipped_when_a_later_window_is_placed() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            term_leaf_on("1"),
            LayoutWorkspaceConfigBuilder::new("2")
                .with_tree(TreeLayoutNode::Container {
                    split: Some(SplitMode::Horizontal),
                    children: vec![
                        TreeLayoutNode::Leaf(titled_matcher("CCC")),
                        TreeLayoutNode::Leaf(titled_matcher("DDD")),
                        TreeLayoutNode::Leaf(titled_matcher("term")),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("2", None);
    for title in ["CCC", "DDD"] {
        hub.insert_window(titled(title), default_rect(), WindowRestrictions::None)
            .unwrap();
    }
    hub.focus_workspace("1", None);
    insert_term(&mut hub);
    hub.move_focused_to_workspace("2", None);
    hub.focus_workspace("2", None);
    insert_term(&mut hub);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=113.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=38.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=38.00, y=0.00, w=37.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=38.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[CCC, DDD, term, term])
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
}

#[test]
fn apply_preferred_layouts_resets_an_unnamed_workspace_to_spawn_order() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .build();
    for title in ["w0", "w1", "w2"] {
        hub.insert_window(titled(title), default_rect(), WindowRestrictions::None);
    }
    hub.toggle_container_layout();

    hub.apply_preferred_layouts(PreferredLayouts::default());

    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2])
      )

    **************************************************+------------------------------------------------++------------------------------------------------+
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                       W0                       *|                       W1                       ||                       W2                       |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    *                                                *|                                                ||                                                |
    **************************************************+------------------------------------------------++------------------------------------------------+
    ");
    hub.validate();
}

#[test]
fn matched_window_joins_held_slot_after_move_removes_its_container() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: None,
                    children: vec![
                        TreeLayoutNode::Container {
                            split: None,
                            children: vec![
                                TreeLayoutNode::Leaf(titled_matcher("AAA")),
                                TreeLayoutNode::Leaf(titled_matcher("BBB")),
                            ],
                        },
                        TreeLayoutNode::Leaf(titled_matcher("CCC")),
                        TreeLayoutNode::Leaf(titled_matcher("DDD")),
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);

    let a = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let b = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _x = hub
        .insert_window(titled("xxx"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let y = hub
        .insert_window(titled("yyy"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _c = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.move_down();
    hub.delete_window(a);
    hub.delete_window(b);
    hub.delete_window(y);
    hub.insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=75.00, y=15.00, w=75.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=0.00, y=15.00, w=75.00, h=15.00)
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=15.00)
        Container(id=ContainerId(2), x=0.00, y=0.00, w=150.00, h=30.00, titles=[xxx, Container])
        Container(id=ContainerId(3), x=0.00, y=15.00, w=150.00, h=15.00, titles=[CCC, DDD])
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W2                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W4                                   |*                                    W5                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
    hub.validate();
}

/// Two workspaces with separate preferred layouts each take two matching
/// windows. Every window stays on the workspace it was inserted into.
#[test]
fn lowest_common_ancestor_stays_within_each_workspace() {
    let leaf = |title: &str| {
        TreeLayoutNode::Leaf(WindowMatcher {
            title: Some(title.into()),
            ..Default::default()
        })
    };
    let pair = |a: &str, b: &str| TreeLayoutNode::Container {
        split: Some(SplitMode::Horizontal),
        children: vec![leaf(a), leaf(b)],
    };

    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("0")
                .with_tree(pair("A0", "B0"))
                .build(),
            LayoutWorkspaceConfigBuilder::new("2")
                .with_tree(pair("A2", "B2"))
                .build(),
        ])
        .build();

    hub.focus_workspace("0", None);
    hub.insert_window(titled("A0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("B0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[A0, B0])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W1                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");

    hub.focus_workspace("2", None);
    hub.insert_window(titled("A2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.insert_window(titled("B2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(2), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(1), x=0.00, y=0.00, w=150.00, h=30.00, titles=[A2, B2])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W2                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    ");
}

#[test]
fn split_groups_send_matched_window_to_first_group_in_layout() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(TilingConfigBuilder::new().build())
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_tree(TreeLayoutNode::Container {
                    split: None,
                    children: vec![
                        TreeLayoutNode::Container {
                            split: None,
                            children: vec![
                                TreeLayoutNode::Leaf(titled_matcher("AAA")),
                                TreeLayoutNode::Leaf(titled_matcher("BBB")),
                            ],
                        },
                        TreeLayoutNode::Container {
                            split: None,
                            children: vec![
                                TreeLayoutNode::Leaf(titled_matcher("CCC")),
                                TreeLayoutNode::Leaf(titled_matcher("DDD")),
                            ],
                        },
                    ],
                })
                .build(),
        ])
        .build();
    hub.focus_workspace("1", None);

    let _a = hub
        .insert_window(titled("AAA"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let b = hub
        .insert_window(titled("BBB"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _c = hub
        .insert_window(titled("CCC"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.move_down();
    hub.delete_window(b);
    hub.insert_window(titled("DDD"), default_rect(), WindowRestrictions::None)
        .unwrap();
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(3))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=15.00, w=150.00, h=15.00)
        Window(id=WindowId(3), x=75.00, y=0.00, w=75.00, h=15.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=15.00)
        Container(id=ContainerId(2), x=0.00, y=0.00, w=150.00, h=30.00, titles=[Container, CCC])
        Container(id=ContainerId(3), x=0.00, y=0.00, w=150.00, h=15.00, titles=[AAA, DDD])
      )

    +-------------------------------------------------------------------------+***************************************************************************
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                    W0                                   |*                                    W3                                   *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    |                                                                         |*                                                                         *
    +-------------------------------------------------------------------------+***************************************************************************
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                         W2                                                                         |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    ");
    hub.validate();
}
