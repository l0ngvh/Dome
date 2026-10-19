use super::{PARTITION_CONFIG, fixture::TestDome};
use dome_core::PixelRect;
use windows::Win32::UI::WindowsAndMessaging::{WS_EX_APPWINDOW, WS_OVERLAPPEDWINDOW, WS_POPUP};

#[test]
fn observe_window_while_pending_preserves_layout_and_focus() {
    let mut dome = TestDome::new(PARTITION_CONFIG);
    let first = dome.open("First");
    let pending = dome.begin_window_observation("Pending");
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));
    assert_eq!(dome.frame(pending), PixelRect::new(5, 5, 40, 10));
    assert_eq!(dome.focused(), Some(first));
    assert!(!dome.dome.is_managed(pending));
    dome.complete_window_observation(pending);
    assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 18));
    assert_eq!(dome.frame(pending), PixelRect::new(51, 1, 48, 18));
    assert_eq!(dome.focused(), Some(pending));
}

#[test]
fn observe_window_while_ineligible_preserves_layout_until_manageable() {
    for reason in [
        "hidden",
        "minimized",
        "cloaked",
        "child",
        "owned",
        "small popup",
        "unconfirmed popup",
    ] {
        let mut dome = TestDome::new(PARTITION_CONFIG);
        let first = dome.open("First");
        let window = dome.begin_window_observation("Window");
        dome.change(window, |window| match reason {
            "hidden" => window.state.visible = false,
            "minimized" => window.state.minimized = true,
            "cloaked" => window.state.cloaked = true,
            "child" => window.state.is_root = false,
            "owned" => window.state.has_owner = true,
            "small popup" => window.state.style = WS_POPUP.0,
            "unconfirmed popup" => {
                window.state.style = WS_POPUP.0;
                window.state.window_rect = PixelRect::new(0, 0, 100, 20);
                window.state.monitor_work_area = None;
            }
            _ => unreachable!(),
        });
        let initial = dome.frame(window);
        dome.complete_window_observation(window);
        assert!(!dome.dome.is_managed(window), "{reason}");
        assert_eq!(dome.frame(window), initial, "{reason}");
        assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18), "{reason}");
        assert_eq!(dome.focused(), Some(first), "{reason}");
        dome.change(window, |window| {
            window.state.visible = true;
            window.state.minimized = false;
            window.state.cloaked = false;
            window.state.is_root = true;
            window.state.extended_style = WS_EX_APPWINDOW.0;
            if reason == "small popup" || reason == "unconfirmed popup" {
                window.state.window_rect = PixelRect::new(0, 0, 100, 20);
                window.state.monitor_work_area = Some(PixelRect::new(0, 0, 100, 20));
            } else {
                window.state.style = WS_OVERLAPPEDWINDOW.0;
            }
        });
        dome.complete_window_observation(window);
        assert!(dome.dome.is_managed(window), "{reason}");
        assert_eq!(dome.focused(), Some(window), "{reason}");
        assert!(dome.in_taskbar(window), "{reason}");
        if reason == "small popup" || reason == "unconfirmed popup" {
            assert_eq!(dome.frame(window), PixelRect::new(0, 0, 100, 20));
            assert!(dome.frame(first).x().value() < 0);
        } else {
            assert_eq!(dome.frame(window), PixelRect::new(51, 1, 48, 18));
            assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 18));
        }
    }
}

#[test]
fn observe_windows_with_empty_or_helper_titles_admits_only_document_windows() {
    for (matcher, ignored) in [
        ("{ app = 'Test' }", true),
        ("{ app = 'Other' }", false),
        ("{ process = 'test.exe' }", true),
        ("{ process = 'other.exe' }", false),
        ("{ class = 'DocumentWindow' }", true),
        ("{ class = 'OtherWindow' }", false),
        ("{}", false),
    ] {
        let config =
            PARTITION_CONFIG.replace("layout =", &format!("ignore = {{ {matcher} }}, layout ="));
        let mut dome = TestDome::new(&config);
        let window = dome.begin_window_observation("Document");
        dome.change(window, |window| {
            window.metadata.class = Some("DocumentWindow".into());
        });
        dome.complete_window_observation(window);
        assert_eq!(dome.dome.is_managed(window), !ignored, "{matcher}");
        assert_eq!(
            dome.frame(window),
            if ignored {
                PixelRect::new(5, 5, 40, 10)
            } else {
                PixelRect::new(1, 1, 98, 18)
            },
            "{matcher}",
        );
        assert_eq!(dome.in_taskbar(window), !ignored, "{matcher}");
    }

    for title in [
        None,
        Some(""),
        Some("OleMainThreadWndName"),
        Some("OLEChannelWnd"),
        Some("Default IME"),
        Some("MSCTFIME UI"),
    ] {
        let mut dome = TestDome::new(PARTITION_CONFIG);
        let first = dome.open("First");
        let window = dome.begin_window_observation("Window");
        dome.change(window, |window| {
            window.metadata.title = title.map(str::to_owned)
        });
        dome.complete_window_observation(window);
        if let Some(helper) = title.filter(|title| !title.is_empty()) {
            assert!(!dome.dome.is_managed(window));
            assert_eq!(dome.frame(first), PixelRect::new(1, 1, 98, 18));
            assert_eq!(dome.focused(), Some(first));
            dome.change(window, |window| {
                window.metadata.title = Some(format!("{helper} - notes.txt"));
            });
            dome.complete_window_observation(window);
        }
        assert_eq!(dome.frame(window), PixelRect::new(51, 1, 48, 18));
        assert_eq!(dome.frame(first), PixelRect::new(1, 1, 48, 18));
        assert_eq!(dome.focused(), Some(window));
    }
}
