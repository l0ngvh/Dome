use super::{
    border_boxes_by_window, column_widths, dev_hub, insert, process_matcher, scrolling_hub_with,
    stack,
};
use crate::action::{Action, Actions};
use crate::config::lua::test_support::{RecordingEffects, RecordingKeymap};
use crate::core::hub::{Hub, MonitorLayout};
use crate::core::node::{Direction, Length, LimitObservation, LimitUpdate, WindowId};
use crate::core::tests::{
    LayoutWorkspaceConfigBuilder, TestHubBuilder, TilingConfigBuilder, snapshot_text, validate_hub,
};
use crate::core::{ColumnConfig, PixelRect, ScrollingConfig, SizeConstraint, Strategy};

fn narrow_scrolling() -> ScrollingConfig {
    ScrollingConfig {
        column_width: SizeConstraint::Percent(20.0),
    }
}

fn narrow_hub() -> Hub {
    scrolling_hub_with(narrow_scrolling())
}

/// A stack of a over b, then c in its own column. c has focus.
fn stack_beside_one() -> (Hub, WindowId, WindowId, WindowId) {
    let mut hub = dev_hub(vec![
        stack(&["a.exe", "b.exe"]),
        ColumnConfig::bare(process_matcher("c.exe")),
    ]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    let c = insert(&mut hub, "c.exe");
    (hub, a, b, c)
}

/// The windows of the highlighted column placement, top to bottom, with its spawn
/// direction.
fn selected_column(hub: &Hub) -> Option<(Vec<WindowId>, Option<Direction>)> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { containers, .. } = &placements.monitors[0].layout else {
        panic!("expected a normal layout");
    };
    let mut highlighted = containers.iter().filter(|c| c.is_highlighted);
    let column = highlighted.next()?;
    assert!(
        highlighted.next().is_none(),
        "more than one column is highlighted"
    );
    let windows = hub
        .access
        .containers
        .get(column.id)
        .children()
        .iter()
        .map(|child| match child {
            crate::core::node::Child::Window(wid) => *wid,
            crate::core::node::Child::Container(_) => panic!("a column holds a container"),
        })
        .collect();
    Some((windows, column.spawn_direction))
}

fn highlighted_windows(hub: &Hub) -> Vec<(WindowId, Option<Direction>)> {
    let placements = hub.get_visible_placements();
    let MonitorLayout::Normal { tiling_windows, .. } = &placements.monitors[0].layout else {
        panic!("expected a normal layout");
    };
    tiling_windows
        .iter()
        .filter(|w| w.is_highlighted)
        .map(|w| (w.id, w.spawn_direction))
        .collect()
}

fn selected(windows: &[WindowId]) -> Option<(Vec<WindowId>, Option<Direction>)> {
    Some((windows.to_vec(), Some(Direction::Vertical)))
}

#[test]
fn focus_parent_selects_the_column_of_the_focused_window() {
    let (mut hub, a, b, _) = stack_beside_one();
    let ws = hub.current_workspace();
    hub.set_focus(b);

    for _ in 0..2 {
        hub.focus_parent();
        assert_eq!(selected_column(&hub), selected(&[a, b]));
        assert_eq!(highlighted_windows(&hub), Vec::new());
        assert_eq!(hub.focused_window(ws), None);
    }
    validate_hub(&hub);
}

#[test]
fn a_click_on_a_window_ends_the_selection() {
    let (mut hub, a, b, _) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();

    hub.set_focus(a);
    assert_eq!(selected_column(&hub), None);
    assert_eq!(
        highlighted_windows(&hub),
        vec![(a, Some(Direction::Horizontal))]
    );
    validate_hub(&hub);
}

#[test]
fn move_left_and_right_swap_the_selected_column() {
    let (mut hub, a, b, c) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();
    let start = border_boxes_by_window(&hub);

    hub.move_right();
    let swapped = vec![
        (a, PixelRect::new(75, 0, 30, 15)),
        (b, PixelRect::new(75, 15, 30, 15)),
        (c, PixelRect::new(45, 0, 30, 30)),
    ];
    assert_eq!(border_boxes_by_window(&hub), swapped);
    assert_eq!(selected_column(&hub), selected(&[a, b]));

    hub.move_right();
    assert_eq!(border_boxes_by_window(&hub), swapped);

    hub.move_left();
    assert_eq!(border_boxes_by_window(&hub), start);
    assert_eq!(selected_column(&hub), selected(&[a, b]));
    validate_hub(&hub);
}

