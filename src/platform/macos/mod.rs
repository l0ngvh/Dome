use crate::platform::keymap::{KeymapPublisher, KeymapView};
mod accessibility;
mod corner_radius;
mod dispatcher;
mod dome;
mod focus;
mod font;
mod keyboard;
mod listeners;
mod login_item;
mod objc2_wrapper;
mod permissions;
mod runner;
mod running_application;
mod spawn;
mod throttle;
mod ui;
mod window_server;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(in crate::platform::macos) enum Wake {
    Ui,
    WindowServer,
    FocusRequest,
    Capture,
}

#[cfg(test)]
mod tests;

#[cfg(feature = "native-ui-test")]
pub fn run_native_ui_loop_test() {
    ui::run_native_loop();
}

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, OnceLock};
use std::thread;

use objc2::MainThreadMarker;

use crate::config::watch::start_file_watcher;
use crate::config::{
    Appearance, LuaRuntime, PreferredLayouts, bootstrap::resolve_config_path, paths,
};
use crate::ipc;
use crate::logging::Logger;
pub(in crate::platform::macos) use dome::MonitorInfo;
use dome::{Dome, HubEvent, get_all_monitors};
use focus::SharedFocus;
use ui::{Ui, UiSender};
use window_server::MacOsWindowServer;

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> anyhow::Result<()> {
    let logger = Logger::init();

    let config_path = resolve_config_path(config_path);

    let layout_path = layout_path.unwrap_or_else(|| {
        paths::layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });
    let layout = PreferredLayouts::load_or_default(&layout_path);
    tracing::info!(path = %layout_path, "Loaded layout");

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

    let mtm = MainThreadMarker::new().unwrap();

    if !permissions::ensure_permissions() {
        return Ok(());
    }

    let timeout_result = unsafe {
        objc2_application_services::AXUIElement::new_system_wide().set_messaging_timeout(0.25)
    };
    if timeout_result != objc2_application_services::AXError::Success {
        anyhow::bail!("Failed to set the process accessibility timeout: {timeout_result:?}");
    }

    let (event_tx, event_rx) = calloop::channel::channel();

    let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapView>();

    let monitors = get_all_monitors(mtm)?;
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let hub_layout = layout.clone();

    let (init_tx, init_rx) = mpsc::channel::<Appearance>();
    let (sender_tx, sender_rx) = mpsc::channel::<UiSender>();

    // Only the domain thread may touch the VM.
    let domain_thread = thread::spawn({
        let logger = logger.clone();
        let bundle_path = bundle_path.clone();
        let config_path = config_path.clone();
        move || {
            // Key binding results wait on this thread. The thread also runs
            // Lua callbacks. At USER_INTERACTIVE, a slow callback would take
            // CPU time from the focused app.
            set_current_thread_qos(libc::qos_class_t::QOS_CLASS_USER_INITIATED);
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut keymap = KeymapPublisher::new(KeymapView::new(), keymap_tx);
                // Load before init_tx.send, so no keypress resolves against an
                // empty keymap.
                let (runtime, config) = match LuaRuntime::load(config_path.clone(), &mut keymap) {
                    Ok(loaded) => loaded,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to build the Lua VM, aborting startup");
                        return;
                    }
                };
                tracing::info!(%config_path, "Loaded config");
                logger.set_level(config.log_level);
                login_item::sync_login_item(config.start_at_login, bundle_path.as_deref());
                init_tx.send(config.appearance).ok();
                let sender = sender_rx.recv().expect("main dropped the UI sender");
                let _exit = runner::DomainExitGuard::new(Box::new(sender.clone()));
                let dome = Dome::new(
                    &monitors,
                    config.tiling,
                    hub_layout,
                    Box::new(sender),
                    runtime,
                    keymap,
                    config.env,
                );
                runner::run_dome(dome, event_rx, logger, bundle_path);
            }))
            .ok();
        }
    });

    let appearance = init_rx
        .recv()
        .map_err(|_| anyhow::anyhow!("domain thread exited before the initial config load"))?;

    let _config_watcher = start_file_watcher(&config_path, {
        let tx = event_tx.clone();
        move || {
            tx.send(HubEvent::ReloadConfig).ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    ipc::start_server(layout_path.clone(), {
        let tx = event_tx.clone();
        move |ev| match ev {
            ipc::IpcEvent::Action(actions) => tx
                .send(HubEvent::Action(actions))
                .or(Err(anyhow::anyhow!("channel closed"))),
            ipc::IpcEvent::Query { query, reply } => tx
                .send(HubEvent::Query {
                    query,
                    sender: reply,
                })
                .or(Err(anyhow::anyhow!("channel closed"))),
            ipc::IpcEvent::SaveLayout(path) => tx
                .send(HubEvent::SaveLayout(path))
                .or(Err(anyhow::anyhow!("channel closed"))),
            ipc::IpcEvent::ApplyLayout(path) => tx
                .send(HubEvent::ApplyLayout(path))
                .or(Err(anyhow::anyhow!("channel closed"))),
        }
    })?;

    let is_suspended = Arc::new(AtomicBool::new(false));
    let focus = Arc::new(SharedFocus::new());
    let loop_waker = Arc::new(OnceLock::new());
    let (native_sender, native_events) = mpsc::channel();
    let windows = MacOsWindowServer::new(
        native_events,
        native_sender,
        focus.clone(),
        loop_waker.clone(),
        is_suspended.clone(),
        std::time::Instant::now(),
    );

    thread::Builder::new()
        .name("dome-event-tap".to_owned())
        .spawn({
            let event_sender = event_tx.clone();
            move || keyboard::run_event_tap(keymap_rx, is_suspended, event_sender)
        })?;

    let (ui, sender) = Ui::new(
        mtm,
        event_tx,
        Box::new(windows),
        focus,
        loop_waker,
        appearance,
    );
    if sender_tx.send(sender).is_err() {
        anyhow::bail!("domain thread exited before it could receive the UI sender");
    }

    ui.run();

    domain_thread.join().ok();
    Ok(())
}

/// A `std::thread` starts at `QOS_CLASS_DEFAULT`, not at the QoS of the thread
/// that spawned it.
fn set_current_thread_qos(class: libc::qos_class_t) {
    let rc = unsafe { libc::pthread_set_qos_class_self_np(class, 0) };
    if rc != 0 {
        tracing::warn!(rc, ?class, "Failed to set the thread QoS");
    }
}
