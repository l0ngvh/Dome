use crate::platform::keymap::{KeymapPublisher, KeymapView};
mod accessibility;
mod capture;
mod corner_radius;
mod display;
pub(crate) mod dome;
mod keyboard;
mod login_item;
mod objc2_wrapper;
mod permissions;
mod running_application;
pub(super) mod spawn;
mod throttle;
pub(crate) mod ui;
mod window_server;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;

use crate::config::watch::start_file_watcher;
use crate::config::{LuaRuntime, PreferredLayouts, bootstrap::resolve_config_path, paths};
use crate::ipc;
use crate::logging::{FileLogger, Logger};
use crate::platform::macos::event_loop::EventSender;
use crate::platform::macos::event_loop::RunLoop;
use crate::platform::macos::event_loop::Shutdown;
use crate::platform::macos::menu_bar::{MenuBar, load_icon};
use crate::platform::messages::HubSender;
pub(in crate::platform::macos) use dome::MonitorInfo;
use dome::{Dome, HubEvent};
use window_server::NativeWindowServer;

const STATUS_BAR_ICON_PNG: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../resources/macos/status_bar_icon.png"
));

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> anyhow::Result<()> {
    let logger = FileLogger::init();

    let config_path = resolve_config_path(config_path);

    let layout_path = layout_path.unwrap_or_else(|| {
        paths::layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });

    let bundle_path = login_item::detect_bundle_path();

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        tracing::error!(
            thread = %thread_name,
            "Application panicked: {panic_info}. Backtrace: {backtrace:?}"
        );
    }));

    if !permissions::ensure_permissions() {
        return Ok(());
    }

    let (app, app_events) =
        RunLoop::new(load_icon(STATUS_BAR_ICON_PNG)?, Rc::new(handle_menu_action))?;

    let application = run_application(
        app_events,
        app.menu_bar(),
        config_path,
        layout_path,
        logger,
        bundle_path,
        app.shutdown_handle(),
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
            dome.update_menu_bar();
        });
    });
}

async fn run_application(
    app_events: futures::channel::mpsc::UnboundedReceiver<
        crate::platform::macos::event_loop::NativeEvent,
    >,
    menu_bar: Rc<dyn MenuBar>,
    config_path: String,
    layout_path: String,
    logger: FileLogger,
    bundle_path: Option<String>,
    shutdown: Shutdown,
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

    let is_suspended = Arc::new(AtomicBool::new(false));
    let (event_tx, events) = RunLoop::channel();
    let (window_server, notifications) = NativeWindowServer::new(is_suspended.clone())?;
    let monitors: Vec<_> =
        display::get_all_monitors(objc2::MainThreadMarker::new().expect("owner thread"))?
            .into_iter()
            .map(MonitorInfo::from)
            .collect();
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

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
    login_item::sync_login_item(config.start_at_login, bundle_path.as_deref());
    let gpu = Rc::new(ui::drawing::create_wgpu_context().await?);
    let captures = Rc::new(ui::capture_stream::NativeCaptures {
        server: window_server.clone(),
        queue: dispatch2::DispatchQueue::new("dome.capture", None),
    });
    let overlays = ui::NativeOverlays::new(gpu, captures);
    let dome = Dome::new(
        &monitors,
        config.tiling,
        layout,
        runtime,
        keymap,
        config.env,
        window_server.clone(),
        Box::new(overlays),
        menu_bar,
        config.appearance,
        Box::new(logger),
        bundle_path,
    );
    RunLoop::install(dome);
    let (ipc_sender, config_sender) = Dome::register_sources(events, notifications);
    display::register(app_events);
    let _config_watcher = start_file_watcher(&config_path, move || {
        config_sender.send(()).ok();
    })
    .inspect_err(|error| tracing::warn!(%error, "Failed to setup config watcher"))
    .ok();
    let listener = ipc::bind_server()?;
    start_ipc_server(listener, layout_path, ipc_sender, shutdown)?;
    let _event_tap = start_event_tap(keymap_rx, is_suspended, event_tx)?;
    RunLoop::with_dome(|dome| dome.update_menu_bar());
    std::future::pending().await
}

fn start_ipc_server(
    listener: interprocess::local_socket::Listener,
    layout_path: String,
    ipc_sender: HubSender,
    shutdown: Shutdown,
) -> anyhow::Result<()> {
    let operations = Arc::new(std::sync::Mutex::new(()));
    std::thread::Builder::new()
        .name("dome-ipc".to_owned())
        .spawn(move || {
            ipc::serve(listener, layout_path, move |request| {
                let hub = ipc_sender.clone();
                let shutdown = shutdown.clone();
                let operations = operations.clone();
                std::thread::Builder::new()
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

fn start_event_tap(
    keymap_rx: mpsc::Receiver<KeymapView>,
    is_suspended: Arc<AtomicBool>,
    event_tx: EventSender<HubEvent>,
) -> anyhow::Result<keyboard::EventTap> {
    let (ready, readiness) = mpsc::sync_channel(1);
    let worker = std::thread::Builder::new()
        .name("dome-event-tap".to_owned())
        .spawn(move || {
            keyboard::run_event_tap(keymap_rx, is_suspended, event_tx, ready);
        })?;
    let result = readiness
        .recv()
        .map_err(anyhow::Error::from)
        .and_then(|result| result);
    match result {
        Ok(shutdown) => Ok(keyboard::EventTap::new(shutdown, worker)),
        Err(error) => {
            let _ = worker.join();
            Err(error)
        }
    }
}

fn set_current_thread_qos(class: libc::qos_class_t) {
    let result = unsafe { libc::pthread_set_qos_class_self_np(class, 0) };
    if result != 0 {
        tracing::warn!(result, ?class, "Failed to set the thread QoS");
    }
}

mod dispatcher;
pub(crate) mod event_loop;
mod font;
mod menu_bar;
mod signals;
mod timer;
mod waker;
