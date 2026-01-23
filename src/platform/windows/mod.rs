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
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};
use windows::Win32::Foundation::{LPARAM, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
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

    let screen = get_primary_screen()?;

    let (tx, rx) = mpsc::channel();
    let sender = tx.clone();
    let config_clone = config.clone();
    let main_thread_id = unsafe { GetCurrentThreadId() };
    let dome_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .expect("CoInitializeEx failed");
            Dome::new(config_clone, screen).run(rx);
        }));
        if result.is_err() {
            recovery::restore_all();
        }
        unsafe { PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    });

    let _app = App::new(screen, sender.clone())?;

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

fn get_primary_screen() -> Result<Dimension> {
    let mut result = Dimension {
        x: 0.0,
        y: 0.0,
        width: 1920.0,
        height: 1080.0,
    };

    unsafe extern "system" fn monitor_enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if unsafe { GetMonitorInfoW(hmonitor, &mut info) }.as_bool() && info.dwFlags & 1 != 0 {
            let result = unsafe { &mut *(lparam.0 as *mut Dimension) };
            let rc = info.rcWork;
            result.x = rc.left as f32;
            result.y = rc.top as f32;
            result.width = (rc.right - rc.left) as f32;
            result.height = (rc.bottom - rc.top) as f32;
        }
        BOOL(1)
    }

    let success = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(monitor_enum_proc),
            LPARAM(&mut result as *mut _ as isize),
        )
    };
    if !success.as_bool() {
        anyhow::bail!("EnumDisplayMonitors failed");
    }

    Ok(result)
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
