use crate::core::allocator::NodeId;

use super::{
    LayoutConfigBuilder, PixelRect, default_rect, setup, setup_with_layout, snapshot, titled,
    titled_matcher,
};
use crate::core::GlobalLayoutConfig;
use crate::core::node::{
    Length, LimitObservation, LimitUpdate, MinimizedWindowEntry, MonitorId, WindowRestrictions,
};
use insta::assert_snapshot;

/// Float matchers by exact title, since this file also inserts tiling windows named `wN`.
fn layout_floating(titles: &[&str]) -> GlobalLayoutConfig {
    LayoutConfigBuilder::new()
        .with_float(titles.iter().map(|t| titled_matcher(t)).collect())
        .build()
}

#[test]
fn minimize_tiling_window() {
    let mut hub = setup();
    let _w0 = hub
        .insert_window(titled("w0"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w1"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
    let mut hub = setup_with_layout(layout_floating(&["w3"]));
    let _w0 = hub
        .insert_window(titled("w2"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(
            titled("w3"),
            PixelRect::new(10, 5, 40, 10),
            WindowRestrictions::None,
        )
        .unwrap();
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
    let _w0 = hub
        .insert_window(titled("w4"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w5"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
    let w0 = hub
        .insert_window(titled("w6"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w0);
    hub.minimize_window(w0);
    assert_eq!(hub.minimized_window_entries().len(), 1);
}

#[test]
fn unminimize_restores_to_current_workspace() {
    let mut hub = setup();
    let _w0 = hub
        .insert_window(titled("w7"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w8"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w1);
    hub.focus_workspace("1", None);
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
    let w0 = hub
        .insert_window(titled("w9"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
    let _w0 = hub
        .insert_window(titled("w10"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w11"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
    let _w0 = hub
        .insert_window(titled("w12"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w13"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w1);
    hub.set_focus(w1);
}

#[test]
#[should_panic(expected = "non-minimized window has a workspace")]
fn set_fullscreen_on_minimized_panics() {
    let mut hub = setup();
    let _w0 = hub
        .insert_window(titled("w14"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w15"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w1);
    hub.set_fullscreen(w1, WindowRestrictions::None);
}

#[test]
fn minimize_last_window_on_workspace() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w16"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
    let mut hub = setup_with_layout(layout_floating(&["w18"]));
    let w0 = hub
        .insert_window(titled("w17"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let _w1 = hub
        .insert_window(
            titled("w18"),
            PixelRect::new(10, 5, 40, 10),
            WindowRestrictions::None,
        )
        .unwrap();
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
    let w0 = hub
        .insert_window(titled("w19"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w0);
    hub.set_window_constraint(
        w0,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(100.0)),
            min_height: LimitUpdate::Set(Length::new(50.0)),
            ..Default::default()
        },
    );
    assert_eq!(hub.minimized_window_entries().len(), 1);
}

#[test]
#[should_panic(expected = "non-minimized float window has a workspace")]
fn update_float_rect_on_minimized_panics() {
    let mut hub = setup_with_layout(layout_floating(&["w20"]));
    let dim = PixelRect::new(10, 5, 40, 10);
    let w0 = hub
        .insert_window(titled("w20"), dim, WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w0);
    hub.update_float_rect(w0, PixelRect::new(20, 10, 50, 20), MonitorId::new(0));
}

#[test]
fn set_window_title_on_minimized_no_panic() {
    let mut hub = setup();
    let w0 = hub
        .insert_window(titled("w21"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
    let w0 = hub
        .insert_window(titled("w22"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w23"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.set_window_title(w0, "Firefox".into());
    hub.set_window_title(w1, "Terminal".into());
    hub.minimize_window(w0);
    hub.minimize_window(w1);
    let entries = hub.minimized_window_entries();
    assert_eq!(
        entries,
        vec![
            MinimizedWindowEntry {
                id: w0,
                title: "Firefox".into(),
                app_name: None,
                bundle_id: None,
                executable_path: None,
            },
            MinimizedWindowEntry {
                id: w1,
                title: "Terminal".into(),
                app_name: None,
                bundle_id: None,
                executable_path: None,
            },
        ]
    );
}

#[test]
fn minimized_window_entries_empty_when_none_minimized() {
    let mut hub = setup();
    let _w0 = hub
        .insert_window(titled("w24"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let entries = hub.minimized_window_entries();
    assert!(entries.is_empty());
}

#[test]
fn unminimize_deleted_window_is_noop() {
    let mut hub = setup();
    let _w0 = hub
        .insert_window(titled("w25"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w26"), default_rect(), WindowRestrictions::None)
        .unwrap();
    hub.minimize_window(w1);
    hub.delete_window(w1);
    hub.unminimize_window(w1);
    assert!(hub.minimized_window_entries().is_empty());
}

#[test]
fn unminimize_float_window_restores_mode_and_dimension() {
    let mut hub = setup_with_layout(layout_floating(&["w28"]));
    let _w0 = hub
        .insert_window(titled("w27"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let float_dim = PixelRect::new(10, 5, 40, 10);
    let w_float = hub
        .insert_window(titled("w28"), float_dim, WindowRestrictions::None)
        .unwrap();

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
    let _w0 = hub
        .insert_window(titled("w29"), default_rect(), WindowRestrictions::None)
        .unwrap();
    let w1 = hub
        .insert_window(titled("w30"), default_rect(), WindowRestrictions::None)
        .unwrap();
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
