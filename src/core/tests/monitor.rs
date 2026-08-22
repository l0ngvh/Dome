use insta::assert_snapshot;

use super::LayoutConfigBuilder;
#[cfg(target_os = "windows")]
use super::{PartitionTreeConfigBuilder, TestHubBuilder};
use crate::action::{MonitorTarget, WorkspaceState};
#[cfg(target_os = "windows")]
use crate::config::SizeConstraint;
use crate::core::GlobalLayoutConfig;
#[cfg(target_os = "windows")]
use crate::core::hub::MonitorLayout;
#[cfg(target_os = "windows")]
use crate::core::node::Pixels;
use crate::core::node::{PixelRect, WindowRestrictions};

use crate::core::tests::{
    default_rect, focused_monitor_name, reported_monitor, setup, setup_with_layout, snapshot,
    snapshot_text, titled, titled_matcher, work_area_at,
};

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn add_monitor_creates_workspace_on_new_monitor() {
    let mut hub = setup();
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "monitor-1".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

    hub.focus_monitor(&MonitorTarget::Name("monitor-1".to_string()));
    hub.focus_workspace("0", None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);

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
    hub.insert_window(titled("a1"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "external".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

    // Reaching the same name on the external monitor via the monitor selector
    // resolves a distinct workspace there, not the primary's own "1".
    hub.focus_workspace("1", Some("external"));
    hub.insert_window(titled("b1"), default_rect(), WindowRestrictions::None);

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
    let external = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.focus_monitor(&MonitorTarget::Name("primary".to_string()));

    hub.remove_monitor(external);

    // Focus stays on primary because the removed monitor was not focused.
    assert_eq!(focused_monitor_name(&hub), "primary");
}

#[test]
fn replugging_monitor_moves_workspaces_back_to_it() {
    let mut hub = setup();
    let b = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    // Two sibling workspaces on B, each with a distinctly-titled window, so the
    // placement shows exactly which of B's workspaces are live at each stage.
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.insert_window(titled("b1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("b2"), default_rect(), WindowRestrictions::None);
    hub.focus_workspace("2", None);
    hub.insert_window(titled("b3"), default_rect(), WindowRestrictions::None);

    // Stage 1: B is present with both windows visible.
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=150.00, y=0.00, w=100.00, h=30.00, highlighted, spawn=right)
      )
    "#);

    // Stage 2: unplug B. Its workspaces park and hide, so b1 and b2 are absent.
    hub.remove_monitor(b);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
    ");

    // Stage 3: replug with the same device name and position so the returning
    // monitor recomputes to B's old name. Both parked workspaces re-home onto
    // it, so no fresh "0" is minted. Visiting each by name shows the reattached
    // window on screen.
    hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
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
}

#[test]
fn replug_cycles_do_not_accumulate_default_workspaces() {
    let mut hub = setup();
    let external = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    // A window on external's own workspace, so a returning workspace stays
    // distinguishable from a freshly minted one.
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.insert_window(titled("e1"), default_rect(), WindowRestrictions::None);

    hub.remove_monitor(external);
    let external = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    // The second cycle is the one that matters. A default minted on the first
    // replug would park here and return alongside the next one.
    hub.remove_monitor(external);
    hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    let rows: Vec<_> = hub
        .query_workspaces()
        .into_iter()
        .filter(|w| w.monitor == "external")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].name, "0");
    assert_eq!(rows[0].window_count, 1);
    assert!(rows[0].is_visible);
}

#[test]
fn replug_shows_a_returning_workspace_that_holds_windows() {
    let mut hub = setup();
    let external = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    // external keeps its empty default "0" and puts its window on "2", so the
    // lowest-named returning workspace is not the one worth showing.
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("2", None);
    hub.insert_window(titled("e1"), default_rect(), WindowRestrictions::None);

    hub.remove_monitor(external);
    hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    // Landing on the empty "0" while the window sits on "2" reads as the replug
    // having lost it, so the workspace holding windows is the one shown.
    let visible: Vec<_> = hub
        .query_workspaces()
        .into_iter()
        .filter(|w| w.monitor == "external" && w.is_visible)
        .collect();
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].name, "2");
    assert_eq!(visible[0].window_count, 1);
}

