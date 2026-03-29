mod dome;
mod event_listener;
pub(super) mod external;
mod handle;
mod keyboard;
mod taskbar;
mod wm;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::thread;
use std::time::Duration;

use crate::logging::Logger;
use anyhow::Result;
use calloop::RegistrationToken;
use calloop::channel::{Channel, Event as ChannelEvent, channel};
use calloop::timer::{TimeoutAction, Timer};
use std::mem::size_of;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DispatchMessageW, GWLP_USERDATA, GetMessageW, GetWindowLongPtrW,
    MONITORINFOF_PRIMARY, MSG, PostThreadMessageW, TranslateMessage, WM_DISPLAYCHANGE, WM_PAINT,
    WM_QUIT,
};
use windows::core::BOOL;

use crate::action::{Action, Actions};
use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use dome::throttle::{Throttle, ThrottleResult};
use dome::{Dome, HubEvent, LayoutFrame, TitleUpdate};
use event_listener::install_event_hooks;
use external::HwndId;
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use wm::Wm;

#[derive(Clone)]
pub(super) struct ScreenInfo {
    pub handle: isize,
    pub name: String,
    pub dimension: Dimension,
    pub is_primary: bool,
}

pub(super) const WM_APP_LAYOUT: u32 = 0x8001;
pub(super) const WM_APP_CONFIG: u32 = 0x8002;
pub(super) const WM_APP_TITLE: u32 = 0x8003;

