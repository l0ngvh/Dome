use crate::platform::keymap::{KeymapPublisher, KeymapView};
mod accessibility;
mod corner_radius;
mod dispatcher;
mod dome;
mod event_loop;
mod font;
mod keyboard;
mod listeners;
mod login_item;
mod objc2_wrapper;
mod permissions;
mod running_application;
mod spawn;
mod throttle;
mod ui;

#[cfg(test)]
mod tests;

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
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
use listeners::EventListener;
use ui::{MessageSender, Ui};

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

    let (event_tx, event_rx) = calloop::channel::channel();

    let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapView>();

    let monitors = get_all_monitors(mtm)?;
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let hub_layout = layout.clone();

    // Only the hub thread may touch the VM.
    let (init_tx, init_rx) = mpsc::channel::<Appearance>();
    let (sender_tx, sender_rx) = mpsc::channel::<MessageSender>();

    let hub_thread = thread::spawn({
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
                let dome = Dome::new(
                    &monitors,
                    config.tiling,
                    hub_layout,
                    Box::new(sender),
                    runtime,
                    keymap,
                    config.env,
                );
                event_loop::run_dome(dome, event_rx, logger, bundle_path);
            }))
            .ok();
        }
    });

    let appearance = init_rx
        .recv()
        .map_err(|_| anyhow::anyhow!("hub thread exited before the initial config load"))?;

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
    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());

    thread::Builder::new()
        .name("dome-event-tap".to_owned())
        .spawn({
            let event_sender = event_tx.clone();
            move || keyboard::run_event_tap(keymap_rx, is_suspended, event_sender)
        })?;

    let (ui, sender) = Ui::new(mtm, event_tx, event_listener, appearance);
    if sender_tx.send(sender).is_err() {
        anyhow::bail!("hub thread exited before it could receive the UI sender");
    }

    ui.run();

    hub_thread.join().ok();
    Ok(())
}

fn send_hub_event(hub_sender: &calloop::channel::Sender<HubEvent>, event: HubEvent) {
    if hub_sender.send(event).is_err() {
        tracing::error!("Hub thread died, shutting down");
        // Off-main callers leave termination to the main thread, which hits the
        // same closed channel on its next send.
        if let Some(mtm) = MainThreadMarker::new() {
            objc2_app_kit::NSApplication::sharedApplication(mtm).terminate(None);
        }
    }
}

/// A `std::thread` starts at `QOS_CLASS_DEFAULT`, not at the QoS of the thread
/// that spawned it.
fn set_current_thread_qos(class: libc::qos_class_t) {
    let rc = unsafe { libc::pthread_set_qos_class_self_np(class, 0) };
    if rc != 0 {
        tracing::warn!(rc, ?class, "Failed to set the thread QoS");
    }
}
