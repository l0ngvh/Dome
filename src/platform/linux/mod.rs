mod focus;
mod handlers;
mod input;
mod render;
mod state;
mod udev_backend;
mod window;
mod winit_backend;

use anyhow::Result;
use calloop::EventLoop;
use calloop::channel;

use crate::config::{Config, start_config_watcher};
use crate::core::GlobalLayoutConfig;
use crate::ipc;
use crate::logging::Logger;
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

pub fn run_app(config_path: Option<String>, _layout_path: Option<String>) -> Result<()> {
    // layout.toml not yet loaded on linux. Follow-up will plumb workspace_overrides
    // once wl_output fractional-scale is finalized.
    // Reap child processes automatically (prevents zombies from Action::Exec)
    // SAFETY: called at process start, before any child processes are spawned
    unsafe { libc::signal(libc::SIGCHLD, libc::SIG_IGN); }

    let logger = Logger::init();

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    logger.set_level(config.log_level);

    let mut event_loop: EventLoop<CalloopData> = EventLoop::try_new()?;

    let (config_tx, config_rx) = channel::channel::<Config>();
    let _config_watcher = start_config_watcher(&config_path, Config::load, move |cfg| {
        config_tx.send(cfg).ok();
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    event_loop
        .handle()
        .insert_source(config_rx, |event, _, data| {
            if let channel::Event::Msg(config) = event {
                tracing::info!("Config reloaded");
                data.state
                    .hub
                    .sync_configuration(GlobalLayoutConfig::from(&config));
                data.state.config = config;
                data.state.sync_window_positions();
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to insert config channel: {e}"))?;

    // DomeState::new sets WAYLAND_DISPLAY env var, so it must run before
    // spawning any threads (IPC server below) to avoid set_var UB.
    let mut state = DomeState::new(&mut event_loop, config)?;

    let (ipc_tx, ipc_rx) = channel::channel();
    ipc::start_server(move |msg| {
        use crate::action::IpcMessage;
        match msg {
            IpcMessage::Action(action) => {
                ipc_tx
                    .send(action)
                    .map_err(|e| anyhow::anyhow!("ipc channel closed: {e}"))?;
                Ok("ok".to_string())
            }
            IpcMessage::Query(_) => Ok("{}".to_string()),
            // layout.toml is not wired on linux yet. Report unsupported
            // instead of silently succeeding so the CLI surfaces the gap.
            IpcMessage::ExportLayout => {
                Ok(r#"{"error":"export_layout not supported on linux"}"#.to_string())
            }
        }
    })?;

    event_loop
        .handle()
        .insert_source(ipc_rx, |event, _, data| {
            if let channel::Event::Msg(action) = event {
                data.state.handle_action(&action);
            }
        })
        .map_err(|e| anyhow::anyhow!("failed to insert IPC channel: {e}"))?;

    if should_use_winit() {
        let mut winit_data = winit_backend::init_winit_backend(&mut event_loop, state.loop_signal.clone())?;
        winit_data.output.create_global::<DomeState>(&state.display_handle);
        state.space.map_output(&winit_data.output, (0, 0));

        let size = winit_data.backend.window_size();
        let monitor_id = state.hub.focused_monitor();
        state.hub.update_monitor(monitor_id, crate::core::Dimension::new(
            crate::core::Length::new(0.0),
            crate::core::Length::new(0.0),
            crate::core::Length::new(size.w as f32),
            crate::core::Length::new(size.h as f32),
        ), 1.0f32);
        state.monitor_outputs.insert(monitor_id, winit_data.output.clone());

        state.init_egui_painter(winit_data.gl.clone());

        // Init dmabuf from winit renderer's EGL context
        {
            use smithay::backend::egl::EGLDevice;
            use smithay::backend::renderer::ImportDma;
            let renderer = winit_data.backend.renderer();
            let formats = renderer.dmabuf_formats();
            let render_node = EGLDevice::device_for_display(renderer.egl_context().display())
                .ok()
                .and_then(|device| device.try_get_render_node().ok().flatten());
            if let Some(render_node) = render_node {
                state.init_dmabuf(render_node, formats);
            }
        }

        state.winit_data = Some(winit_data);

        let mut data = CalloopData { state };
        tracing::info!("Starting Dome on Linux (winit backend)");
        event_loop.run(None, &mut data, |_| {})?;
    } else {
        let mut data = CalloopData { state };
        udev_backend::init_udev_backend(&mut event_loop, &mut data)?;

        if data.state.config.linux.xwayland {
            data.state.spawn_xwayland_satellite();

            // Only set up health-check if spawn succeeded
            if data.state.xwayland_child.is_some() {
                event_loop
                    .handle()
                    .insert_source(
                        calloop::timer::Timer::from_duration(std::time::Duration::from_secs(5)),
                        |_, _, data| {
                            data.state.respawn_xwayland_satellite();
                            calloop::timer::TimeoutAction::ToDuration(std::time::Duration::from_secs(5))
                        },
                    )
                    .map_err(|e| anyhow::anyhow!("failed to insert xwayland health-check timer: {e}"))?;
            }
        }

        tracing::info!("Starting Dome on Linux (udev backend)");
        event_loop.run(None, &mut data, |data| {
            data.state.space.refresh();
            data.state.popups.cleanup();
            data.state.display_handle.flush_clients().ok();
        })?;

        data.state.kill_xwayland_satellite();
    }

    Ok(())
}
