use super::{
    border_boxes, border_boxes_by_window, dev_hub, insert, process_matcher, scrolling_hub,
    scrolling_hub_with,
};
use crate::core::hub::Hub;
use crate::core::node::{Length, LimitObservation, LimitUpdate, Pixels, WindowId};
use crate::core::tests::{
    TestHubBuilder, TilingConfigBuilder, default_rect, reported_monitor, setup_modes_on, titled,
    validate_hub,
};
use crate::core::{
    ColumnConfig, MonitorSelector, PixelRect, PreferredLayouts, PreferredTiling, ScrollingConfig,
    SizeConstraint, Strategy, WindowRestrictions,
};

fn hub_with_width(percent: f32) -> Hub {
    scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Percent(percent),
    })
}

fn reload(hub: &mut Hub, percent: f32) {
    hub.sync_configuration(
        TilingConfigBuilder::new()
            .with_strategy(Strategy::Scrolling)
            .with_scrolling_config(ScrollingConfig {
                column_width: SizeConstraint::Percent(percent),
            })
            .build(),
    );
}

fn insert_windows<const N: usize>(hub: &mut Hub) -> [WindowId; N] {
    std::array::from_fn(|i| {
        hub.insert_window(
            titled(&format!("w{i}")),
            default_rect(),
            WindowRestrictions::None,
        )
        .unwrap()
    })
}

fn set_min_width(hub: &mut Hub, window: WindowId, update: LimitUpdate) {
    hub.set_window_constraint(
        window,
        LimitObservation {
            min_width: update,
            ..Default::default()
        },
    );
}

fn set_max_width(hub: &mut Hub, window: WindowId, update: LimitUpdate) {
    hub.set_window_constraint(
        window,
        LimitObservation {
            max_width: update,
            ..Default::default()
        },
    );
}

fn limit(value: f32) -> LimitUpdate {
    LimitUpdate::Set(Length::new(value))
}

fn press(hub: &mut Hub, action: fn(&mut Hub), times: usize) {
    for _ in 0..times {
        action(hub);
    }
}

/// The stored width of each column of the current workspace, left to right.
fn column_widths(hub: &Hub) -> Vec<Option<SizeConstraint>> {
    match hub.export_workspace(hub.current_workspace()).tiling {
        PreferredTiling::Scrolling(scrolling) => scrolling
            .columns
            .iter()
            .map(|column| column.width)
            .collect(),
        other => panic!("workspace should be scrolling, got {other:?}"),
    }
}

#[test]
fn grow_widens_the_focused_column_by_five_percent() {
    let mut hub = hub_with_width(20.0);
    insert_windows::<2>(&mut hub);
    press(&mut hub, Hub::grow, 2);
    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(38, 0, 30, 30), PixelRect::new(68, 0, 45, 30)]
    );
    validate_hub(&hub);
}

#[test]
fn shrink_narrows_the_focused_column_by_five_percent() {
    let mut hub = hub_with_width(20.0);
    insert_windows::<2>(&mut hub);
    press(&mut hub, Hub::shrink, 2);
    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(53, 0, 30, 30), PixelRect::new(83, 0, 15, 30)]
    );
    validate_hub(&hub);
}

#[test]
fn grow_stops_at_the_work_area_width() {
    let mut hub = hub_with_width(90.0);
    insert_windows::<1>(&mut hub);
    press(&mut hub, Hub::grow, 3);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(0, 0, 150, 30)]);
    validate_hub(&hub);
}

#[test]
fn grow_stops_at_the_column_maximum() {
    let mut hub = hub_with_width(20.0);
    let [w0] = insert_windows(&mut hub);
    set_max_width(&mut hub, w0, limit(38.0));
    press(&mut hub, Hub::grow, 3);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(55, 0, 40, 30)]);
    validate_hub(&hub);
}

#[test]
fn shrink_stops_at_the_column_minimum() {
    let mut hub = hub_with_width(20.0);
    let [w0] = insert_windows(&mut hub);
    set_min_width(&mut hub, w0, limit(18.0));
    press(&mut hub, Hub::shrink, 3);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(65, 0, 20, 30)]);
    validate_hub(&hub);
}

#[test]
fn shrink_reaches_zero_width_without_a_minimum() {
    let mut tiling = TilingConfigBuilder::new()
        .with_strategy(Strategy::Scrolling)
        .with_scrolling_config(ScrollingConfig {
            column_width: SizeConstraint::Percent(20.0),
        })
        .build();
    tiling.size_constraints.minimum_width = SizeConstraint::Pixels(Pixels::new(0));
    let mut hub = TestHubBuilder::new().with_tiling(tiling).build();
    let [w0, w1] = insert_windows(&mut hub);

    press(&mut hub, Hub::shrink, 5);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w0, PixelRect::new(60, 0, 30, 30))]
    );
    validate_hub(&hub);

    press(&mut hub, Hub::grow, 2);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(53, 0, 30, 30)),
            (w1, PixelRect::new(83, 0, 15, 30))
        ]
    );
    validate_hub(&hub);
}

#[test]
fn shrink_measures_from_the_shown_width() {
    let mut hub = hub_with_width(80.0);
    let [w0] = insert_windows(&mut hub);
    set_max_width(&mut hub, w0, limit(43.0));
    press(&mut hub, Hub::shrink, 2);
    set_max_width(&mut hub, w0, LimitUpdate::Cleared);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(60, 0, 30, 30)]);
    validate_hub(&hub);
}

