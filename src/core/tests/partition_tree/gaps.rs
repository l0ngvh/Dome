use crate::config::{GapsConfig, InnerGaps, LayoutConfig};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{Dimension, Length, WindowId};
use crate::core::tests::{default_layout_for_tests, setup};

fn set_inner_gaps(hub: &mut Hub, horizontal: f32, vertical: f32) {
    hub.sync_config(LayoutConfig {
        gaps: GapsConfig {
            inner: InnerGaps {
                horizontal: Length::new(horizontal),
                vertical: Length::new(vertical),
            },
            ..GapsConfig::default()
        },
        ..default_layout_for_tests()
    });
}

fn tiling_frame(hub: &Hub, id: WindowId) -> Dimension {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { tiling_windows, .. } = &placements.monitors[0].layout else {
        panic!("expected normal layout");
    };
    tiling_windows
        .iter()
        .find(|p| p.id == id)
        .unwrap_or_else(|| panic!("missing placement for {id:?}"))
        .frame
}

#[test]
fn horizontal_inner_gap_separates_side_by_side_children() {
    let mut hub = setup();
    set_inner_gaps(&mut hub, 10.0, 0.0);

    let left = hub.insert_tiling(hub.current_workspace());
    let right = hub.insert_tiling(hub.current_workspace());

    let left = tiling_frame(&hub, left);
    let right = tiling_frame(&hub, right);
    assert_eq!(left.x, Length::ZERO);
    assert_eq!(left.x + left.width + Length::new(10.0), right.x);
    assert_eq!(right.x + right.width, Length::new(150.0));
}

#[test]
fn vertical_inner_gap_separates_stacked_children() {
    let mut hub = setup();
    set_inner_gaps(&mut hub, 0.0, 12.0);

    let top = hub.insert_tiling(hub.current_workspace());
    let bottom = hub.insert_tiling(hub.current_workspace());
    hub.toggle_direction();

    let top = tiling_frame(&hub, top);
    let bottom = tiling_frame(&hub, bottom);
    assert_eq!(top.y, Length::ZERO);
    assert_eq!(top.y + top.height + Length::new(12.0), bottom.y);
    assert_eq!(bottom.y + bottom.height, Length::new(30.0));
}

#[test]
fn single_child_is_not_shrunk_by_inner_gaps() {
    let mut hub = setup();
    set_inner_gaps(&mut hub, 10.0, 12.0);

    let id = hub.insert_tiling(hub.current_workspace());

    assert_eq!(
        tiling_frame(&hub, id),
        Dimension::new(
            Length::ZERO,
            Length::ZERO,
            Length::new(150.0),
            Length::new(30.0)
        )
    );
}

#[test]
fn split_container_min_size_includes_inner_gap() {
    let mut hub = setup();
    set_inner_gaps(&mut hub, 10.0, 0.0);

    let left = hub.insert_tiling(hub.current_workspace());
    let right = hub.insert_tiling(hub.current_workspace());
    hub.set_window_constraint(left, Some(100.0), None, None, None);
    hub.set_window_constraint(right, Some(100.0), None, None, None);

    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { containers, .. } = &placements.monitors[0].layout else {
        panic!("expected normal layout");
    };
    assert_eq!(containers[0].frame.width, Length::new(210.0));
}
