//! Hub workspace and monitor behavior, exercised on the default partition-tree strategy.

use insta::assert_snapshot;

use crate::tests::fixtures::{TestDome, monitor, window};

#[test]
fn add_monitor_with_coincident_work_area_keeps_windows_separate() {
    let mut dome = TestDome::new("return {}");
    dome.update_monitor(
        dome.primary_monitor(),
        monitor("primary", [0, 0, 20, 6]),
        None,
    );
    dome.open(window("home"));
    dome.add_monitor(monitor("DELL", [0, 0, 20, 6]));
    dome.run("focus monitor DELL");
    dome.open(window("away"));
    assert_snapshot!(dome.snapshot(), @"
    focused away on DELL
    workspaces: 0 on primary (visible), 0 on DELL (focused)
    primary 0,0 20x6 border 1
      home 0,0 20x6
    DELL 0,0 20x6 border 1
      away 0,0 20x6 highlighted spawn=right
    +------------------+
    |                  |
    |       home       |
    |                  |
    |                  |
    +------------------+

    ********************
    *                  *
    *       away       *
    *                  *
    *                  *
    ********************
    ");
}

#[test]
fn update_monitor_with_new_work_area_lays_out_windows_again() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    let primary = dome.primary_monitor();
    dome.update_monitor(primary, monitor("primary", [0, 0, 60, 10]), None);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 60x10 border 1
      w0 0,0 30x10
      w1 30,0 30x10 highlighted spawn=right
    +----------------------------+******************************
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |             w0             |*             w1             *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    |                            |*                            *
    +----------------------------+******************************
    ");
}

