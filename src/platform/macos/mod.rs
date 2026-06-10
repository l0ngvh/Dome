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
mod throttle;
mod ui;

#[cfg(test)]
mod tests;

use std::cell::Cell;
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::thread;

use objc2::MainThreadMarker;
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFDictionary, kCFBooleanTrue};
use objc2_core_graphics::{CGPreflightScreenCaptureAccess, CGRequestScreenCaptureAccess};

use crate::config::{Config, start_config_watcher};
use crate::ipc;
use crate::keymap::KeymapState;
use crate::logging::Logger;
pub(in crate::platform::macos) use dome::MonitorInfo;
use dome::{Dome, HubEvent, get_all_monitors};
use keyboard::KeyboardListener;
use listeners::EventListener;
use ui::Ui;

const QUERY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(1);

pub fn run_app(config_path: Option<String>) -> anyhow::Result<()> {
    let logger = Logger::init();

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load_or_default(&config_path);
    logger.set_level(config.log_level);
    tracing::info!(%config_path, "Loaded config");

    let bundle_path = login_item::detect_bundle_path();
    login_item::sync_login_item(config.start_at_login, bundle_path.as_deref());

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    tracing::debug!("Accessibility: {}", unsafe {
        AXIsProcessTrustedWithOptions(Some(
            CFDictionary::from_slices(&[kAXTrustedCheckOptionPrompt], &[kCFBooleanTrue.unwrap()])
                .as_opaque(),
        ))
    });

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

    let hub_config = config.clone();
    let keymap_state = Arc::new(RwLock::new(KeymapState::new(config.keymaps.clone())));

    let _config_watcher = start_config_watcher(&config_path, {
        let keymap_state = keymap_state.clone();
        let tx = event_tx.clone();
        let bundle_path_for_watcher = bundle_path.clone();
        move |cfg| {
            logger.set_level(cfg.log_level);
            keymap_state
                .write()
                .unwrap()
                .update_keymaps(cfg.keymaps.clone());
            let start_at_login = cfg.start_at_login;
            tx.send(HubEvent::ConfigChanged(Box::new(cfg))).ok();
            login_item::sync_login_item(start_at_login, bundle_path_for_watcher.as_deref());
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    ipc::start_server({
        let tx = event_tx.clone();
        move |msg| {
            use crate::action::IpcMessage;
            match msg {
                IpcMessage::Action(action) => {
                    tx.send(HubEvent::Action(crate::action::Actions::new(vec![action])))
                        .or(Err(anyhow::anyhow!("channel closed")))?;
                    Ok("ok".to_string())
                }
                IpcMessage::Query(query) => {
                    let (resp_tx, resp_rx) = std::sync::mpsc::sync_channel(1);
                    tx.send(HubEvent::Query {
                        query,
                        sender: resp_tx,
                    })
                    .or(Err(anyhow::anyhow!("channel closed")))?;
                    match resp_rx.recv_timeout(QUERY_TIMEOUT) {
                        Ok(json) => Ok(json),
                        Err(_) => Ok(r#"{"error":"query timed out"}"#.to_string()),
                    }
                }
            }
        }
    })?;

    let monitors = get_all_monitors(mtm);
    if monitors.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let is_suspended = Rc::new(Cell::new(false));
    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());
    let _keyboard_listener =
        KeyboardListener::new(keymap_state.clone(), is_suspended, event_tx.clone())?;

    let (ui, sender) = Ui::new(mtm, event_tx, event_listener, config.clone());

    let hub_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dome = Dome::new(&monitors, hub_config, Box::new(sender));
            event_loop::run_dome(dome, event_rx, keymap_state);
        }))
        .ok();
    });

    ui.run();

    hub_thread.join().ok();
    Ok(())
}

fn send_hub_event(hub_sender: &calloop::channel::Sender<HubEvent>, event: HubEvent) {
    if hub_sender.send(event).is_err() {
        tracing::error!("Hub thread died, shutting down");
        let mtm = MainThreadMarker::new().unwrap();
        objc2_app_kit::NSApplication::sharedApplication(mtm).terminate(None);
    }
}
