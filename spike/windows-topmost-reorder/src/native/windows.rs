use super::Result;
use ::windows::{
    Win32::{
        Foundation::{GetLastError, HWND, LPARAM, SetLastError, WIN32_ERROR},
        System::{
            Console::{CTRL_BREAK_EVENT, CTRL_C_EVENT, SetConsoleCtrlHandler},
            SystemInformation::GetTickCount64,
        },
        UI::{
            Accessibility::{HWINEVENTHOOK, SetWinEventHook, UnhookWinEvent},
            WindowsAndMessaging::*,
        },
    },
    core::BOOL,
};
use serde_json::{Value, json};
use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};
use windows_topmost_reorder::{CaseBoundary, LocalBoundary, Neighbor, Reorder, expand_tick};

static INTERRUPTED: AtomicBool = AtomicBool::new(false);
thread_local! {
    static EVENTS: RefCell<Vec<Reorder>> = const { RefCell::new(Vec::new()) };
}

pub fn tick() -> u64 {
    unsafe { GetTickCount64() }
}

pub fn handle(value: usize) -> HWND {
    HWND(value as *mut _)
}

pub fn number(value: HWND) -> usize {
    value.0 as usize
}

pub fn interrupted() -> bool {
    INTERRUPTED.load(Ordering::Relaxed)
}

unsafe extern "system" fn interrupt(event: u32) -> BOOL {
    if event == CTRL_C_EVENT || event == CTRL_BREAK_EVENT {
        INTERRUPTED.store(true, Ordering::Relaxed);
        BOOL(1)
    } else {
        BOOL(0)
    }
}

pub struct Interrupt;
impl Interrupt {
    pub fn install() -> Result<Self> {
        unsafe { SetConsoleCtrlHandler(Some(interrupt), true)? };
        Ok(Self)
    }
}
impl Drop for Interrupt {
    fn drop(&mut self) {
        unsafe {
            let _ = SetConsoleCtrlHandler(Some(interrupt), false);
        }
    }
}

unsafe extern "system" fn callback(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    thread: u32,
    native_tick: u32,
) {
    let delivered_tick = tick();
    EVENTS.with(|events| {
        events.borrow_mut().push(Reorder {
            sequence: 0,
            native_tick,
            expanded_tick: expand_tick(native_tick, delivered_tick),
            delivered_tick,
            hwnd: number(hwnd),
            object,
            child,
            thread,
        })
    });
}

