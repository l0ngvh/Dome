use super::*;
use crate::core::WindowMatcher;

fn assert_bundle_id_ignore(pattern: &str, ignores_finder: bool) {
    let mut macos = MacOS::new();
    let mut dome = macos
        .dome_builder()
        .tiling(|tiling| {
            tiling.ignore = vec![WindowMatcher {
                bundle_id: Some(pattern.to_owned()),
                ..Default::default()
            }];
        })
        .build();

    let original_frame = (100, 150, 800, 600);
    let finder = macos.spawn_window_at(100, "Finder", "Trash", 100, 150, 800, 600);
    dome.add_windows_with_bundle_id(&macos, &[finder], "com.apple.finder");
    macos.settle(&mut dome, 10);
    assert_eq!(
        macos.window_frame(finder),
        if ignores_finder {
            original_frame
        } else {
            (4, 4, 1912, 1072)
        },
        "Finder placement for {pattern:?}"
    );

    let other = macos.spawn_window_at(101, "Finder", "Trash", 200, 250, 800, 600);
    dome.add_windows_with_bundle_id(&macos, &[other], "org.example.editor");
    macos.settle(&mut dome, 10);
    if ignores_finder {
        assert_eq!(macos.window_frame(finder), original_frame, "{pattern:?}");
        assert_eq!(macos.window_frame(other), (4, 4, 1912, 1072), "{pattern:?}");
    } else {
        assert_eq!(macos.window_frame(finder), (4, 4, 952, 1072), "{pattern:?}");
        assert_eq!(
            macos.window_frame(other),
            (964, 4, 952, 1072),
            "{pattern:?}"
        );
    }
}

#[test]
fn ignore_by_bundle_id_literal_leaves_only_matching_window_untiled() {
    assert_bundle_id_ignore("com.apple.finder", true);
    assert_bundle_id_ignore("com.apple.safari", false);
}

#[test]
fn ignore_by_bundle_id_regex_leaves_only_matching_window_untiled() {
    assert_bundle_id_ignore(r"/^com\.apple\./", true);
    assert_bundle_id_ignore("/finder$/", true);
    assert_bundle_id_ignore(r"/^com\.google\./", false);
}

#[test]
fn ignore_by_bundle_id_without_closing_delimiter_keeps_window_tiled() {
    assert_bundle_id_ignore(r"/^com\.apple\.", false);
}
