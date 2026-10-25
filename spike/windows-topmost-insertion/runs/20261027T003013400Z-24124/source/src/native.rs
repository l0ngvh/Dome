use crate::observation::{
    Neighbor, Observation, capture_local_neighbors, clean_topmost_prefix, local_above,
    validated_local, validated_local_with_ownership, validated_order,
    validated_order_with_ownership,
};
use crate::protocol::{CASES, owned_by, scheduled_cases};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
    os::windows::process::CommandExt,
    path::Path,
    process::{Child, ChildStdin, Command, Stdio},
    sync::mpsc::{self, Receiver},
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
use windows::{
    Win32::{
        Foundation::{GetLastError, HWND, LPARAM, LRESULT, SetLastError, WIN32_ERROR, WPARAM},
        System::{
            LibraryLoader::GetModuleHandleW, StationsAndDesktops::GetThreadDesktop,
            Threading::GetCurrentThreadId,
        },
        UI::{
            Input::Ime::ImmDisableIME,
            WindowsAndMessaging::{
                CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
                GWL_EXSTYLE, GetClassNameW, GetWindowLongPtrW, GetWindowThreadProcessId,
                HWND_BOTTOM, HWND_NOTOPMOST, HWND_TOPMOST, IsWindow, IsWindowVisible, MSG,
                PM_REMOVE, PeekMessageW, RegisterClassW, SET_WINDOW_POS_FLAGS, SWP_ASYNCWINDOWPOS,
                SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SetWindowPos, TranslateMessage,
                WINDOW_EX_STYLE, WNDCLASSW, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST,
                WS_OVERLAPPEDWINDOW, WS_POPUP, WS_VISIBLE,
            },
        },
    },
    core::{BOOL, w},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WAIT: Duration = Duration::from_secs(3);

#[link(name = "user32")]
unsafe extern "system" {
    #[link_name = "GetWindow"]
    fn raw_get_window(window: HWND, command: u32) -> HWND;
    #[link_name = "GetUserObjectInformationW"]
    fn raw_user_object_information(
        object: *mut std::ffi::c_void,
        index: i32,
        information: *mut std::ffi::c_void,
        length: u32,
        needed: *mut u32,
    ) -> BOOL;
}

pub(crate) fn desktop() -> Result<Value> {
    let desktop = unsafe { GetThreadDesktop(GetCurrentThreadId()) }?;
    let mut name = [0u16; 512];
    let mut needed = 0;
    let result = unsafe {
        raw_user_object_information(desktop.0, 2, name.as_mut_ptr().cast(), 1024, &mut needed)
    };
    if !result.as_bool() {
        return Err(windows::core::Error::from_thread().into());
    }
    let end = name
        .iter()
        .position(|value| *value == 0)
        .ok_or("desktop name exceeded buffer")?;
    Ok(json!({"handle": desktop.0 as usize, "name": String::from_utf16(&name[..end])?}))
}

fn number(window: HWND) -> i64 {
    window.0 as isize as i64
}

fn handle(value: i64) -> HWND {
    HWND(value as isize as *mut std::ffi::c_void)
}

fn timestamp() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock after Unix epoch")
        .as_micros()
}

fn emit(output: &mut impl Write, value: Value) -> Result<()> {
    let mut bytes = serde_json::to_vec(&json!({"unix_us": timestamp(), "record": value}))?;
    bytes.push(b'\n');
    output.write_all(&bytes)?;
    output.flush()?;
    Ok(())
}

fn reply(value: Value) -> Result<()> {
    emit(&mut std::io::stdout().lock(), value)
}

fn pump() {
    unsafe {
        let mut message = MSG::default();
        for _ in 0..256 {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                break;
            }
            let _translated_key = TranslateMessage(&message).as_bool();
            DispatchMessageW(&message);
        }
    }
}

unsafe extern "system" fn procedure(
    window: HWND,
    message: u32,
    word: WPARAM,
    long: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(window, message, word, long) }
}

struct OwnedWindow(HWND);
impl OwnedWindow {
    fn create(overlay: bool, owner: Option<HWND>) -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance.into(),
                lpszClassName: w!("DomeTopmostInsertionSpike"),
                ..Default::default()
            };
            SetLastError(WIN32_ERROR(0));
            if RegisterClassW(&class) == 0 && GetLastError().0 != 1410 {
                return Err(windows::core::Error::from_thread().into());
            }
            let window = CreateWindowExW(
                WS_EX_NOACTIVATE
                    | if overlay {
                        WS_EX_TOOLWINDOW
                    } else {
                        WINDOW_EX_STYLE(0)
                    },
                w!("DomeTopmostInsertionSpike"),
                if overlay {
                    w!("Insertion spike X")
                } else {
                    w!("Insertion spike source")
                },
                (if overlay {
                    WS_POPUP
                } else {
                    WS_OVERLAPPEDWINDOW
                }) | WS_VISIBLE,
                if overlay { 120 } else { 160 },
                160,
                300,
                180,
                owner,
                None,
                Some(instance.into()),
                None,
            )?;
            Ok(Self(window))
        }
    }
}
impl Drop for OwnedWindow {
    fn drop(&mut self) {
        if let Err(error) = unsafe { DestroyWindow(self.0) } {
            eprintln!("fixture DestroyWindow failed: {error}");
        }
    }
}

#[derive(Serialize)]
struct NativeCall {
    api: &'static str,
    window: i64,
    after: i64,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
    flags: u32,
    flags_names: &'static str,
    calling_process: u32,
    calling_thread: u32,
    started_us: u128,
    returned_us: u128,
    ok: bool,
    last_error_on_failure: Option<u32>,
    error: Option<String>,
}

