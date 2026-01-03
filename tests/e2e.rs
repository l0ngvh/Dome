use std::os::unix::net::UnixStream;
use std::process::{Child, Command};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

fn spawn_server() -> Child {
    Command::new(env!("CARGO_BIN_EXE_dome"))
        .args(["launch", "--config", "examples/config.toml"])
        .spawn()
        .expect("failed to start server")
}

fn wait_for_server(timeout: Duration) -> bool {
    let socket = std::env::temp_dir().join("dome.sock");
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if UnixStream::connect(&socket).is_ok() {
            return true;
        }
        thread::sleep(Duration::from_millis(50));
    }
    false
}

fn dome(args: &[&str]) -> bool {
    Command::new(env!("CARGO_BIN_EXE_dome"))
        .args(args)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(target_os = "macos")]
static FIRST_WINDOW: AtomicBool = AtomicBool::new(true);

#[cfg(target_os = "macos")]
fn spawn_test_window() {
    // First window doesn't fire events reliably, spawn a dummy
    if FIRST_WINDOW.swap(false, Ordering::SeqCst) {
        Command::new("open")
            .args(["-na", "TextEdit"])
            .output()
            .expect("failed to spawn test window");
        thread::sleep(Duration::from_millis(500));
    }
    Command::new("open")
        .args(["-na", "TextEdit"])
        .output()
        .expect("failed to spawn test window");
    thread::sleep(Duration::from_millis(500));
}

#[cfg(target_os = "macos")]
fn close_front_window() {
    Command::new("osascript")
        .args(["-e", "tell application \"TextEdit\" to close front window"])
        .output()
        .ok();
    thread::sleep(Duration::from_millis(100));
}

#[cfg(target_os = "macos")]
fn quit_test_app() {
    Command::new("osascript")
        .args(["-e", "tell application \"TextEdit\" to quit saving no"])
        .output()
        .ok();
    thread::sleep(Duration::from_millis(300));
}

#[cfg(target_os = "macos")]
fn kill_test_app() {
    // Wait until all TextEdit instances are killed
    Command::new("killall").arg("TextEdit").output().ok();
}

struct TestEnv {
    server: Child,
}

impl TestEnv {
    fn new() -> Self {
        let server = spawn_server();
        assert!(
            wait_for_server(Duration::from_secs(5)),
            "server failed to start"
        );
        Self { server }
    }

    fn shutdown(mut self) {
        dome(&["exit"]);
        self.server.wait().unwrap();
        kill_test_app();
    }
}

#[test]
fn test_horizontal_navigation() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    assert!(dome(&["focus", "left"]));
    assert!(dome(&["focus", "right"]));

    env.shutdown();
}

#[test]
fn test_vertical_navigation() {
    let env = TestEnv::new();
    spawn_test_window();
    assert!(dome(&["toggle", "direction"]));
    spawn_test_window();

    assert!(dome(&["focus", "up"]));
    assert!(dome(&["focus", "down"]));

    env.shutdown();
}

#[test]
fn test_move_to_workspace() {
    let env = TestEnv::new();
    spawn_test_window();

    assert!(dome(&["move", "workspace", "1"]));
    assert!(dome(&["focus", "workspace", "1"]));
    assert!(dome(&["focus", "workspace", "0"]));

    env.shutdown();
}

#[test]
fn test_move_window_position() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    assert!(dome(&["move", "left"]));
    assert!(dome(&["move", "right"]));

    env.shutdown();
}

#[test]
fn test_float_toggle() {
    let env = TestEnv::new();
    spawn_test_window();

    assert!(dome(&["toggle", "float"]));
    assert!(dome(&["toggle", "float"]));

    env.shutdown();
}

#[test]
fn test_tabbed_navigation() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();
    assert!(dome(&["toggle", "layout"]));

    assert!(dome(&["focus", "prev-tab"]));
    assert!(dome(&["focus", "next-tab"]));

    env.shutdown();
}

#[test]
fn test_focus_parent() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    assert!(dome(&["focus", "parent"]));

    env.shutdown();
}

#[test]
fn test_close_window() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    close_front_window();

    env.shutdown();
}

#[test]
fn test_terminate_app() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();
    spawn_test_window();

    quit_test_app();

    env.shutdown();
}
