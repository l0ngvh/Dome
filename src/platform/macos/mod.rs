mod accessibility;
mod dispatcher;
mod dome;
mod event_loop;
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
use objc2_app_kit::NSScreen;
use objc2_application_services::{AXIsProcessTrustedWithOptions, kAXTrustedCheckOptionPrompt};
use objc2_core_foundation::{CFDictionary, kCFBooleanTrue};
use objc2_core_graphics::{
    CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID, CGPreflightScreenCaptureAccess,
    CGRequestScreenCaptureAccess,
};
use objc2_foundation::{NSNumber, NSString};

use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use crate::keymap::KeymapState;
use crate::logging::Logger;
use dome::{Dome, HubEvent};
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

    let screens = get_all_screens(mtm);
    if screens.is_empty() {
        return Err(anyhow::anyhow!("No monitors detected"));
    }

    let is_suspended = Rc::new(Cell::new(false));
    let event_listener = EventListener::new(event_tx.clone(), is_suspended.clone());
    let _keyboard_listener =
        KeyboardListener::new(keymap_state.clone(), is_suspended, event_tx.clone())?;

    let (ui, sender) = Ui::new(mtm, event_tx, event_listener, config.clone());

    let hub_thread = thread::spawn(move || {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let dome = Dome::new(&screens, hub_config, Box::new(sender));
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

#[derive(Clone, Debug)]
pub(in crate::platform::macos) struct MonitorInfo {
    pub(in crate::platform::macos) display_id: CGDirectDisplayID,
    pub(in crate::platform::macos) name: String,
    pub(in crate::platform::macos) dimension: Dimension,
    pub(in crate::platform::macos) full_height: f32,
    pub(in crate::platform::macos) is_primary: bool,
    /// NSScreen.backingScaleFactor — used for egui render density only.
    /// This is NOT core Monitor.scale (which is always 1.0 on macOS because
    /// AppKit already reports points, so no DPI conversion is needed).
    pub(in crate::platform::macos) scale: f64,
}

impl std::fmt::Display for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, dim={:?}, scale={})",
            self.name, self.display_id, self.dimension, self.scale
        )
    }
}

fn get_display_id(screen: &NSScreen) -> CGDirectDisplayID {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    desc.objectForKey(&key)
        .and_then(|obj| {
            let num: Option<&NSNumber> = obj.downcast_ref();
            num.map(|n| n.unsignedIntValue())
        })
        .unwrap_or(0)
}

fn get_all_screens(mtm: MainThreadMarker) -> Vec<MonitorInfo> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen);
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            MonitorInfo {
                display_id,
                name,
                dimension: Dimension {
                    x: bounds.origin.x as f32,
                    y: (bounds.origin.y + top_inset) as f32,
                    width: bounds.size.width as f32,
                    height: (bounds.size.height - top_inset - bottom_inset) as f32,
                },
                full_height: bounds.size.height as f32,
                is_primary: display_id == primary_id,
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}