fn place(window: HWND, after: HWND, asynchronous: bool) -> NativeCall {
    let flags = SWP_NOMOVE
        | SWP_NOSIZE
        | SWP_NOACTIVATE
        | if asynchronous {
            SWP_ASYNCWINDOWPOS
        } else {
            SET_WINDOW_POS_FLAGS(0)
        };
    let calling_process = std::process::id();
    let calling_thread = unsafe { GetCurrentThreadId() };
    let started_us = timestamp();
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let result = SetWindowPos(window, Some(after), 0, 0, 0, 0, flags);
        let last_error_on_failure = if result.is_err() {
            Some(GetLastError().0)
        } else {
            None
        };
        let returned_us = timestamp();
        NativeCall {
            api: "SetWindowPos",
            window: number(window),
            after: number(after),
            x: 0,
            y: 0,
            width: 0,
            height: 0,
            flags: flags.0,
            flags_names: if asynchronous {
                "NOMOVE|NOSIZE|NOACTIVATE|ASYNCWINDOWPOS"
            } else {
                "NOMOVE|NOSIZE|NOACTIVATE"
            },
            calling_process,
            calling_thread,
            started_us,
            returned_us,
            ok: result.is_ok(),
            last_error_on_failure,
            error: result.err().map(|error| error.to_string()),
        }
    }
}

fn observe(window: HWND) -> Observation {
    unsafe {
        let mut process = 0;
        let thread = GetWindowThreadProcessId(window, Some(&mut process));
        SetLastError(WIN32_ERROR(0));
        let extended_style = GetWindowLongPtrW(window, GWL_EXSTYLE);
        let error = GetLastError().0;
        Observation {
            handle: number(window),
            valid: IsWindow(Some(window)).as_bool(),
            process,
            thread,
            extended_style,
            style_read_error: (extended_style == 0 && error != 0).then_some(error),
            topmost: extended_style & WS_EX_TOPMOST.0 as isize != 0,
            visible: IsWindowVisible(window).as_bool(),
            above: number(raw_get_window(window, 3)),
            below: number(raw_get_window(window, 2)),
            owner: number(raw_get_window(window, 4)),
        }
    }
}

unsafe extern "system" fn enumerate(window: HWND, parameter: LPARAM) -> BOOL {
    let handles = unsafe { &mut *(parameter.0 as *mut Vec<HWND>) };
    handles.push(window);
    BOOL::from(handles.len() < 8192)
}

#[derive(Serialize)]
struct Snapshot {
    scope: &'static str,
    started_us: u128,
    finished_us: u128,
    #[serde(skip_serializing_if = "Option::is_none")]
    enumeration_ok: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    enumeration_error: Option<String>,
    invalid_reason: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    raw_enumeration: Option<Vec<Observation>>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    neighbors: Vec<Neighbor>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fixture_process_classes: Vec<Value>,
    fixtures: Vec<Observation>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fixture_rereads: Vec<Observation>,
    #[serde(skip)]
    global_order: Vec<Observation>,
}

impl Snapshot {
    fn above(&self, upper: i64, lower: i64) -> Option<bool> {
        if self.invalid_reason.is_some() {
            return None;
        }
        if self.scope == "local" {
            local_above(&self.neighbors, upper, lower)
        } else {
            Some(
                self.global_order
                    .iter()
                    .position(|item| item.handle == upper)?
                    < self
                        .global_order
                        .iter()
                        .position(|item| item.handle == lower)?,
            )
        }
    }
}

fn observe_neighbor(window: HWND) -> Neighbor {
    unsafe {
        let mut process = 0;
        let thread = GetWindowThreadProcessId(window, Some(&mut process));
        Neighbor {
            handle: number(window),
            valid: IsWindow(Some(window)).as_bool(),
            process,
            thread,
            above: number(raw_get_window(window, 3)),
            below: number(raw_get_window(window, 2)),
        }
    }
}

struct FixtureSet {
    handles: Vec<HWND>,
    ownership: Option<(i64, i64)>,
}

fn disable_input_method() -> Value {
    let ok = unsafe { ImmDisableIME(0) }.as_bool();
    json!({"api": "ImmDisableIME", "argument": 0, "ok": ok,
        "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() },
        "before_first_window": true, "unix_us": timestamp()})
}

fn snapshot(handles: &FixtureSet, global: bool, diagnostics: bool) -> Snapshot {
    let started_us = timestamp();
    let mut desktop_handles = Vec::<HWND>::new();
    let enumeration = (global || diagnostics).then(|| unsafe {
        EnumWindows(
            Some(enumerate),
            LPARAM(&mut desktop_handles as *mut _ as isize),
        )
    });
    let raw_enumeration: Vec<_> = if global {
        desktop_handles.iter().copied().map(observe).collect()
    } else {
        Vec::new()
    };
    let fixtures: Vec<_> = handles.handles.iter().copied().map(observe).collect();
    let neighbors = if global {
        Vec::new()
    } else {
        capture_local_neighbors(&fixtures, |value| observe_neighbor(handle(value)))
    };
    let fixture_rereads = if global {
        Vec::new()
    } else {
        handles.handles.iter().copied().map(observe).collect()
    };
    let fixture_processes: HashSet<_> = fixtures.iter().map(|item| item.process).collect();
    let mut fixture_process_classes = Vec::new();
    if diagnostics {
        for window in desktop_handles {
            let mut process = 0;
            let thread = unsafe { GetWindowThreadProcessId(window, Some(&mut process)) };
            if process == 0 || !fixture_processes.contains(&process) {
                continue;
            }
            let item = observe(window);
            if item.process != process || item.thread != thread {
                fixture_process_classes.push(
                    json!({"handle": number(window), "class_read_error": "identity changed"}),
                );
                continue;
            }
            let mut buffer = [0u16; 256];
            unsafe { SetLastError(WIN32_ERROR(0)) };
            let length = unsafe { GetClassNameW(window, &mut buffer) };
            let error = (length == 0).then(|| unsafe { GetLastError().0 });
            fixture_process_classes.push(json!({"window": item,
                "window_style": unsafe { GetWindowLongPtrW(window, windows::Win32::UI::WindowsAndMessaging::GWL_STYLE) },
                "class_name": String::from_utf16_lossy(&buffer[..length as usize]), "class_read_error": error}));
        }
    }
    let finished_us = timestamp();
    let enumeration_ok = enumeration.as_ref().map(|result| result.is_ok());
    let enumeration_error =
        enumeration.and_then(|result| result.err().map(|error| error.to_string()));
    let validated = if global {
        if let Some(ownership) = handles.ownership {
            validated_order_with_ownership(
                enumeration_ok == Some(true),
                started_us,
                finished_us,
                &raw_enumeration,
                &fixtures,
                Some(ownership),
            )
        } else {
            validated_order(
                enumeration_ok == Some(true),
                started_us,
                finished_us,
                &raw_enumeration,
                &fixtures,
            )
        }
    } else {
        (if let Some(ownership) = handles.ownership {
            validated_local_with_ownership(
                started_us,
                finished_us,
                &neighbors,
                &fixtures,
                &fixture_rereads,
                Some(ownership),
            )
        } else {
            validated_local(
                started_us,
                finished_us,
                &neighbors,
                &fixtures,
                &fixture_rereads,
            )
        })
        .map(|()| Vec::new())
    };
    let invalid_reason = validated.as_ref().err().copied();
    Snapshot {
        scope: if global { "global" } else { "local" },
        started_us,
        finished_us,
        enumeration_ok,
        enumeration_error,
        invalid_reason,
        raw_enumeration: global.then_some(raw_enumeration),
        neighbors,
        fixture_process_classes,
        fixtures,
        fixture_rereads,
        global_order: validated.unwrap_or_default(),
    }
}

