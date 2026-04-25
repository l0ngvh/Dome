mod display;
mod dome;
mod event_listener;
mod external;
mod handle;
mod keyboard;
mod login_item;
mod runner;
mod taskbar;
mod throttle;

#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::Arc;
use std::thread;

use crate::logging::Logger;
use anyhow::Result;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{BeginPaint, EndPaint, PAINTSTRUCT};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, IDC_ARROW, LoadCursorW, MSG,
    PostThreadMessageW, RegisterClassW, TranslateMessage, WM_APP, WM_DISPLAYCHANGE, WM_ERASEBKGND,
    WM_PAINT, WM_QUIT, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, PCWSTR};

use crate::config::{Config, start_config_watcher};
use crate::core::{Dimension, WindowId};
use crate::ipc;
use dome::overlay::{
    FLOAT_OVERLAY_CLASS, TILING_OVERLAY_CLASS, raw_window_handle, tiling_overlay_wnd_proc,
};
use dome::picker::{PICKER_OVERLAY_CLASS, picker_wnd_proc};
use dome::{Dome, HubEvent};
use event_listener::install_event_hooks;
use external::HwndId;

const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
use glutin::display::{Display as GlDisplay, DisplayApiPreference};
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};
use taskbar::Taskbar;

#[derive(Clone)]
pub(super) struct ScreenInfo {
    pub handle: isize,
    pub name: String,
    pub dimension: Dimension,
    pub is_primary: bool,
}

pub(super) const WM_APP_HUBEVENT: u32 = WM_APP;
pub(super) const WM_APP_DISPLAY_CHANGE: u32 = WM_APP + 1;
pub(super) const WM_APP_DISPATCH_RESULT: u32 = WM_APP + 2;

static MAIN_THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[derive(Clone)]
struct HubSender {
    thread_id: u32,
}

impl HubSender {
    fn send(&self, event: HubEvent) {
        let ptr = Box::into_raw(Box::new(event)) as usize;
        unsafe {
            PostThreadMessageW(self.thread_id, WM_APP_HUBEVENT, WPARAM(ptr), LPARAM(0)).ok();
        }
    }
}

