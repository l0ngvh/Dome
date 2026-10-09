use super::{border_boxes_by_window, dev_hub, insert, process_matcher, stack};
use crate::core::tests::validate_hub;
use crate::core::{ColumnConfig, PixelRect};

#[test]
fn a_window_moved_out_of_its_column_leaves_the_column_slot_behind() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let b = insert(&mut hub, "b.exe");
    hub.move_right();
    hub.delete_window(a);

    let a2 = insert(&mut hub, "a.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (b, PixelRect::new(45, 0, 30, 30)),
            (a2, PixelRect::new(75, 0, 30, 30)),
        ],
        "b still holds a slot of the column slot, but a2 opens a column of its own"
    );
    validate_hub(&hub);
}

#[test]
fn a_window_the_spawn_rules_put_in_the_live_column_counts_for_the_order() {
    let mut hub = dev_hub(vec![stack(&["a.exe", "b.exe", "c.exe"])]);
    let a = insert(&mut hub, "a.exe");
    let c = insert(&mut hub, "c.exe");
    hub.set_focus(a);
    hub.toggle_spawn_mode();
    hub.set_focus(c);
    hub.toggle_fullscreen();
    hub.toggle_fullscreen();

    let b = insert(&mut hub, "b.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (c, PixelRect::new(60, 20, 30, 10)),
            (b, PixelRect::new(60, 10, 30, 10)),
        ],
        "c tiled back below a by the spawn rules and still holds the slot after b's"
    );
    validate_hub(&hub);
}

#[test]
fn a_live_column_keeps_its_column_slot_after_its_matched_window_closes() {
    let mut hub = dev_hub(vec![
        ColumnConfig::bare(process_matcher("a.exe")),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    let a = insert(&mut hub, "a.exe");
    hub.toggle_spawn_mode();
    let u = insert(&mut hub, "u.exe");
    hub.delete_window(a);
    let b = insert(&mut hub, "b.exe");

    let a2 = insert(&mut hub, "a.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (u, PixelRect::new(45, 0, 30, 15)),
            (b, PixelRect::new(75, 0, 30, 30)),
            (a2, PixelRect::new(45, 15, 30, 15)),
        ],
        "a2 joins the column that a opened, below u, which holds no slot"
    );
    validate_hub(&hub);
}

#[test]
fn a_window_holding_a_slot_of_another_column_slot_does_not_move_a_match() {
    let mut hub = dev_hub(vec![
        stack(&["a.exe", "c.exe"]),
        ColumnConfig::bare(process_matcher("b.exe")),
    ]);
    let a = insert(&mut hub, "a.exe");
    hub.toggle_spawn_mode();
    let b = insert(&mut hub, "b.exe");
    hub.minimize_window(b);
    hub.unminimize_window(b);

    let c = insert(&mut hub, "c.exe");

    assert_eq!(
        border_boxes_by_window(&hub),
        vec![
            (a, PixelRect::new(60, 0, 30, 10)),
            (b, PixelRect::new(60, 10, 30, 10)),
            (c, PixelRect::new(60, 20, 30, 10)),
        ],
        "b's slot comes after c's in the arena, but b holds it in another column slot"
    );
    validate_hub(&hub);
}