#[test]
fn vertical_actions_and_toggle_split_do_nothing_on_a_selected_column() {
    let (mut hub, _, b, _) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();
    let before = snapshot_text(&hub);

    hub.focus_up();
    assert_eq!(snapshot_text(&hub), before);
    hub.focus_down();
    assert_eq!(snapshot_text(&hub), before);
    hub.move_up();
    assert_eq!(snapshot_text(&hub), before);
    hub.move_down();
    assert_eq!(snapshot_text(&hub), before);
    hub.toggle_spawn_mode();
    assert_eq!(snapshot_text(&hub), before);

    hub.set_focus(b);
    assert_eq!(
        highlighted_windows(&hub),
        vec![(b, Some(Direction::Horizontal))]
    );
    validate_hub(&hub);
}

#[test]
fn focus_left_and_right_leave_the_selected_column() {
    let (mut hub, a, b, c) = stack_beside_one();
    let ws = hub.current_workspace();
    hub.set_focus(b);
    hub.focus_parent();

    hub.focus_left();
    assert_eq!(selected_column(&hub), selected(&[a, b]));

    hub.focus_right();
    assert_eq!(hub.focused_window(ws), Some(c));
    assert_eq!(selected_column(&hub), None);
    validate_hub(&hub);
}

#[test]
fn focus_on_a_selected_column_first_reveals_its_hidden_part() {
    let (mut hub, a, b, _) = stack_beside_one();
    hub.set_window_constraint(
        a,
        LimitObservation {
            min_width: LimitUpdate::Set(Length::new(200.0)),
            ..Default::default()
        },
    );
    hub.set_window_constraint(
        b,
        LimitObservation {
            max_width: LimitUpdate::Set(Length::new(20.0)),
            ..Default::default()
        },
    );
    hub.set_focus(b);
    hub.focus_parent();
    let b_box = |hub: &Hub| {
        border_boxes_by_window(hub)
            .into_iter()
            .find(|&(w, _)| w == b)
            .map(|(_, rect)| rect)
    };
    assert_eq!(b_box(&hub), Some(PixelRect::new(8, 15, 22, 15)));

    hub.focus_left();
    assert_eq!(b_box(&hub), Some(PixelRect::new(90, 15, 22, 15)));
    assert_eq!(selected_column(&hub), selected(&[a, b]));

    hub.focus_left();
    assert_eq!(b_box(&hub), Some(PixelRect::new(90, 15, 22, 15)));
    assert_eq!(selected_column(&hub), selected(&[a, b]));
    validate_hub(&hub);
}

#[test]
fn a_new_window_opens_at_the_bottom_of_the_selected_column() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();
    let w1 = insert(&mut hub, "w1.exe");
    hub.set_focus(w0);
    hub.focus_parent();

    let w2 = insert(&mut hub, "w2.exe");
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 10)),
            (w1, PixelRect::new(60, 10, 30, 10)),
            (w2, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w2));
    assert_eq!(selected_column(&hub), None);
    validate_hub(&hub);
}

#[test]
fn a_window_opened_at_the_bottom_of_a_selected_column_spawns_like_the_window_above_it() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();
    let w1 = insert(&mut hub, "w1.exe");
    hub.toggle_spawn_mode();
    hub.set_focus(w0);
    hub.focus_parent();

    let w2 = insert(&mut hub, "w2.exe");
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 10)),
            (w1, PixelRect::new(60, 10, 30, 10)),
            (w2, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    assert_eq!(
        highlighted_windows(&hub),
        vec![(w2, Some(Direction::Horizontal))]
    );
    validate_hub(&hub);
}

#[test]
fn a_matched_window_ignores_a_selected_column() {
    let mut hub = dev_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    let b = insert(&mut hub, "b.exe");
    let u = insert(&mut hub, "u.exe");
    hub.focus_parent();
    assert_eq!(selected_column(&hub), selected(&[u]));

    let a = insert(&mut hub, "a.exe");
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (b, PixelRect::new(60, 0, 30, 30)),
            (u, PixelRect::new(90, 0, 30, 30)),
            (a, PixelRect::new(30, 0, 30, 30)),
        ]
    );
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(a));
    assert_eq!(selected_column(&hub), None);
    validate_hub(&hub);
}

#[test]
fn grow_and_shrink_resize_a_selected_column() {
    let (mut hub, a, b, _) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();

    hub.grow();
    assert_eq!(
        column_widths(&hub),
        [
            Some(SizeConstraint::Percent(25.0)),
            Some(SizeConstraint::Percent(20.0))
        ]
    );
    assert_eq!(selected_column(&hub), selected(&[a, b]));

    hub.shrink();
    assert_eq!(
        column_widths(&hub),
        [
            Some(SizeConstraint::Percent(20.0)),
            Some(SizeConstraint::Percent(20.0))
        ]
    );
    assert_eq!(selected_column(&hub), selected(&[a, b]));
    validate_hub(&hub);
}