#[test]
fn parked_workspace_is_not_reachable_by_name() {
    let mut hub = setup();

    // Primary's own attached "1" with a distinctly-titled window. Its id is the
    // focused workspace right after creation, captured behaviorally.
    hub.focus_workspace("1", None);
    hub.insert_window(titled("own1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("own2"), default_rect(), WindowRestrictions::None);
    let primary_native_1 = hub.current_workspace();

    // A distinct "1" on B carrying a different window, parked onto primary after
    // unplug.
    let b = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.insert_window(titled("b1"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("b2"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("b3"), default_rect(), WindowRestrictions::None);
    hub.remove_monitor(b);

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
    hub.add_monitor(reported_monitor(
        "second".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    let third = hub.add_monitor(reported_monitor(
        "third".to_string(),
        work_area_at(300, 0),
        1.0,
    ));

    // Focus a non-primary monitor and give it a distinctly-titled window so its
    // presence in the placement tracks whether its workspace is live.
    hub.focus_monitor(&MonitorTarget::Name("third".to_string()));
    assert_eq!(focused_monitor_name(&hub), "third");

    hub.remove_monitor(third);

    // Focus follows to the primary after its monitor is unplugged.
    assert_eq!(focused_monitor_name(&hub), "primary");
}

#[test]
fn visiting_parked_workspace_shows_its_windows() {
    let mut hub = setup();

    // Primary's own attached "0" with a distinctly-titled window, captured for
    // the name-resolution check. Its presence in the placement tracks whether
    // the primary is showing its own workspace or a visitor.
    hub.insert_window(titled("ownw"), default_rect(), WindowRestrictions::None);

    // B with a window, then unplug so B's workspace parks onto primary.
    let b = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.insert_window(titled("bwin"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("bwin1"), default_rect(), WindowRestrictions::None);
    hub.remove_monitor(b);

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
        Window(id=WindowId(2), x=75.00, y=0.00, w=75.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=0.00, y=0.00, w=75.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[bwin, bwin1])
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
    |                                    W1                                   |*                                    W2                                   *
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
    let b = hub.add_monitor(reported_monitor(
        "beta".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.add_monitor(reported_monitor(
        "gamma".to_string(),
        work_area_at(300, 0),
        1.0,
    ));

    // A window on gamma's own workspace so gamma stays visible throughout.
    hub.focus_monitor(&MonitorTarget::Name("gamma".to_string()));
    hub.insert_window(titled("cwin"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("cigwin"), default_rect(), WindowRestrictions::None);

    // Window on B, then unplug only B so its workspace parks onto primary.
    hub.focus_monitor(&MonitorTarget::Name("beta".to_string()));
    hub.insert_window(titled("bwin"), default_rect(), WindowRestrictions::None);
    hub.remove_monitor(b);

    // Reach the parked workspace by name plus its origin monitor's name.
    hub.focus_workspace("0", Some("beta"));

    // The visitor bwin is on the primary, cwin on gamma.
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Monitor(id=MonitorId(2), name="gamma", screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=350.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=300.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cigwin])
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
    "#);

    // Focus a workspace on a different surviving monitor. The primary's active
    // workspace never switches off the visitor, so bwin stays on screen.
    hub.focus_monitor(&MonitorTarget::Name("gamma".to_string()));
    hub.focus_workspace("0", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(2), x=0.00, y=0.00, w=150.00, h=30.00)
      )
      Monitor(id=MonitorId(2), name="gamma", screen=(x=300.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=350.00, y=0.00, w=50.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(0), x=300.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=300.00, y=0.00, w=100.00, h=30.00, titles=[cwin, cigwin])
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
    |                                                                         W2                                                                         |
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
fn workspace_returns_to_the_right_monitor_among_same_named() {
    let mut hub = setup();

    // First same-named monitor, with a window on a distinctly named workspace
    // so it is unambiguous after replug.
    let d1 = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(200, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("7", None);
    hub.insert_window(titled("d1win"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("d1win"), default_rect(), WindowRestrictions::None);

    // Second same-named monitor to its right, reached by direction because the
    // shared device name resolves to the first one.
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(400, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("DELL #2".to_string()));
    hub.insert_window(titled("d2win"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("d2win"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("d2win"), default_rect(), WindowRestrictions::None);

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #1", screen=(x=200.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(1), x=250.00, y=0.00, w=50.00, h=30.00)
        Window(id=WindowId(0), x=200.00, y=0.00, w=50.00, h=30.00)
        Container(id=ContainerId(0), x=200.00, y=0.00, w=100.00, h=30.00, titles=[d1win, d1win])
      )
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=400.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(4), x=467.00, y=0.00, w=33.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=433.00, y=0.00, w=34.00, h=30.00)
        Window(id=WindowId(2), x=400.00, y=0.00, w=33.00, h=30.00)
        Container(id=ContainerId(1), x=400.00, y=0.00, w=100.00, h=30.00, titles=[d2win, d2win, d2win])
      )
    "#);

    // Unplug the first DELL. Its workspace parks onto primary so d1win drops
    // out, and the lone survivor is restamped back to the bare "DELL".
    hub.remove_monitor(d1);
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=WindowId(4))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL", screen=(x=400.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(4), x=467.00, y=0.00, w=33.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(3), x=433.00, y=0.00, w=34.00, h=30.00)
        Window(id=WindowId(2), x=400.00, y=0.00, w=33.00, h=30.00)
        Container(id=ContainerId(1), x=400.00, y=0.00, w=100.00, h=30.00, titles=[d2win, d2win, d2win])
      )
    "#);

    // Replug a DELL positioned so it recomputes to "DELL #1" again. The parked
    // workspace re-homes onto it only if it remembered the disambiguated #1
    // origin rather than the bare "DELL", so d1win returning proves the frozen
    // origin carried the pre-removal disambiguated name.
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(200, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("DELL #1".to_string()));
    hub.focus_workspace("7", None);
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=400.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(4), x=467.00, y=0.00, w=33.00, h=30.00)
        Window(id=WindowId(3), x=433.00, y=0.00, w=34.00, h=30.00)
        Window(id=WindowId(2), x=400.00, y=0.00, w=33.00, h=30.00)
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

    // Two identically-named monitors at distinct positions, so they collide on
    // device name and disambiguate to "DELL #1" (leftmost) / "DELL #2". Each
    // carries a distinctly-titled window on its own named workspace, so the
    // placement tracks exactly which monitor's workspace is live at each stage.
    // The first DELL is focused by name while it is the only DELL, so the name
    // still resolves. The second is reached by direction because once both are
    // present the shared name has disambiguated and no longer matches either.
    let d1 = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(200, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("a", None);
    hub.insert_window(titled("d1win"), default_rect(), WindowRestrictions::None);
    let d2 = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(400, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Right);
    hub.focus_workspace("b", None);
    hub.insert_window(titled("d2win"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("d2win"), default_rect(), WindowRestrictions::None);

    // Two explicit successive removals in a TEST-PINNED order. Production
    // platform iteration over the removed monitors has no guaranteed order, so
    // the test pins it to make the assertion deterministic. Removing "DELL #1"
    // first freezes its workspace's origin as "DELL #1", then the tail recompute
    // restamps the lone survivor from "DELL #2" down to bare "DELL", so the
    // second removal freezes that workspace's origin as bare "DELL".
    hub.remove_monitor(d1);
    hub.remove_monitor(d2);

    // A single DELL plugged back in recomputes to bare "DELL". Only the
    // second-removed monitor's workspace (frozen bare "DELL") matches and
    // reattaches. The first-removed's (frozen "DELL #1") stays parked. Focusing
    // the returning monitor's "b" shows d2win while d1win stays hidden, which
    // proves the per-call recompute made a single replug reclaim only the
    // last-removed monitor's workspace.
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(200, 0),
        1.0,
    ));
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
    hub.insert_window(titled("w5"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w6"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "external".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

    let primary = hub.primary_monitor();
    hub.update_monitor(
        primary,
        reported_monitor("primary".to_string(), PixelRect::new(0, 0, 200, 50), 1.0),
        None,
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
    hub.insert_window(titled("w7"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "right-monitor".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

    hub.add_monitor(reported_monitor(
        "bottom-monitor".to_string(),
        PixelRect::new(0, 30, 150, 30),
        1.0,
    ));

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

    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    let after_name = snapshot_text(&hub);
    hub.focus_monitor(&MonitorTarget::Name("right-monitor".to_string()));
    assert_eq!(snapshot_text(&hub), after_name);
}

#[test]
fn focus_monitor_by_name() {
    let mut hub = setup();
    hub.insert_window(titled("w8"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "external".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

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
    hub.insert_window(titled("w9"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w10"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "right-monitor".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

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
    hub.insert_window(titled("w11"), default_rect(), WindowRestrictions::None);

    hub.add_monitor(reported_monitor(
        "external".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

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
    let mut hub = setup_with_layout(layout_floating(&["w12"]));
    hub.insert_window(
        titled("w12"),
        PixelRect::new(10, 10, 50, 20),
        WindowRestrictions::None,
    )
    .unwrap();

    hub.add_monitor(reported_monitor(
        "external".to_string(),
        PixelRect::new(150, 0, 100, 30),
        1.0,
    ));

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
        hub.insert_window(titled("w13"), default_rect(), WindowRestrictions::None);
        let before = snapshot_text(&hub);
        hub.focus_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    {
        let mut hub = setup();
        hub.insert_window(titled("w14"), default_rect(), WindowRestrictions::None);
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }

    // The hub starts focused on the monitor named "primary".
    {
        let mut hub = setup();
        hub.insert_window(titled("w15"), default_rect(), WindowRestrictions::None);
        hub.add_monitor(reported_monitor(
            "external".to_string(),
            PixelRect::new(150, 0, 100, 30),
            1.0,
        ));
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Name("primary".to_string()));
        assert_eq!(snapshot_text(&hub), before);
    }

    {
        let mut hub = setup();
        hub.add_monitor(reported_monitor(
            "right-monitor".to_string(),
            PixelRect::new(150, 0, 100, 30),
            1.0,
        ));
        let before = snapshot_text(&hub);
        hub.move_focused_to_monitor(&MonitorTarget::Right);
        assert_eq!(snapshot_text(&hub), before);
    }
}

#[test]
fn unique_name_unique_stays_bare() {
    // Two monitors with distinct device names: neither gets a suffix, so the
    // snapshot proves the bare/unsuffixed case for a distinct-named monitor.
    let mut hub = setup();
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(1920, 0),
        1.0,
    ));
    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
    "#);
}

#[test]
fn unique_name_colliders_numbered() {
    let mut hub = setup();
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(0, 0),
        1.0,
    ));
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(1920, 0),
        1.0,
    ));

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
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(1920, 0),
        1.0,
    ));
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(0, 0),
        1.0,
    ));

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #2", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #1", screen=(x=0.00 y=0.00 w=100.00 h=30.00))
    "#);

    // Same x, so y breaks the tie: topmost (smaller y) ranks first.
    let mut hub = setup();
    hub.add_monitor(reported_monitor(
        "ACER".to_string(),
        work_area_at(0, 1080),
        1.0,
    ));
    hub.add_monitor(reported_monitor(
        "ACER".to_string(),
        work_area_at(0, 0),
        1.0,
    ));

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
    let first = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(0, 0),
        1.0,
    ));
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(1920, 0),
        1.0,
    ));

    assert_snapshot!(snapshot_text(&hub), @r#"
    Hub(focused=None)
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(1), name="DELL #1", screen=(x=0.00 y=0.00 w=100.00 h=30.00))
      Monitor(id=MonitorId(2), name="DELL #2", screen=(x=1920.00 y=0.00 w=100.00 h=30.00))
    "#);

    // Move the current #1 to the right of its sibling: the ranks must swap.
    hub.update_monitor(
        first,
        reported_monitor("DELL".to_string(), work_area_at(3840, 0), 1.0),
        None,
    );

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
                .with_tab_bar_height(Pixels::new(5))
                .with_automatic_tiling(true)
                .build(),
        )
        .build();
    let mut hub = TestHubBuilder::new().with_scale(2.0).with_layout(l).build();
    hub.insert_window(titled("w16"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w17"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=0.00, y=10.00, w=150.00, h=20.00, highlighted, spawn=right)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, tabbed, active_tab=1, titles=[w16, w17])
      )
    ");

    let monitor_id = hub.primary_monitor();
    hub.update_monitor(
        monitor_id,
        reported_monitor("primary".to_string(), PixelRect::new(0, 0, 1000, 1000), 3.0),
        None,
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
    use crate::core::node::Pixels;

    let mut hub = TestHubBuilder::new()
        .with_scale(2.0)
        .with_layout(
            LayoutConfigBuilder::new()
                .with_partition_tree_config(
                    PartitionTreeConfigBuilder::new()
                        .with_tab_bar_height(Pixels::new(10))
                        .with_automatic_tiling(false)
                        .build(),
                )
                .with_min_width(SizeConstraint::Pixels(Pixels::new(40)))
                .build(),
        )
        .build();
    for i in 0..6 {
        hub.insert_window(
            titled(format!("w{i}").as_str()),
            default_rect(),
            WindowRestrictions::None,
        );
    }
    assert_snapshot!(snapshot_text(&hub), @r"
    Hub(focused=WindowId(5))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(5), x=70.00, y=0.00, w=80.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(4), x=0.00, y=0.00, w=70.00, h=30.00)
        Container(id=ContainerId(0), x=0.00, y=0.00, w=150.00, h=30.00, titles=[w0, w1, w2, w3, w4, w5])
      )
    ");

    let monitor_id = hub.primary_monitor();
    hub.update_monitor(
        monitor_id,
        reported_monitor("primary".to_string(), PixelRect::new(0, 0, 500, 1000), 3.0),
        None,
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

/// Only `Unit = Physical` scales the tab bar height, so Windows is the one target where an
/// integral configured height still yields a fractional band. The scale and odd work area put
/// the origin and the band height on half units, where `round(y) + round(h)` diverges from the
/// `round(y + h)` the content box uses.
#[cfg(target_os = "windows")]
#[test]
fn tabbed_band_bottom_lands_on_the_content_top() {
    let mut hub = setup_with_layout(
        LayoutConfigBuilder::new()
            .with_partition_tree_config(
                PartitionTreeConfigBuilder::new()
                    .with_tab_bar_height(Pixels::new(25))
                    .build(),
            )
            .build(),
    );
    let monitor_id = hub.primary_monitor();
    hub.update_monitor(
        monitor_id,
        reported_monitor("primary".to_string(), PixelRect::new(0, 0, 1000, 201), 1.5),
        None,
    );

    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("w1"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w2"), default_rect(), WindowRestrictions::None);
    hub.toggle_spawn_mode();
    hub.insert_window(titled("w3"), default_rect(), WindowRestrictions::None);
    hub.toggle_container_layout();

    let placements = hub.get_visible_placements();
    let active_tab = placements.focused_window.expect("focus on the active tab");
    let MonitorLayout::Normal {
        tiling_windows,
        containers,
        ..
    } = &placements.monitors[0].layout
    else {
        panic!("expected a normally tiled monitor");
    };
    let tabbed = containers
        .iter()
        .find(|c| c.is_tabbed)
        .expect("a tabbed container");
    let content_top = tiling_windows
        .iter()
        .find(|w| w.id == active_tab)
        .expect("the active tab is placed")
        .border_box
        .y();

    assert_eq!(tabbed.tab_bar_band.bottom(), content_top);
}

#[test]
fn move_focused_to_workspace_targets_named_monitor() {
    let mut hub = setup();
    hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);

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

    // A "1" on B carrying its own window, parked onto the primary after B
    // unplugs. Its frozen origin is B's name, so the detached selector can find it.
    let b = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_workspace("1", Some("external"));
    hub.insert_window(titled("bwin"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("bwin"), default_rect(), WindowRestrictions::None);
    hub.remove_monitor(b);

    // A window on the primary moved to B's "1" via B's frozen name lands in the
    // hidden parked workspace. Visiting that workspace by B's name surfaces it,
    // showing the moved window sitting there alongside B's own bwin.
    hub.insert_window(titled("mover"), default_rect(), WindowRestrictions::None);
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
    hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_workspace("1", Some("external"));
    assert_snapshot!(snapshot(&hub), @r#"
    Hub(focused=WindowId(2))
      Monitor(id=MonitorId(0), name="primary", screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Monitor(id=MonitorId(2), name="external", screen=(x=150.00 y=0.00 w=100.00 h=30.00),
        Window(id=WindowId(2), x=217.00, y=0.00, w=33.00, h=30.00, highlighted, spawn=right)
        Window(id=WindowId(1), x=183.00, y=0.00, w=34.00, h=30.00)
        Window(id=WindowId(0), x=150.00, y=0.00, w=33.00, h=30.00)
        Container(id=ContainerId(0), x=150.00, y=0.00, w=100.00, h=30.00, titles=[bwin, bwin, mover])
      )
    "#);
}

#[test]
fn move_to_workspace_on_same_monitor() {
    let mut hub = setup();

    // A "1" on B carrying a window, parked onto the primary after B unplugs.
    let b = hub.add_monitor(reported_monitor(
        "external".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("external".to_string()));
    hub.focus_workspace("1", None);
    hub.insert_window(titled("bwin"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("bwin"), default_rect(), WindowRestrictions::None);
    hub.remove_monitor(b);

    // A move to "1" from the primary builds (or reuses) the primary's own
    // attached "1", never the hidden parked one, so the snapshot shows mover on
    // the primary while bwin stays out of view in the parked workspace.
    hub.focus_monitor(&MonitorTarget::Name("primary".to_string()));
    hub.insert_window(titled("mover"), default_rect(), WindowRestrictions::None);
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
    let dell = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.remove_monitor(dell);

    let before_current = hub.current_workspace();
    hub.focus_workspace("3", Some("DELL"));
    assert!(hub.query_workspaces().iter().all(|w| w.name != "3"));
    assert_eq!(hub.current_workspace(), before_current);

    // A parked workspace under the detached origin monitor but with a different
    // name is still a miss, so the selector remains a no-op.
    let mut hub = setup();
    let dell = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(150, 0),
        1.0,
    ));
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("5", None);
    hub.insert_window(titled("dwin"), default_rect(), WindowRestrictions::None);
    hub.remove_monitor(dell);

    let before_current = hub.current_workspace();
    hub.focus_workspace("3", Some("DELL"));
    assert!(hub.query_workspaces().iter().all(|w| w.name != "3"));
    assert_eq!(hub.current_workspace(), before_current);
}

#[test]
fn renaming_into_a_taken_name_ranks_both_and_renaming_back_restores_them() {
    let mut hub = setup();
    let primary = hub.primary_monitor();
    hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    hub.update_monitor(
        primary,
        reported_monitor("DELL".to_string(), PixelRect::new(0, 0, 150, 30), 1.0),
        None,
    );
    let mut ranked: Vec<String> = hub
        .query_monitors()
        .into_iter()
        .map(|m| m.unique_name)
        .collect();
    ranked.sort();
    assert_eq!(ranked, ["DELL #1", "DELL #2"]);

    hub.update_monitor(
        primary,
        reported_monitor("primary".to_string(), PixelRect::new(0, 0, 150, 30), 1.0),
        None,
    );
    let mut restored: Vec<String> = hub
        .query_monitors()
        .into_iter()
        .map(|m| m.unique_name)
        .collect();
    restored.sort();
    assert_eq!(restored, ["DELL", "primary"]);
}

#[test]
fn primary_change_onto_an_occupied_display_parks_the_displaced_under_its_bare_name() {
    let mut hub = setup();
    let dell = hub.add_monitor(reported_monitor(
        "DELL".to_string(),
        work_area_at(150, 0),
        1.0,
    ));

    hub.focus_workspace("p", None);
    hub.insert_window(titled("pwin"), default_rect(), WindowRestrictions::None);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    hub.focus_workspace("d", None);
    hub.insert_window(titled("dwin"), default_rect(), WindowRestrictions::None);

    let primary = hub.primary_monitor();
    hub.update_monitor(
        primary,
        reported_monitor("DELL".to_string(), PixelRect::new(0, 0, 150, 30), 1.0),
        Some(dell),
    );

    // The displaced monitor is gone and the primary answers to its name, so
    // both rows report "DELL". The states tell them apart.
    let parked: Vec<_> = hub
        .query_workspaces()
        .into_iter()
        .filter(|w| w.state == WorkspaceState::Parked)
        .collect();
    assert_eq!(parked.len(), 2);
    // DELL's own default workspace parks alongside "d". Both froze against the
    // same departing monitor, so both report its bare name. Bare, not position
    // ranked: renaming before the removal would have collided the two monitors
    // and frozen a suffixed origin here.
    assert!(parked.iter().any(|w| w.name == "d"));
    assert!(parked.iter().all(|w| w.monitor == "DELL"));

    let carried: Vec<_> = hub
        .query_workspaces()
        .into_iter()
        .filter(|w| w.state == WorkspaceState::Attached && w.window_count > 0)
        .collect();
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].name, "p");
    assert_eq!(carried[0].monitor, "DELL");
}

#[test]
fn primary_change_onto_an_untracked_display_carries_the_workspaces() {
    let mut hub = setup();
    let primary = hub.primary_monitor();
    hub.focus_workspace("p", None);
    hub.insert_window(titled("pwin"), default_rect(), WindowRestrictions::None);

    hub.update_monitor(
        primary,
        reported_monitor("DELL".to_string(), PixelRect::new(0, 0, 150, 30), 1.0),
        None,
    );

    let names: Vec<String> = hub
        .query_monitors()
        .into_iter()
        .map(|m| m.unique_name)
        .collect();
    assert_eq!(names, ["DELL"]);
    hub.focus_monitor(&MonitorTarget::Name("DELL".to_string()));
    assert_eq!(focused_monitor_name(&hub), "DELL");

    assert!(
        hub.query_workspaces()
            .iter()
            .all(|w| w.state == WorkspaceState::Attached)
    );
    let carried: Vec<_> = hub
        .query_workspaces()
        .into_iter()
        .filter(|w| w.window_count > 0)
        .collect();
    assert_eq!(carried.len(), 1);
    assert_eq!(carried[0].name, "p");
    assert_eq!(carried[0].monitor, "DELL");
}

#[test]
#[should_panic(expected = "must not be the rental host primary")]
fn primary_change_whose_displaced_is_the_primary_panics() {
    let mut hub = setup();
    let primary = hub.primary_monitor();

    hub.update_monitor(
        primary,
        reported_monitor("DELL".to_string(), PixelRect::new(0, 0, 150, 30), 1.0),
        Some(primary),
    );
}

#[test]
#[should_panic(expected = "must not be the rental host primary")]
fn removing_the_primary_monitor_panics() {
    let mut hub = setup();
    let primary = hub.primary_monitor();
    hub.remove_monitor(primary);
}
