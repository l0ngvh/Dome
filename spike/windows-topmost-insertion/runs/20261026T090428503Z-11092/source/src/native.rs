use crate::observation::{
    Neighbor, Observation, capture_local_neighbors, local_above, validated_local, validated_order,
};
use serde::Serialize;
use serde_json::{Value, json};
use std::{
    collections::HashSet,
    fs::{File, OpenOptions},
    io::{BufRead, BufReader, Write},
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
        UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, EnumWindows,
            GWL_EXSTYLE, GetClassNameW, GetWindowLongPtrW, GetWindowThreadProcessId,
            HWND_NOTOPMOST, HWND_TOPMOST, IsWindow, IsWindowVisible, MSG, PM_REMOVE, PeekMessageW,
            RegisterClassW, SET_WINDOW_POS_FLAGS, SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE,
            SWP_NOSIZE, SetWindowPos, TranslateMessage, WINDOW_EX_STYLE, WNDCLASSW,
            WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TOPMOST, WS_OVERLAPPEDWINDOW, WS_POPUP,
            WS_VISIBLE,
        },
    },
    core::{BOOL, w},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const WAIT: Duration = Duration::from_secs(3);
const CASES: [&str; 21] = [
    "normal_x_after_topmost_a",
    "notopmost_x_then_after_topmost_a",
    "topmost_x_then_after_topmost_a",
    "topmost_x_after_normal_a",
    "held_async_a_demotion_then_local_x_after_a",
    "normal_x_after_sole_global_topmost_a",
    "normal_x_after_sole_controlled_topmost_a",
    "normal_x_after_bottommost_topmost_a_with_b_above",
    "already_topmost_x_after_topmost_a",
    "demoted_topmost_x_then_after_topmost_a",
    "normal_x_after_normal_reference",
    "repeat_x_after_topmost_a",
    "wait_for_async_a_demotion_then_x_after_a",
    "held_async_a_promotion_then_local_x_after_a",
    "wait_for_async_a_promotion_then_x_after_a",
    "held_c_after_observed_normal_a_then_promote_a",
    "held_c_after_observed_normal_a_keep_a_normal",
    "held_c_after_observed_normal_a_promote_then_demote_a",
    "topmost_a_after_normal_x_below_overlay_control",
    "topmost_a_after_normal_predecessor_of_x",
    "topmost_a_after_topmost_predecessor_of_x",
];

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
    fn create(overlay: bool) -> Result<Self> {
        unsafe {
            let instance = GetModuleHandleW(None)?;
            let class = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance.into(),
                lpszClassName: w!("DomeTopmostInsertionSpike"),
                ..Default::default()
            };
            SetLastError(WIN32_ERROR(0));
            if RegisterClassW(&class) == 0 {
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
                None,
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

fn snapshot(handles: &[HWND], global: bool, diagnostics: bool) -> Snapshot {
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
    let fixtures: Vec<_> = handles.iter().copied().map(observe).collect();
    let neighbors = if global {
        Vec::new()
    } else {
        capture_local_neighbors(&fixtures, |value| observe_neighbor(handle(value)))
    };
    let fixture_rereads = if global {
        Vec::new()
    } else {
        handles.iter().copied().map(observe).collect()
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
                "class_name": String::from_utf16_lossy(&buffer[..length as usize]), "class_read_error": error}));
        }
    }
    let finished_us = timestamp();
    let enumeration_ok = enumeration.as_ref().map(|result| result.is_ok());
    let enumeration_error =
        enumeration.and_then(|result| result.err().map(|error| error.to_string()));
    let validated = if global {
        validated_order(
            enumeration_ok == Some(true),
            started_us,
            finished_us,
            &raw_enumeration,
            &fixtures,
        )
    } else {
        validated_local(
            started_us,
            finished_us,
            &neighbors,
            &fixtures,
            &fixture_rereads,
        )
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
            | "normal_x_after_bottommost_topmost_a_with_b_above"
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
    ) -> Result<Self> {
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--fixture")
            .arg(lifetime_seconds.to_string())
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
        observation: Option<(&[HWND], &str)>,
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
        observation: Option<(&[HWND], &str)>,
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

impl Drop for Fixture {
    fn drop(&mut self) {
        let sent =
            writeln!(self.input, "{{\"operation\":\"stop\"}}").and_then(|()| self.input.flush());
        if let Err(error) = sent {
            eprintln!("fixture stop write failed: {error}");
        }
        for _ in 0..100 {
            pump();
            match self.child.try_wait() {
                Ok(Some(_)) => return,
                Ok(None) => thread::sleep(Duration::from_millis(10)),
                Err(error) => {
                    eprintln!("fixture wait failed: {error}");
                    break;
                }
            }
        }
        if let Err(error) = self.child.kill() {
            eprintln!("fixture kill failed: {error}");
        }
        if let Err(error) = self.child.wait() {
            eprintln!("fixture reap failed: {error}");
        }
    }
}

fn fixture(lifetime_seconds: u64) -> Result<()> {
    let window = OwnedWindow::create(false)?;
    reply(
        json!({"kind": "ready", "handle": number(window.0), "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() }}),
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
    reply(json!({"kind": "fixture_exit"}))?;
    Ok(())
}

fn record_snapshot(output: &mut File, kind: &str, handles: &[HWND]) -> Result<Snapshot> {
    let observation = snapshot(handles, kind == "boundary_anchor_scan", false);
    emit(output, json!({"kind": kind, "snapshot": observation}))?;
    Ok(observation)
}

fn local_place(
    output: &mut File,
    overlay: HWND,
    after: HWND,
    handles: &[HWND],
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
    handles: &[HWND],
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
        !item.valid || !item.visible || item.owner != 0 || item.style_read_error.is_some()
    }) {
        return Some("fixture invalid, hidden, owned or unreadable");
    }
    if fixtures[0].topmost == initially_normal_a(case)
        || fixtures[1].topmost == sole_controlled(case)
        || fixtures[2].topmost != (case == "topmost_a_after_topmost_predecessor_of_x")
        || fixtures[3].topmost != initially_topmost_overlay(case)
        || (fixtures.len() == 5 && fixtures[4].topmost)
    {
        return Some("actual initial bands differ from requested bands");
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
        && case != "normal_x_after_bottommost_topmost_a_with_b_above"
        && observation.above(fixtures[0].handle, fixtures[1].handle) != Some(true)
    {
        return Some("controlled B is not below A");
    }
    if case == "normal_x_after_sole_global_topmost_a"
        && observation
            .global_order
            .iter()
            .any(|item| item.topmost && item.handle != fixtures[0].handle)
    {
        return Some("another global topmost window exists, including hidden windows");
    }
    if case == "normal_x_after_bottommost_topmost_a_with_b_above" {
        let target = position(fixtures[0].handle);
        if position(fixtures[1].handle) >= target
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
        if observation.global_order[predecessor + 1..]
            .iter()
            .any(|item| item.topmost)
        {
            return Some("P is not the actual bottommost flagged predecessor");
        }
    }
    None
}

fn place_at_bottommost_flagged(
    output: &mut File,
    source: &mut Fixture,
    handles: &[HWND],
) -> Result<()> {
    let observation = record_snapshot(output, "boundary_anchor_scan", handles)?;
    if observation.invalid_reason.is_some() {
        return Err(observation
            .invalid_reason
            .unwrap_or("cannot choose boundary anchor from incomplete desktop scan")
            .into());
    }
    let anchor = observation
        .global_order
        .iter()
        .rev()
        .find(|item| item.topmost && item.handle != number(source.window))
        .ok_or("no flagged predecessor available")?;
    source_place(
        output,
        source,
        handle(anchor.handle),
        handles,
        "after_boundary_setup",
    )?;
    Ok(())
}

fn external_post(output: &mut File, window: HWND, after: HWND, handles: &[HWND]) -> Result<()> {
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

fn wait_for_reference(output: &mut File, handles: &[HWND], topmost: bool) -> Result<()> {
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
    handles: &[HWND],
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

fn settle(output: &mut File, handles: &[HWND], kind: &str) -> Result<()> {
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
        if case == "normal_x_after_bottommost_topmost_a_with_b_above" {
            place_at_bottommost_flagged(output, self.a, &handles)?;
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
            place_at_bottommost_flagged(output, self.normal, &handles)?;
            local_place(
                output,
                self.overlay,
                HWND_NOTOPMOST,
                &handles,
                "setup_boundary_overlay",
            )?;
        }
        pump();
        thread::sleep(Duration::from_millis(50));
        let before = snapshot(
            &handles,
            global_premise(case),
            case.contains("predecessor_of_x"),
        );
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
            record_snapshot(output, "before_c_release", &handles)?;
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
            if case == "topmost_x_then_after_topmost_a" {
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
                || case.contains("bottommost")
                || case.contains("predecessor_of_x"))
                && let Some(reason) = setup_reason(&before_insert, case)
            {
                return Err(reason.into());
            }
            let (target, reference) = if case == "topmost_a_after_normal_x_below_overlay_control" {
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
        Ok(())
    }
}

fn collect(directory: &Path, repetitions: u32) -> Result<()> {
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(directory.join("raw.jsonl"))?;
    let limit_seconds = u64::from(repetitions) * 180;
    emit(
        &mut output,
        json!({"kind": "session", "schema_version": 4,
        "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() }, "desktop": desktop()?,
        "session_name": std::env::var("SESSIONNAME").ok(), "repetitions": repetitions,
        "case_names": CASES, "sampling": "21 post-write snapshots with requested 25ms sleep, actual spans and gaps recorded",
        "fixture_read_order": ["A", "B", "normalA_or_P", "X", "C_if_race"],
        "HWND_TOPMOST": -1, "HWND_NOTOPMOST": -2,
        "snapshot_caveat": "non-atomic scoped reads, bounded local neighbor paths, full desktop only for global premises, fixture-process classes at predecessor premises, no titles or executables"}),
    )?;
    let mut a = Fixture::spawn(&mut output, "A", directory, limit_seconds + 30)?;
    let mut b = Fixture::spawn(&mut output, "B", directory, limit_seconds + 30)?;
    let mut normal = Fixture::spawn(&mut output, "normalA", directory, limit_seconds + 30)?;
    let overlay = OwnedWindow::create(true)?;
    emit(
        &mut output,
        json!({"kind": "overlay_identity", "handle": number(overlay.0),
        "process": std::process::id(), "thread": unsafe { GetCurrentThreadId() }}),
    )?;
    let beginning = Instant::now();
    for repetition in 0..repetitions {
        for (index, case) in CASES.iter().enumerate() {
            if beginning.elapsed() > Duration::from_secs(limit_seconds - 20) {
                return Err("run exceeded admission limit".into());
            }
            emit(
                &mut output,
                json!({"kind": "trial_begin", "case": case, "repetition": repetition}),
            )?;
            let mut other = if is_race(case) {
                Some(Fixture::spawn(
                    &mut output,
                    &format!("C-{repetition}-{index}"),
                    directory,
                    30,
                )?)
            } else {
                None
            };
            let result = Trial {
                output: &mut output,
                a: &mut a,
                b: &mut b,
                normal: &mut normal,
                other: other.as_mut(),
                overlay: overlay.0,
                case,
            }
            .collect();
            let release_a = a.release(&mut output);
            let release_other = other
                .as_mut()
                .map(|fixture| fixture.release(&mut output))
                .transpose();
            emit(
                &mut output,
                json!({"kind": "trial_end", "case": case, "repetition": repetition,
                "status": if result.is_ok() && release_a.is_ok() && release_other.is_ok() { "collected_pending_independent_analysis" } else { "ineligible_or_failed" },
                "error": result.err().map(|error| error.to_string()),
                "release_a_error": release_a.as_ref().err().map(|error| error.to_string()),
                "release_c_error": release_other.as_ref().err().map(|error| error.to_string())}),
            )?;
            drop(other);
            release_a?;
            release_other?;
        }
    }
    emit(
        &mut output,
        json!({"kind": "collection_complete", "elapsed_ms": beginning.elapsed().as_millis()}),
    )?;
    drop(overlay);
    drop(normal);
    drop(b);
    drop(a);
    emit(&mut output, json!({"kind": "cleanup_complete"}))?;
    Ok(())
}

pub fn run() -> Result<()> {
    let arguments: Vec<_> = std::env::args_os().collect();
    match arguments.get(1).and_then(|argument| argument.to_str()) {
        Some("--fixture") if arguments.len() == 3 => {
            fixture(arguments[2].to_str().ok_or("invalid lifetime")?.parse()?)
        }
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