#[test]
fn grow_leaves_a_column_held_wider_than_the_work_area_alone() {
    let mut hub = hub_with_width(20.0);
    let [w0] = insert_windows(&mut hub);
    set_min_width(&mut hub, w0, limit(198.0));
    hub.grow();
    set_min_width(&mut hub, w0, LimitUpdate::Cleared);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(60, 0, 30, 30)]);
    validate_hub(&hub);
}

#[test]
fn grow_widens_every_window_of_a_stack() {
    let mut hub = hub_with_width(20.0);
    insert_windows::<1>(&mut hub);
    hub.toggle_spawn_mode();
    insert_windows::<1>(&mut hub);
    press(&mut hub, Hub::grow, 2);
    assert_eq!(
        border_boxes(&hub),
        vec![
            PixelRect::new(53, 0, 45, 15),
            PixelRect::new(53, 15, 45, 15)
        ]
    );
    validate_hub(&hub);
}

#[test]
fn growing_the_last_column_keeps_it_in_view() {
    let mut hub = hub_with_width(40.0);
    insert_windows::<3>(&mut hub);
    press(&mut hub, Hub::grow, 2);
    assert_eq!(
        border_boxes(&hub),
        vec![
            PixelRect::new(-45, 0, 60, 30),
            PixelRect::new(15, 0, 60, 30),
            PixelRect::new(75, 0, 75, 30)
        ]
    );
    validate_hub(&hub);
}

#[test]
fn grow_and_shrink_without_a_window_do_nothing() {
    let mut hub = scrolling_hub();
    hub.grow();
    hub.shrink();
    validate_hub(&hub);
}

#[test]
fn a_resized_column_stores_a_rounded_percentage() {
    let mut hub = hub_with_width(20.0);
    insert_windows::<1>(&mut hub);
    press(&mut hub, Hub::grow, 2);
    let columns = match hub.export_workspace(hub.current_workspace()).tiling {
        PreferredTiling::Scrolling(scrolling) => scrolling.columns,
        other => panic!("workspace should be scrolling, got {other:?}"),
    };
    assert_eq!(columns[0].width, Some(SizeConstraint::Percent(30.0)));
}

#[test]
fn a_reload_applies_column_width_to_every_column() {
    let mut hub = hub_with_width(20.0);
    insert_windows::<2>(&mut hub);
    press(&mut hub, Hub::grow, 2);
    reload(&mut hub, 40.0);
    assert_eq!(
        border_boxes(&hub),
        vec![PixelRect::new(15, 0, 60, 30), PixelRect::new(75, 0, 60, 30)]
    );
    validate_hub(&hub);
}

#[test]
fn a_reload_with_the_same_width_undoes_a_resize() {
    let mut hub = hub_with_width(20.0);
    insert_windows::<1>(&mut hub);
    press(&mut hub, Hub::grow, 2);
    reload(&mut hub, 20.0);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(60, 0, 30, 30)]);
    validate_hub(&hub);
}

#[test]
fn a_reload_keeps_the_widths_of_a_layout_workspace() {
    let mut hub = dev_hub(vec![ColumnConfig::bare(process_matcher("a.exe"))]);
    let a = insert(&mut hub, "a.exe");
    let u = insert(&mut hub, "u.exe");
    press(&mut hub, Hub::grow, 2);
    reload(&mut hub, 40.0);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(38, 0, 30, 30)),
            (u, PixelRect::new(68, 0, 45, 30))
        ]
    );
    validate_hub(&hub);
}

#[test]
fn a_reload_after_the_layout_entry_is_dropped_applies_column_width() {
    let mut hub = dev_hub(vec![ColumnConfig {
        width: Some(SizeConstraint::Percent(40.0)),
        children: vec![process_matcher("a.exe")],
    }]);
    insert(&mut hub, "a.exe");
    hub.apply_preferred_layouts(PreferredLayouts::default());
    reload(&mut hub, 20.0);
    assert_eq!(border_boxes(&hub), vec![PixelRect::new(60, 0, 30, 30)]);
    validate_hub(&hub);
}

#[test]
fn grow_and_shrink_do_nothing_while_fullscreen_has_focus() {
    let mut hub = setup_modes_on(Strategy::Scrolling, "0", &[], &["fs"]);
    hub.insert_window(titled("t"), default_rect(), WindowRestrictions::None);
    hub.insert_window(titled("fs"), default_rect(), WindowRestrictions::None);

    hub.grow();
    assert_eq!(column_widths(&hub), [Some(SizeConstraint::Percent(50.0))]);
    hub.shrink();
    assert_eq!(column_widths(&hub), [Some(SizeConstraint::Percent(50.0))]);
    validate_hub(&hub);
}

#[test]
fn shrink_on_an_empty_work_area_keeps_the_column_width() {
    let mut hub = scrolling_hub_with(ScrollingConfig {
        column_width: SizeConstraint::Pixels(Pixels::new(60)),
    });
    hub.add_monitor(reported_monitor(
        "empty".to_string(),
        PixelRect::new(150, 0, 0, 30),
        1.0,
    ));
    hub.focus_monitor(&MonitorSelector::Name("empty".to_string()));
    hub.insert_window(titled("w0"), default_rect(), WindowRestrictions::None);

    hub.shrink();

    validate_hub(&hub);
    assert_eq!(
        column_widths(&hub),
        [Some(SizeConstraint::Pixels(Pixels::new(60)))]
    );
}