fn global_premise(case: &str) -> bool {
    matches!(
        case,
        "normal_x_after_sole_global_topmost_a"
            | "normal_x_after_clean_topmost_prefix_a_with_b_above"
            | "topmost_a_after_topmost_predecessor_of_x"
    )
}

struct Fixture {
    child: Child,
    input: ChildStdin,
    replies: Receiver<Value>,
    window: HWND,
    thread: u32,
    held: bool,
}

impl Fixture {
    fn spawn(
        output: &mut File,
        label: &str,
        directory: &Path,
        lifetime_seconds: u64,
        desktop_name: &str,
    ) -> Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--fixture")
            .arg(lifetime_seconds.to_string())
            .arg(desktop_name)
            .creation_flags(0x08000000)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(
                OpenOptions::new()
                    .create_new(true)
                    .write(true)
                    .open(directory.join(format!("{label}-stderr.log")))?,
            ))
            .spawn()?;
        let input = child.stdin.take().expect("piped stdin");
        let stdout = child.stdout.take().expect("piped stdout");
        let (sender, replies) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let parsed = match line {
                    Ok(line) => serde_json::from_str(&line).unwrap_or_else(
                        |error| json!({"record": {"error": error.to_string(), "raw": line}}),
                    ),
                    Err(error) => json!({"record": {"error": error.to_string()}}),
                };
                if sender.send(parsed).is_err() {
                    break;
                }
            }
        });
        let mut fixture = Self {
            child,
            input,
            replies,
            window: HWND::default(),
            thread: 0,
            held: false,
        };
        emit(
            output,
            json!({"kind": "spawn", "label": label, "process": fixture.child.id()}),
        )?;
        let response = fixture.receive(output)?;
        if response["desktop"]["verified"] != true
            || response["desktop"]["requested"] != desktop_name
            || response["input_method"]["ok"] != true
        {
            return Err("fixture desktop or IME preflight failed".into());
        }
        fixture.window = handle(response["handle"].as_i64().ok_or("fixture has no handle")?);
        fixture.thread = response["thread"].as_u64().ok_or("fixture has no thread")? as u32;
        Ok(fixture)
    }

    fn receive(&self, output: &mut File) -> Result<Value> {
        let response = self.replies.recv_timeout(WAIT)?;
        emit(
            output,
            json!({"kind": "fixture_reply", "process": self.child.id(), "reply": response}),
        )?;
        Ok(response["record"].clone())
    }

    fn command(&mut self, output: &mut File, command: Value) -> Result<Value> {
        self.command_observed(output, command, None)
    }

    fn command_observed(
        &mut self,
        output: &mut File,
        command: Value,
        observation: Option<(&FixtureSet, &str)>,
    ) -> Result<Value> {
        emit(
            output,
            json!({"kind": "fixture_command", "process": self.child.id(), "command": command}),
        )?;
        let mut bytes = serde_json::to_vec(&command)?;
        bytes.push(b'\n');
        self.input.write_all(&bytes)?;
        self.input.flush()?;
        let response = self.replies.recv_timeout(WAIT)?;
        let captured = observation.map(|(handles, kind)| (kind, snapshot(handles, false, false)));
        emit(
            output,
            json!({"kind": "fixture_reply", "process": self.child.id(), "reply": response}),
        )?;
        if let Some((kind, captured)) = captured {
            emit(output, json!({"kind": kind, "snapshot": captured}))?;
        }
        Ok(response["record"].clone())
    }

    fn hold(&mut self, output: &mut File) -> Result<()> {
        self.held = true;
        let response = self.command(output, json!({"operation": "hold"}))?;
        if response["kind"] != "held" {
            return Err("pump hold was not acknowledged".into());
        }
        Ok(())
    }

    fn release(&mut self, output: &mut File) -> Result<()> {
        self.release_observed(output, None)
    }

    fn release_observed(
        &mut self,
        output: &mut File,
        observation: Option<(&FixtureSet, &str)>,
    ) -> Result<()> {
        if self.held {
            let response =
                self.command_observed(output, json!({"operation": "release"}), observation)?;
            if response["kind"] != "released_before_pump" {
                return Err("pump release was not acknowledged".into());
            }
            self.held = false;
        }
        Ok(())
    }
}

impl Fixture {
    fn stop(&mut self) -> Result<()> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        let sent =
            writeln!(self.input, "{{\"operation\":\"stop\"}}").and_then(|()| self.input.flush());
        if let Err(error) = sent {
            eprintln!("fixture stop write failed: {error}");
        }
        for _ in 0..100 {
            pump();
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        self.child.kill()?;
        for _ in 0..500 {
            if self.child.try_wait()?.is_some() {
                return Ok(());
            }
            thread::sleep(Duration::from_millis(10));
        }
        Err("fixture did not exit within bounded cleanup".into())
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Err(error) = self.stop() {
            eprintln!("fixture cleanup failed: {error}");
        }
    }
}

