use super::{PARTITION_CONFIG, fixture::TestDome};
use crate::platform::windows::window_server::WindowNotification;

#[test]
fn complete_title_updates_minimized_query_and_ignores_removed_window() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let window = dome.open("Window");
    dome.change(window, |window| window.state.minimized = true);
    dome.deliver_notification(WindowNotification::WindowMinimized(window));
    dome.deliver_notification(WindowNotification::TitleChanged {
        window_id: window,
        title: Some("Delivered".into()),
    });
    let entries = dome.dome.query_minimized_windows_value();
    let entries = entries.as_array().expect("minimized window rows");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["id"], serde_json::to_value(window).unwrap());
    assert_eq!(entries[0]["title"], "Delivered");
    assert!(dome.minimized(window));

    dome.remove(window);
    dome.deliver_notification(WindowNotification::TitleChanged {
        window_id: window,
        title: Some("Late".into()),
    });
    assert_eq!(
        dome.dome.query_minimized_windows_value(),
        serde_json::json!([])
    );
}
