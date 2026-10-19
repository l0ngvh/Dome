use crate::tests::fixtures::{TestDome, monitor, window};
use insta::assert_snapshot;

#[test]
fn boot_with_reserved_area_shrinks_work_area_by_its_insets() {
    let mut dome = TestDome::new(
        "return { reserved_area = function() return { top = 2, bottom = 1, left = 3, right = 4 } end }",
    );
    dome.open(window("w0"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 3,2 93x17 border 1
      w0 3,2 93x17 highlighted spawn=right
    *********************************************************************************************
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                             w0                                            *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *                                                                                           *
    *********************************************************************************************
    ");

    let mut collapsed =
        TestDome::new("return { reserved_area = function() return { top = 30, left = 120 } end }");
    assert_snapshot!(collapsed.snapshot(), @"
    focused none on primary
    workspaces: 0 on primary (focused)
    primary 100,20 0x0 border 1
    ");

    #[cfg(target_os = "windows")]
    {
        let mut scaled = TestDome::new(
            "return { reserved_area = function(monitor)
                if monitor.name == 'DELL' then return { top = 1, bottom = 1 } end
            end }",
        );
        scaled.add_monitor(monitor("DELL", [100, 0, 100, 20]).scale(1.5));
        let mut reference = TestDome::new("return {}");
        reference.add_monitor(monitor("DELL", [100, 2, 100, 16]).scale(1.5));
        assert_eq!(scaled.snapshot(), reference.snapshot());
    }
}

#[test]
fn add_monitor_applies_reserved_area_for_each_monitor_name() {
    let mut dome = TestDome::new(
        "return { reserved_area = function(monitor)
            if monitor.name == 'DELL' then return { top = 3, left = 1, right = 5 } end
            return { top = 1, left = 1 }
        end }",
    );
    dome.open(window("w0"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 1,1 99x19 border 1
      w0 1,1 99x19 highlighted spawn=right
    ***************************************************************************************************
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                w0                                               *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    ***************************************************************************************************
    ");
    dome.add_monitor(monitor("DELL", [100, 0, 100, 20]));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused), 0 on DELL (visible)
    primary 1,1 99x19 border 1
      w0 1,1 99x19 highlighted spawn=right
    DELL 101,3 94x17 border 1
    ***************************************************************************************************
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                w0                                               *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    *                                                                                                 *
    ***************************************************************************************************
    ");
}

#[test]
fn boot_with_unusable_reserved_area_reserves_only_usable_insets() {
    let mut reference = TestDome::new("return {}");
    reference.open(window("w0"));
    let expected = reference.snapshot();
    assert_snapshot!(expected, @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
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
    for reserved_area in [
        "function() error('boom') end",
        "function() return 5 end",
        "function() return 'top' end",
        "function() return true end",
        "function() return nil end",
        "5",
    ] {
        let mut dome = TestDome::new(&format!("return {{ reserved_area = {reserved_area} }}"));
        dome.open(window("w0"));
        assert_eq!(dome.snapshot(), expected, "{reserved_area}");
    }
    let mut dome = TestDome::new(
        "return { reserved_area = function() return { top = -1, bottom = 1.5, left = '3', right = 4 } end }",
    );
    dome.open(window("w0"));
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 0,0 96x20 border 1
      w0 0,0 96x20 highlighted spawn=right
    ************************************************************************************************
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                              w0                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    *                                                                                              *
    ************************************************************************************************
    ");
}

#[test]
fn reload_with_new_reserved_area_applies_its_insets() {
    let mut dome = TestDome::new("return { reserved_area = function() return { top = 2 } end }");
    dome.open(window("w0"));
    assert!(
        dome.reload("return { reserved_area = function() return { top = 4, left = 3 } end }")
            .is_some()
    );
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
    primary 3,4 97x16 border 1
      w0 3,4 97x16 highlighted spawn=right
    *************************************************************************************************
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                               w0                                              *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *                                                                                               *
    *************************************************************************************************
    ");
    assert!(dome.reload("return {}").is_some());
    assert_snapshot!(dome.snapshot(), @"
    focused w0 on primary
    workspaces: 0 on primary (focused)
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
