use insta::assert_snapshot;

#[cfg(target_os = "windows")]
use super::{LayoutConfigBuilder, PartitionTreeConfigBuilder, TestHubBuilder};
#[cfg(target_os = "windows")]
use crate::config::SizeConstraint;
#[cfg(target_os = "windows")]
use crate::core::node::Logical;
use crate::core::node::{Dimension, Length};

use crate::action::MonitorTarget;
use crate::core::tests::{setup, snapshot, snapshot_text, titled};

#[test]
fn add_monitor_creates_workspace_on_new_monitor() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w0"));

    hub.add_monitor(
        "monitor-1".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_monitor(&MonitorTarget::Name("monitor-1".to_string()));
    hub.focus_workspace("0", None);
    hub.insert_tiling(hub.current_workspace(), titled("w1"));

    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), name="monitor-1", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
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
    "#);
}

#[test]
fn per_monitor_same_name_workspace() {
    let mut hub = setup();

    hub.focus_workspace("1", None);
    hub.insert_tiling(hub.current_workspace(), titled("a1"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    // Reaching the same name on the external monitor via the monitor selector
    // resolves a distinct workspace there, not the primary's own "1".
    hub.focus_workspace("1", Some("external"));
    hub.insert_tiling(hub.current_workspace(), titled("b1"));

    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
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
    "#);
}

#[test]
fn unplugging_unfocused_monitor_leaves_focus_unchanged() {
    let mut hub = setup();
    let primary = hub.focused_monitor();
    let external = hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);

    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.focus_monitor(&MonitorTarget::Name("primary".to_string()));

    hub.remove_monitor(external, primary);

    // Focus stays on primary because the removed monitor was not focused.
    assert_eq!(hub.focused_monitor(), primary);
}

#[test]
fn replugging_monitor_moves_workspaces_back_to_it() {
    let mut hub = setup();
    let primary = hub.focused_monitor();
    let b = hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);

    // Two sibling workspaces on B, each with a distinctly-titled window, so the
    // placement shows exactly which of B's workspaces are live at each stage.
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.insert_tiling(hub.current_workspace(), titled("b1"));
    hub.insert_tiling(hub.current_workspace(), titled("b2"));
    hub.focus_workspace("2", None);
    hub.insert_tiling(hub.current_workspace(), titled("b3"));

    // Stage 1: B is present with both windows visible.
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
      )
    "#);

    // Stage 2: unplug B. Its workspaces park and hide, so b1 and b2 are absent.
    hub.remove_monitor(b, primary);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");

    // Stage 3: replug with the same device name and position so the returning
    // monitor recomputes to B's old name. Both parked workspaces re-home onto
    // it, and it also gets its own fresh "0". Visiting each by name shows the
    // reattached window on screen.
    hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=200.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=150.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=150.00, y=0.00, w=100.00, h=30.00, titles=[b1, b2])
      )
    "#);
    hub.focus_workspace("2", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
      )
    "#);
    hub.focus_workspace("0", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn parked_workspace_is_not_reachable_by_name() {
    let mut hub = setup();
    let primary = hub.focused_monitor();

    // Primary's own attached "1" with a distinctly-titled window. Its id is the
    // focused workspace right after creation, captured behaviorally.
    hub.focus_workspace("1", None);
    hub.insert_tiling(hub.current_workspace(), titled("own1"));
    hub.insert_tiling(hub.current_workspace(), titled("own2"));
    let primary_native_1 = hub.current_workspace();

    // A distinct "1" on B carrying a different window, parked onto primary after
    // unplug.
    let b = hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.insert_tiling(hub.current_workspace(), titled("b1"));
    hub.insert_tiling(hub.current_workspace(), titled("b2"));
    hub.insert_tiling(hub.current_workspace(), titled("b3"));
    hub.remove_monitor(b, primary);

    // Focus back to primary, then resolve "1" by name.
    hub.focus_monitor(&MonitorTarget::Name("primary".to_string()));
    hub.focus_workspace("1", None);

    // Name resolution lands on primary's own attached "1", never the parked one.
    assert_eq!(hub.current_workspace(), primary_native_1);

    // Its window "own1" is on screen and the parked "b1" stays hidden, so a
    // parked workspace is not addressable by name.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[own1, own2])
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
}

#[test]
fn unplugging_focused_monitor_moves_focus_to_primary() {
    let mut hub = setup();
    let primary = hub.focused_monitor();
    hub.add_monitor("second".to_string(), dim_at(150.0, 0.0), 1.0);
    let third = hub.add_monitor("third".to_string(), dim_at(300.0, 0.0), 1.0);

    // Focus a non-primary monitor and give it a distinctly-titled window so its
    // presence in the placement tracks whether its workspace is live.
    hub.focus_monitor(&MonitorTarget::Name("third".to_string()));
    assert_eq!(hub.focused_monitor(), third);

    hub.remove_monitor(third, primary);

    // Focus follows to the primary after its monitor is unplugged.
    assert_eq!(hub.focused_monitor(), primary);
}

#[test]
fn visiting_parked_workspace_shows_its_windows() {
    let mut hub = setup();
    let primary = hub.focused_monitor();

    // Primary's own attached "0" with a distinctly-titled window, captured for
    // the name-resolution check. Its presence in the placement tracks whether
    // the primary is showing its own workspace or a visitor.
    hub.insert_tiling(hub.current_workspace(), titled("ownw"));

    // B with a window, then unplug so B's workspace parks onto primary.
    let b = hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.insert_tiling(hub.current_workspace(), titled("bwin1"));
    hub.remove_monitor(b, primary);

    // Parked, so bwin drops out of the placement while the primary shows ownw.
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
    *                                                                         W0                                                                         *
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

    // Visiting pulls the parked workspace into view, so bwin shows and ownw
    // hides. It is reached by name plus its origin monitor's name ("external").
    hub.focus_workspace("0", Some("external"));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=50.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=100.00, h=30.00, titles=[bwin, bwin1])
      )

    +------------------------------------------------+**************************************************                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                       W1                       |*                       W2                       *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    |                                                |*                                                *                                                  
    +------------------------------------------------+**************************************************
    ");

    // Resolving the primary's own attached name lands on the primary's native
    // workspace, never the visitor.
    hub.focus_workspace("0", None);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
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
    *                                                                         W0                                                                         *
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
fn focusing_another_monitor_leaves_visitor_in_place() {
    let mut hub = setup();
    let primary = hub.focused_monitor();
    let b = hub.add_monitor("beta".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.add_monitor("gamma".to_string(), dim_at(300.0, 0.0), 1.0);

    // A window on gamma's own workspace so gamma stays visible throughout.
    hub.focus_monitor(&MonitorTarget::Name("gamma".to_string()));
    hub.insert_tiling(hub.current_workspace(), titled("cwin"));
    hub.insert_tiling(hub.current_workspace(), titled("cigwin"));

    // Window on B, then unplug only B so its workspace parks onto primary.
    hub.focus_monitor(&MonitorTarget::Name("beta".to_string()));
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.remove_monitor(b, primary);

    // Reach the parked workspace by name plus its origin monitor's name.
    hub.focus_workspace("0", Some("beta"));

    // The visitor bwin is on the primary, cwin on gamma.
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(2), name="gamma", screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=350.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=300.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cigwin])
      )

    ****************************************************************************************************                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                W2                                                *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    *                                                                                                  *                                                  
    ****************************************************************************************************
    "#);

    // Focus a workspace on a different surviving monitor. The primary's active
    // workspace never switches off the visitor, so bwin stays on screen.
    hub.focus_monitor(&MonitorTarget::Name("gamma".to_string()));
    hub.focus_workspace("0", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=100.00, h=30.00)
      )
      Monitor(id=MonitorId(2), name="gamma", screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=350.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=300.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cigwin])
      )

    +--------------------------------------------------------------------------------------------------+                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                W2                                                |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    |                                                                                                  |                                                  
    +--------------------------------------------------------------------------------------------------+
    "#);
}

