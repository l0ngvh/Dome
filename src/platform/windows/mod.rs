mod dome;
mod event_listener;
mod external;
mod font;
mod handle;
mod keyboard;
mod login_item;
mod process;
mod runner;
mod spawn;
mod taskbar;
mod throttle;
mod timer_registry;

#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::thread;

use crate::logging::Logger;
use anyhow::Result;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    BeginPaint, EndPaint, MONITOR_DEFAULTTONEAREST, MonitorFromWindow, PAINTSTRUCT,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    GetDpiAwarenessContextForProcess, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DefWindowProcW, DispatchMessageW, GetClientRect, GetMessageW, IDC_ARROW, LoadCursorW,
    MA_NOACTIVATE, MSG, PostThreadMessageW, RegisterClassW, TranslateMessage, WM_APP,
    WM_DPICHANGED, WM_ERASEBKGND, WM_MOUSEACTIVATE, WM_PAINT, WM_QUIT, WM_TIMER, WNDCLASSW,
};
use windows::core::BOOL;

use crate::config::{
    Config, LayoutConfig, layout_default_path, load_or_default, start_config_watcher,
};
use crate::ipc;
use crate::keymap::KeymapState;
use dome::overlay::{
    FLOAT_OVERLAY_CLASS, TAB_BAR_OVERLAY_CLASS, TILING_OVERLAY_CLASS, WgpuOverlayFactory,
    tab_bar_overlay_wnd_proc, tiling_overlay_wnd_proc,
};
use dome::picker::{PICKER_OVERLAY_CLASS, PickerWindow, picker_wnd_proc};
use dome::{Dome, HubEvent};
use event_listener::install_event_hooks;
use external::HwndId;

const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use taskbar::Taskbar;

