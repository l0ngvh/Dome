use super::{setup, snapshot, titled};
use crate::core::node::{Dimension, Length, PickerEntry, WindowRestrictions};
use insta::assert_snapshot;

#[test]
fn minimize_tiling_window() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w0"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w1"));
    hub.minimize_window(w1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Minimized: [WindowId(1)]

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
fn minimize_float_window() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w2"));
    let w1 = hub.insert_float(
        hub.current_workspace(),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(40.0),
            Length::new(10.0),
        ),
        titled("w3"),
    );
    hub.minimize_window(w1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Minimized: [WindowId(1)]

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
fn minimize_fullscreen_window() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w4"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w5"));
    hub.set_fullscreen(w1, WindowRestrictions::None);
    hub.minimize_window(w1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(0))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00, highlighted, spawn=right)
      )
      Minimized: [WindowId(1)]

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
fn minimize_already_minimized_noop() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w6"));
    hub.minimize_window(w0);
    hub.minimize_window(w0);
    assert_eq!(hub.minimized_window_entries().len(), 1);
}

#[test]
fn unminimize_restores_to_current_workspace() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w7"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w8"));
    hub.minimize_window(w1);
    hub.focus_workspace("1");
    hub.unminimize_window(w1);
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
fn unminimize_not_minimized_noop() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w9"));
    hub.unminimize_window(w0);
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
fn delete_minimized_window() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w10"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w11"));
    hub.minimize_window(w1);
    hub.delete_window(w1);
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
#[should_panic(expected = "non-minimized window has a workspace")]
fn set_focus_on_minimized_panics() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w12"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w13"));
    hub.minimize_window(w1);
    hub.set_focus(w1);
}

#[test]
#[should_panic(expected = "non-minimized window has a workspace")]
fn set_fullscreen_on_minimized_panics() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w14"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w15"));
    hub.minimize_window(w1);
    hub.set_fullscreen(w1, WindowRestrictions::None);
}

#[test]
fn minimize_last_window_on_workspace() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w16"));
    hub.minimize_window(w0);
    assert_eq!(hub.minimized_window_entries().len(), 1);
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=None)
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00))
      Minimized: [WindowId(0)]
    ");
}

#[test]
fn minimize_last_tiling_with_floats_present() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w17"));
    let _w1 = hub.insert_float(
        hub.current_workspace(),
        Dimension::new(
            Length::new(10.0),
            Length::new(5.0),
            Length::new(40.0),
            Length::new(10.0),
        ),
        titled("w18"),
    );
    hub.minimize_window(w0);
    assert_eq!(hub.minimized_window_entries().len(), 1);
    assert_snapshot!(snapshot(&hub), @r"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00, float, highlighted)
      )
      Minimized: [WindowId(0)]

                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
                                                                                                                                                          
              ****************************************                                                                                                    
              *                                      *                                                                                                    
              *                                      *                                                                                                    
              *                                      *                                                                                                    
              *                                      *                                                                                                    
              *                  F1                  *                                                                                                    
              *                                      *                                                                                                    
              *                                      *                                                                                                    
              *                                      *                                                                                                    
              ****************************************
    ");
}

#[test]
fn set_window_constraint_on_minimized_no_panic() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w19"));
    hub.minimize_window(w0);
    hub.set_window_constraint(w0, Some(100.0), Some(50.0), None, None);
    assert_eq!(hub.minimized_window_entries().len(), 1);
}

#[test]
#[should_panic(expected = "non-minimized float window has a workspace")]
fn update_float_dimension_on_minimized_panics() {
    let mut hub = setup();
    let dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(40.0),
        Length::new(10.0),
    );
    let w0 = hub.insert_float(hub.current_workspace(), dim, titled("w20"));
    hub.minimize_window(w0);
    hub.update_float_dimension(
        w0,
        Dimension::new(
            Length::new(20.0),
            Length::new(10.0),
            Length::new(50.0),
            Length::new(20.0),
        ),
    );
}

#[test]
fn set_window_title_on_minimized_no_panic() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w21"));
    hub.set_window_title(w0, "original".into());
    hub.minimize_window(w0);
    hub.set_window_title(w0, "updated".into());
    let entries = hub.minimized_window_entries();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].title, "updated");
}

#[test]
fn minimized_window_entries_returns_id_and_title() {
    let mut hub = setup();
    let w0 = hub.insert_tiling(hub.current_workspace(), titled("w22"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w23"));
    hub.set_window_title(w0, "Firefox".into());
    hub.set_window_title(w1, "Terminal".into());
    hub.minimize_window(w0);
    hub.minimize_window(w1);
    let entries = hub.minimized_window_entries();
    assert_eq!(
        entries,
        vec![
            PickerEntry {
                id: w0,
                title: "Firefox".into(),
                app_id: None,
                app_name: None,
            },
            PickerEntry {
                id: w1,
                title: "Terminal".into(),
                app_id: None,
                app_name: None,
            },
        ]
    );
}

#[test]
fn minimized_window_entries_empty_when_none_minimized() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w24"));
    let entries = hub.minimized_window_entries();
    assert!(entries.is_empty());
}

#[test]
fn unminimize_deleted_window_is_noop() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w25"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w26"));
    hub.minimize_window(w1);
    hub.delete_window(w1);
    hub.unminimize_window(w1);
    assert!(hub.minimized_window_entries().is_empty());
}

#[test]
fn unminimize_float_window_restores_mode_and_dimension() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w27"));
    let float_dim = Dimension::new(
        Length::new(10.0),
        Length::new(5.0),
        Length::new(40.0),
        Length::new(10.0),
    );
    let w_float = hub.insert_float(hub.current_workspace(), float_dim, titled("w28"));

    hub.minimize_window(w_float);
    assert_eq!(hub.minimized_window_entries().len(), 1);

    hub.unminimize_window(w_float);
    assert!(hub.minimized_window_entries().is_empty());
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Window(id=WindowId(0), x=0.00, y=0.00, w=150.00, h=30.00)
        Window(id=WindowId(1), x=10.00, y=5.00, w=40.00, h=10.00, float, highlighted)
      )

    +----------------------------------------------------------------------------------------------------------------------------------------------------+
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |                                                                                                                                                    |
    |         ****************************************                                                                                                   |
    |         *                                      *                                                                                                   |
    |         *                                      *                                                                                                   |
    |         *                                      *                                                                                                   |
    |         *                                      *                                                                                                   |
    |         *                  F1                  *                                                                                                   |
    |         *                                      *                                                                                                   |
    |         *                                      *                                                                                                   |
    |         *                                      *                                                                                                   |
    |         ****************************************                                                                                                   |
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
fn unminimize_fullscreen_window_restores_mode_and_restrictions() {
    let mut hub = setup();
    let _w0 = hub.insert_tiling(hub.current_workspace(), titled("w29"));
    let w1 = hub.insert_tiling(hub.current_workspace(), titled("w30"));
    hub.set_fullscreen(w1, WindowRestrictions::BlockAll);

    hub.minimize_window(w1);
    assert_eq!(hub.minimized_window_entries().len(), 1);

    hub.unminimize_window(w1);
    assert!(hub.minimized_window_entries().is_empty());
    assert_snapshot!(snapshot(&hub), @"
    Hub(focused=WindowId(1))
      Monitor(id=MonitorId(0), screen=(x=0.00 y=0.00 w=150.00 h=30.00),
        Fullscreen(id=WindowId(1))
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
    |                                                                         W1                                                                         |
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