#[test]
fn parked_workspace_remembers_origin_when_rental_host_unplugged() {
    let mut hub = setup();
    let primary = hub.focused_monitor();
    hub.insert_tiling(hub.current_workspace(), titled("win"));

    // B with a window on a distinctly named workspace so it is unambiguous
    // after replug.
    let b = hub.add_monitor("beta".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("beta".to_string()));
    hub.focus_workspace("9", None);
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));

    // C survives the later primary removal.
    let c = hub.add_monitor("gamma".to_string(), dim_at(300.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("gamma".to_string()));
    hub.insert_tiling(hub.current_workspace(), titled("cwin"));
    hub.insert_tiling(hub.current_workspace(), titled("cwin"));
    hub.insert_tiling(hub.current_workspace(), titled("cwin"));

    // Park B onto primary: bwin drops out of the placement.
    hub.remove_monitor(b, primary);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(2), name="gamma", screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(5), x=366.67, y=0.00, w=33.33, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=333.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(3), x=300.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(1), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cwin, cwin])
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
    "#);

    // Unplug the primary itself while it hosts B's parked workspace. The parked
    // workspace is re-rented to C, and bwin stays absent.
    hub.remove_monitor(primary, c);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(2), screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(5), x=366.67, y=0.00, w=33.33, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=333.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(3), x=300.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(1), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cwin, cwin])
      )
    ");

    // Replug a monitor that recomputes to B's original name. If the parked
    // workspace had lost B's origin when the primary was removed, it would not
    // reattach here. bwin returning on the beta-named monitor proves the frozen
    // origin survived the primary's own removal.
    hub.add_monitor("beta".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("beta".to_string()));
    hub.focus_workspace("9", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(2), name="gamma", screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(5), x=366.67, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(4), x=333.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(3), x=300.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(1), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cwin, cwin])
      )
      Monitor(id=MonitorId(3), name="beta", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=200.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=150.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=150.00, y=0.00, w=100.00, h=30.00, titles=[bwin, bwin])
      )
    "#);
}

