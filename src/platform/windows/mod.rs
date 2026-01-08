mod app;
mod config_watcher;
mod event_listener;
mod hub;
mod ipc;
mod keyboard;
mod window;

use anyhow::Result;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};
use windows::Win32::Foundation::{LPARAM, RECT};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFO,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, TranslateMessage,
};
use windows::core::BOOL;

use crate::config::Config;
use crate::core::Dimension;
use app::App;
use event_listener::{install_event_hook, uninstall_event_hook};
use hub::{HubEvent, HubThread, WindowHandle};
use ipc::start_server;
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use window::{Taskbar, enum_windows, is_manageable_window};

use config_watcher::start_config_watcher;

pub use ipc::send_action;

pub fn run_app(config_path: Option<String>) -> Result<()> {
    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    init_tracing(&config);

    let screen = get_primary_screen()?;
    let taskbar = Taskbar::new()?;

    let app = App::new(taskbar, screen, config.border_size)?;

    let hub_thread = HubThread::spawn(config, screen, WindowHandle(app.hwnd()));
    let sender = hub_thread.sender();

    let keyboard_hook = install_keyboard_hook(sender.clone())?;
    let event_hook = install_event_hook(sender.clone())?;

    start_server(sender.clone());
    start_config_watcher(config_path, sender.clone());

    if let Err(e) = enum_windows(|hwnd| {
        if is_manageable_window(hwnd) {
            sender
                .send(HubEvent::WindowCreated(WindowHandle(hwnd)))
                .ok();
        }
    }) {
        tracing::warn!("Failed to enumerate windows: {e}");
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    hub_thread.shutdown();
    uninstall_keyboard_hook(keyboard_hook);
    uninstall_event_hook(event_hook);

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
    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));
}