/// Verifies the process is running at Per-Monitor V2 DPI awareness.
///
/// Tries to set PMv2 via `SetProcessDpiAwarenessContext`. On success, returns Ok.
/// On error (e.g. awareness already pinned by a manifest, compat shim, or prior call),
/// probes the current process awareness and accepts it if it is already PMv2.
/// Aborts with an error otherwise, because every downstream geometry and rendering
/// assumption requires PMv2. See BRD risk #6.
fn ensure_per_monitor_v2_awareness() -> anyhow::Result<()> {
    let result =
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if result.is_ok() {
        return Ok(());
    }
    let err = result.unwrap_err();

    // Probe-and-compare: if something else already set awareness to PMv2
    // (manifest, user compat-shim dialog, prior call), that is fine.
    // GetDpiAwarenessContextForProcess + AreDpiAwarenessContextsEqual require
    // Windows 10 1803+ (build 17134). On older builds this path is unreachable
    // because PMv2 itself requires 1703+, and the Set call would have succeeded
    // unless awareness was pinned -- which only happens via manifest/shim on 1803+.
    let current_ctx = unsafe { GetDpiAwarenessContextForProcess(GetCurrentProcess()) };
    let is_pmv2 = unsafe {
        AreDpiAwarenessContextsEqual(current_ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
    };
    if is_pmv2.as_bool() {
        tracing::info!(
            err = %err,
            "DPI awareness already PMv2 (likely manifest or compat shim); continuing"
        );
        return Ok(());
    }

    tracing::error!(
        err = %err,
        "Failed to set PMv2 DPI awareness; refusing to start because geometry would be wrong"
    );
    anyhow::bail!(
        "Process DPI awareness is not Per-Monitor V2. \
         Dome requires PMv2 for correct geometry. \
         Check compatibility settings or application manifest. Original error: {err}"
    );
}

pub(super) const WM_APP_HUBEVENT: u32 = WM_APP;
pub(super) const WM_APP_DISPLAY_CHANGE: u32 = WM_APP + 1;
pub(super) const WM_APP_DISPATCH_RESULT: u32 = WM_APP + 2;
/// Thread-message for live DPI changes. WPARAM = new DPI (u32 as usize),
/// LPARAM = HMONITOR handle (isize). Posted by every Dome-owned wnd-proc
/// on WM_DPICHANGED; decoded by the dome-thread message loop.
pub(super) const WM_APP_DPI_CHANGE: u32 = WM_APP + 3;
/// Not exported by the `windows` crate as of v0.62. Defined in WinUser.h.
/// Sent before WM_DPICHANGED; the handler writes the desired scaled window
/// size into the SIZE* at lparam and returns TRUE.
/// Remove this constant if the `windows` crate adds `WM_GETDPISCALEDSIZE`.
/// Revisit after next `windows` crate minor bump; target check: 2026-11.
pub(super) const WM_GETDPISCALEDSIZE: u32 = 0x02E4;

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

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> Result<()> {
    ensure_per_monitor_v2_awareness()?;

    // COM needed for shell APIs on main thread
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    let logger = Logger::init();

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = load_or_default(&config_path, Config::load);
    logger.set_level(config.log_level);
    tracing::info!(%config_path, "Loaded config");

    let layout_path = layout_path.unwrap_or_else(|| {
        layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });
    let layout = load_or_default(&layout_path, LayoutConfig::load);
    tracing::info!(path = %layout_path, "Loaded layout");

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
    let layout_clone = layout.clone();
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
            run_dome(config_clone, layout_clone, main_thread_id, keymap_clone);
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

    let _config_watcher = start_config_watcher(&config_path, Config::load, {
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

    let _layout_watcher = start_config_watcher(&layout_path, LayoutConfig::load, {
        let sender = hub_sender.clone();
        move |new_layout| {
            sender.send(HubEvent::LayoutConfigChanged(Box::new(new_layout)));
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup layout watcher: {e:#}"))
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

/// Returns the current window size unchanged. Called from every Dome-owned
/// wnd-proc's WM_GETDPISCALEDSIZE handler to suppress Windows 11's automatic
/// DPI resize. By reporting the current size as the "desired scaled size",
/// Windows' auto-resize becomes a no-op.
///
/// Dome's HWNDs are borderless WS_POPUP with no non-client area, so
/// GetClientRect == window size. Future window classes with a title bar or
/// border must NOT copy this pattern without adding the non-client delta.
pub(super) fn wm_getdpiscaledsize_reply(
    current: windows::Win32::Foundation::SIZE,
) -> windows::Win32::Foundation::SIZE {
    current
}

/// Universal prologue for every Dome-owned wnd-proc.
///
/// Returns `Some(LRESULT)` when the message was handled and the per-class
/// proc should return that value immediately. Returns `None` when the
/// per-class proc should continue processing.
///
/// Centralising these arms turns AGENTS.md's wnd-proc maintenance rule
/// (every Dome class must handle WM_DPICHANGED + WM_GETDPISCALEDSIZE) into a
/// structural invariant: any class whose proc calls this helper as its
/// prologue automatically satisfies the rule.
///
/// WM_DPICHANGED is per-window. Duplicate posts from multiple Dome wnd-procs
/// on the same monitor are absorbed by monitor_dpi_changed.
pub(super) fn dome_wnd_proc_common(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> Option<LRESULT> {
    match msg {
        WM_ERASEBKGND => Some(LRESULT(1)),
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xFFFF) as u32;
            let handle = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) }.0 as isize;
            unsafe {
                PostThreadMessageW(
                    GetCurrentThreadId(),
                    WM_APP_DPI_CHANGE,
                    WPARAM(dpi as usize),
                    LPARAM(handle),
                )
                .ok()
            };
            Some(LRESULT(0))
        }
        WM_GETDPISCALEDSIZE => {
            let mut rect = RECT::default();
            unsafe { GetClientRect(hwnd, &mut rect).ok() };
            let size = SIZE {
                cx: rect.right - rect.left,
                cy: rect.bottom - rect.top,
            };
            let out = lparam.0 as *mut SIZE;
            unsafe { *out = wm_getdpiscaledsize_reply(size) };
            Some(LRESULT(1))
        }
        _ => None,
    }
}

unsafe extern "system" fn float_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    if let Some(lr) = dome_wnd_proc_common(hwnd, msg, wparam, lparam) {
        return lr;
    }
    match msg {
        WM_MOUSEACTIVATE => LRESULT(MA_NOACTIVATE as isize),
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

fn run_dome(
    config: Config,
    layout: LayoutConfig,
    main_thread_id: u32,
    keymap_state: Arc<RwLock<KeymapState>>,
) {
    let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW failed");
    // https://devblogs.microsoft.com/oldnewthing/20250424-00/?p=111114
    let arrow = unsafe { LoadCursorW(None, IDC_ARROW) }.expect("LoadCursorW failed");

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

    let wc_tab_bar = WNDCLASSW {
        lpfnWndProc: Some(tab_bar_overlay_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: TAB_BAR_OVERLAY_CLASS,
        hCursor: arrow,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_tab_bar) };

    let wc_picker = WNDCLASSW {
        lpfnWndProc: Some(picker_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: PICKER_OVERLAY_CLASS,
        hCursor: arrow,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_picker) };

    // DX12 is the only backend we target. All other descriptor fields (flags, memory
    // budget thresholds, backend options, display) stay at their defaults. wgpu 29
    // dropped Default on InstanceDescriptor and now exposes explicit constructors
    // instead. new_without_display_handle is the right one for a headless overlay
    // that never presents to a winit display.
    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(instance_descriptor);
    let adapter = pollster::block_on(instance.request_adapter(
        // No power-preference hint (system picks the DX12 adapter), no compatible_surface
        // required before surface creation, force_fallback_adapter = false.
        &wgpu::RequestAdapterOptions::default(),
    ))
    .expect("No DX12 adapter");
    let (device, queue) = pollster::block_on(adapter.request_device(
        // No required features. Default (downlevel) limits are more than enough for
        // 2D egui rendering. No memory hints, no trace path.
        &wgpu::DeviceDescriptor::default(),
    ))
    .expect("Failed to create wgpu device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let taskbar = Taskbar::new().expect("Failed to create Taskbar");

    let hub_sender = HubSender {
        thread_id: unsafe { GetCurrentThreadId() },
    };

    let picker = PickerWindow::new(
        &instance,
        &adapter,
        Arc::clone(&device),
        Arc::clone(&queue),
        hub_sender.clone(),
        config.clone(),
    )
    .expect("Failed to create picker window");

    let overlays = WgpuOverlayFactory {
        instance,
        adapter,
        device,
        queue,
        hub_sender: hub_sender.clone(),
    };

    let dome = Dome::new(
        config.clone(),
        layout,
        Rc::new(taskbar),
        Box::new(overlays),
        Box::new(dome::Win32Display),
        picker,
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
                WM_APP_DPI_CHANGE => {
                    let dpi = msg.wParam.0 as u32;
                    let handle = msg.lParam.0;
                    runner.handle_dpi_change(handle, dpi);
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