#[test]
fn workspace_returns_to_the_right_monitor_among_same_named() {
    let mut hub = setup();
    let primary = hub.focused_monitor();

    // First same-named monitor, with a window on a distinctly named workspace
    // so it is unambiguous after replug.
    let d1 = hub.add_monitor("DELL".to_string(), dim_at(200.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("7", None);
    hub.insert_tiling(hub.current_workspace(), titled("d1win"));
    hub.insert_tiling(hub.current_workspace(), titled("d1win"));

    // Second same-named monitor to its right, reached by direction because the
    // shared device name resolves to the first one.
    hub.add_monitor("DELL".to_string(), dim_at(400.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL #2".to_string()));
    hub.insert_tiling(hub.current_workspace(), titled("d2win"));
    hub.insert_tiling(hub.current_workspace(), titled("d2win"));
    hub.insert_tiling(hub.current_workspace(), titled("d2win"));

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #1", screen=(x=200.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=250.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=200.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=200.00, y=0.00, w=100.00, h=30.00, titles=[d1win, d1win])
      )
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=400.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(4), x=466.67, y=0.00, w=33.33, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=433.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(2), x=400.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(1), x=400.00, y=0.00, w=100.00, h=30.00, titles=[d2win, d2win, d2win])
      )
    "#);

    // Unplug the first DELL. Its workspace parks onto primary so d1win drops
    // out, and the lone survivor is restamped back to the bare "DELL".
    hub.remove_monitor(d1, primary);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL", screen=(x=400.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(4), x=466.67, y=0.00, w=33.33, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=433.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(2), x=400.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(1), x=400.00, y=0.00, w=100.00, h=30.00, titles=[d2win, d2win, d2win])
      )
    "#);

    // Replug a DELL positioned so it recomputes to "DELL #1" again. The parked
    // workspace re-homes onto it only if it remembered the disambiguated #1
    // origin rather than the bare "DELL", so d1win returning proves the frozen
    // origin carried the pre-removal disambiguated name.
    hub.add_monitor("DELL".to_string(), dim_at(200.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL #1".to_string()));
    hub.focus_workspace("7", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=400.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(4), x=466.67, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(3), x=433.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(2), x=400.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(1), x=400.00, y=0.00, w=100.00, h=30.00, titles=[d2win, d2win, d2win])
      )
      Monitor(id=MonitorId(3), name="DELL #1", screen=(x=200.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=250.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=200.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=200.00, y=0.00, w=100.00, h=30.00, titles=[d1win, d1win])
      )
    "#);
}

#[test]
fn simultaneous_same_name_removal_single_replug_reattaches_last_removed() {
    let mut hub = setup();
    let primary = hub.focused_monitor();

    // Two identically-named monitors at distinct positions, so they collide on
    // device name and disambiguate to "DELL #1" (leftmost) / "DELL #2". Each
    // carries a distinctly-titled window on its own named workspace, so the
    // placement tracks exactly which monitor's workspace is live at each stage.
    // The first DELL is focused by name while it is the only DELL, so the name
    // still resolves; the second is reached by direction because once both are
    // present the shared name has disambiguated and no longer matches either.
    let d1 = hub.add_monitor("DELL".to_string(), dim_at(200.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("a", None);
    hub.insert_tiling(hub.current_workspace(), titled("d1win"));
    let d2 = hub.add_monitor("DELL".to_string(), dim_at(400.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Right);
    hub.focus_workspace("b", None);
    hub.insert_tiling(hub.current_workspace(), titled("d2win"));
    hub.insert_tiling(hub.current_workspace(), titled("d2win"));

    // Two explicit successive removals in a TEST-PINNED order. Production
    // platform iteration over the removed monitors has no guaranteed order, so
    // the test pins it to make the assertion deterministic. Removing "DELL #1"
    // first freezes its workspace's origin as "DELL #1", then the tail recompute
    // restamps the lone survivor from "DELL #2" down to bare "DELL", so the
    // second removal freezes that workspace's origin as bare "DELL".
    hub.remove_monitor(d1, primary);
    hub.remove_monitor(d2, primary);

    // A single DELL plugged back in recomputes to bare "DELL". Only the
    // second-removed monitor's workspace (frozen bare "DELL") matches and
    // reattaches; the first-removed's (frozen "DELL #1") stays parked. Focusing
    // the returning monitor's "b" shows d2win while d1win stays hidden, which
    // proves the per-call recompute made a single replug reclaim only the
    // last-removed monitor's workspace.
    hub.add_monitor("DELL".to_string(), dim_at(200.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("b", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(3), name="DELL", screen=(x=200.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=250.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=200.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=200.00, y=0.00, w=100.00, h=30.00, titles=[d2win, d2win])
      )
    "#);
}

#[test]
fn update_monitor_dimension_adjusts_workspaces() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.insert_tiling(hub.current_workspace(), titled("w6"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    let primary = hub.focused_monitor();
    hub.update_monitor(
        primary,
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(200.0),
            Length::new(50.0),
        ),
        1.0,
    );

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=200.00 h=50.00),
        Window(id=WindowId(1), x=100.00, y=0.00, w=100.00, h=50.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=0.00, y=0.00, w=100.00, h=50.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=200.00, h=50.00, titles=[w5, w6])
      )
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn focus_monitor_by_direction() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w7"));

    // Monitor to the right
    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    // Monitor below
    hub.add_monitor(
        "bottom-monitor".to_string(),
        Dimension::new(
            Length::new(0.0),
            Length::new(30.0),
            Length::new(150.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_monitor(&MonitorTarget::Right);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), name="right-monitor", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="bottom-monitor", screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    "#);

    hub.focus_monitor(&MonitorTarget::Left);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(1), name="right-monitor", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="bottom-monitor", screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    "#);

    hub.focus_monitor(&MonitorTarget::Down);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), name="right-monitor", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="bottom-monitor", screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    "#);

    hub.focus_monitor(&MonitorTarget::Up);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(1), name="right-monitor", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="bottom-monitor", screen=(x=0.00 y=30.00 w=150.00 h=30.00))
    "#);

    // Focus by name twice: second call is no-op
    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    let after_name = snapshot_text(&hub);
    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    assert_eq!(snapshot_text(&hub), after_name);
}

#[test]
fn focus_monitor_by_name() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w8"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn move_to_monitor_moves_focused_window() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w9"));
    hub.insert_tiling(hub.current_workspace(), titled("w10"));

    hub.add_monitor(
        "right-monitor".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.move_focused_to_monitor(&MonitorTarget::Right);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(1), name="right-monitor", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    "#);
}

#[test]
fn move_to_monitor_by_name() {
    let mut hub = setup();
    hub.insert_tiling(hub.current_workspace(), titled("w11"));

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(0), x=150.00, y=0.00, w=100.00, h=30.00)
      )
    "#);
}