fn fixture(lifetime_seconds: u64, desktop_name: &str) -> Result<()> {
    let connection = crate::desktop::Desktop::open(desktop_name)?;
    let desktop_record = crate::desktop::Desktop::attach(connection.handle_number(), desktop_name)?;
    connection.keep_until_process_exit();
    let input_method = disable_input_method();
    if input_method["ok"] != true {
        reply(
            json!({"kind": "preflight_failed", "desktop": desktop_record, "input_method": input_method}),
        )?;
        return Err("ImmDisableIME failed before window creation".into());
    }
    let window = OwnedWindow::create(false, None)?;
    let mut owned: Option<OwnedWindow> = None;
    reply(
        json!({"kind": "ready", "desktop": desktop_record, "input_method": input_method, "initial": observe(window.0), "handle": number(window.0), "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() }}),
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
    let lifetime = Instant::now();
    let mut held_since = None;
    while lifetime.elapsed() < Duration::from_secs(lifetime_seconds) {
        if held_since.is_none() {
            pump();
        }
        if held_since.is_some_and(|started: Instant| started.elapsed() >= WAIT) {
            reply(json!({"kind": "hold_timeout"}))?;
            break;
        }
        match commands.try_recv() {
            Ok(line) => {
                let command: Value = serde_json::from_str(&line)?;
                match command["operation"].as_str().ok_or("missing operation")? {
                    "stop" => break,
                    "place" => {
                        let call = place(
                            window.0,
                            handle(command["after"].as_i64().ok_or("missing after")?),
                            false,
                        );
                        let read_started_us = timestamp();
                        let immediate = observe(window.0);
                        let read_finished_us = timestamp();
                        reply(json!({"kind": "placed", "call": call,
                            "post_return_target_read": immediate, "read_started_us": read_started_us,
                            "read_finished_us": read_finished_us}))?;
                    }
                    "create_owned" => {
                        if owned.is_some() {
                            return Err("owned fixture already exists".into());
                        }
                        let child = OwnedWindow::create(true, Some(window.0))?;
                        reply(
                            json!({"kind": "owned_ready", "window": observe(child.0), "owner": number(window.0)}),
                        )?;
                        owned = Some(child);
                    }
                    "destroy_owned" => {
                        drop(owned.take());
                        reply(json!({"kind": "owned_destroyed"}))?;
                    }
                    "hold" => {
                        held_since = Some(Instant::now());
                        reply(json!({"kind": "held", "handle": number(window.0)}))?;
                    }
                    "release" => {
                        held_since = None;
                        reply(json!({"kind": "released_before_pump", "handle": number(window.0)}))?;
                    }
                    _ => return Err("unknown fixture operation".into()),
                }
            }
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => thread::sleep(Duration::from_millis(2)),
        }
    }
    drop(owned);
    drop(window);
    reply(json!({"kind": "fixture_exit"}))?;
    Ok(())
}

fn record_snapshot(output: &mut File, kind: &str, handles: &FixtureSet) -> Result<Snapshot> {
    let observation = snapshot(handles, kind == "boundary_anchor_scan", false);
    emit(output, json!({"kind": kind, "snapshot": observation}))?;
    Ok(observation)
}

fn local_place(
    output: &mut File,
    overlay: HWND,
    after: HWND,
    handles: &FixtureSet,
    kind: &str,
) -> Result<()> {
    let call = place(overlay, after, false);
    let immediate = snapshot(handles, false, false);
    let ok = call.ok;
    emit(output, json!({"kind": "local_overlay_call", "call": call}))?;
    emit(output, json!({"kind": kind, "snapshot": immediate}))?;
    if !ok {
        return Err("local placement failed".into());
    }
    Ok(())
}

fn source_place(
    output: &mut File,
    source: &mut Fixture,
    after: HWND,
    handles: &FixtureSet,
    kind: &str,
) -> Result<()> {
    let response = source.command_observed(
        output,
        json!({"operation": "place", "after": number(after)}),
        Some((handles, kind)),
    )?;
    if response["call"]["ok"] != true {
        return Err("source placement failed".into());
    }
    Ok(())
}

fn is_race(case: &str) -> bool {
    case.starts_with("held_c_")
}

fn is_transition(case: &str) -> bool {
    case.contains("async_a_")
}

fn initially_normal_a(case: &str) -> bool {
    is_race(case) || case.contains("a_promotion")
}

fn initially_topmost_overlay(case: &str) -> bool {
    matches!(
        case,
        "topmost_x_after_normal_a"
            | "already_topmost_x_after_topmost_a"
            | "demoted_topmost_x_then_after_topmost_a"
            | "owner_x_demotion_demotes_owned_o"
            | "owned_o_demotion_demotes_owner_x"
            | "topmost_x_to_bottom"
    )
}

fn sole_controlled(case: &str) -> bool {
    case.contains("sole_")
}

fn setup_reason(observation: &Snapshot, case: &str) -> Option<&'static str> {
    if global_premise(case) && observation.scope != "global" {
        return Some("global premise requires full desktop evidence");
    }
    if observation.invalid_reason.is_some() {
        return Some(
            observation
                .invalid_reason
                .unwrap_or("desktop enumeration incomplete or chain changed"),
        );
    }
    let fixtures = &observation.fixtures;
    if fixtures.iter().any(|item| {
        !item.valid
            || !item.visible
            || (item.owner != 0 && !(owned_by(case).is_some() && Some(item) == fixtures.last()))
            || item.style_read_error.is_some()
    }) {
        return Some("fixture invalid, hidden, owned or unreadable");
    }
    if fixtures[0].topmost == initially_normal_a(case)
        || fixtures[1].topmost == sole_controlled(case)
        || fixtures[2].topmost != (case == "topmost_a_after_topmost_predecessor_of_x")
        || fixtures[3].topmost != initially_topmost_overlay(case)
        || (is_race(case) && fixtures[4].topmost)
        || (owned_by(case).is_some()
            && fixtures[4].topmost
                != if owned_by(case) == Some("A") {
                    fixtures[0].topmost
                } else {
                    fixtures[3].topmost
                })
    {
        return Some("actual initial bands differ from requested bands");
    }
    if is_race(case) {
        let chain = [&fixtures[0], &fixtures[3], &fixtures[2], &fixtures[4]];
        if chain
            .windows(2)
            .any(|pair| pair[0].below != pair[1].handle || pair[1].above != pair[0].handle)
        {
            return Some("race requires actual normal A-X-P-C chain");
        }
    }
    let position = |window| {
        observation
            .global_order
            .iter()
            .position(|item| item.handle == window)
            .expect("complete chain contains fixtures")
    };
    if !initially_normal_a(case)
        && !sole_controlled(case)
        && case != "normal_x_after_clean_topmost_prefix_a_with_b_above"
        && observation.above(fixtures[0].handle, fixtures[1].handle) != Some(true)
    {
        return Some("controlled B is not below A");
    }
    if case == "normal_x_after_sole_global_topmost_a"
        && observation.global_order.first().map(|item| item.handle) != Some(fixtures[0].handle)
    {
        return Some("sole global A is not chain head");
    }
    if case == "normal_x_after_sole_global_topmost_a"
        && observation
            .global_order
            .iter()
            .any(|item| item.topmost && item.handle != fixtures[0].handle)
    {
        return Some("another global topmost window exists, including hidden windows");
    }
    if case == "normal_x_after_clean_topmost_prefix_a_with_b_above" {
        let target = position(fixtures[0].handle);
        if !clean_topmost_prefix(&observation.global_order, fixtures[0].handle)
            || position(fixtures[1].handle) >= target
            || observation.global_order[target + 1..]
                .iter()
                .any(|item| item.topmost)
        {
            return Some("A is not the actual bottommost flagged target with B above");
        }
    }
    if case.contains("predecessor_of_x")
        && (fixtures[3].above != fixtures[2].handle || fixtures[2].below != fixtures[3].handle)
    {
        return Some("P is not the actual immediate predecessor of normal X");
    }
    if case == "topmost_a_after_topmost_predecessor_of_x" {
        let predecessor = position(fixtures[2].handle);
        if !clean_topmost_prefix(&observation.global_order, fixtures[2].handle)
            || observation.global_order[predecessor + 1..]
                .iter()
                .any(|item| item.topmost)
        {
            return Some("P is not the actual bottommost flagged predecessor");
        }
    }
    None
}

fn external_post(output: &mut File, window: HWND, after: HWND, handles: &FixtureSet) -> Result<()> {
    let call = place(window, after, true);
    let read_started_us = timestamp();
    let immediate_read = observe(window);
    let read_finished_us = timestamp();
    let immediate = snapshot(handles, false, false);
    let ok = call.ok;
    emit(
        output,
        json!({"kind": "external_async_call", "call": call,
        "post_return_target_read": immediate_read, "read_started_us": read_started_us, "read_finished_us": read_finished_us}),
    )?;
    emit(
        output,
        json!({"kind": "after_async_post_while_held", "snapshot": immediate}),
    )?;
    if !ok {
        return Err("external async placement failed".into());
    }
    Ok(())
}

fn wait_for_reference(output: &mut File, handles: &FixtureSet, topmost: bool) -> Result<()> {
    let start = Instant::now();
    let mut sample = 0;
    while start.elapsed() < Duration::from_secs(2) {
        pump();
        let observation = snapshot(handles, false, false);
        let observed =
            observation.invalid_reason.is_none() && observation.fixtures[0].topmost == topmost;
        emit(
            output,
            json!({"kind": if observed { "reference_transition_observed" } else { "waiting_for_reference" }, "sample": sample, "snapshot": observation}),
        )?;
        if observed {
            return Ok(());
        }
        sample += 1;
        thread::sleep(Duration::from_millis(25));
    }
    Err("reference transition was not observed within two seconds".into())
}

fn treatment(
    output: &mut File,
    window: HWND,
    after: HWND,
    source: Option<&mut Fixture>,
    handles: &FixtureSet,
    kind: &str,
) -> Result<()> {
    let call = if let Some(source) = source {
        let response = source.command_observed(
            output,
            json!({"operation": "place", "after": number(after)}),
            Some((handles, kind)),
        )?;
        response["call"].clone()
    } else {
        let call = place(window, after, false);
        let immediate = snapshot(handles, false, false);
        emit(output, json!({"kind": kind, "snapshot": immediate}))?;
        serde_json::to_value(call)?
    };
    let ok = call["ok"] == true;
    emit(output, json!({"kind": "treatment_call", "call": call}))?;
    if !ok {
        return Err("treatment placement failed".into());
    }
    Ok(())
}

fn settle(output: &mut File, handles: &FixtureSet, kind: &str) -> Result<()> {
    for sample in 0..21 {
        pump();
        if sample != 0 {
            thread::sleep(Duration::from_millis(25));
        }
        emit(
            output,
            json!({"kind": kind, "sample": sample, "snapshot": snapshot(handles, false, false)}),
        )?;
    }
    Ok(())
}

struct Trial<'a> {
    output: &'a mut File,
    a: &'a mut Fixture,
    b: &'a mut Fixture,
    normal: &'a mut Fixture,
    other: Option<&'a mut Fixture>,
    overlay: HWND,
    owned: Option<HWND>,
    case: &'a str,
}

impl Trial<'_> {
    fn collect(&mut self) -> Result<()> {
        let mut handles = vec![
            self.a.window,
            self.b.window,
            self.normal.window,
            self.overlay,
        ];
        if let Some(other) = self.other.as_ref() {
            handles.push(other.window);
        }
        if let Some(owned) = self.owned {
            handles.push(owned);
        }
        let handles = FixtureSet {
            handles,
            ownership: self.owned.map(|owned| {
                (
                    number(owned),
                    number(if owned_by(self.case) == Some("A") {
                        self.a.window
                    } else {
                        self.overlay
                    }),
                )
            }),
        };
        let output = &mut *self.output;
        let case = self.case;
        source_place(
            output,
            self.normal,
            HWND_NOTOPMOST,
            &handles,
            "setup_normal_reference",
        )?;
        source_place(
            output,
            self.b,
            if sole_controlled(case) {
                HWND_NOTOPMOST
            } else {
                HWND_TOPMOST
            },
            &handles,
            "setup_b",
        )?;
        source_place(
            output,
            self.a,
            if initially_normal_a(case) {
                HWND_NOTOPMOST
            } else {
                HWND_TOPMOST
            },
            &handles,
            "setup_a",
        )?;
        if let Some(other) = self.other.as_mut() {
            source_place(output, other, HWND_NOTOPMOST, &handles, "setup_c")?;
        }
        local_place(
            output,
            self.overlay,
            if initially_topmost_overlay(case) {
                HWND_TOPMOST
            } else {
                HWND_NOTOPMOST
            },
            &handles,
            "setup_x",
        )?;
        if case == "normal_x_after_clean_topmost_prefix_a_with_b_above" {
            source_place(
                output,
                self.a,
                self.b.window,
                &handles,
                "setup_clean_prefix_a",
            )?;
        }
        if case == "topmost_a_after_normal_predecessor_of_x" {
            local_place(
                output,
                self.overlay,
                self.normal.window,
                &handles,
                "setup_normal_predecessor",
            )?;
        }
        if case == "topmost_a_after_topmost_predecessor_of_x" {
            source_place(
                output,
                self.normal,
                HWND_TOPMOST,
                &handles,
                "setup_topmost_predecessor",
            )?;
            source_place(
                output,
                self.normal,
                self.b.window,
                &handles,
                "setup_clean_prefix_p",
            )?;
            local_place(
                output,
                self.overlay,
                HWND_TOPMOST,
                &handles,
                "setup_boundary_overlay_promote",
            )?;
            local_place(
                output,
                self.overlay,
                HWND_NOTOPMOST,
                &handles,
                "setup_boundary_overlay",
            )?;
        }
        if let Some(other) = self.other.as_mut() {
            local_place(
                output,
                self.overlay,
                self.a.window,
                &handles,
                "setup_race_x_after_a",
            )?;
            source_place(
                output,
                self.normal,
                self.overlay,
                &handles,
                "setup_race_p_after_x",
            )?;
            source_place(
                output,
                other,
                self.normal.window,
                &handles,
                "setup_race_c_after_p",
            )?;
        }
        pump();
        thread::sleep(Duration::from_millis(50));
        let before = snapshot(&handles, global_premise(case), true);
        emit(output, json!({"kind": "before", "snapshot": before}))?;
        let setup_error = setup_reason(&before, case);
        emit(
            output,
            json!({"kind": "setup_eligibility", "eligible": setup_error.is_none(), "reason": setup_error}),
        )?;
        if let Some(reason) = setup_error {
            return Err(reason.into());
        }
        if is_transition(case) {
            self.a.hold(output)?;
            record_snapshot(output, "held_before_post", &handles)?;
            external_post(
                output,
                self.a.window,
                if case.contains("demotion") {
                    HWND_NOTOPMOST
                } else {
                    HWND_TOPMOST
                },
                &handles,
            )?;
            if case.starts_with("wait_") {
                self.a
                    .release_observed(output, Some((&handles, "after_release")))?;
                wait_for_reference(output, &handles, case.contains("promotion"))?;
            }
        }
        if let Some(other) = self.other.as_mut() {
            other.hold(output)?;
            record_snapshot(output, "queried_normal_a_before_c_post", &handles)?;
            external_post(output, other.window, self.a.window, &handles)?;
            if case != "held_c_after_observed_normal_a_keep_a_normal" {
                source_place(
                    output,
                    self.a,
                    HWND_TOPMOST,
                    &handles,
                    "after_a_promotion_while_c_held",
                )?;
            }
            if case == "held_c_after_observed_normal_a_promote_then_demote_a" {
                source_place(
                    output,
                    self.a,
                    HWND_NOTOPMOST,
                    &handles,
                    "after_a_demotion_while_c_held",
                )?;
            }
            if case != "held_c_after_observed_normal_a_keep_a_normal" {
                wait_for_reference(
                    output,
                    &handles,
                    case == "held_c_after_observed_normal_a_then_promote_a",
                )?;
            }
            let pre_release = record_snapshot(output, "before_c_release", &handles)?;
            if pre_release.invalid_reason.is_some()
                || pre_release.above(number(self.normal.window), number(other.window)) != Some(true)
                || pre_release.fixtures[4].above == number(self.a.window)
            {
                return Err("C moved across stationary P before release".into());
            }
            other.release_observed(output, Some((&handles, "immediate_after_release")))?;
        } else {
            if matches!(
                case,
                "notopmost_x_then_after_topmost_a" | "demoted_topmost_x_then_after_topmost_a"
            ) {
                local_place(
                    output,
                    self.overlay,
                    HWND_NOTOPMOST,
                    &handles,
                    "after_explicit_notopmost",
                )?;
            }
            if matches!(
                case,
                "topmost_x_then_after_topmost_a" | "topmost_x_then_after_a_with_x_owned_o"
            ) {
                local_place(
                    output,
                    self.overlay,
                    HWND_TOPMOST,
                    &handles,
                    "after_explicit_topmost",
                )?;
            }
            let before_insert = snapshot(&handles, global_premise(case), false);
            emit(
                output,
                json!({"kind": "before_insert", "snapshot": before_insert}),
            )?;
            if let Some(reason) = before_insert.invalid_reason {
                return Err(reason.into());
            }
            if (sole_controlled(case)
                || case.contains("clean_topmost_prefix")
                || case.contains("predecessor_of_x"))
                && let Some(reason) = setup_reason(&before_insert, case)
            {
                return Err(reason.into());
            }
            let (target, reference) = if case.ends_with("_to_bottom") {
                (self.overlay, HWND_BOTTOM)
            } else if case == "owner_x_promotion_promotes_owned_o" {
                (self.overlay, HWND_TOPMOST)
            } else if case == "owner_x_demotion_demotes_owned_o" {
                (self.overlay, HWND_NOTOPMOST)
            } else if case == "owned_o_promotion_keeps_owner_x_normal" {
                (self.owned.expect("scheduled O"), HWND_TOPMOST)
            } else if case == "owned_o_demotion_demotes_owner_x" {
                (self.owned.expect("scheduled O"), HWND_NOTOPMOST)
            } else if matches!(
                case,
                "topmost_a_after_normal_x_below_overlay_control"
                    | "topmost_a_after_x_with_a_owned_o"
            ) {
                (self.a.window, self.overlay)
            } else if case.contains("predecessor_of_x") {
                (self.a.window, self.normal.window)
            } else if matches!(
                case,
                "topmost_x_after_normal_a" | "normal_x_after_normal_reference"
            ) {
                (self.overlay, self.normal.window)
            } else {
                (self.overlay, self.a.window)
            };
            treatment(
                output,
                target,
                reference,
                if target == self.a.window {
                    Some(self.a)
                } else {
                    None
                },
                &handles,
                "immediate_after_local_write",
            )?;
            if is_transition(case) && !case.starts_with("wait_") {
                thread::sleep(Duration::from_millis(100));
                record_snapshot(output, "held_after_100ms", &handles)?;
                self.a
                    .release_observed(output, Some((&handles, "after_release")))?;
            }
            if case == "repeat_x_after_topmost_a" {
                settle(output, &handles, "first_settling")?;
                record_snapshot(output, "before_repeated_insert", &handles)?;
                treatment(
                    output,
                    self.overlay,
                    self.a.window,
                    None,
                    &handles,
                    "immediate_after_repeated_write",
                )?;
            }
        }
        settle(output, &handles, "settling")?;
        if case.ends_with("_to_bottom") {
            emit(
                output,
                json!({"kind": "bottom_global_endpoint", "snapshot": snapshot(&handles, true, true)}),
            )?;
        }
        Ok(())
    }
}

