use super::{Result, windows};
use ::windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, WPARAM},
        System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    },
    core::w,
};
use serde_json::{Value, json};
use std::{
    io::{BufRead, Write},
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

pub struct Fixture {
    child: Child,
    input: ChildStdin,
    pub replies: Receiver<Value>,
}

impl Fixture {
    pub fn spawn() -> Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--fixture")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()?;
        let input = child.stdin.take().expect("piped stdin");
        let output = child.stdout.take().expect("piped stdout");
        let (sender, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in std::io::BufReader::new(output).lines() {
                let value = match line {
                    Ok(line) => serde_json::from_str(&line).unwrap_or_else(
                        |error| json!({"reply": "protocol_error", "error": error.to_string()}),
                    ),
                    Err(error) => json!({"reply": "protocol_error", "error": error.to_string()}),
                };
                if sender.send(value).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            input,
            replies,
        })
    }

    pub fn send(&mut self, command: Value) -> Result<()> {
        serde_json::to_writer(&mut self.input, &command)?;
        self.input.write_all(b"\n")?;
        self.input.flush()?;
        Ok(())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.send(json!({"command": "stop"}));
        for _ in 0..100 {
            windows::pump();
            if self.child.try_wait().ok().flatten().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct Windows(Vec<HWND>);
impl Drop for Windows {
    fn drop(&mut self) {
        for hwnd in &self.0 {
            unsafe {
                let _ = DestroyWindow(*hwnd);
            }
        }
    }
}

unsafe extern "system" fn procedure(
    hwnd: HWND,
    message: u32,
    word: WPARAM,
    long: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, message, word, long) }
}

fn create() -> Result<Windows> {
    unsafe {
        let instance = GetModuleHandleW(None)?;
        let class = WNDCLASSW {
            lpfnWndProc: Some(procedure),
            hInstance: instance.into(),
            lpszClassName: w!("TopmostReorderFixture"),
            ..Default::default()
        };
        if RegisterClassW(&class) == 0 {
            return Err(::windows::core::Error::from_thread().into());
        }
        let mut windows = Windows(Vec::new());
        for (index, name) in [
            w!("Reorder spike TARGET"),
            w!("Reorder spike ordinary PEER"),
            w!("Reorder spike band PEER"),
        ]
        .into_iter()
        .enumerate()
        {
            let hwnd = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                w!("TopmostReorderFixture"),
                name,
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100 + index as i32 * 180,
                100 + index as i32 * 100,
                360,
                240,
                None,
                None,
                Some(instance.into()),
                None,
            )?;
            windows.0.push(hwnd);
        }
        Ok(windows)
    }
}

fn place(hwnd: HWND, after: HWND) -> Result<()> {
    unsafe {
        SetWindowPos(
            hwnd,
            Some(after),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )?;
    }
    Ok(())
}

fn setup(windows: &Windows, case: &str, evidence: &mut Value) -> Result<()> {
    if case == "lowest_topmost_unchanged_adjacency" {
        *evidence = json!({"eligible": false, "phase": "reset_fixtures"});
    }
    for hwnd in &windows.0 {
        place(*hwnd, HWND_NOTOPMOST)?;
    }
    let target = windows.0[0];
    let peer = windows.0[1];
    match case {
        "below_ordinary_peer" => {
            place(peer, HWND_TOP)?;
            place(target, peer)?;
        }
        "highest_ordinary_no_other_topmost" => place(target, HWND_TOP)?,
        "alongside_fixture_topmost" => {
            place(windows.0[2], HWND_TOPMOST)?;
            place(target, HWND_TOP)?;
        }
        "lowest_topmost_unchanged_adjacency" => {
            evidence["phase"] = json!("capture_normal_boundary");
            place(windows.0[2], HWND_TOPMOST)?;
            place(target, HWND_TOP)?;
            let reference = windows::snapshot(windows::number(target));
            let eligible = reference.local.eligible(false)
                && reference.ordinary.contains(&windows::number(peer))
                && reference
                    .other_topmost
                    .contains(&windows::number(windows.0[2]));
            evidence["reference"] = reference.value;
            if !eligible {
                evidence["reason"] =
                    json!("normal target lacks a complete adjacent band boundary or fixture peers");
                return Ok(());
            }
            evidence["phase"] = json!("promote_and_restore_boundary");
            place(target, HWND_TOPMOST)?;
            let predecessor = reference
                .local
                .previous
                .expect("validated predecessor")
                .handle;
            place(target, windows::handle(predecessor))?;
            let prepared = windows::snapshot(windows::number(target));
            let eligible = prepared
                .local
                .at_reference(reference.local.neighbors(), true);
            evidence["prepared"] = prepared.value;
            evidence["eligible"] = json!(eligible);
            evidence["phase"] = json!("prepared_boundary_observed");
            if !eligible {
                evidence["reason"] =
                    json!("promoted target did not retain the captured adjacent band boundary");
            }
        }
        _ => return Err("unknown fixture case".into()),
    }
    Ok(())
}

fn reply(value: Value) -> Result<()> {
    let mut output = std::io::stdout().lock();
    serde_json::to_writer(&mut output, &value)?;
    output.write_all(b"\n")?;
    output.flush()?;
    Ok(())
}

pub fn run() -> Result<()> {
    let _interrupt = windows::Interrupt::install()?;
    let windows = create()?;
    reply(
        json!({"reply": "ready", "ok": true, "process": std::process::id(), "handles": windows.0.iter().copied().map(windows::number).collect::<Vec<_>>() }),
    )?;
    let (sender, commands) = mpsc::channel();
    thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if sender.send(line).is_err() {
                break;
            }
        }
    });
    loop {
        windows::pump();
        if windows::interrupted() {
            break;
        }
        match commands.try_recv() {
            Ok(line) => {
                let command: Value = serde_json::from_str(&line)?;
                let operation = command["command"].as_str().ok_or("missing command")?;
                if operation == "stop" {
                    break;
                }
                let started = windows::tick();
                let mut setup_evidence = Value::Null;
                let result = match operation {
                    "setup" => setup(
                        &windows,
                        command["case"].as_str().ok_or("missing case")?,
                        &mut setup_evidence,
                    ),
                    "toggle" => place(
                        windows.0[0],
                        if command["topmost"].as_bool().ok_or("missing topmost")? {
                            HWND_TOPMOST
                        } else {
                            HWND_NOTOPMOST
                        },
                    ),
                    _ => Err("unknown command".into()),
                };
                reply(
                    json!({"reply": operation, "request": command["request"], "started_tick": started,
                    "finished_tick": windows::tick(), "ok": result.is_ok(), "error": result.err().map(|error| error.to_string()),
                    "setup": setup_evidence }),
                )?;
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        thread::sleep(Duration::from_millis(1));
    }
    Ok(())
}
