use std::os::unix::net::UnixStream;
use std::process::{Child, Command};
#[cfg(target_os = "macos")]
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use dome::{Action, FocusTarget, MoveTarget, ToggleTarget, send_action};

fn spawn_server() -> Child {
    Command::new(env!("CARGO_BIN_EXE_dome"))
        .arg("launch")
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
        send_action(&Action::Exit).ok();
        self.server.wait().unwrap();
        kill_test_app();
    }
}

#[test]
fn test_horizontal_navigation() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Left
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Right
        })
        .is_ok()
    );

    env.shutdown();
}

#[test]
fn test_vertical_navigation() {
    let env = TestEnv::new();
    spawn_test_window();
    assert!(
        send_action(&Action::Toggle {
            target: ToggleTarget::Direction
        })
        .is_ok()
    );
    spawn_test_window();

    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Up
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Down
        })
        .is_ok()
    );

    env.shutdown();
}

#[test]
fn test_move_to_workspace() {
    let env = TestEnv::new();
    spawn_test_window();

    assert!(
        send_action(&Action::Move {
            target: MoveTarget::Workspace { index: 1 }
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Workspace { index: 1 }
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Workspace { index: 0 }
        })
        .is_ok()
    );

    env.shutdown();
}

#[test]
fn test_move_window_position() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    assert!(
        send_action(&Action::Move {
            target: MoveTarget::Left
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Move {
            target: MoveTarget::Right
        })
        .is_ok()
    );

    env.shutdown();
}

#[test]
fn test_float_toggle() {
    let env = TestEnv::new();
    spawn_test_window();

    assert!(
        send_action(&Action::Toggle {
            target: ToggleTarget::Float
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Toggle {
            target: ToggleTarget::Float
        })
        .is_ok()
    );

    env.shutdown();
}

#[test]
fn test_tabbed_navigation() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();
    assert!(
        send_action(&Action::Toggle {
            target: ToggleTarget::Layout
        })
        .is_ok()
    );

    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::PrevTab
        })
        .is_ok()
    );
    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::NextTab
        })
        .is_ok()
    );

    env.shutdown();
}

#[test]
fn test_focus_parent() {
    let env = TestEnv::new();
    spawn_test_window();
    spawn_test_window();

    assert!(
        send_action(&Action::Focus {
            target: FocusTarget::Parent
        })
        .is_ok()
    );

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
