use std::process::{Child, Command};
use std::thread;
use std::time::Duration;

use dome::DomeClient;

fn spawn_server(config_path: &str) -> Child {
    Command::new(env!("CARGO_BIN_EXE_dome"))
        .args(["launch", "--config", config_path])
        .spawn()
        .expect("failed to start server")
}

fn wait_for_server(timeout: Duration) -> bool {
    let client = DomeClient;
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if client.ping() {
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
fn kill_test_app() {
    Command::new("killall").arg("TextEdit").output().ok();
}

#[cfg(target_os = "windows")]
fn kill_test_app() {
    Command::new("taskkill")
        .args(["/IM", "notepad.exe", "/F"])
        .output()
        .ok();
}

struct TestEnv {
    server: Child,
}

impl TestEnv {
    fn new() -> Self {
        Self::with_config("resources/config.starter.lua")
    }

    fn with_config(config_path: &str) -> Self {
        let server = spawn_server(config_path);
        assert!(
            wait_for_server(Duration::from_secs(5)),
            "server failed to start"
        );
        Self { server }
    }
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        dome(&["exit"]);
        self.server.wait().ok();
        kill_test_app();
    }
}

#[test]
#[cfg(target_os = "macos")]
fn test_exec() {
    let _env = TestEnv::new();

    let marker = std::env::temp_dir().join("dome_exec_test_marker");
    std::fs::remove_file(&marker).ok();

    let cmd = format!("touch {}", marker.display());
    assert!(dome(&["execute", &cmd]));

    thread::sleep(Duration::from_millis(1000));

    assert!(
        marker.exists(),
        "execute command did not create marker file"
    );

    std::fs::remove_file(&marker).ok();
}

#[test]
#[cfg(target_os = "windows")]
fn test_exec() {
    let _env = TestEnv::new();

    let marker = std::env::temp_dir().join("dome_exec_test_marker");
    std::fs::remove_file(&marker).ok();

    let cmd = format!("type nul > {}", marker.display());
    assert!(dome(&["execute", &cmd]));

    thread::sleep(Duration::from_millis(1000));

    assert!(
        marker.exists(),
        "execute command did not create marker file"
    );

    std::fs::remove_file(&marker).ok();
}