#[test]
fn close_does_nothing_while_a_column_is_selected() {
    let (mut hub, _, b, _) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();
    let close = Actions::new(vec![Action::Close]);
    let mut effects = RecordingEffects::default();

    hub.handle_actions(&close, &mut effects, &mut RecordingKeymap::default());
    assert_eq!(effects.closed, Vec::new());

    hub.set_focus(b);
    hub.handle_actions(&close, &mut effects, &mut RecordingKeymap::default());
    assert_eq!(effects.closed, vec![b]);
}

#[test]
fn float_and_fullscreen_skip_a_selected_column() {
    let (mut hub, _, b, _) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();
    let before = snapshot_text(&hub);

    hub.toggle_float();
    assert_eq!(snapshot_text(&hub), before);
    hub.toggle_fullscreen();
    assert_eq!(snapshot_text(&hub), before);
    validate_hub(&hub);
}

#[test]
fn a_float_that_takes_focus_keeps_the_column_selection() {
    let (mut hub, a, b, c) = stack_beside_one();
    let ws = hub.current_workspace();
    hub.toggle_float();
    hub.set_focus(b);
    hub.focus_parent();

    hub.set_focus(c);
    assert_eq!(hub.focused_window(ws), Some(c));
    assert_eq!(selected_column(&hub), None);

    hub.delete_window(c);
    assert_eq!(selected_column(&hub), selected(&[a, b]));
    assert_eq!(hub.focused_window(ws), None);
    validate_hub(&hub);
}

#[test]
fn a_float_that_tiles_again_opens_at_the_bottom_of_the_selected_column() {
    let (mut hub, a, b, c) = stack_beside_one();
    let ws = hub.current_workspace();
    hub.toggle_float();
    hub.set_focus(a);
    hub.focus_parent();
    hub.set_focus(c);

    hub.toggle_float();
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (b, PixelRect::new(60, 10, 30, 10)),
            (c, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    assert_eq!(hub.focused_window(ws), Some(c));
    assert_eq!(selected_column(&hub), None);
    validate_hub(&hub);
}

#[test]
fn closing_a_window_of_the_selected_column_keeps_the_selection() {
    let (mut hub, a, b, _) = stack_beside_one();
    hub.set_focus(b);
    hub.focus_parent();

    hub.delete_window(b);
    assert_eq!(selected_column(&hub), selected(&[a]));
    validate_hub(&hub);
}

#[test]
fn closing_the_last_window_of_the_selected_column_ends_the_selection() {
    let (mut hub, _, b, c) = stack_beside_one();
    let ws = hub.current_workspace();
    hub.set_focus(c);
    hub.focus_parent();

    hub.delete_window(c);
    assert_eq!(hub.focused_window(ws), Some(b));
    assert_eq!(selected_column(&hub), None);
    assert_eq!(
        highlighted_windows(&hub),
        vec![(b, Some(Direction::Horizontal))]
    );
    validate_hub(&hub);
}

/// The windows that record the named workspace, sorted by id.
fn windows_on(hub: &Hub, workspace: &str) -> Vec<WindowId> {
    let ws_id = hub
        .access
        .workspaces
        .sorted_ids()
        .into_iter()
        .find(|&id| hub.access.workspaces.get(id).name == workspace)
        .expect("workspace present");
    hub.access
        .windows
        .sorted_ids()
        .into_iter()
        .filter(|&id| hub.access.windows.get(id).workspace() == Some(ws_id))
        .collect()
}

/// Opens a over b in a new column on the current workspace, and selects it.
fn select_new_stack(hub: &mut Hub, a: &str, b: &str) -> (WindowId, WindowId) {
    let top = insert(hub, a);
    hub.toggle_spawn_mode();
    let bottom = insert(hub, b);
    hub.focus_parent();
    (top, bottom)
}

#[test]
fn moving_a_selected_column_takes_it_whole_to_another_workspace() {
    let mut hub = narrow_hub();
    hub.focus_workspace("1", None);
    let w3 = insert(&mut hub, "w3.exe");
    hub.focus_workspace("0", None);
    let w0 = insert(&mut hub, "w0.exe");
    hub.toggle_spawn_mode();
    let w1 = insert(&mut hub, "w1.exe");
    hub.toggle_spawn_mode();
    let w2 = insert(&mut hub, "w2.exe");
    hub.set_focus(w1);
    hub.grow();
    hub.focus_parent();

    hub.move_focused_to_workspace("1", None);
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w2));
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![(w2, PixelRect::new(60, 0, 30, 30))]
    );
    assert_eq!(selected_column(&hub), None);

    hub.focus_workspace("1", None);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w3, PixelRect::new(45, 0, 30, 30)),
            (w0, PixelRect::new(75, 0, 30, 15)),
            (w1, PixelRect::new(75, 15, 30, 15)),
        ],
        "the column arrives right of the focused column, at the column width"
    );
    assert_eq!(selected_column(&hub), selected(&[w0, w1]));
    assert_eq!(hub.focused_window(hub.current_workspace()), None);
    validate_hub(&hub);
}

