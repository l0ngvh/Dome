use crate::platform::keymap::{KeymapPublisher, KeymapView};
mod display;
pub(crate) mod dome;
mod event_listener;
mod external;
mod foreground;
mod handle;
mod keyboard;
mod login_item;
mod process;
pub(super) mod spawn;
mod taskbar;
mod thumbnail;
pub(crate) mod ui;
mod window_server;

use std::rc::Rc;
use std::sync::mpsc;
use std::thread;

use crate::logging::{FileLogger, Logger};
use crate::platform::messages::HubSender;
use anyhow::Result;

use crate::platform::windows::event_loop::native::CallbackPanic;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize};

use windows::Win32::System::Threading::GetCurrentProcess;
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    GetDpiAwarenessContextForProcess, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{MSG, PM_NOREMOVE, PeekMessageW};

use crate::config::watch::start_file_watcher;
use crate::config::{LuaRuntime, PreferredLayouts, bootstrap::resolve_config_path, paths};
use crate::ipc;
use crate::platform::windows::event_loop::EventSender;
use crate::platform::windows::event_loop::RunLoop;
use crate::platform::windows::event_loop::Shutdown;
use crate::platform::windows::tray::{Tray, load_icon};
use dome::Dome;
use dome::HubEvent;
use ui::NativeOverlays;
use window_server::NativeWindowServer;

use keyboard::KeyboardHookHandle;

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> Result<()> {
    ensure_per_monitor_v2_awareness()?;

    let _apartment = Apartment::new()?;

    let logger = FileLogger::init();

    let config_path = resolve_config_path(config_path);

    let layout_path = layout_path.unwrap_or_else(|| {
        paths::layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        tracing::error!(
            thread = %thread_name,
            "Application panicked: {panic_info}. Backtrace: {backtrace:?}"
        );
    }));

    unsafe {
        let _ = PeekMessageW(&mut MSG::default(), None, 0, 0, PM_NOREMOVE);
    }

    let _console_signals = crate::platform::windows::signals::ConsoleSignals::for_current_thread()
        .inspect_err(|error| tracing::warn!(%error, "Failed to install console control handler"))
        .ok();

    let (app, app_events, timers) = RunLoop::new(
        load_icon(TRAY_ICON_RESOURCE_ID)?,
        Rc::new(handle_menu_action),
    )?;
    let application = run_application(
        app_events,
        app.tray(),
        (app.timer_window(), timers),
        app.callback_panic(),
        app.shutdown_handle(),
        config_path,
        layout_path,
        logger,
    );
    app.spawn(async move {
        application
            .await
            .unwrap_or_else(|error| panic!("Application failed: {error:#}"));
    });
    app.run()
}

fn handle_menu_action(action: crate::action::Action) {
    RunLoop::current().spawn(async move {
        RunLoop::with_dome(|dome| {
            if let Err(error) = dome.handle_action(&action) {
                tracing::warn!(%error, "Action failed");
            }
            dome.update_tray();
        });
    });
}

#[expect(
    clippy::too_many_arguments,
    reason = "startup receives native resources and application configuration before installation"
)]
async fn run_application(
    app_events: futures::channel::mpsc::UnboundedReceiver<
        crate::platform::windows::event_loop::NativeEvent,
    >,
    tray: Rc<dyn Tray>,
    (timer_window, timers): (HWND, futures::channel::mpsc::UnboundedReceiver<usize>),
    callback_panic: Rc<CallbackPanic>,
    shutdown: Shutdown,
    config_path: String,
    layout_path: String,
    logger: FileLogger,
) -> anyhow::Result<()> {
    let path = layout_path.clone();
    let layout_source = RunLoop::dispatch(move |_| std::fs::read_to_string(path));
    let layout_source = layout_source
        .await
        .map_err(anyhow::Error::from)
        .and_then(|source| source.map_err(Into::into));
    let layout = PreferredLayouts::load_or_default(&layout_path, layout_source);
    tracing::info!(path = %layout_path, "Loaded layout");

    let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapView>();

    let mut keymap = KeymapPublisher::new(KeymapView::new(), keymap_tx);
    let path = config_path.clone();
    let source = RunLoop::dispatch(move |_| std::fs::read_to_string(path));
    let source = source
        .await
        .map_err(anyhow::Error::from)
        .and_then(|source| source.map_err(Into::into));
    let (runtime, config) = LuaRuntime::load(config_path.clone(), source, &mut keymap)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?;
    tracing::info!(%config_path, "Loaded config");
    logger.set_level(config.log_level);
    login_item::sync_login_item(config.start_at_login);
    let (window_server, notifications) =
        NativeWindowServer::new(timers, timer_window, shutdown.clone())?;
    let (dpi_sender, dpi_changes) = RunLoop::channel();
    let overlays = NativeOverlays::new(
        callback_panic,
        dpi_sender,
        ui::drawing::create_wgpu_context().await,
        window_server.clone(),
    );
    let monitors = RunLoop::dispatch(display::get_all_monitors);
    let monitors = monitors.await??;
    let dome = Dome::new(
        monitors,
        config.tiling,
        layout,
        window_server.clone(),
        config.appearance,
        runtime,
        keymap,
        config.env,
        Box::new(overlays),
        tray,
        Box::new(logger),
    )?;
    RunLoop::install(dome);
    display::register(app_events, dpi_changes);
    let (hub_sender, ipc_sender, config_sender) = Dome::register_sources(notifications);

    let _keyboard_hook = start_keyboard_hook(keymap_rx, hub_sender.clone())?;

    let listener = ipc::bind_server()?;
    let _config_watcher = start_file_watcher(&config_path, move || {
        let _ = config_sender.send(());
    })
    .inspect_err(|error| tracing::warn!(%error, "Failed to setup config watcher"))
    .ok();
    start_ipc_server(listener, layout_path, ipc_sender, shutdown)?;
    RunLoop::with_dome(|dome| {
        dome.apply_layout();
        dome.update_tray();
    });
    std::future::pending().await
}