/// Handles Ctrl+C, Ctrl+Break, and console close by posting WM_QUIT to the main
/// thread, triggering the existing graceful shutdown path (Dome drop -> recovery).
/// Reinstated after accidental removal in commit efb409e.
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
            tracing::info!(ctrl_type, "Received console control event");
            let thread_id = MAIN_THREAD_ID.load(std::sync::atomic::Ordering::Relaxed);
            if thread_id != 0 {
                // Result ignored: the handler can't meaningfully recover from a failure,
                // and returning TRUE still prevents the default handler from killing the process.
                unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
            }
            // Windows terminates the process shortly after the handler returns for
            // CTRL_CLOSE_EVENT. Sleep to give the main thread time to shut down gracefully.
            if ctrl_type == CTRL_CLOSE_EVENT {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            BOOL(1)
        }
        _ => BOOL(0),
    }
}

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
    tracing::info!(%config_path, "Loaded config");

    login_item::sync_login_item(config.start_at_login);

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    let main_thread_id = unsafe { GetCurrentThreadId() };

    MAIN_THREAD_ID.store(main_thread_id, std::sync::atomic::Ordering::Release);
    if unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), true) }.is_err() {
        tracing::warn!("Failed to install console control handler");
    }

    let dome_thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let config_clone = config.clone();
    let tid = Arc::clone(&dome_thread_id);
    let bar = Arc::clone(&barrier);
    let dome_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .expect("CoInitializeEx failed");
            tid.store(
                unsafe { GetCurrentThreadId() },
                std::sync::atomic::Ordering::Release,
            );
            bar.wait();
            run_dome(config_clone, main_thread_id);
        }));
        if result.is_err() {
            tracing::error!("Dome thread panicked");
        }
        unsafe { PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    });

    barrier.wait();
    let hub_sender = HubSender {
        thread_id: dome_thread_id.load(std::sync::atomic::Ordering::Acquire),
    };

    let keyboard_hook = install_keyboard_hook(hub_sender.clone(), config)?;
    let _event_hooks = install_event_hooks(hub_sender.clone())?;

    ipc::start_server({
        let sender = hub_sender.clone();
        move |msg| {
            use crate::action::IpcMessage;
            match msg {
                IpcMessage::Action(action) => {
                    sender.send(HubEvent::Action(crate::action::Actions::new(vec![action])));
                    Ok("ok".to_string())
                }
                IpcMessage::Query(query) => {
                    let (resp_tx, resp_rx) = std::sync::mpsc::sync_channel(1);
                    sender.send(HubEvent::Query {
                        query,
                        sender: resp_tx,
                    });
                    match resp_rx.recv_timeout(QUERY_TIMEOUT) {
                        Ok(json) => Ok(json),
                        Err(_) => Ok(r#"{"error":"query timed out"}"#.to_string()),
                    }
                }
            }
        }
    })?;

    let _config_watcher = start_config_watcher(&config_path, {
        let sender = hub_sender.clone();
        move |cfg| {
            logger.set_level(cfg.log_level);
            keyboard::update_config(cfg.clone());
            let start_at_login = cfg.start_at_login;
            sender.send(HubEvent::ConfigChanged(Box::new(cfg)));
            login_item::sync_login_item(start_at_login);
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

    hub_sender.send(HubEvent::Shutdown);
    dome_thread.join().ok();
    uninstall_keyboard_hook(keyboard_hook);

    Ok(())
}

struct GlOverlayFactory {
    display: glutin::display::Display,
    hub_sender: HubSender,
}

impl dome::CreateOverlay for GlOverlayFactory {
    fn create_tiling_overlay(
        &self,
        config: Config,
    ) -> anyhow::Result<Box<dyn dome::overlay::TilingOverlayApi>> {
        Ok(dome::overlay::TilingOverlay::new(
            &self.display,
            config,
            self.hub_sender.clone(),
        )?)
    }
    fn create_float_overlay(&self) -> anyhow::Result<Box<dyn dome::overlay::FloatOverlayApi>> {
        dome::overlay::create_float_overlay(&self.display)
    }
    fn create_picker(
        &self,
        entries: Vec<(WindowId, String)>,
        monitor_dim: Dimension,
    ) -> anyhow::Result<Box<dyn dome::overlay::PickerApi>> {
        Ok(dome::picker::PickerWindow::new(
            &self.display,
            entries,
            monitor_dim,
            self.hub_sender.clone(),
        )?)
    }
}

unsafe extern "system" fn app_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DISPLAYCHANGE => {
            unsafe {
                PostThreadMessageW(
                    GetCurrentThreadId(),
                    WM_APP_DISPLAY_CHANGE,
                    WPARAM(0),
                    LPARAM(0),
                )
                .ok()
            };
            LRESULT(0)
        }
        // App window is never shown (WS_POPUP + WS_EX_TOOLWINDOW, no ShowWindow); these arms are defensive only.
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn float_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_ERASEBKGND => LRESULT(1),
        WM_PAINT => {
            let mut ps = PAINTSTRUCT::default();
            unsafe { BeginPaint(hwnd, &mut ps) };
            // EndPaint always succeeds; .ok().ok() silences the unused Result lint.
            unsafe { EndPaint(hwnd, &ps).ok().ok() };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn run_dome(config: Config, main_thread_id: u32) {
    let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW failed");
    // https://devblogs.microsoft.com/oldnewthing/20250424-00/?p=111114
    let arrow = unsafe { LoadCursorW(None, IDC_ARROW) }.expect("LoadCursorW failed");

    const APP_CLASS: PCWSTR = windows::core::w!("DomeApp");

    let wc = WNDCLASSW {
        lpfnWndProc: Some(app_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: APP_CLASS,
        hCursor: arrow,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    let wc_window = WNDCLASSW {
        lpfnWndProc: Some(float_overlay_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: FLOAT_OVERLAY_CLASS,
        hCursor: arrow,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_window) };

    let wc_tiling = WNDCLASSW {
        lpfnWndProc: Some(tiling_overlay_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: TILING_OVERLAY_CLASS,
        hCursor: arrow,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_tiling) };

    let wc_picker = WNDCLASSW {
        lpfnWndProc: Some(picker_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: PICKER_OVERLAY_CLASS,
        hCursor: arrow,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_picker) };

    let app_hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            APP_CLASS,
            windows::core::w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
    }
    .expect("Failed to create app window");

    let raw_display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
    let raw_window = raw_window_handle(app_hwnd);
    let display =
        unsafe { GlDisplay::new(raw_display, DisplayApiPreference::Wgl(Some(raw_window))) }
            .expect("Failed to create GL display");

    let taskbar = Taskbar::new().expect("Failed to create Taskbar");

    let hub_sender = HubSender {
        thread_id: unsafe { GetCurrentThreadId() },
    };
    let overlays = GlOverlayFactory {
        display: display.clone(),
        hub_sender: hub_sender.clone(),
    };

    let mut dome = Dome::new(
        config.clone(),
        Rc::new(taskbar),
        Box::new(overlays),
        Box::new(display::Win32Display),
    )
    .expect("Failed to initialize Dome");

    let mut initial_hwnds = Vec::new();
    if let Err(e) = handle::enum_windows(|hwnd| {
        initial_hwnds.push(HwndId::from(hwnd));
    }) {
        tracing::warn!("Failed to enumerate windows: {e}");
    }

    let mut runner = runner::Runner::new(dome, unsafe { GetCurrentThreadId() }, main_thread_id);

    for hwnd_id in initial_hwnds {
        runner.dispatch_window_created(hwnd_id);
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            match msg.message {
                WM_APP_HUBEVENT => {
                    let event = *Box::from_raw(msg.wParam.0 as *mut HubEvent);
                    runner.handle_event(event);
                }
                WM_APP_DISPLAY_CHANGE => {
                    runner.handle_display_change();
                }
                WM_APP_DISPATCH_RESULT => {
                    let apply = *Box::from_raw(msg.wParam.0 as *mut runner::ApplyFn);
                    apply(&mut runner);
                }
                WM_TIMER => {
                    runner.handle_timer(msg.wParam.0);
                }
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}