#[test]
fn move_float_to_monitor() {
    use crate::core::Dimension;

    let mut hub = setup();
    hub.insert_float(
        hub.current_workspace(),
        Dimension::new(
            Length::new(10.0),
            Length::new(10.0),
            Length::new(50.0),
            Length::new(20.0),
        ),
        titled("w12"),
    );

    hub.add_monitor(
        "external".to_string(),
        Dimension::new(
            Length::new(150.0),
            Length::new(0.0),
            Length::new(100.0),
            Length::new(30.0),
        ),
        1.0,
    );

    hub.move_focused_to_monitor(&MonitorTarget::Name("external".to_string()));

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn monitor_noop_cases() {
    // Single monitor: focus_monitor is no-op
    {
        let mut hub = setup();
        hub.insert_tiling(hub.current_workspace(), titled("w13"));
        let before = snapshot_text(&hub);
        hub.focus_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // Single monitor with tiling: move_focused_to_monitor is no-op
    {
        let mut hub = setup();
        hub.insert_tiling(hub.current_workspace(), titled("w14"));
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // Two monitors, move to same monitor: no-op
    {
        let mut hub = setup();
        hub.insert_tiling(hub.current_workspace(), titled("w15"));
        hub.add_monitor(
            "external".to_string(),
            Dimension::new(
                Length::new(150.0),
                Length::new(0.0),
                Length::new(100.0),
                Length::new(30.0),
            ),
            1.0,
        );
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Name("primary".to_string()));
        assert_eq!(snapshot_text(&hub), before);
    }

    // Two monitors, no windows: move_focused_to_monitor is no-op
    {
        let mut hub = setup();
        hub.add_monitor(
            "right-monitor".to_string(),
            Dimension::new(
                Length::new(150.0),
                Length::new(0.0),
                Length::new(100.0),
                Length::new(30.0),
            ),
            1.0,
        );
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }
}

fn dim_at(x: f32, y: f32) -> Dimension {
    Dimension::new(
        Length::new(x),
        Length::new(y),
        Length::new(100.0),
        Length::new(30.0),
    )
}

#[test]
fn unique_name_unique_stays_bare() {
    // Two monitors with distinct device names: neither gets a suffix, so the
    // snapshot proves the bare/unsuffixed case for a distinct-named monitor.
    let mut hub = setup();
    hub.add_monitor("DELL".to_string(), dim_at(1920.0, 0.0), 1.0);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn unique_name_colliders_numbered() {
    let mut hub = setup();
    hub.add_monitor("DELL".to_string(), dim_at(0.0, 0.0), 1.0);
    hub.add_monitor("DELL".to_string(), dim_at(1920.0, 0.0), 1.0);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #1", screen=(x=0.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn unique_name_sort_by_position() {
    // Inserted rightmost-first, so ranking depends on position not insert order.
    let mut hub = setup();
    hub.add_monitor("DELL".to_string(), dim_at(1920.0, 0.0), 1.0);
    hub.add_monitor("DELL".to_string(), dim_at(0.0, 0.0), 1.0);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #2", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #1", screen=(x=0.00 y=0.00 w=100.00 h=30.00))
    "#);

    // Same x, so y breaks the tie: topmost (smaller y) ranks first.
    let mut hub = setup();
    hub.add_monitor("ACER".to_string(), dim_at(0.0, 1080.0), 1.0);
    hub.add_monitor("ACER".to_string(), dim_at(0.0, 0.0), 1.0);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="ACER #2", screen=(x=0.00 y=1080.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="ACER #1", screen=(x=0.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn unique_name_recomputes_on_position_change() {
    let mut hub = setup();
    let first = hub.add_monitor("DELL".to_string(), dim_at(0.0, 0.0), 1.0);
    hub.add_monitor("DELL".to_string(), dim_at(1920.0, 0.0), 1.0);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #1", screen=(x=0.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
    "#);

    // Move the current #1 to the right of its sibling: the ranks must swap.
    hub.update_monitor(first, dim_at(3840.0, 0.0), 1.0);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #2", screen=(x=3840.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #1", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
    "#);
}

// Has to be gated behind windows rn, since Hub is not generic over unit type
#[cfg(target_os = "windows")]
#[test]
fn monitor_scale_multiplies_tab_bar_height() {
    let l = LayoutConfigBuilder::new()
        .with_partition_tree_config(
            PartitionTreeConfigBuilder::new()
                .with_tab_bar_height(Length::<Logical>::new(5.0))
                .with_automatic_tiling(true)
                .build(),
        )
        .build();
    let mut hub = TestHubBuilder::new().with_scale(2.0).with_layout(l).build();
    hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.insert_tiling(hub.current_workspace(), titled("w17"));
    hub.toggle_container_layout();
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=20.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[w16, w17])
      )
    ");

    let monitor_id = hub.focused_monitor();
    hub.update_monitor(
        monitor_id,
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1000.0),
            Length::new(1000.0),
        ),
        3.0,
    );
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=1000.00 h=1000.00),
        Window(id=WindowId(1), x=0.00, y=15.00, w=1000.00, h=985.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=1000.00, h=1000.00, tabbed, active_tab=1, titles=[w16, w17])
      )
    ");
}

