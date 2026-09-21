use crate::core::{WindowMatcher, WindowMetadata};
use crate::platform::macos::dome::MacOSMetadata;

fn finder() -> MacOSMetadata {
    MacOSMetadata {
        title: Some("Trash".to_string()),
        app_name: Some("Finder".to_string()),
        bundle_id: Some("com.apple.finder".to_string()),
    }
}

fn bundle_id_matcher(pattern: &str) -> WindowMatcher {
    WindowMatcher {
        bundle_id: Some(pattern.to_string()),
        ..Default::default()
    }
}

#[test]
fn bundle_id_matches_a_literal() {
    assert!(finder().matches_window_matcher(&bundle_id_matcher("com.apple.finder")));
    assert!(!finder().matches_window_matcher(&bundle_id_matcher("com.apple.safari")));
}

#[test]
fn bundle_id_matches_a_regex() {
    assert!(finder().matches_window_matcher(&bundle_id_matcher("/^com\\.apple\\./")));
    assert!(finder().matches_window_matcher(&bundle_id_matcher("/finder$/")));
    assert!(!finder().matches_window_matcher(&bundle_id_matcher("/^com\\.google\\./")));
}

#[test]
fn bundle_id_regex_needs_both_delimiters() {
    assert!(!finder().matches_window_matcher(&bundle_id_matcher("/^com\\.apple\\.")));
}
