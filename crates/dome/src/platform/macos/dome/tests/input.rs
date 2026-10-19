use super::fixture::{PARTITION_CONFIG, TestDome};
use dome_core::PixelRect;

#[test]
fn click_tab_focuses_selected_window_and_updates_presented_active_tab() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let second = dome.open("Second");
    dome.run("toggle layout");
    let titles = ["First", "Second"];
    let container = dome.tab_container(&titles, None);
    assert_eq!(dome.focused(), Some(second));
    assert_eq!(
        dome.overlays().requested.as_ref().unwrap().tabs[&container]
            .1
            .placement
            .active_tab_index,
        1
    );
    let displayed = dome.frame(second);
    assert_eq!(dome.frame(first).x().value(), 99);

    dome.click_tab(&titles, None, 0);
    assert_eq!(dome.focused(), Some(first));
    assert_eq!(dome.frame(first), displayed);
    assert_eq!(
        dome.frame(second),
        PixelRect::new(
            99,
            19,
            displayed.width().value(),
            displayed.height().value()
        )
    );
    assert_eq!(
        dome.overlays().requested.as_ref().unwrap().tabs[&container]
            .1
            .placement
            .active_tab_index,
        0
    );
}