#[test]
fn moving_a_selected_column_away_focuses_by_position() {
    let mut hub = narrow_hub();
    let w0 = insert(&mut hub, "w0.exe");
    let w1 = insert(&mut hub, "w1.exe");
    let w2 = insert(&mut hub, "w2.exe");
    hub.set_focus(w0);
    hub.set_focus(w1);
    hub.focus_parent();

    hub.move_focused_to_workspace("1", None);
    assert_eq!(
        hub.focused_window(hub.current_workspace()),
        Some(w2),
        "focus goes to the column that takes the moved column's place, not to w0, which was \
         focused more recently"
    );
    validate_hub(&hub);
}

#[test]
fn an_arriving_column_opens_right_of_the_focused_column() {
    let mut hub = narrow_hub();
    hub.focus_workspace("1", None);
    let w0 = insert(&mut hub, "w0.exe");
    let w1 = insert(&mut hub, "w1.exe");
    hub.set_focus(w0);
    hub.focus_workspace("0", None);
    let (a, b) = select_new_stack(&mut hub, "a.exe", "b.exe");

    hub.move_focused_to_workspace("1", None);
    hub.focus_workspace("1", None);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(30, 0, 30, 30)),
            (w1, PixelRect::new(90, 0, 30, 30)),
            (a, PixelRect::new(60, 0, 30, 15)),
            (b, PixelRect::new(60, 15, 30, 15)),
        ]
    );
    assert_eq!(selected_column(&hub), selected(&[a, b]));

    hub.focus_left();
    hub.focus_right();
    assert_eq!(
        hub.focused_window(hub.current_workspace()),
        Some(a),
        "the top window of an arriving column heads its focus history"
    );
    validate_hub(&hub);
}

#[test]
fn a_partition_tree_container_arrives_as_one_selected_column() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_scrolling_config(narrow_scrolling())
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Scrolling)
                .build(),
        ])
        .build();
    let w0 = insert(&mut hub, "w0.exe");
    let w1 = insert(&mut hub, "w1.exe");
    hub.toggle_spawn_mode();
    let w2 = insert(&mut hub, "w2.exe");
    hub.focus_parent();
    hub.focus_parent();

    hub.move_focused_to_workspace("1", None);
    hub.focus_workspace("1", None);
    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (w0, PixelRect::new(60, 0, 30, 10)),
            (w1, PixelRect::new(60, 10, 30, 10)),
            (w2, PixelRect::new(60, 20, 30, 10)),
        ]
    );
    assert_eq!(selected_column(&hub), selected(&[w0, w1, w2]));
    validate_hub(&hub);
}

#[test]
fn a_selected_column_moved_into_master_or_partition_tree_keeps_its_windows() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Scrolling)
                .with_scrolling_config(narrow_scrolling())
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("1")
                .with_strategy(Strategy::Master)
                .build(),
            LayoutWorkspaceConfigBuilder::new("2")
                .with_strategy(Strategy::PartitionTree)
                .build(),
        ])
        .build();
    let (w0, w1) = select_new_stack(&mut hub, "w0.exe", "w1.exe");
    hub.move_focused_to_workspace("1", None);
    let (w2, w3) = select_new_stack(&mut hub, "w2.exe", "w3.exe");
    hub.move_focused_to_workspace("2", None);

    assert_eq!(windows_on(&hub, "0"), Vec::new());
    assert_eq!(windows_on(&hub, "1"), vec![w0, w1]);
    assert_eq!(windows_on(&hub, "2"), vec![w2, w3]);
    validate_hub(&hub);
}

#[test]
fn a_selected_column_of_one_window_moved_into_partition_tree_arrives_as_that_window() {
    let mut hub = TestHubBuilder::new()
        .with_tiling(
            TilingConfigBuilder::new()
                .with_strategy(Strategy::Scrolling)
                .with_scrolling_config(narrow_scrolling())
                .build(),
        )
        .with_preferred_layout(vec![
            LayoutWorkspaceConfigBuilder::new("2")
                .with_strategy(Strategy::PartitionTree)
                .build(),
        ])
        .build();
    let w0 = insert(&mut hub, "w0.exe");
    hub.focus_parent();

    hub.move_focused_to_workspace("2", None);
    hub.focus_workspace("2", None);
    assert_eq!(hub.focused_window(hub.current_workspace()), Some(w0));
    validate_hub(&hub);
}