#[cfg(target_os = "windows")]
#[test]
fn monitor_scale_multiplies_size_constraints() {
    let mut hub = TestHubBuilder::new()
        .with_scale(2.0)
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_tab_bar_height(Length::<Logical>::new(10.0))
                        .with_automatic_tiling(false)
                        .build(),
                )
                // At scale of 2.0, min width should be 80
                .with_min_width(SizeConstraint::Pixels(Length::new(40.0)))
                .build(),
        )
        .build();
    for i in 0..6 {
        hub.insert_tiling(hub.current_workspace(), titled(format!("w{i}").as_str()));
    }
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=70.00, y=0.00, w=80.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=0.00, y=0.00, w=70.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2, w3, w4, w5])
      )
    ");

    let monitor_id = hub.focused_monitor();
    hub.update_monitor(
        monitor_id,
        Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(500.0),
            Length::new(1000.0),
        ),
        // At scale of 3.0, min width should be 120
        3.0,
    );

    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=500.00 h=1000.00),
        Window(id=WindowId(5), x=380.00, y=0.00, w=120.00, h=1000.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=260.00, y=0.00, w=120.00, h=1000.00)
        Window(id=WindowId(3), x=140.00, y=0.00, w=120.00, h=1000.00)
        Window(id=WindowId(2), x=20.00, y=0.00, w=120.00, h=1000.00)
        Window(id=WindowId(1), x=0.00, y=0.00, w=20.00, h=1000.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=500.00, h=1000.00, titles=[w0, w1, w2, w3, w4, w5])
      )
    ");
}

