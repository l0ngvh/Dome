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
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::MainThreadMarker;
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFDictionary, kCFBooleanTrue};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

use crate::config::{
    Config, LayoutConfig, ModalKeymaps, layout_default_path, load_or_default, start_config_watcher,
    start_file_watcher,
};
use crate::ipc;
use crate::keymap::KeymapState;
use crate::logging::Logger;
use crate::lua_runtime::{self, RuntimeMsg, RuntimeOut};
pub(in crate::platform::macos) use dome::MonitorInfo;
use dome::{Dome, HubEvent, get_all_monitors};
use listeners::EventListener;
use ui::Ui;

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> anyhow::Result<()> {
    let logger = Logger::init();

    let config_path = config_path.unwrap_or_else(Config::default_path);

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

    // KeymapState starts empty and is filled from the runtime thread's initial
    // config below, before the event tap and watchers that read it start.
    let keymap_state = Arc::new(RwLock::new(KeymapState::new(ModalKeymaps::default())));

    let out: Box<dyn Fn(RuntimeOut) + Send> = {
        let keymap_state = keymap_state.clone();
        let tx = event_tx.clone();
        let logger = logger.clone();
        let bundle_path = bundle_path.clone();
        Box::new(move |event| match event {
            RuntimeOut::Actions(actions) => send_hub_event(&tx, HubEvent::Action(actions)),
            RuntimeOut::SwitchMode(name) => {
                if let Ok(mut ks) = keymap_state.write() {
                    ks.switch_mode(&name);
                }
            }
            RuntimeOut::Reloaded(config) => {
                logger.set_level(config.log_level);
                if let Ok(mut ks) = keymap_state.write() {
                    ks.update_keymaps(config.keymaps.clone());
                }
                let start_at_login = config.start_at_login;
                send_hub_event(&tx, HubEvent::ConfigChanged(config));
                login_item::sync_login_item(start_at_login, bundle_path.as_deref());
            }
        })
    };

    let (runtime_handle, runtime_tx, config) = lua_runtime::spawn(config_path.clone(), out)?;
    logger.set_level(config.log_level);
    tracing::info!(%config_path, "Loaded config");
    keymap_state
        .write()
        .unwrap()
        .update_keymaps(config.keymaps.clone());
    login_item::sync_login_item(config.start_at_login, bundle_path.as_deref());

    let hub_config = config.clone();
    let hub_layout = layout.workspace.clone();

    let _config_watcher = start_file_watcher(&config_path, {
        let runtime_tx = runtime_tx.clone();
        move || {
            runtime_tx.send(RuntimeMsg::Reload).ok();
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

    let monitors = get_all_monitors(mtm)?;
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let is_suspended = Arc::new(AtomicBool::new(false));
    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());

    thread::Builder::new()
        .name("dome-event-tap".to_owned())
        .spawn({
            let keymap_state = keymap_state.clone();
            let hub_sender = event_tx.clone();
            let runtime_sender = runtime_tx.clone();
            move || keyboard::run_event_tap(keymap_state, is_suspended, hub_sender, runtime_sender)
        })?;

    let (ui, sender) = Ui::new(mtm, event_tx, event_listener, config.clone());

    let hub_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dome = Dome::new(&monitors, hub_config, hub_layout, Box::new(sender));
            event_loop::run_dome(dome, event_rx, keymap_state);
        }))
        .ok();
    });

    ui.run();

    runtime_tx.send(RuntimeMsg::Shutdown).ok();
    runtime_handle.join().ok();
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