pub struct Hook(HWINEVENTHOOK);
impl Hook {
    pub fn install() -> Result<Self> {
        let hook = unsafe {
            SetWinEventHook(
                EVENT_OBJECT_REORDER,
                EVENT_OBJECT_REORDER,
                None,
                Some(callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            )
        };
        if hook.0.is_null() {
            return Err("SetWinEventHook returned null".into());
        }
        Ok(Self(hook))
    }
}
impl Drop for Hook {
    fn drop(&mut self) {
        unsafe {
            let _ = UnhookWinEvent(self.0);
        }
    }
}

pub fn pump() {
    unsafe {
        let mut message = MSG::default();
        for _ in 0..128 {
            if !PeekMessageW(&mut message, None, 0, 0, PM_REMOVE).as_bool() {
                break;
            }
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

pub fn drain_events() -> Vec<Reorder> {
    EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
}

pub fn identity(hwnd: usize) -> Option<(u32, u32)> {
    unsafe {
        if !IsWindow(Some(handle(hwnd))).as_bool() {
            return None;
        }
        let mut process = 0;
        let thread = GetWindowThreadProcessId(handle(hwnd), Some(&mut process));
        if thread == 0 {
            None
        } else {
            Some((process, thread))
        }
    }
}

pub fn topmost(hwnd: usize) -> Result<bool> {
    unsafe {
        SetLastError(WIN32_ERROR(0));
        let style = GetWindowLongPtrW(handle(hwnd), GWL_EXSTYLE);
        let error = GetLastError();
        if style == 0 && error.0 != 0 {
            return Err(format!("GetWindowLongPtrW error {}", error.0).into());
        }
        Ok(style as u32 & WS_EX_TOPMOST.0 != 0)
    }
}

pub fn adjacent(hwnd: usize, direction: GET_WINDOW_CMD) -> Option<usize> {
    read_adjacent(hwnd, direction).ok().flatten()
}

fn read_adjacent(hwnd: usize, direction: GET_WINDOW_CMD) -> Result<Option<usize>> {
    unsafe {
        SetLastError(WIN32_ERROR(0));
        match GetWindow(handle(hwnd), direction) {
            Ok(hwnd) => Ok(Some(number(hwnd))),
            Err(_) if GetLastError().0 == 0 => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

fn observe_neighbor(hwnd: Option<usize>) -> Result<Option<Neighbor>> {
    let Some(hwnd) = hwnd else { return Ok(None) };
    let before = identity(hwnd).ok_or("neighbor disappeared before flag read")?;
    let topmost = topmost(hwnd)?;
    if identity(hwnd) != Some(before) {
        return Err("neighbor identity changed during flag read".into());
    }
    Ok(Some(Neighbor {
        handle: hwnd,
        topmost: Some(topmost),
    }))
}

unsafe extern "system" fn enumerate(hwnd: HWND, parameter: LPARAM) -> BOOL {
    let handles = unsafe { &mut *(parameter.0 as *mut Vec<usize>) };
    handles.push(number(hwnd));
    BOOL(1)
}

pub struct Snapshot {
    pub value: Value,
    pub local: LocalBoundary,
    pub complete: bool,
    pub above: Vec<usize>,
    pub ordinary: Vec<usize>,
    pub ordinary_above: Vec<usize>,
    pub other_topmost: Vec<usize>,
}

impl Snapshot {
    pub fn boundary(&self) -> CaseBoundary<'_> {
        CaseBoundary {
            complete: self.complete,
            local: self.local,
            above: &self.above,
            ordinary: &self.ordinary,
            ordinary_above: &self.ordinary_above,
            other_topmost: &self.other_topmost,
        }
    }
}

pub fn snapshot(target: usize) -> Snapshot {
    let started_tick = tick();
    let mut handles = Vec::<usize>::new();
    let enumeration =
        unsafe { EnumWindows(Some(enumerate), LPARAM(&mut handles as *mut _ as isize)) };
    let mut complete = enumeration.is_ok() && handles.contains(&target);
    let mut rows = Vec::new();
    let mut ordinary = Vec::new();
    let mut other_topmost = Vec::new();
    let mut target_topmost = None;
    for hwnd in handles {
        let state = topmost(hwnd);
        if hwnd == target {
            target_topmost = state.as_ref().ok().copied();
        }
        if state.is_err() {
            complete = false;
        }
        if state.as_ref().is_ok_and(|state| !state) {
            ordinary.push(hwnd);
        }
        if state.as_ref().is_ok_and(|state| *state) && hwnd != target {
            other_topmost.push(hwnd);
        }
        let mut class = [0_u16; 256];
        let length = unsafe { GetClassNameW(handle(hwnd), &mut class) } as usize;
        rows.push(json!({
            "hwnd": hwnd, "identity": identity(hwnd),
            "topmost": state.as_ref().ok(), "style_error": state.as_ref().err().map(|error| error.to_string()),
            "visible": unsafe { IsWindowVisible(handle(hwnd)).as_bool() },
            "class": String::from_utf16_lossy(&class[..length]),
        }));
    }
    let mut above = Vec::new();
    let mut ordinary_above = Vec::new();
    let mut cursor = read_adjacent(target, GW_HWNDPREV);
    let mut adjacency_error = None;
    loop {
        let hwnd = match cursor {
            Ok(Some(hwnd)) => hwnd,
            Ok(None) => break,
            Err(error) => {
                complete = false;
                adjacency_error = Some(error.to_string());
                break;
            }
        };
        if above.contains(&hwnd) || above.len() >= 10_000 {
            complete = false;
            break;
        }
        above.push(hwnd);
        match topmost(hwnd) {
            Ok(false) => ordinary_above.push(hwnd),
            Ok(true) => {}
            Err(_) => complete = false,
        }
        cursor = read_adjacent(hwnd, GW_HWNDPREV);
    }
    let previous = read_adjacent(target, GW_HWNDPREV);
    let next = read_adjacent(target, GW_HWNDNEXT);
    if identity(target).is_none() || previous.is_err() || next.is_err() {
        complete = false;
    }
    let previous_band = observe_neighbor(previous.as_ref().ok().copied().flatten());
    let next_band = observe_neighbor(next.as_ref().ok().copied().flatten());
    let local = LocalBoundary {
        complete: complete && previous_band.is_ok() && next_band.is_ok(),
        target_topmost,
        previous: previous_band.as_ref().ok().copied().flatten(),
        next: next_band.as_ref().ok().copied().flatten(),
    };
    let value = json!({
        "local_boundary": local,
        "previous_band_error": previous_band.err().map(|error| error.to_string()),
        "next_band_error": next_band.err().map(|error| error.to_string()),
        "target": target, "target_topmost": target_topmost, "complete": complete,
        "started_tick": started_tick, "finished_tick": tick(),
        "enumeration_error": enumeration.err().map(|error| error.to_string()),
        "adjacency_error": adjacency_error,
        "previous_error": previous.as_ref().err().map(|error| error.to_string()),
        "next_error": next.as_ref().err().map(|error| error.to_string()),
        "previous": previous.ok().flatten(), "next": next.ok().flatten(),
        "above": above, "ordinary_above": ordinary_above, "ordinary": ordinary,
        "other_topmost": other_topmost, "windows": rows,
    });
    Snapshot {
        value,
        local,
        complete,
        above,
        ordinary,
        ordinary_above,
        other_topmost,
    }
}