fn start_keyboard_hook(
    keymap_rx: mpsc::Receiver<KeymapView>,
    hub_sender: EventSender<HubEvent>,
) -> Result<KeyboardHookHandle> {
    let (keyboard_ready, keyboard_readiness) = mpsc::sync_channel(0);
    let keyboard_thread = thread::Builder::new()
        .name("dome-keyboard-hook".to_owned())
        .spawn(move || keyboard::run_keyboard_hook(keymap_rx, hub_sender, keyboard_ready))?;
    let keyboard_result = keyboard_readiness
        .recv()
        .map_err(|_| anyhow::anyhow!("keyboard hook thread died"))
        .and_then(|result| result.map_err(anyhow::Error::from));
    match keyboard_result {
        Ok(thread_id) => Ok(KeyboardHookHandle::new(thread_id, keyboard_thread)),
        Err(error) => {
            let _ = keyboard_thread.join();
            Err(error)
        }
    }
}

fn start_ipc_server(
    listener: interprocess::local_socket::Listener,
    layout_path: String,
    ipc_sender: HubSender,
    shutdown: Shutdown,
) -> Result<()> {
    let operations = std::sync::Arc::new(std::sync::Mutex::new(()));
    thread::Builder::new()
        .name("dome-ipc".to_owned())
        .spawn(move || {
            ipc::serve(listener, layout_path, move |request| {
                let hub = ipc_sender.clone();
                let shutdown = shutdown.clone();
                let operations = operations.clone();
                thread::Builder::new()
                    .name("dome-ipc-connection".to_owned())
                    .spawn(move || {
                        if let Err(error) =
                            dome::ipc::handle_connection(request, &hub, &operations, &shutdown)
                        {
                            tracing::warn!(%error, "IPC connection failed");
                        }
                    })?;
                Ok(())
            });
        })?;
    Ok(())
}

struct Apartment(std::marker::PhantomData<Rc<()>>);

impl Apartment {
    fn new() -> Result<Self> {
        unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };
        Ok(Self(std::marker::PhantomData))
    }
}

impl Drop for Apartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

/// Must match the id in the resource script the build embeds.
const TRAY_ICON_RESOURCE_ID: u16 = 1;

/// Returns an error unless the process has Per-Monitor V2 DPI awareness.
/// Geometry and rendering require this awareness mode.
fn ensure_per_monitor_v2_awareness() -> anyhow::Result<()> {
    let result =
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if result.is_ok() {
        return Ok(());
    }
    let err = result.unwrap_err();

    // These awareness-query APIs require Windows 10 1803 or later.
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

mod dispatcher;
pub(crate) mod event_loop;
mod font;
mod signals;
mod tray;
mod waker;

use windows::core::{PCWSTR, w};

const WM_APP_WAKE: u32 = WM_APP;

/// The tray icon's callback message, posted by the shell to the owner window. The crate
/// picks it, so any value at or above `WM_APP` works.
const WM_APP_TRAY: u32 = WM_APP + 1;

/// The tray icon's identifier within its owner window. The crate installs one icon per
/// window, so a fixed id is enough.
const TRAY_UID: u32 = 1;

/// The single class every auxiliary window shares. Self-exclusion from window
/// management keys on `WS_EX_TOOLWINDOW`, not the class name, so one class is enough.
const CLASS_NAME: PCWSTR = w!("DomeAuxiliaryWindow");

/// The tray's hidden window needs its own class, because a class binds one wnd-proc.
const TRAY_CLASS_NAME: PCWSTR = w!("DomeTrayWindow");
const WAKE_CLASS_NAME: PCWSTR = w!("DomeWakeWindow");

use windows::Win32::UI::WindowsAndMessaging::WM_APP;