fn collect_environment(
    output: &mut File,
    directory: &Path,
    repetitions: u32,
    environment: &str,
    desktop_name: &str,
    desktop_handle: usize,
) -> Result<()> {
    let connected = crate::desktop::Desktop::attach(desktop_handle, desktop_name)?;
    let input_method = disable_input_method();
    emit(
        output,
        json!({"kind": "environment_ready", "environment": environment,
        "desktop": connected, "input_method": input_method, "process": std::process::id(),
        "thread": unsafe { GetCurrentThreadId() }}),
    )?;
    if input_method["ok"] != true {
        return Err("collector ImmDisableIME failed".into());
    }
    let limit_seconds = u64::from(repetitions) * 240;
    let mut a = Fixture::spawn(output, "A", directory, limit_seconds + 30, desktop_name)?;
    let mut b = Fixture::spawn(output, "B", directory, limit_seconds + 30, desktop_name)?;
    let mut normal = Fixture::spawn(
        output,
        "normalA",
        directory,
        limit_seconds + 30,
        desktop_name,
    )?;
    let overlay = OwnedWindow::create(true, None)?;
    emit(
        output,
        json!({"kind": "overlay_identity", "environment": environment,
        "handle": number(overlay.0), "initial": observe(overlay.0),
        "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() }}),
    )?;
    let base = FixtureSet {
        handles: vec![a.window, b.window, normal.window, overlay.0],
        ownership: None,
    };
    emit(
        output,
        json!({"kind": "creation_inventory", "snapshot": snapshot(&base, false, true)}),
    )?;
    let beginning = Instant::now();
    for repetition in 0..repetitions {
        for case in scheduled_cases(environment) {
            emit(
                output,
                json!({"kind": "trial_begin", "environment": environment,
                "case": case, "repetition": repetition,
                "fixture_slots": if is_race(case) { vec!["A", "B", "P", "X", "C"] }
                    else if owned_by(case).is_some() { vec!["A", "B", "P", "X", "O"] }
                    else { vec!["A", "B", "P", "X"] }, "owned_by": owned_by(case)}),
            )?;
            let mut other = None;
            let mut local_owned = None;
            let result = (|| -> Result<()> {
                if beginning.elapsed() > Duration::from_secs(limit_seconds - 20) {
                    return Err("environment exceeded admission limit".into());
                }
                if is_race(case) {
                    other = Some(Fixture::spawn(
                        output,
                        &format!("C-{repetition}-{case}"),
                        directory,
                        30,
                        desktop_name,
                    )?);
                }
                let owned = match owned_by(case) {
                    Some("A") => {
                        let response = a.command(output, json!({"operation": "create_owned"}))?;
                        let window = handle(
                            response["window"]["handle"]
                                .as_i64()
                                .ok_or("O creation missing handle")?,
                        );
                        emit(
                            output,
                            json!({"kind": "owned_identity", "window": response["window"], "owner_slot": "A"}),
                        )?;
                        Some(window)
                    }
                    Some("X") => {
                        let child = OwnedWindow::create(true, Some(overlay.0))?;
                        let window = child.0;
                        emit(
                            output,
                            json!({"kind": "owned_identity", "window": observe(window), "owner_slot": "X"}),
                        )?;
                        local_owned = Some(child);
                        Some(window)
                    }
                    None => None,
                    Some(_) => unreachable!("fixed ownership schedule"),
                };
                Trial {
                    output,
                    a: &mut a,
                    b: &mut b,
                    normal: &mut normal,
                    other: other.as_mut(),
                    overlay: overlay.0,
                    owned,
                    case,
                }
                .collect()
            })();
            let release_a = a.release(output);
            let release_other = other
                .as_mut()
                .map(|fixture| fixture.release(output))
                .transpose();
            if result.is_err() {
                let mut failure_handles = vec![a.window, b.window, normal.window, overlay.0];
                if let Some(other) = &other {
                    failure_handles.push(other.window);
                }
                let failure = FixtureSet {
                    handles: std::mem::take(&mut failure_handles),
                    ownership: None,
                };
                emit(
                    output,
                    json!({"kind": "failure_inventory", "snapshot": snapshot(&failure, false, true)}),
                )?;
            }
            let destroy_owned = if owned_by(case) == Some("A") {
                a.command(output, json!({"operation": "destroy_owned"}))
                    .map(|_| ())
            } else {
                Ok(())
            };
            drop(local_owned);
            let other_cleanup = other.as_mut().map(Fixture::stop).transpose();
            emit(
                output,
                json!({"kind": "trial_end", "environment": environment,
                "case": case, "repetition": repetition,
                "status": if result.is_ok() && release_a.is_ok() && release_other.is_ok() && destroy_owned.is_ok() && other_cleanup.is_ok() {
                    "collected_pending_independent_analysis" } else { "ineligible_or_failed" },
                "error": result.err().map(|error| error.to_string()),
                "release_a_error": release_a.as_ref().err().map(|error| error.to_string()),
                "release_c_error": release_other.as_ref().err().map(|error| error.to_string()),
                "owned_cleanup_error": destroy_owned.as_ref().err().map(|error| error.to_string()),
                "other_cleanup_error": other_cleanup.as_ref().err().map(|error| error.to_string())}),
            )?;
            drop(other);
            release_a?;
            release_other?;
            destroy_owned?;
            other_cleanup?;
        }
    }
    drop(overlay);
    let normal_cleanup = normal.stop();
    let b_cleanup = b.stop();
    let a_cleanup = a.stop();
    normal_cleanup?;
    b_cleanup?;
    a_cleanup?;
    drop(normal);
    drop(b);
    drop(a);
    Ok(())
}

fn collect(directory: &Path, repetitions: u32) -> Result<()> {
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("raw.jsonl"))?;
    let schedule: Vec<_> = ["ordinary", "private"]
        .into_iter()
        .flat_map(|environment| {
            (0..repetitions).flat_map(move |repetition| {
                scheduled_cases(environment).into_iter().map(move |case| {
            json!({"environment": environment, "case": case, "repetition": repetition})
        })
            })
        })
        .collect();
    emit(
        &mut output,
        json!({"kind": "session", "schema_version": 5, "protocol": "bounded-topmost-v5",
        "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() },
        "desktop": desktop()?, "repetitions": repetitions, "case_names": CASES.to_vec(), "schedule": schedule,
        "scenario_count": 29, "environment_case_count": 33,
        "sampling": "21 scheduled samples, final five must be valid and consistent, no replacements",
        "HWND_TOPMOST": -1, "HWND_NOTOPMOST": -2, "HWND_BOTTOM": 1}),
    )?;
    for environment in ["ordinary", "private"] {
        let environment_directory = directory.join(environment);
        std::fs::create_dir(&environment_directory)?;
        let desktop_result = if environment == "private" {
            crate::desktop::Desktop::create(&format!(
                "DomeTopmost-{}-{}",
                std::process::id(),
                timestamp()
            ))
        } else {
            crate::desktop::Desktop::current()
        };
        let outcome = match desktop_result {
            Ok(owned_desktop) => {
                let name = owned_desktop.name().to_owned();
                let raw_handle = owned_desktop.handle_number();
                let mut worker_output = output.try_clone()?;
                let worker = thread::spawn(move || {
                    collect_environment(
                        &mut worker_output,
                        &environment_directory,
                        repetitions,
                        environment,
                        &name,
                        raw_handle,
                    )
                    .map_err(|error| error.to_string())
                });
                let result = worker
                    .join()
                    .unwrap_or_else(|_| Err("environment thread panicked".to_owned()));
                let closed = if result.is_ok() {
                    owned_desktop.close().map_err(|error| error.to_string())
                } else {
                    owned_desktop.keep_until_process_exit();
                    Err(
                        "desktop close deferred to process exit after environment failure"
                            .to_owned(),
                    )
                };
                emit(
                    &mut output,
                    json!({"kind": "environment_cleanup", "environment": environment,
                    "desktop_closed": environment == "private" && closed.is_ok(),
                    "borrowed_original_retained": environment == "ordinary", "error": closed.as_ref().err()}),
                )?;
                result.and(closed)
            }
            Err(error) => Err(error.to_string()),
        };
        emit(
            &mut output,
            json!({"kind": "environment_end", "environment": environment,
            "ok": outcome.is_ok(), "error": outcome.as_ref().err()}),
        )?;
        if let Err(reason) = outcome {
            let mut attempted = HashSet::new();
            for line in BufReader::new(File::open(directory.join("raw.jsonl"))?).lines() {
                let value: Value = serde_json::from_str(&line?)?;
                let record = &value["record"];
                if record["kind"] == "trial_begin" && record["environment"] == environment {
                    attempted.insert((
                        record["case"].as_str().unwrap_or_default().to_owned(),
                        record["repetition"].as_u64().unwrap_or_default(),
                    ));
                }
            }
            for repetition in 0..repetitions {
                for case in scheduled_cases(environment) {
                    if !attempted.contains(&(case.to_owned(), u64::from(repetition))) {
                        emit(
                            &mut output,
                            json!({"kind": "trial_not_run", "environment": environment,
                            "case": case, "repetition": repetition, "reason": reason}),
                        )?;
                    }
                }
            }
        }
    }
    emit(&mut output, json!({"kind": "collection_complete"}))?;
    emit(&mut output, json!({"kind": "cleanup_complete"}))?;
    Ok(())
}

pub fn run() -> Result<()> {
    let arguments: Vec<_> = std::env::args_os().collect();
    match arguments.get(1).and_then(|argument| argument.to_str()) {
        Some("--fixture") if arguments.len() == 4 => fixture(
            arguments[2].to_str().ok_or("invalid lifetime")?.parse()?,
            arguments[3].to_str().ok_or("invalid desktop")?,
        ),
        Some("--collect") if arguments.len() == 4 => {
            let repetitions = arguments[3]
                .to_str()
                .ok_or("invalid repetitions")?
                .parse()?;
            if !(1..=20).contains(&repetitions) {
                return Err("repetitions must be between 1 and 20".into());
            }
            // Collection must wait for admission after job assignment.
            let mut start = String::new();
            std::io::stdin().read_line(&mut start)?;
            if start.trim() != "start" {
                return Err("collector was not admitted by the launcher".into());
            }
            collect(Path::new(&arguments[2]), repetitions)
        }
        _ => crate::run_capture::run(&arguments[1..]),
    }
}
