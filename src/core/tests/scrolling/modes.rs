use super::{border_boxes_by_window, scrolling_hub_with};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{PixelRect, WindowId, WindowRestrictions};
use crate::core::tests::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, default_rect, preferred_layout, setup_modes_on,
    titled, titled_matcher, validate_hub,
};
use crate::core::{ScrollingConfig, SizeConstraint, Strategy};

fn narrow_hub() -> Hub {
    scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Percent(20.0),
    })
}

fn insert(hub: &mut Hub, title: &str) -> WindowId {
    hub.insert_window(titled(title), default_rect(), WindowRestrictions::None)
        .unwrap()
}

fn fullscreen_window(hub: &Hub) -> Option<WindowId> {
    match hub.get_visible_placements().monitors[0].layout {
        MonitorLayout::Fullscreen(id) => Some(id),
        MonitorLayout::Normal { .. } => None,
    }
}

#[test]
fn toggle_fullscreen_covers_the_monitor_and_tiles_back_by_the_spawn_rules() {
    let mut hub = narrow_hub();
    let ws = hub.current_workspace();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    hub.set_focus(w0);

    hub.toggle_fullscreen();
    validate_hub(&hub);
    assert_eq!(fullscreen_window(&hub), Some(w0));
    assert_eq!(hub.focused_window(ws), Some(w0));

    hub.toggle_fullscreen();
    validate_hub(&hub);
    assert_eq!(hub.focused_window(ws), Some(w0));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(75, 0, 30, 30)),
            (w1, PixelRect::new(45, 0, 30, 30)),
        ],
        "w0 opens right of w1, which took the tiling focus when w0 left its column"
    );
}

#[test]
fn an_app_ending_fullscreen_tiles_the_window_by_the_spawn_rules() {
    let mut hub = narrow_hub();
    let ws = hub.current_workspace();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    hub.set_focus(w0);
    hub.toggle_spawn_mode();

    hub.set_fullscreen(w1, WindowRestrictions::ProtectFullscreen);
    validate_hub(&hub);
    assert_eq!(fullscreen_window(&hub), Some(w1));
    assert_eq!(hub.focused_window(ws), Some(w1));

    hub.set_fullscreen(w1, WindowRestrictions::ProtectFullscreen);
    validate_hub(&hub);
    assert_eq!(
        fullscreen_window(&hub),
        Some(w1),
        "a repeated report changes nothing"
    );

    hub.unset_fullscreen(w1);
    validate_hub(&hub);
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 15)),
            (w1, PixelRect::new(60, 15, 30, 15)),
        ],
        "w1 opens below w0, the tiling focus, which spawns vertically"
    );
}

#[test]
fn a_focus_request_under_a_fullscreen_window_moves_the_tiling_focus() {
    let mut hub = setup_modes_on(Strategy::Scrolling, "0", &[], &["fs"]);
    let ws = hub.current_workspace();
    let t0 = insert(&mut hub, "t0");
    let t1 = insert(&mut hub, "t1");
    insert(&mut hub, "t2");
    let fs = insert(&mut hub, "fs");

    hub.set_focus(t0);
    validate_hub(&hub);
    assert_eq!(hub.focused_window(ws), Some(fs));

    hub.delete_window(fs);
    validate_hub(&hub);
    assert_eq!(hub.focused_window(ws), Some(t0));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (t0, PixelRect::new(0, 0, 75, 30)),
            (t1, PixelRect::new(75, 0, 75, 30)),
        ],
        "the request scrolled t0 into view behind the fullscreen window"
    );
}

#[test]
fn minimizing_the_focused_window_focuses_by_position_and_a_restore_tiles_by_the_spawn_rules() {
    let mut hub = narrow_hub();
    let ws = hub.current_workspace();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");
    hub.set_focus(w0);
    hub.set_focus(w1);

    hub.minimize_window(w1);
    validate_hub(&hub);
    assert_eq!(
        hub.focused_window(ws),
        Some(w2),
        "focus takes the column that moved into w1's place, not the history head {w0}"
    );

    hub.unminimize_window(w1);
    validate_hub(&hub);
    assert_eq!(hub.focused_window(ws), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(30, 0, 30, 30)),
            (w1, PixelRect::new(90, 0, 30, 30)),
            (w2, PixelRect::new(60, 0, 30, 30)),
        ],
        "w1 opens right of w2, the tiling focus"
    );
}

#[test]
fn moving_the_focused_window_away_focuses_by_position() {
    let mut hub = narrow_hub();
    let ws = hub.current_workspace();
    let w0 = insert(&mut hub, "w0");
    let w1 = insert(&mut hub, "w1");
    let w2 = insert(&mut hub, "w2");
    hub.set_focus(w0);
    hub.set_focus(w1);

    hub.move_focused_to_workspace("1", None);
    validate_hub(&hub);
    assert_eq!(
        hub.focused_window(ws),
        Some(w2),
        "focus takes the column that moved into w1's place, not the history head {w0}"
    );
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(45, 0, 30, 30)),
            (w2, PixelRect::new(75, 0, 30, 30)),
        ]
    );

    hub.focus_workspace("1", None);
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w1));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w1, PixelRect::new(60, 0, 30, 30))]
    );
}

#[test]
fn a_reset_keeps_a_fullscreen_window_fullscreen_over_its_tiling_windows() {
    let layout = LayoutWorkspaceConfigBuilder::new("0")
        .with_strategy(Strategy::Scrolling)
        .with_fullscreen(vec![titled_matcher("fs")])
        .build();
    let mut hub = TestHubBuilder::new()
        .with_preferred_layout([layout.clone()])
        .build();
    let ws = hub.current_workspace();
    let t = insert(&mut hub, "t");
    let fs = insert(&mut hub, "fs");
    hub.set_focus(t);

    hub.apply_preferred_layouts(preferred_layout([layout]));

    validate_hub(&hub);
    assert_eq!(fullscreen_window(&hub), Some(fs));
    hub.delete_window(fs);
    validate_hub(&hub);
    assert_eq!(hub.focused_window(ws), Some(t));
}
