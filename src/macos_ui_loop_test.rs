#[cfg(target_os = "macos")]
fn main() {
    dome::run_native_ui_loop_test();
}

#[cfg(not(target_os = "macos"))]
fn main() {
    eprintln!("UNAVAILABLE macos_ui_loop requires a macOS desktop");
    std::process::exit(2);
}