#[test]
fn move_focused_to_workspace_targets_named_monitor() {
    let mut hub = setup();
    hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.insert_tiling(hub.current_workspace(), titled("w0"));

    hub.move_focused_to_workspace("2", Some("external"));

    // The window is now on external's "2", shown by focusing that workspace.
    hub.focus_workspace("2", Some("external"));
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(0), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
      )
    "#);
}

#[test]
fn move_to_detached_monitor_deposits_into_parked_workspace() {
    let mut hub = setup();
    let primary = hub.focused_monitor();

    // A "1" on B carrying its own window, parked onto the primary after B
    // unplugs. Its frozen origin is B's name, so the detached selector can find it.
    let b = hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_workspace("1", Some("external"));
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.remove_monitor(b, primary);

    // A window on the primary moved to B's "1" via B's frozen name lands in the
    // hidden parked workspace. Visiting that workspace by B's name surfaces it,
    // showing the moved window sitting there alongside B's own bwin.
    hub.insert_tiling(hub.current_workspace(), titled("mover"));
    hub.move_focused_to_workspace("1", Some("external"));
    hub.focus_workspace("1", Some("external"));
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=100.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=50.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=0.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[bwin, bwin, mover])
      )

    +------------------------------------------------++------------------------------------------------+**************************************************
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                       W0                       ||                       W1                       |*                       W2                       *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    |                                                ||                                                |*                                                *
    +------------------------------------------------++------------------------------------------------+**************************************************
    ");

    // Replugging B re-homes its "1", so the deposited window travels back and
    // shows on the returning monitor alongside bwin.
    hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_workspace("1", Some("external"));
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=216.67, y=0.00, w=33.33, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=183.33, y=0.00, w=33.33, h=30.00)
        Window(id=WindowId(0), x=150.00, y=0.00, w=33.33, h=30.00)
        Container(id=ContainerId(0), x=150.00, y=0.00, w=100.00, h=30.00, titles=[bwin, bwin, mover])
      )
    "#);
}

