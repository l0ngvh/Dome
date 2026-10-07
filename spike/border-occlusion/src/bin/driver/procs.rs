//! Child processes of the driver, and the cleanup that runs on every exit path.

use std::cell::RefCell;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, channel};
use std::time::{Duration, Instant};

use spike::geometry::Rect;

use crate::input;

const SLOTS: usize = 64;
/// Pids to kill on exit. Atomics, so the signal handler can read them.
static CHILDREN: [AtomicI32; SLOTS] = [const { AtomicI32::new(0) }; SLOTS];
static CLEANED: AtomicBool = AtomicBool::new(false);

pub fn register(pid: u32) {
    for slot in &CHILDREN {
        if slot
            .compare_exchange(0, pid as i32, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            return;
        }
    }
}

pub fn unregister(pid: u32) {
    for slot in &CHILDREN {
        let _ = slot.compare_exchange(pid as i32, 0, Ordering::SeqCst, Ordering::SeqCst);
    }
}

pub fn is_child(pid: i32) -> bool {
    pid > 0 && CHILDREN.iter().any(|slot| slot.load(Ordering::SeqCst) == pid)
}

/// Releases any held mouse button and kills every registered child. Safe to
/// call more than once.
pub fn cleanup() {
    if CLEANED.swap(true, Ordering::SeqCst) {
        return;
    }
    input::release_buttons();
    for slot in &CHILDREN {
        let pid = slot.swap(0, Ordering::SeqCst);
        if pid > 0 {
            unsafe { libc::kill(pid, libc::SIGKILL) };
        }
    }
}

extern "C" fn on_signal(_: libc::c_int) {
    cleanup();
    unsafe { libc::_exit(130) };
}

pub fn install_cleanup() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        cleanup();
        default_hook(info);
    }));
    for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        unsafe { libc::signal(signal, on_signal as *const () as libc::sighandler_t) };
    }
}

/// Spawns and registers a child for cleanup.
pub fn spawn(command: &mut Command) -> Result<Child, String> {
    let child = command
        .spawn()
        .map_err(|e| format!("spawn {command:?}: {e}"))?;
    register(child.id());
    Ok(child)
}

pub fn reap(mut child: Child) {
    unsafe { libc::kill(child.id() as i32, libc::SIGTERM) };
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            unregister(child.id());
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    let _ = child.kill();
    let _ = child.wait();
    unregister(child.id());
}

pub struct Fixture {
    pub name: String,
    pub pid: i32,
    child: Option<Child>,
    stdin: ChildStdin,
    rx: Receiver<String>,
}

impl Fixture {
    pub fn spawn(bin: &Path, name: &str) -> Result<Fixture, String> {
        let mut child = spawn(
            Command::new(bin)
                .args(["--name", name])
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit()),
        )?;
        let stdin = child.stdin.take().ok_or("fixture stdin")?;
        let stdout = child.stdout.take().ok_or("fixture stdout")?;
        let (tx, rx) = channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { return };
                if tx.send(line).is_err() {
                    return;
                }
            }
        });
        let pid = child.id() as i32;
        let mut fixture = Fixture {
            name: name.to_string(),
            pid,
            child: Some(child),
            stdin,
            rx,
        };
        let ready = fixture.read_reply()?;
        if !ready.starts_with("ready") {
            return Err(format!("fixture {name} said {ready}"));
        }
        Ok(fixture)
    }

    fn read_reply(&mut self) -> Result<String, String> {
        self.read_reply_within(Duration::from_secs(5))
    }

    fn read_reply_within(&mut self, limit: Duration) -> Result<String, String> {
        match self.rx.recv_timeout(limit) {
            Ok(line) => match line.strip_prefix("ok ") {
                Some(rest) => Ok(rest.to_string()),
                None => Err(format!("fixture {}: {line}", self.name)),
            },
            Err(RecvTimeoutError::Timeout) => Err(format!("fixture {} timed out", self.name)),
            Err(RecvTimeoutError::Disconnected) => Err(format!("fixture {} exited", self.name)),
        }
    }

    pub fn cmd(&mut self, line: &str) -> Result<String, String> {
        self.cmd_within(line, Duration::from_secs(5))
    }

    /// For a command whose reply waits until the command finishes.
    pub fn cmd_within(&mut self, line: &str, limit: Duration) -> Result<String, String> {
        writeln!(self.stdin, "{line}").map_err(|e| format!("fixture {}: {e}", self.name))?;
        self.stdin
            .flush()
            .map_err(|e| format!("fixture {}: {e}", self.name))?;
        self.read_reply_within(limit)
    }

    /// Returns the CGWindowID.
    pub fn open(&mut self, label: &str, r: Rect, color: &str) -> Result<u32, String> {
        let reply = self.cmd(&format!(
            "open {label} {} {} {} {} {color}",
            r.x, r.y, r.w, r.h
        ))?;
        parse_window_number(&reply)
    }

    pub fn backdrop(&mut self) -> Result<u32, String> {
        let reply = self.cmd("backdrop")?;
        parse_window_number(&reply)
    }
}

fn parse_window_number(reply: &str) -> Result<u32, String> {
    reply
        .split_whitespace()
        .last()
        .and_then(|n| n.parse().ok())
        .ok_or_else(|| format!("no window number in {reply}"))
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = writeln!(self.stdin, "quit");
        if let Some(child) = self.child.take() {
            reap(child);
        }
    }
}

/// Writes `manage` and `unmanage` lines to the overlay's stdin. A write to an
/// overlay that already exited fails quietly.
#[derive(Clone)]
pub struct Control(Rc<RefCell<ChildStdin>>);

impl Control {
    pub fn send(&self, line: &str) {
        let mut stdin = self.0.borrow_mut();
        let _ = writeln!(stdin, "{line}");
        let _ = stdin.flush();
    }
}

pub struct Overlay {
    pub pid: i32,
    pub log: PathBuf,
    pub control: Control,
    child: Option<Child>,
}

impl Overlay {
    pub fn start(bin: &Path, args: &[String], log: &Path) -> Result<Overlay, String> {
        let file = std::fs::File::create(log).map_err(|e| format!("{}: {e}", log.display()))?;
        let mut child = spawn(
            Command::new(bin)
                .args(args)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(file),
        )?;
        let stdin = child.stdin.take().ok_or("overlay stdin")?;
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            let text = std::fs::read_to_string(log).unwrap_or_default();
            if text.contains("event=start") {
                break;
            }
            if let Ok(Some(status)) = child.try_wait() {
                unregister(child.id());
                return Err(format!("overlay exited with {status}: {text}"));
            }
            if Instant::now() > deadline {
                return Err(format!("overlay did not start: {text}"));
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        Ok(Overlay {
            pid: child.id() as i32,
            log: log.to_path_buf(),
            control: Control(Rc::new(RefCell::new(stdin))),
            child: Some(child),
        })
    }
}

impl Drop for Overlay {
    fn drop(&mut self) {
        if let Some(child) = self.child.take() {
            reap(child);
        }
    }
}