pub fn run_app(config_path: Option<String>) -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).ok() };

    // COM needed for shell APIs on main thread
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    let logger = Logger::init(&config);

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    let screens = get_all_screens()?;
    anyhow::ensure!(!screens.is_empty(), "No monitors detected");

    let (tx, rx) = channel();
    let sender = tx.clone();
    let config_clone = config.clone();
    let main_thread_id = unsafe { GetCurrentThreadId() };
    let ui_thread_id = Arc::new(AtomicU32::new(0));

    // Spawn UI thread with its own message pump
    let ui_sender = tx.clone();
    let ui_config = config.clone();
    let ui_tid = Arc::clone(&ui_thread_id);
    let ui_thread = thread::spawn(move || {
        ui_tid.store(unsafe { GetCurrentThreadId() }, Ordering::Release);
        run_wm(ui_sender, ui_config);
    });

    let dome_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .expect("CoInitializeEx failed");
            let event_loop = calloop::EventLoop::try_new().expect("Failed to create event loop");
            let dome = Dome::new(config_clone, screens, None);
            run_dome(dome, rx, event_loop, main_thread_id);
        }));
        if result.is_err() {
            tracing::error!("Dome thread panicked");
        }
        unsafe { PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    });

    let keyboard_hook = install_keyboard_hook(sender.clone(), config)?;
    let _event_hooks = install_event_hooks(sender.clone())?;

    ipc::start_server({
        let tx = sender.clone();
        move |actions| {
            tx.send(HubEvent::Action(actions))
                .ok()
                .ok_or(anyhow::anyhow!("channel closed"))
        }
    })?;

    let _config_watcher = start_config_watcher(&config_path, {
        let tx = sender.clone();
        move |cfg| {
            logger.set_level(cfg.log_level);
            keyboard::update_config(cfg.clone());
            tx.send(HubEvent::ConfigChanged(cfg)).ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    // Main thread: bare message pump for hooks only
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    sender.send(HubEvent::Shutdown).ok();
    dome_thread.join().ok();
    let ui_tid = ui_thread_id.load(Ordering::Acquire);
    if ui_tid != 0 {
        unsafe { PostThreadMessageW(ui_tid, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    }
    ui_thread.join().ok();
    uninstall_keyboard_hook(keyboard_hook);

    Ok(())
}

pub(super) fn get_all_screens() -> Result<Vec<ScreenInfo>> {
    let mut monitors = Vec::new();

    unsafe extern "system" fn enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(lparam.0 as *mut Vec<ScreenInfo>) };
        let mut info = MONITORINFOEXW {
            monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        if unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) }.as_bool() {
            let rc = info.monitorInfo.rcWork;
            let name = String::from_utf16_lossy(
                &info
                    .szDevice
                    .iter()
                    .take_while(|&&c| c != 0)
                    .copied()
                    .collect::<Vec<_>>(),
            );

            monitors.push(ScreenInfo {
                handle: hmonitor.0 as isize,
                name,
                dimension: Dimension {
                    x: rc.left as f32,
                    y: rc.top as f32,
                    width: (rc.right - rc.left) as f32,
                    height: (rc.bottom - rc.top) as f32,
                },
                is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        BOOL(1)
    }

    let success = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        )
    };
    anyhow::ensure!(success.as_bool(), "EnumDisplayMonitors failed");
    Ok(monitors)
}

const FOCUS_THROTTLE_INTERVAL: Duration = Duration::from_millis(500);
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_TIMEOUT: Duration = Duration::from_secs(60);

/// Wraps Dome with all timing concerns: focus throttle, placement
/// debounce/timeout timers. Dome is a pure state machine; DomeRunner
/// owns the calloop handle and registers timers on its behalf.
struct DomeRunner {
    dome: Dome,
    /// Focus events are throttled to avoid excessive layout recalculations
    /// during rapid focus changes (e.g. alt-tab cycling).
    focus_throttle: Throttle<HwndId>,
    /// Active placement timers keyed by window. A window has at most one
    /// timer — either a drag safety timeout (60s) or a resize debounce
    /// (100ms), never both.
    timers: HashMap<HwndId, RegistrationToken>,
    handle: calloop::LoopHandle<'static, Self>,
    signal: calloop::LoopSignal,
    main_thread_id: u32,
}

fn run_dome(
    dome: Dome,
    channel: Channel<HubEvent>,
    mut event_loop: calloop::EventLoop<'static, DomeRunner>,
    main_thread_id: u32,
) {
    let handle = event_loop.handle();
    let signal = event_loop.get_signal();
    let mut runner = DomeRunner {
        dome,
        focus_throttle: Throttle::new(FOCUS_THROTTLE_INTERVAL),
        timers: HashMap::new(),
        handle: handle.clone(),
        signal,
        main_thread_id,
    };

    handle
        .insert_source(channel, |event, _, runner| match event {
            ChannelEvent::Msg(hub_event) => match hub_event {
                HubEvent::AppInitialized { app_hwnd, windows } => {
                    let on_open = runner.dome.app_initialized(app_hwnd, windows);
                    for actions in on_open {
                        runner.dome.run_hub_actions(&actions);
                        handle_system_actions(runner, &actions);
                    }
                }
                HubEvent::Shutdown => runner.signal.stop(),
                HubEvent::ConfigChanged(c) => runner.dome.config_changed(c),
                HubEvent::WindowCreated(ext) => {
                    if let Some(actions) = runner.dome.window_created(ext) {
                        runner.dome.run_hub_actions(&actions);
                        handle_system_actions(runner, &actions);
                    }
                }
                HubEvent::WindowDestroyed(ext) => runner.dome.window_destroyed(ext),
                HubEvent::WindowMinimized(ext) => runner.dome.window_minimized(ext),
                HubEvent::WindowFocused(ext) => {
                    let id = ext.id();
                    match runner.focus_throttle.submit(id) {
                        ThrottleResult::Send(id) => runner.dome.handle_focus(id),
                        ThrottleResult::Pending => {}
                        ThrottleResult::ScheduleFlush(delay) => {
                            runner.focus_throttle.mark_timer_scheduled();
                            // Fire-and-forget: throttle prevents duplicates,
                            // flush() is idempotent if already flushed.
                            let handle = runner.handle.clone();
                            handle
                                .insert_source(Timer::from_duration(delay), |_, _, runner| {
                                    if let Some(id) = runner.focus_throttle.flush() {
                                        runner.dome.handle_focus(id);
                                    }
                                    TimeoutAction::Drop
                                })
                                .expect("Failed to insert focus timer");
                        }
                    }
                }
                HubEvent::MoveSizeStart(ext) => {
                    let id = ext.id();
                    // Cancel existing timer (may be a debounce from LocationChanged)
                    if let Some(token) = runner.timers.remove(&id) {
                        runner.handle.remove(token);
                    }
                    runner.dome.move_size_started(ext);
                    let handle = runner.handle.clone();
                    let token = handle
                        .insert_source(
                            Timer::from_duration(DRAG_SAFETY_TIMEOUT),
                            move |_, _, runner| {
                                runner.timers.remove(&id);
                                runner.dome.placement_timeout(id);
                                TimeoutAction::Drop
                            },
                        )
                        .expect("Failed to insert drag timer");
                    runner.timers.insert(id, token);
                }
                HubEvent::MoveSizeEnd(ext) => {
                    let id = ext.id();
                    if let Some(token) = runner.timers.remove(&id) {
                        runner.handle.remove(token);
                    }
                    runner.dome.move_size_ended(ext);
                }
                HubEvent::LocationChanged(ext) => {
                    let id = ext.id();
                    if runner.dome.location_changed(ext) {
                        if let Some(token) = runner.timers.remove(&id) {
                            runner.handle.remove(token);
                        }
                        let handle = runner.handle.clone();
                        let token = handle
                            .insert_source(
                                Timer::from_duration(DEBOUNCE_INTERVAL),
                                move |_, _, runner| {
                                    runner.timers.remove(&id);
                                    runner.dome.placement_timeout(id);
                                    TimeoutAction::Drop
                                },
                            )
                            .expect("Failed to insert debounce timer");
                        runner.timers.insert(id, token);
                    }
                }
                HubEvent::WindowTitleChanged(ext) => {
                    if let Some(actions) = runner.dome.title_changed(ext) {
                        runner.dome.run_hub_actions(&actions);
                        handle_system_actions(runner, &actions);
                    }
                }
                HubEvent::ScreensChanged(s) => runner.dome.screens_changed(s),
                HubEvent::Action(a) => {
                    runner.dome.run_hub_actions(&a);
                    handle_system_actions(runner, &a);
                }
                HubEvent::TabClicked(id, idx) => runner.dome.tab_clicked(id, idx),
                HubEvent::SetFullscreen(id) => runner.dome.set_fullscreen(id),
            },
            ChannelEvent::Closed => runner.signal.stop(),
        })
        .expect("Failed to insert channel source");

    event_loop
        .run(None, &mut runner, |_| {})
        .expect("Event loop failed");
}

fn handle_system_actions(runner: &mut DomeRunner, actions: &Actions) {
    for action in actions {
        if let Action::Exec { command } = action {
            if let Err(e) = std::process::Command::new("cmd")
                .args(["/C", command])
                .spawn()
            {
                tracing::warn!(%command, "Failed to exec: {e}");
            }
        } else if let Action::Exit = action {
            unsafe {
                PostThreadMessageW(runner.main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok()
            };
            runner.signal.stop();
        }
    }
}

fn run_wm(hub_sender: calloop::channel::Sender<HubEvent>, config: Config) {
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
        .ok()
        .expect("COM init failed on UI thread");
    let _app = Wm::new(hub_sender, config, wm_wnd_proc, window_overlay_wnd_proc)
        .expect("Failed to create Wm");
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

unsafe extern "system" fn wm_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_APP_LAYOUT => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
            let frame = unsafe { *Box::from_raw(wparam.0 as *mut LayoutFrame) };
            unsafe { (*ptr).apply_layout_frame(frame) };
            LRESULT(0)
        }
        WM_APP_TITLE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
            let update = unsafe { *Box::from_raw(wparam.0 as *mut TitleUpdate) };
            unsafe { (*ptr).apply_title_update(update) };
            LRESULT(0)
        }
        WM_APP_CONFIG => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
            let config = unsafe { *Box::from_raw(wparam.0 as *mut Config) };
            unsafe { (*ptr).apply_config(config) };
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            let ptr = unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut Wm;
            if !ptr.is_null() {
                unsafe { (*ptr).handle_display_change() };
            }
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn window_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

// Unlike macOS, we are allowed to move windows completely offscreen on Windows
pub(super) const OFFSCREEN_POS: f32 = -32000.0;
