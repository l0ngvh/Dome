mod accessibility;
mod dispatcher;
mod dome;
mod event_loop;
mod font;
mod keyboard;
mod listeners;
mod login_item;
mod objc2_wrapper;
mod running_application;
mod spawn;
mod throttle;
mod ui;

#[cfg(test)]
mod tests;

use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::MainThreadMarker;
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFDictionary, kCFBooleanTrue};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

use crate::config::{
    Config, LayoutConfig, ModalKeymaps, layout_default_path, load_or_default, resolve_config_path,
    start_config_watcher, start_file_watcher,
};
use crate::ipc;
use crate::keymap::KeymapState;
use crate::logging::Logger;
use crate::lua_runtime::LuaRuntime;
pub(in crate::platform::macos) use dome::MonitorInfo;
use dome::{Dome, HubEvent, get_all_monitors};
use listeners::EventListener;
use ui::{MessageSender, Ui};

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> anyhow::Result<()> {
    let logger = Logger::init();

    let config_path = resolve_config_path(config_path);

    let layout_path = layout_path.unwrap_or_else(|| {
        layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });
    let layout = load_or_default(&layout_path, LayoutConfig::load);
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

    let trusted = unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    };
    if !trusted {
        return Err(anyhow::anyhow!(
            "Accessibility permission required. Please grant permission in System Settings > Privacy & Security > Accessibility, then restart Dome."
        ));
    }

    if !CGPreflightScreenCaptureAccess() {
        tracing::info!("Screen recording permission not granted, requesting...");
        if !CGRequestScreenCaptureAccess() {
            return Err(anyhow::anyhow!(
                "Screen recording permission required. Please grant permission in System Settings > Privacy & Security > Screen Recording, then restart Dome."
            ));
        }
    }

    let mtm = MainThreadMarker::new().unwrap();

    let (event_tx, event_rx) = calloop::channel::channel();

    // Filled from the runtime thread's initial config below, before the event
    // tap and watchers read it.
    let keymap_state = Arc::new(RwLock::new(KeymapState::new(ModalKeymaps::default())));

    let monitors = get_all_monitors(mtm)?;
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let hub_layout = layout.workspace.clone();

    // Two-way startup handshake. The hub owns the VM, so it loads the config and
    // sends it to main. Main builds the UI and sends the sender back to the hub.
    let (init_tx, init_rx) = mpsc::channel::<Config>();
    let (sender_tx, sender_rx) = mpsc::channel::<MessageSender>();

    let hub_thread = thread::spawn({
        let keymap_state = keymap_state.clone();
        let logger = logger.clone();
        let bundle_path = bundle_path.clone();
        let config_path = config_path.clone();
        move || {
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let mut runtime = match LuaRuntime::new(config_path) {
                    Ok(runtime) => runtime,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to build the Lua VM, aborting startup");
                        return;
                    }
                };
                let config = runtime.load();
                // Populate keymaps before init_tx.send. Main starts the event tap
                // only after init_rx.recv, so an early keypress cannot resolve
                // against an empty keymap.
                keymap_state
                    .write()
                    .unwrap()
                    .update_keymaps(config.keymaps.clone());
                init_tx.send(config.clone()).ok();
                let sender = sender_rx.recv().expect("main dropped the UI sender");
                let env = config.env.clone();
                let dome = Dome::new(&monitors, config, hub_layout, Box::new(sender));
                event_loop::run_dome(
                    dome,
                    event_rx,
                    keymap_state,
                    runtime,
                    logger,
                    bundle_path,
                    env,
                );
            }))
            .ok();
        }
    });

    let config = init_rx
        .recv()
        .map_err(|_| anyhow::anyhow!("hub thread exited before the initial config load"))?;
    logger.set_level(config.log_level);
    tracing::info!(%config_path, "Loaded config");
    login_item::sync_login_item(config.start_at_login, bundle_path.as_deref());

    let _config_watcher = start_file_watcher(&config_path, {
        let tx = event_tx.clone();
        move || {
            tx.send(HubEvent::ReloadConfig).ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    let _layout_watcher = start_config_watcher(&layout_path, LayoutConfig::load, {
        let tx = event_tx.clone();
        move |new_layout| {
            tx.send(HubEvent::LayoutConfigChanged(Box::new(new_layout)))
                .ok();
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup layout watcher: {e:#}"))
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
            ipc::IpcEvent::ExportLayout(path) => tx
                .send(HubEvent::ExportLayout(path))
                .or(Err(anyhow::anyhow!("channel closed"))),
        }
    })?;

    let is_suspended = Arc::new(AtomicBool::new(false));
    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());

    thread::Builder::new()
        .name("dome-event-tap".to_owned())
        .spawn({
            let keymap_state = keymap_state.clone();
            let event_sender = event_tx.clone();
            move || keyboard::run_event_tap(keymap_state, is_suspended, event_sender)
        })?;

    let (ui, sender) = Ui::new(mtm, event_tx, event_listener, config);
    sender_tx.send(sender).ok();

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