#[test]
fn add_or_update_monitor_suffixes_shared_device_names_by_position() {
    let mut dome = TestDome::new("return {}");
    dome.add_monitor(monitor("DELL", [120, 0, 20, 4]));
    let bare_names = dome.snapshot();
    assert_snapshot!(bare_names, @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
    DELL 120,0 20x4 border 1
    ");

    let primary = dome.primary_monitor();
    dome.update_monitor(primary, monitor("DELL", [0, 0, 100, 20]), None);
    assert_snapshot!(dome.snapshot(), @"
    focused none on DELL #1
    workspaces: 0 on DELL #1 (focused), 0 on DELL #2 (visible)
    DELL #1 0,0 100x20 border 1
    DELL #2 120,0 20x4 border 1
    ");
    dome.update_monitor(primary, monitor("primary", [0, 0, 100, 20]), None);
    assert_eq!(dome.snapshot(), bare_names);

    let dell = dome.add_monitor(monitor("DELL", [100, 0, 20, 4]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL #2 (visible), 0 on DELL #1 (visible)
    primary 0,0 100x20 border 1
    DELL #1 100,0 20x4 border 1
    DELL #2 120,0 20x4 border 1
    ");
    dome.update_monitor(dell, monitor("DELL", [140, 0, 20, 4]), None);
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL #1 (visible), 0 on DELL #2 (visible)
    primary 0,0 100x20 border 1
    DELL #1 120,0 20x4 border 1
    DELL #2 140,0 20x4 border 1
    ");
    dome.add_monitor(monitor("ACER", [160, 4, 20, 4]));
    dome.add_monitor(monitor("ACER", [160, 0, 20, 4]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL #1 (visible), 0 on DELL #2 (visible), 0 on ACER #2 (visible), 0 on ACER #1 (visible)
    primary 0,0 100x20 border 1
    DELL #1 120,0 20x4 border 1
    DELL #2 140,0 20x4 border 1
    ACER #1 160,0 20x4 border 1
    ACER #2 160,4 20x4 border 1
    ");
}

#[test]
fn add_monitor_under_reserved_area_ranks_and_orders_by_system_work_area() {
    let mut dome = TestDome::new(
        "return { reserved_area = function(monitor)
            if monitor.name == 'DELL #1' then return { left = 10 } end
            return {}
        end }",
    );
    dome.add_monitor(monitor("DELL", [100, 0, 20, 4]));
    dome.add_monitor(monitor("DELL", [100, 4, 20, 4]));
    dome.add_monitor(monitor("LG", [120, 0, 20, 4]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL #1 (visible), 0 on DELL #2 (visible), 0 on LG (visible)
    primary 0,0 100x20 border 1
    DELL #1 110,0 10x4 border 1
    DELL #2 100,4 20x4 border 1
    LG 120,0 20x4 border 1
    ");
}

#[test]
fn update_monitor_renaming_primary_carries_its_workspaces_and_parks_displaced_ones() {
    let mut dome = TestDome::new("return {}");
    dome.run("focus workspace p");
    dome.open(window("w0"));
    let primary = dome.primary_monitor();
    dome.update_monitor(primary, monitor("LG", [0, 0, 100, 20]), None);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on LG
    workspaces: p on LG (focused)
    LG 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");

    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.open(window("w1"));
    dome.run("focus workspace d");
    dome.open(window("w2"));
    dome.update_monitor(primary, monitor("DELL", [0, 0, 100, 20]), Some(dell));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on DELL
    workspaces: p on DELL (focused), 0 parked from DELL, d parked from DELL
    DELL 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");
}

#[test]
fn query_monitors_lists_left_to_right_with_platform_identifiers() {
    let mut dome = TestDome::new("return {}");
    dome.add_monitor(monitor("DELL", [140, 0, 20, 4]).cg_display_id(11));
    dome.add_monitor(monitor("DELL", [100, 0, 20, 4]).cg_display_id(12));
    dome.add_monitor(monitor("DELL", [120, 0, 20, 4]).cg_display_id(13));
    let primary = dome.primary_monitor();
    dome.update_monitor(
        primary,
        monitor("primary", [0, 0, 100, 20])
            .cg_display_id(1)
            .gdi_device("DISPLAY1"),
        None,
    );
    dome.update_monitor(
        primary,
        monitor("primary", [0, 0, 100, 20])
            .cg_display_id(1)
            .gdi_device("DISPLAY2"),
        None,
    );
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL #3 (visible), 0 on DELL #1 (visible), 0 on DELL #2 (visible)
    primary 0,0 100x20 border 1 cg_display_id 1 gdi_device DISPLAY2
    DELL #1 100,0 20x4 border 1 cg_display_id 12
    DELL #2 120,0 20x4 border 1 cg_display_id 13
    DELL #3 140,0 20x4 border 1 cg_display_id 11
    ");
}

#[cfg(target_os = "windows")]
#[test]
fn update_monitor_with_only_new_scale_lays_out_windows_again() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("toggle layout");
    let primary = dome.primary_monitor();
    dome.update_monitor(
        primary,
        monitor("primary", [0, 0, 100, 20]).scale(2.0),
        None,
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 100x20 border 2
      w1 0,4 100x16 highlighted spawn=right
      container 0,0 100x20 tabbed active=1 titles=[w0, w1]
    +--------------------------------------------------------------------------------------------------+
    |                                                 |                                                |
    |                       w0                        |                     [w1]                       |
    |                                                 |                                                |
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    ****************************************************************************************************
    ");
}

#[test]
fn move_to_workspace_on_named_monitor_lands_there_or_in_its_parked_workspace() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    let before = dome.snapshot();
    dome.run("move workspace 2 --monitor missing");
    assert_eq!(dome.snapshot(), before);
    dome.run("move workspace 2 --monitor DELL");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible), 2 on DELL
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    DELL 100,0 40x10 border 1
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");

    dome.remove_monitor(dell);
    let before = dome.snapshot();
    dome.run("move workspace absent --monitor DELL");
    assert_eq!(dome.snapshot(), before);
    dome.run("move workspace 2 --monitor DELL");
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 2 on DELL (visible)
    primary 0,0 100x20 border 1
    DELL 100,0 40x10 border 1
      w0 120,0 20x10
      w1 100,0 20x10
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        

    +------------------++------------------+
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    |        w1        ||        w0        |
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    +------------------++------------------+
    ");
}

#[test]
fn remove_monitor_moves_focus_to_primary_only_when_it_held_focus() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 20, 4]));
    dome.add_monitor(monitor("LG", [120, 0, 20, 4]));
    let acer = dome.add_monitor(monitor("ACER", [140, 0, 20, 4]));
    dome.run("focus monitor DELL");
    dome.remove_monitor(acer);
    assert_snapshot!(dome.snapshot(), @"
    focused none on DELL
    workspaces: 0 on primary (visible), 0 on DELL (focused), 0 on LG (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
    DELL 100,0 20x4 border 1
    LG 120,0 20x4 border 1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
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
    ");

    dome.remove_monitor(dell);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on LG (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    LG 120,0 20x4 border 1
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");
}

#[test]
fn remove_monitor_parks_its_workspaces_and_hides_their_windows() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.run("focus workspace 1");
    dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("focus workspace 2");
    dome.open(window("w3"));
    dome.run("focus monitor left");
    dome.remove_monitor(dell);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 1 parked from DELL, 2 parked from DELL
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");
}

#[test]
fn add_monitor_after_remove_returns_parked_workspaces_and_shows_occupied_one() {
    for show_parked in [false, true] {
        let mut dome = TestDome::new("return {}");
        dome.open(window("w0"));
        let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
        dome.run("focus monitor DELL");
        dome.run("focus workspace 1");
        dome.open(window("w1"));
        dome.open(window("w2"));
        dome.run("focus workspace 2");
        dome.open(window("w3"));
        dome.run("focus monitor left");
        dome.remove_monitor(dell);
        if show_parked {
            dome.run("focus workspace 1 --monitor DELL");
        }
        dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
        insta::allow_duplicates! {
            assert_snapshot!(dome.snapshot(), @"
            focused w0 on primary
            workspaces: 0 on primary (focused), 1 on DELL (visible), 2 on DELL
            primary 0,0 100x20 border 1
              w0 0,0 100x20 highlighted spawn=right
            DELL 100,0 40x10 border 1
              w1 100,0 20x10
              w2 120,0 20x10
            ****************************************************************************************************
            *                                                                                                  *
            *                                                                                                  *
            *                                                                                                  *
            *                                                                                                  *
            *                                                                                                  *
            *                                                                                                  *
            *                                                                                                  *
            *                                                                                                  *
            *                                                w0                                                *
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

            +------------------++------------------+
            |                  ||                  |
            |                  ||                  |
            |                  ||                  |
            |        w1        ||        w2        |
            |                  ||                  |
            |                  ||                  |
            |                  ||                  |
            |                  ||                  |
            +------------------++------------------+
            ");
        }
    }
}

#[test]
fn focus_or_move_to_bare_workspace_name_skips_parked_workspace() {
    let mut dome = TestDome::new("return {}");
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.run("focus workspace 1");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus monitor left");
    dome.remove_monitor(dell);
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 1 parked from DELL, 1 on primary (focused)
    primary 0,0 100x20 border 1
    ");

    dome.run("focus workspace 0");
    dome.open(window("w2"));
    dome.run("move workspace 1");
    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 1 parked from DELL, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w2 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
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
    ");
}

#[test]
fn focus_workspace_on_unplugged_monitor_shows_its_parked_workspace_on_primary() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.add_monitor(monitor("LG", [140, 0, 40, 10]));
    dome.run("focus monitor LG");
    dome.open(window("w1"));
    dome.run("focus monitor DELL");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.remove_monitor(dell);
    dome.run("focus workspace 0 --monitor DELL");
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on primary
    workspaces: 0 on primary, 0 parked from DELL (focused), 0 on LG (visible)
    primary 0,0 100x20 border 1
      w2 0,0 50x20
      w3 50,0 50x20 highlighted spawn=right
    LG 140,0 40x10 border 1
      w1 140,0 40x10
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w2                       |*                       w3                       *
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

    +--------------------------------------+
    |                                      |
    |                                      |
    |                                      |
    |                  w1                  |
    |                                      |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+
    ");

    dome.run("focus monitor LG");
    dome.run("focus workspace 1");
    let visiting = dome.snapshot();
    assert_snapshot!(visiting, @"
    focused none on LG
    workspaces: 0 on primary, 0 parked from DELL (visible), 0 on LG, 1 on LG (focused)
    primary 0,0 100x20 border 1
      w2 0,0 50x20
      w3 50,0 50x20
    LG 140,0 40x10 border 1
    +------------------------------------------------++------------------------------------------------+
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                       w2                       ||                       w3                       |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    |                                                ||                                                |
    +------------------------------------------------++------------------------------------------------+
    ");
    dome.run("focus monitor primary");
    dome.run("focus workspace 0");
    dome.run("focus monitor LG");
    let before_missing = dome.snapshot();
    assert_snapshot!(before_missing, @"
    focused none on LG
    workspaces: 0 on primary (visible), 0 parked from DELL, 0 on LG, 1 on LG (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
    LG 140,0 40x10 border 1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
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
    ");
    dome.run("focus workspace 3 --monitor DELL");
    assert_eq!(dome.snapshot(), before_missing);
}

#[test]
fn add_monitor_sharing_device_name_returns_workspaces_parked_under_its_ranked_name() {
    let mut dome = TestDome::new("return {}");
    let first = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.run("focus workspace 7");
    dome.open(window("w0"));
    let second = dome.add_monitor(monitor("DELL", [140, 0, 40, 10]));
    dome.run("focus monitor right");
    dome.open(window("w1"));
    dome.remove_monitor(first);
    let returning = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on DELL #2
    workspaces: 0 on primary (visible), 7 on DELL #1 (visible), 0 on DELL #2 (focused)
    primary 0,0 100x20 border 1
    DELL #1 100,0 40x10 border 1
      w0 100,0 40x10
    DELL #2 140,0 40x10 border 1
      w1 140,0 40x10 highlighted spawn=right
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        

    +--------------------------------------+
    |                                      |
    |                                      |
    |                                      |
    |                  w0                  |
    |                                      |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+

    ****************************************
    *                                      *
    *                                      *
    *                                      *
    *                  w1                  *
    *                                      *
    *                                      *
    *                                      *
    *                                      *
    ****************************************
    ");

    dome.remove_monitor(returning);
    dome.remove_monitor(second);
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused), 7 parked from DELL #1, 0 on DELL (visible)
    primary 0,0 100x20 border 1
    DELL 100,0 40x10 border 1
      w1 100,0 40x10
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        

    +--------------------------------------+
    |                                      |
    |                                      |
    |                                      |
    |                  w1                  |
    |                                      |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+
    ");
}

#[test]
fn add_monitor_gets_own_workspace_zero_and_names_separate_from_other_monitors() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.run("focus workspace 1");
    dome.open(window("w1"));
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary, 1 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
      w1 0,0 100x20 highlighted spawn=right
    DELL 100,0 40x10 border 1
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w1                                                *
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
    ");

    dome.run("focus monitor DELL");
    dome.open(window("w2"));
    dome.run("focus workspace 1");
    dome.open(window("w3"));
    assert_snapshot!(dome.snapshot(), @"
    focused w3 on DELL
    workspaces: 0 on primary, 1 on primary (visible), 0 on DELL, 1 on DELL (focused)
    primary 0,0 100x20 border 1
      w1 0,0 100x20
    DELL 100,0 40x10 border 1
      w3 100,0 40x10 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w1                                                |
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

    ****************************************
    *                                      *
    *                                      *
    *                                      *
    *                  w3                  *
    *                                      *
    *                                      *
    *                                      *
    *                                      *
    ****************************************
    ");
}

#[test]
fn focus_monitor_by_direction_or_name_moves_keyboard_focus_there() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.add_monitor(monitor("DELL", [100, 0, 20, 4]));
    dome.add_monitor(monitor("LG", [0, 20, 20, 4]));
    let on_primary = dome.snapshot();
    assert_snapshot!(on_primary, @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible), 0 on LG (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    LG 0,20 20x4 border 1
    DELL 100,0 20x4 border 1
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");

    dome.run("focus monitor right");
    assert_snapshot!(dome.snapshot(), @"
    focused none on DELL
    workspaces: 0 on primary (visible), 0 on DELL (focused), 0 on LG (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
    LG 0,20 20x4 border 1
    DELL 100,0 20x4 border 1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
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
    ");
    dome.run("focus monitor left");
    assert_eq!(dome.snapshot(), on_primary);

    dome.run("focus monitor down");
    let on_lg = dome.snapshot();
    assert_snapshot!(on_lg, @"
    focused none on LG
    workspaces: 0 on primary (visible), 0 on DELL (visible), 0 on LG (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
    LG 0,20 20x4 border 1
    DELL 100,0 20x4 border 1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
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
    ");
    dome.run("focus monitor up");
    assert_eq!(dome.snapshot(), on_primary);

    for command in ["focus monitor LG", "focus monitor LG", "focus monitor down"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), on_lg, "{command}");
    }
}

#[test]
fn move_to_monitor_by_direction_or_name_takes_focused_window_there_and_keeps_focus() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    let before_move = dome.snapshot();
    for command in ["move monitor left", "move monitor primary"] {
        dome.run(command);
        assert_eq!(dome.snapshot(), before_move, "{command}");
    }

    dome.run("move monitor right");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    DELL 100,0 40x10 border 1
      w1 100,0 40x10
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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

    +--------------------------------------+
    |                                      |
    |                                      |
    |                                      |
    |                  w1                  |
    |                                      |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+
    ");
    dome.run("move monitor DELL");
    let empty_primary = dome.snapshot();
    assert_snapshot!(empty_primary, @"
    focused none on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
    DELL 100,0 40x10 border 1
      w0 120,0 20x10
      w1 100,0 20x10
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        

    +------------------++------------------+
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    |        w1        ||        w0        |
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    +------------------++------------------+
    ");
    dome.run("move monitor right");
    assert_eq!(dome.snapshot(), empty_primary);
    dome.run("focus monitor DELL");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on DELL
    workspaces: 0 on primary (visible), 0 on DELL (focused)
    primary 0,0 100x20 border 1
    DELL 100,0 40x10 border 1
      w0 120,0 20x10 highlighted spawn=right
      w1 100,0 20x10
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        
                                                                                                        

    +------------------+********************
    |                  |*                  *
    |                  |*                  *
    |                  |*                  *
    |        w1        |*        w0        *
    |                  |*                  *
    |                  |*                  *
    |                  |*                  *
    |                  |*                  *
    +------------------+********************
    ");
}

#[test]
fn move_to_monitor_with_container_focused_takes_its_windows_and_leaves_floats() {
    let mut dome = TestDome::new("return {}");
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.open(window("w2"));
    dome.run("toggle float");
    dome.focus(w1);
    dome.run("focus parent");

    dome.run("move monitor right");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
      w2 67,0 33x20 float highlighted
    DELL 100,0 40x10 border 1
      w0 100,0 20x10
      w1 120,0 20x10
                                                                       *********************************
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *               w2              *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *                               *
                                                                       *********************************

    +------------------++------------------+
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    |        w0        ||        w1        |
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    |                  ||                  |
    +------------------++------------------+
    ");
}

#[test]
fn open_on_monitor_with_empty_work_area_places_and_focuses_nothing() {
    let scene = |frame| {
        let mut dome = TestDome::new("return {}");
        dome.add_monitor(monitor("empty", frame));
        dome.run("focus monitor empty");
        for title in ["w0", "w1", "w2"] {
            dome.open(window(title));
        }
        dome.run("toggle layout");
        dome.open(window("w3"));
        dome.run("toggle float");
        dome.open(window("w4"));
        dome.run("toggle fullscreen");
        dome.snapshot()
    };
    assert_snapshot!(scene([100, 0, 0, 20]), @"
    focused none on empty
    workspaces: 0 on primary (visible), 0 on empty (focused)
    primary 0,0 100x20 border 1
    empty 100,0 0x20 border 1
    ");
    assert_snapshot!(scene([100, 0, 40, 0]), @"
    focused none on empty
    workspaces: 0 on primary (visible), 0 on empty (focused)
    primary 0,0 100x20 border 1
    empty 100,0 40x0 border 1
    ");
    assert_snapshot!(scene([100, 0, 0, 0]), @"
    focused none on empty
    workspaces: 0 on primary (visible), 0 on empty (focused)
    primary 0,0 100x20 border 1
    empty 100,0 0x0 border 1
    ");
}

#[test]
fn click_tab_switches_tab_on_any_monitor_and_ignores_gone_container() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle layout");
    let removed_container = dome.tab_container(&["w0", "w1"]);
    dome.close(w1);
    let before_gone_container_click = dome.snapshot();
    dome.click_tab(removed_container, 0);
    assert_eq!(dome.snapshot(), before_gone_container_click);

    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.open(window("w2"));
    dome.open(window("w3"));
    dome.run("toggle layout");
    dome.run("focus monitor left");
    dome.click_tab(dome.tab_container(&["w2", "w3"]), 0);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    DELL 100,0 40x10 border 1
      w2 100,2 40x8
      container 100,0 40x10 tabbed active=0 titles=[w2, w3]
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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

    +--------------------------------------+
    |       [w2]        |       w3         |
    +--------------------------------------+
    |                                      |
    |                                      |
    |                  w2                  |
    |                                      |
    |                                      |
    |                                      |
    +--------------------------------------+
    ");
}

#[test]
fn move_float_onto_other_monitor_joins_its_active_workspace_and_keeps_focus() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle float");
    let dell = dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus monitor DELL");
    dome.open(window("w2"));
    dome.run("focus monitor left");

    dome.move_float(w1, [105, 2, 20, 5], dell);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    DELL 100,0 40x10 border 1
      w1 104,1 22x7 float
      w2 100,0 40x10
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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

    +--------------------------------------+
    |   +--------------------+             |
    |   |                    |             |
    |   |                    |             |
    |   |         w1         |             |
    |   |                    |             |
    |   |                    |             |
    |   +--------------------+             |
    |                                      |
    +--------------------------------------+
    ");
    dome.run("focus monitor right");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on DELL
    workspaces: 0 on primary (visible), 0 on DELL (focused)
    primary 0,0 100x20 border 1
      w0 0,0 100x20
    DELL 100,0 40x10 border 1
      w1 104,1 22x7 float highlighted
      w2 100,0 40x10
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                w0                                                |
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

    +--------------------------------------+
    |   **********************             |
    |   *                    *             |
    |   *                    *             |
    |   *         w1         *             |
    |   *                    *             |
    |   *                    *             |
    |   **********************             |
    |                                      |
    +--------------------------------------+
    ");
}

#[test]
fn focus_workspace_shows_its_windows_and_hides_current_ones() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    dome.run("focus workspace 1");
    dome.open(window("w2"));
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w2 0,0 100x20 highlighted spawn=right
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w2                                                *
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
    ");

    dome.run("focus workspace 0");
    let on_zero = dome.snapshot();
    assert_snapshot!(on_zero, @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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
    dome.run("focus workspace 0");
    assert_eq!(dome.snapshot(), on_zero);

    let mut dome = TestDome::new("return {}");
    dome.open(window("home"));
    dome.add_monitor(monitor("DELL", [100, 0, 40, 10]));
    dome.run("focus workspace work --monitor DELL");
    assert_snapshot!(dome.snapshot(), @"
    focused none on DELL
    workspaces: 0 on primary (visible), work on DELL (focused)
    primary 0,0 100x20 border 1
      home 0,0 100x20
    DELL 100,0 40x10 border 1
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                               home                                               |
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
    ");
    dome.open(window("editor"));
    dome.run("focus workspace 0 --monitor primary");
    dome.run("focus workspace work --monitor DELL");
    assert_snapshot!(dome.snapshot(), @"
    focused editor on DELL
    workspaces: 0 on primary (visible), work on DELL (focused)
    primary 0,0 100x20 border 1
      home 0,0 100x20
    DELL 100,0 40x10 border 1
      editor 100,0 40x10 highlighted spawn=right
    +--------------------------------------------------------------------------------------------------+
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                                                                                  |
    |                                               home                                               |
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

    ****************************************
    *                                      *
    *                                      *
    *                                      *
    *                editor                *
    *                                      *
    *                                      *
    *                                      *
    *                                      *
    ****************************************
    ");
}

#[test]
fn move_to_workspace_takes_focused_window_there_and_keeps_focus_on_source() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.open(window("w1"));
    let w2 = dome.open(window("w2"));
    dome.run("move workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w0 0,0 50x20
      w1 50,0 50x20 highlighted spawn=right
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w0                       |*                       w1                       *
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

    dome.run("move workspace 1");
    dome.run("move workspace 1");
    let empty_source = dome.snapshot();
    assert_snapshot!(empty_source, @"
    focused none on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
    ");
    dome.run("move workspace 1");
    assert_eq!(dome.snapshot(), empty_source);

    dome.run("focus workspace 1");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 67,0 33x20 highlighted spawn=right
      w1 33,0 34x20
      w2 0,0 33x20
    +-------------------------------++--------------------------------+*********************************
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |               w2              ||               w1               |*               w0              *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    |                               ||                                |*                               *
    +-------------------------------++--------------------------------+*********************************
    ");
    dome.focus(w2);
    let before_same_workspace_move = dome.snapshot();
    dome.run("move workspace 1");
    assert_eq!(dome.snapshot(), before_same_workspace_move);

    dome.run("toggle float");
    dome.run("move workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary, 1 on primary (focused)
    primary 0,0 100x20 border 1
      w0 50,0 50x20 highlighted spawn=right
      w1 0,0 50x20
    +------------------------------------------------+**************************************************
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                                                |*                                                *
    |                       w1                       |*                       w0                       *
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
    dome.run("focus workspace 0");
    assert_snapshot!(dome.snapshot(), @"
    focused w2 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w2 0,0 33x20 float highlighted
    *********************************                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *               w2              *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *                               *                                                                   
    *********************************
    ");
}

#[test]
fn focus_window_on_hidden_workspace_switches_to_that_workspace() {
    let mut dome = TestDome::new("return {}");
    let w0 = dome.open(window("w0"));
    let w1 = dome.open(window("w1"));
    dome.run("toggle float");
    dome.run("focus workspace 1");
    dome.open(window("w2"));

    dome.focus(w0);
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
      w1 50,0 50x20 float
    **************************************************+------------------------------------------------+
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                w|                       w1                       |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    *                                                 |                                                |
    **************************************************+------------------------------------------------+
    ");
    dome.run("focus workspace 1");
    dome.focus(w1);
    assert_snapshot!(dome.snapshot(), @"
    focused w1 on primary
    workspaces: 0 on primary (focused), 1 on primary
    primary 0,0 100x20 border 1
      w0 0,0 100x20
      w1 50,0 50x20 float highlighted
    +-------------------------------------------------**************************************************
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                w*                       w1                       *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    |                                                 *                                                *
    +-------------------------------------------------**************************************************
    ");
}

#[test]
fn query_workspaces_lists_each_visible_or_occupied_workspace() {
    let mut dome = TestDome::new("return {}");
    dome.open(window("w0"));
    dome.run("focus workspace 1");
    dome.open(window("w1"));
    dome.run("toggle float");
    dome.run("focus workspace 2");
    dome.open(window("w2"));
    dome.run("toggle fullscreen");
    dome.run("focus workspace 3");
    dome.run("focus workspace 0");
    dome.add_monitor(monitor("DELL", [100, 0, 20, 4]));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 1 on primary, 2 on primary, 0 on DELL (visible)
    primary 0,0 100x20 border 1
      w0 0,0 100x20 highlighted spawn=right
    DELL 100,0 20x4 border 1
    ****************************************************************************************************
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                                                                  *
    *                                                w0                                                *
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
    ");
}
