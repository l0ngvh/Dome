mod app;
mod dome;
mod event_listener;
mod keyboard;
mod recovery;
mod throttle;
mod window;

use std::sync::mpsc;
use std::thread;

use anyhow::Result;
use std::mem::size_of;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::MONITORINFOF_PRIMARY;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_QUIT,
};
use windows::core::BOOL;

use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use app::App;
use dome::{Dome, HubEvent};
use event_listener::install_event_hooks;
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};

#[derive(Clone)]
pub(super) struct ScreenInfo {
    pub handle: isize,
    pub name: String,
    pub dimension: Dimension,
    pub is_primary: bool,
}

pub fn run_app(config_path: Option<String>) -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).ok() };

    // COM needed for Direct2D rendering on main thread
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    init_tracing(&config);
    recovery::install_handlers();

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    let screens = get_all_screens()?;
    anyhow::ensure!(!screens.is_empty(), "No monitors detected");
    let global_bounds = compute_global_bounds(&screens);

    let (tx, rx) = mpsc::channel();
    let sender = tx.clone();
    let config_clone = config.clone();
    let main_thread_id = unsafe { GetCurrentThreadId() };
    let dome_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .expect("CoInitializeEx failed");
            Dome::new(config_clone, screens, global_bounds).run(rx);
        }));
        if result.is_err() {
            recovery::restore_all();
        }
        unsafe { PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    });

    let _app = App::new(sender.clone())?;

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
            keyboard::update_config(cfg.clone());
            tx.send(HubEvent::ConfigChanged(cfg)).ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    sender.send(HubEvent::Shutdown).ok();
    dome_thread.join().ok();
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

fn compute_global_bounds(screens: &[ScreenInfo]) -> Dimension {
    if screens.is_empty() {
        return Dimension::default();
    }
    let min_x = screens
        .iter()
        .map(|s| s.dimension.x)
        .fold(f32::MAX, f32::min);
    let min_y = screens
        .iter()
        .map(|s| s.dimension.y)
        .fold(f32::MAX, f32::min);
    let max_x = screens
        .iter()
        .map(|s| s.dimension.x + s.dimension.width)
        .fold(f32::MIN, f32::max);
    let max_y = screens
        .iter()
        .map(|s| s.dimension.y + s.dimension.height)
        .fold(f32::MIN, f32::max);
    Dimension {
        x: min_x,
        y: min_y,
        width: max_x - min_x,
        height: max_y - min_y,
    }
}

fn init_tracing(config: &Config) {
    let filter = config
        .log_level
        .as_ref()
        .and_then(|l| l.parse().ok())
        .unwrap_or_else(EnvFilter::from_default_env);
    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer())
        .with(ErrorLayer::default())
        .init();
}

// Unlike macOS, we are allowed to move windows completely offscreen on Windows
pub(super) const OFFSCREEN_POS: f32 = -32000.0;