#[test]
fn move_to_workspace_on_same_monitor() {
    let mut hub = setup();
    let primary = hub.focused_monitor();

    // A "1" on B carrying a window, parked onto the primary after B unplugs.
    let b = hub.add_monitor("external".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.insert_tiling(hub.current_workspace(), titled("bwin"));
    hub.remove_monitor(b, primary);

    // A move to "1" from the primary builds (or reuses) the primary's own
    // attached "1", never the hidden parked one, so the snapshot shows mover on
    // the primary while bwin stays out of view in the parked workspace.
    hub.focus_monitor(&MonitorTarget::Name("primary".to_string()));
    hub.insert_tiling(hub.current_workspace(), titled("mover"));
    hub.move_focused_to_workspace("1", None);
    hub.focus_workspace("1", None);
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
fn focus_detached_monitor_no_parked_match_is_noop() {
    // A detached origin monitor with no parked workspace of the requested name
    // does nothing, because a workspace cannot be created on a monitor that is
    // gone.
    let mut hub = setup();
    let primary = hub.focused_monitor();
    let dell = hub.add_monitor("DELL".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.remove_monitor(dell, primary);

    let before_focus = hub.focused_monitor();
    let before_current = hub.current_workspace();
    hub.focus_workspace("3", Some("DELL"));
    assert!(hub.query_workspaces().iter().all(|w| w.name != "3"));
    assert_eq!(hub.focused_monitor(), before_focus);
    assert_eq!(hub.current_workspace(), before_current);

    // A parked workspace under the detached origin monitor but with a different
    // name is still a miss, so the selector remains a no-op.
    let mut hub = setup();
    let primary = hub.focused_monitor();
    let dell = hub.add_monitor("DELL".to_string(), dim_at(150.0, 0.0), 1.0);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("5", None);
    hub.insert_tiling(hub.current_workspace(), titled("dwin"));
    hub.remove_monitor(dell, primary);

    let before_focus = hub.focused_monitor();
    let before_current = hub.current_workspace();
    hub.focus_workspace("3", Some("DELL"));
    assert!(hub.query_workspaces().iter().all(|w| w.name != "3"));
    assert_eq!(hub.focused_monitor(), before_focus);
    assert_eq!(hub.current_workspace(), before_current);
}
