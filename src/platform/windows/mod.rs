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

use std::mem::size_of;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
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
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW, HWND_TOP,
    IDC_ARROW, LoadCursorW, MSG, PostThreadMessageW, RegisterClassW, SW_SHOWNA, SWP_NOACTIVATE,
    SWP_NOZORDER, SetForegroundWindow, SetWindowPos, ShowWindow, TranslateMessage, WM_APP,
    WM_DISPLAYCHANGE, WM_ERASEBKGND, WM_PAINT, WM_QUIT, WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW,
    WS_POPUP,
};
use windows::core::{BOOL, PCWSTR};

use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use crate::keymap::KeymapState;
use crate::picker::PickerEntry;
use dome::overlay::{FLOAT_OVERLAY_CLASS, TILING_OVERLAY_CLASS, tiling_overlay_wnd_proc};
use dome::picker::{PICKER_OVERLAY_CLASS, picker_wnd_proc};
use dome::{Dome, HubEvent, KeyboardSinkApi};
use event_listener::install_event_hooks;
use external::HwndId;

const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
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

pub(super) struct AppWindowSink {
    hwnd: HWND,
}

impl KeyboardSinkApi for AppWindowSink {
    fn focus(&self) {
        if unsafe { GetForegroundWindow() } == self.hwnd {
            return;
        }
        // Alt-tap unlocks the foreground lock as a safety net for edge cases
        // (user clicked away, then hotkeyed back).
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        ..Default::default() // dwFlags=0 means key-down
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default() // remaining KEYBDINPUT fields unused for VK_MENU
                    },
                },
            },
        ];
        unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if !unsafe { SetForegroundWindow(self.hwnd) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed for keyboard sink");
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

    let logger = Logger::init();

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load_or_default(&config_path);
    logger.set_level(config.log_level);
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
    let keymap_state = Arc::new(RwLock::new(KeymapState::new(config.keymaps.clone())));

    let config_clone = config.clone();
    let tid = Arc::clone(&dome_thread_id);
    let bar = Arc::clone(&barrier);
    let keymap_clone = Arc::clone(&keymap_state);
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
            run_dome(config_clone, main_thread_id, keymap_clone);
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

    let keyboard_hook = install_keyboard_hook(hub_sender.clone(), Arc::clone(&keymap_state))?;
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
        let keymap_state = Arc::clone(&keymap_state);
        move |cfg| {
            logger.set_level(cfg.log_level);
            keymap_state
                .write()
                .unwrap()
                .update_keymaps(cfg.keymaps.clone());
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

struct WgpuOverlayFactory {
    instance: wgpu::Instance,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    hub_sender: HubSender,
}

impl dome::CreateOverlay for WgpuOverlayFactory {
    fn create_tiling_overlay(
        &self,
        config: Config,
    ) -> anyhow::Result<Box<dyn dome::overlay::TilingOverlayApi>> {
        Ok(dome::overlay::TilingOverlay::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            config,
            self.hub_sender.clone(),
        )?)
    }
    fn create_float_overlay(
        &self,
        flavor: crate::theme::Flavor,
        font: &crate::font::FontConfig,
    ) -> anyhow::Result<Box<dyn dome::overlay::FloatOverlayApi>> {
        dome::overlay::create_float_overlay(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            flavor,
            font,
        )
    }
    fn create_picker(
        &self,
        entries: Vec<PickerEntry>,
        monitor_dim: Dimension,
        flavor: crate::theme::Flavor,
        font: &crate::font::FontConfig,
    ) -> anyhow::Result<Box<dyn dome::overlay::PickerApi>> {
        Ok(dome::picker::PickerWindow::new(
            &self.instance,
            Arc::clone(&self.device),
            Arc::clone(&self.queue),
            entries,
            monitor_dim,
            self.hub_sender.clone(),
            flavor,
            font,
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
        // App window is 1x1 offscreen; these arms are defensive only.
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

fn run_dome(config: Config, main_thread_id: u32, keymap_state: Arc<RwLock<KeymapState>>) {
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

    // The HWND serves as a keyboard sink (holds foreground when no managed window
    // is focused) and a WndProc host (handles WM_DISPLAYCHANGE).
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

    // Move offscreen so activating it is invisible. Same coordinate move_offscreen uses.
    unsafe {
        SetWindowPos(
            app_hwnd,
            Some(HWND_TOP),
            handle::OFFSCREEN_POS as i32,
            handle::OFFSCREEN_POS as i32,
            1,
            1,
            SWP_NOACTIVATE | SWP_NOZORDER,
        )
        .ok();
    }
    // Show without activating. Hidden windows are flaky SetForegroundWindow targets;
    // a 1x1 offscreen window makes activation reliable with no visible effect.
    unsafe { ShowWindow(app_hwnd, SW_SHOWNA).ok().ok() };

    let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
        backends: wgpu::Backends::DX12,
        // DX12 is the only backend we target; no instance flags, no dxc shader compiler
        // (wgsl compiled via wgpu's default path), no GLES minor version.
        ..Default::default()
    });
    let adapter = pollster::block_on(instance.request_adapter(
        // No power-preference hint (system picks the DX12 adapter), no compatible_surface
        // required before surface creation, force_fallback_adapter = false.
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("No DX12 adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        // No required features; default (downlevel) limits are more than enough for
        // 2D egui rendering; no memory hints, no trace path.
        &wgpu::DeviceDescriptor::default(),
    ))
    .expect("Failed to create wgpu device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let taskbar = Taskbar::new().expect("Failed to create Taskbar");

    let hub_sender = HubSender {
        thread_id: unsafe { GetCurrentThreadId() },
    };
    let overlays = WgpuOverlayFactory {
        instance,
        device,
        queue,
        hub_sender: hub_sender.clone(),
    };

    let mut dome = Dome::new(
        config.clone(),
        Rc::new(taskbar),
        Box::new(overlays),
        Box::new(display::Win32Display),
        Box::new(AppWindowSink { hwnd: app_hwnd }),
    )
    .expect("Failed to initialize Dome");

    let mut initial_hwnds = Vec::new();
    if let Err(e) = handle::enum_windows(|hwnd| {
        initial_hwnds.push(HwndId::from(hwnd));
    }) {
        tracing::warn!("Failed to enumerate windows: {e}");
    }

    let mut runner = runner::Runner::new(
        dome,
        unsafe { GetCurrentThreadId() },
        main_thread_id,
        keymap_state,
    );

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
