use crate::config::paths;

/// The passwd record is the fallback when `HOME` is unset, so it has to name the
/// same directory the environment does when both are available.
#[test]
#[cfg(target_os = "macos")]
fn passwd_home_matches_the_environment() {
    let from_env = std::env::var("HOME").expect("HOME set in the test environment");
    assert_eq!(paths::passwd_home().as_deref(), Some(from_env.as_str()));
}

#[test]
#[cfg(target_os = "macos")]
fn log_dir_sits_under_the_home_directory() {
    let home = std::env::var("HOME").expect("HOME set in the test environment");
    assert_eq!(paths::log_dir(), format!("{home}/Library/Logs/dome"));
}
