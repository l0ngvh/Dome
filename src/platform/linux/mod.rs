mod focus;
mod handlers;
mod input;
mod render;
mod state;
mod udev_backend;
mod winit_backend;

use anyhow::Result;
use calloop::EventLoop;
use calloop::channel;
use tracing_error::ErrorLayer;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt};

use crate::config::{Config, start_config_watcher};
use crate::ipc;
use state::DomeState;

pub struct CalloopData {
    pub state: DomeState,
}

fn should_use_winit() -> bool {
    match std::env::var("DOME_BACKEND").as_deref() {
        Ok("winit") => true,
        Ok("udev") => false,
        _ => std::env::var("WAYLAND_DISPLAY").is_ok() || std::env::var("DISPLAY").is_ok(),
    }
}

pub fn run_app(config_path: Option<String>) -> Result<()> {
    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    init_tracing(&config);

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;

    let (config_tx, config_rx) = channel::channel::<Config>();
    let _config_watcher = start_config_watcher(&config_path, move |cfg| {
        config_tx.send(cfg).ok();
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    event_loop
        .handle()
        .insert_source(config_rx, |event, _, data| {
            if let channel::Event::Msg(config) = event {
                tracing::info!("Config reloaded");
                data.state.hub.sync_config(config.clone().into());
                data.state.config = config;
                data.state.sync_window_positions();
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to insert config channel: {e}"))?;

    let (ipc_tx, ipc_rx) = channel::channel();
    ipc::start_server(move |actions| {
        ipc_tx.send(actions).ok();
        Ok(())
    })?;

    event_loop
        .handle()
        .insert_source(ipc_rx, |event, _, data| {
            if let channel::Event::Msg(actions) = event {
                for action in &actions {
                    data.state.handle_action(action);
                }
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to insert IPC channel: {e}"))?;

    let mut state = DomeState::new(&mut event_loop, config)?;

    if should_use_winit() {
        let winit_data = winit_backend::init_winit_backend(&mut event_loop, state.loop_signal.clone())?;
        winit_data.output.create_global::<DomeState>(&state.display_handle);
        state.space.map_output(&winit_data.output, (0, 0));

        let size = winit_data.backend.window_size();
        let monitor_id = state.hub.focused_monitor();
        state.hub.update_monitor_dimension(monitor_id, crate::core::Dimension {
            x: 0.0,
            y: 0.0,
            width: size.w as f32,
            height: size.h as f32,
        });

        state.init_egui_painter(winit_data.gl.clone());
        state.winit_data = Some(winit_data);

        let mut data = CalloopData { state };
        tracing::info!("Starting Dome on Linux (winit backend)");
        event_loop.run(None, &mut data, |_| {})?;
    } else {
        let mut data = CalloopData { state };
        udev_backend::init_udev_backend(&mut event_loop, &mut data)?;
        tracing::info!("Starting Dome on Linux (udev backend)");
        event_loop.run(None, &mut data, |data| {
            data.state.space.refresh();
            data.state.popups.cleanup();
            data.state.display_handle.flush_clients().ok();
        })?;
    }

    Ok(())
}

fn init_tracing(config: &Config) {
    let filter = EnvFilter::try_new(config.log_level.as_str())
        .unwrap_or_else(|_| EnvFilter::from_default_env());
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
