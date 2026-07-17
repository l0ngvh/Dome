use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use calloop::{EventLoop, LoopSignal};
use calloop::generic::Generic;
use smithay::desktop::{PopupManager, Space, Window, layer_map_for_output};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::wayland::compositor::{CompositorClientState, CompositorState, with_states};
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;

use crate::config::Config;
use crate::core::{Dimension, GlobalLayoutConfig, Hub, Length, MonitorId, WindowId};

use super::CalloopData;
use super::udev_backend::UdevData;
use super::winit_backend::WinitBackendData;

pub(super) struct DomeState {
    pub(super) config: Config,
    pub(super) hub: Hub,
    pub(super) display_handle: DisplayHandle,
    pub(super) space: Space<Window>,
    pub(super) loop_signal: LoopSignal,
    pub(super) loop_handle: calloop::LoopHandle<'static, CalloopData>,
    pub(super) window_map: HashMap<WindowId, Window>,
    pub(super) start_time: Instant,
    pub(super) visible_windows: HashSet<WindowId>,
    pub(super) monitor_outputs: HashMap<MonitorId, Output>,

    // Smithay protocol state
    pub(super) compositor_state: CompositorState,
    pub(super) xdg_shell_state: XdgShellState,
    pub(super) xdg_decoration_state: XdgDecorationState,
    pub(super) shm_state: ShmState,
    pub(super) output_manager_state: OutputManagerState,
    pub(super) seat_state: SeatState<Self>,
    pub(super) data_device_state: DataDeviceState,
    pub(super) primary_selection_state: PrimarySelectionState,
    pub(super) popups: PopupManager,
    pub(super) seat: Seat<Self>,
    pub(super) cursor_status: smithay::input::pointer::CursorImageStatus,
    pub(super) layer_shell_state: WlrLayerShellState,
    pub(super) dmabuf_state: DmabufState,
    pub(super) dmabuf_global: Option<DmabufGlobal>,

    // Backend-specific
    pub(super) winit_data: Option<WinitBackendData>,
    pub(super) udev_data: Option<UdevData>,

    // XWayland via xwayland-satellite
    pub(super) x11_display: Option<String>,
    pub(super) xwayland_child: Option<std::process::Child>,

    // Egui overlay rendering
    pub(super) egui_ctx: egui::Context,
    pub(super) egui_painter: Option<egui_glow::Painter>,
}

impl DomeState {
    pub(super) fn new(
        event_loop: &mut EventLoop<'static, CalloopData>,
        config: Config,
    ) -> Result<Self> {
        let display: Display<Self> = Display::new()?;
        let dh = display.handle();

        let compositor_state = CompositorState::new::<Self>(&dh);
        let xdg_shell_state = XdgShellState::new::<Self>(&dh);
        let xdg_decoration_state = XdgDecorationState::new::<Self>(&dh);
        let shm_state = ShmState::new::<Self>(&dh, vec![]);
        let output_manager_state = OutputManagerState::new_with_xdg_output::<Self>(&dh);
        let mut seat_state = SeatState::new();
        let data_device_state = DataDeviceState::new::<Self>(&dh);
        let primary_selection_state = PrimarySelectionState::new::<Self>(&dh);
        let layer_shell_state = WlrLayerShellState::new::<Self>(&dh);
        let popups = PopupManager::default();
        let dmabuf_state = DmabufState::new();
        smithay::wayland::viewporter::ViewporterState::new::<Self>(&dh);
        smithay::wayland::relative_pointer::RelativePointerManagerState::new::<Self>(&dh);
        smithay::wayland::cursor_shape::CursorShapeManagerState::new::<Self>(&dh);
        smithay::wayland::pointer_constraints::PointerConstraintsState::new::<Self>(&dh);

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "dome");
        seat.add_keyboard(Default::default(), 200, 25)?;
        seat.add_pointer();

        let space = Space::default();
        Self::init_wayland_listener(display, event_loop)?;
        let loop_signal = event_loop.get_signal();
        let loop_handle = event_loop.handle();

        let screen = Dimension::new(
            Length::new(0.0),
            Length::new(0.0),
            Length::new(1280.0),
            Length::new(720.0),
        );
        let hub = Hub::new(
            screen,
            1.0,
            GlobalLayoutConfig::from(&config),
            Vec::new(),
        );

        Ok(Self {
            config,
            hub,
            display_handle: dh,
            space,
            loop_signal,
            loop_handle,
            window_map: HashMap::new(),
            start_time: Instant::now(),
            visible_windows: HashSet::new(),
            monitor_outputs: HashMap::new(),
            compositor_state,
            xdg_shell_state,
            xdg_decoration_state,
            shm_state,
            output_manager_state,
            seat_state,
            data_device_state,
            primary_selection_state,
            popups,
            seat,
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            layer_shell_state,
            dmabuf_state,
            dmabuf_global: None,
            winit_data: None,
            udev_data: None,
            x11_display: None,
            xwayland_child: None,
            egui_ctx: egui::Context::default(),
            egui_painter: None,
        })
    }

    pub(super) fn init_egui_painter(&mut self, gl: Arc<glow::Context>) {
        match egui_glow::Painter::new(gl, "", None, false) {
            Ok(painter) => self.egui_painter = Some(painter),
            Err(e) => tracing::error!("Failed to create egui painter: {e}"),
        }
    }

    pub(super) fn init_dmabuf(
        &mut self,
        render_node: smithay::backend::drm::DrmNode,
        formats: smithay::backend::allocator::format::FormatSet,
    ) {
        use smithay::wayland::dmabuf::DmabufFeedbackBuilder;
        let feedback = DmabufFeedbackBuilder::new(render_node.dev_id(), formats)
            .build()
            .expect("Failed to build dmabuf feedback");
        self.dmabuf_global = Some(
            self.dmabuf_state
                .create_global_with_default_feedback::<Self>(&self.display_handle, &feedback),
        );
    }

    pub(super) fn spawn_xwayland_satellite(&mut self) {
        let wayland_display = match std::env::var("WAYLAND_DISPLAY") {
            Ok(v) => v,
            Err(_) => {
                tracing::warn!("WAYLAND_DISPLAY not set, skipping xwayland-satellite");
                return;
            }
        };

        let mut child = match std::process::Command::new("xwayland-satellite")
            .env("WAYLAND_DISPLAY", &wayland_display)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::inherit())
            .spawn()
        {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("xwayland-satellite not available: {e}. X11 apps won't work.");
                return;
            }
        };

        // xwayland-satellite prints the display (e.g. ":0") on stdout.
        // Read it in a thread with a timeout to avoid blocking the compositor.
        let stdout = child.stdout.take();
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            use std::io::BufRead;
            let Some(stdout) = stdout else { return };
            let reader = std::io::BufReader::new(stdout);
            for line in reader.lines().take(20) {
                let Ok(line) = line else { break };
                let trimmed = line.trim();
                if let Some(d) = trimmed.strip_prefix("DISPLAY=") {
                    if let Err(e) = tx.send(d.to_string()) {
                        tracing::warn!("xwayland-satellite reader: receiver dropped: {e:#}");
                    }
                    return;
                }
                if trimmed.starts_with(':') && trimmed[1..].chars().all(|c| c.is_ascii_digit()) {
                    if let Err(e) = tx.send(trimmed.to_string()) {
                        tracing::warn!("xwayland-satellite reader: receiver dropped: {e:#}");
                    }
                    return;
                }
            }
        });

        match rx.recv_timeout(std::time::Duration::from_secs(5)) {
            Ok(d) => {
                tracing::info!("xwayland-satellite started with DISPLAY={d}");
                self.x11_display = Some(d);
                self.xwayland_child = Some(child);
            }
            Err(_) => {
                tracing::warn!("Timed out reading DISPLAY from xwayland-satellite");
                if let Err(e) = child.kill() {
                    tracing::warn!("failed to kill xwayland-satellite: {e:#}");
                }
            }
        }
    }

    pub(super) fn respawn_xwayland_satellite(&mut self) {
        if let Some(ref mut child) = self.xwayland_child {
            match child.try_wait() {
                Ok(None) => return, // still running
                Ok(Some(status)) => {
                    tracing::warn!("xwayland-satellite exited: {status}, respawning");
                }
                Err(_) => {
                    // ECHILD — already reaped (SIG_IGN), treat as dead
                    tracing::warn!("xwayland-satellite was reaped, respawning");
                }
            }
            self.xwayland_child = None;
            self.x11_display = None;
            self.spawn_xwayland_satellite();
        }
    }

    pub(super) fn kill_xwayland_satellite(&mut self) {
        if let Some(ref mut child) = self.xwayland_child {
            if let Err(e) = child.kill() {
                tracing::warn!("failed to kill xwayland-satellite: {e:#}");
            }
        }
        self.xwayland_child = None;
        self.x11_display = None;
    }

    fn init_wayland_listener(
        display: Display<DomeState>,
        event_loop: &mut EventLoop<'static, CalloopData>,
    ) -> Result<()> {
        let listening_socket = ListeningSocketSource::new_auto()?;
        let socket_name = listening_socket.socket_name().to_os_string();
        // SAFETY: called before any threads are spawned (single-threaded init)
        unsafe { std::env::set_var("WAYLAND_DISPLAY", &socket_name) };

        let loop_handle = event_loop.handle();

        loop_handle.insert_source(listening_socket, |client_stream, _, data| {
            data.state
                .display_handle
                .insert_client(client_stream, Arc::new(ClientState::default()))
                .ok();
        })?;

        loop_handle.insert_source(
            Generic::new(display, calloop::Interest::READ, calloop::Mode::Level),
            |_, display, data| {
                unsafe {
                    display.get_mut().dispatch_clients(&mut data.state).ok();
                }
                Ok(calloop::PostAction::Continue)
            },
        )?;

        tracing::info!("Wayland socket: {:?}", socket_name);
        Ok(())
    }

    pub(super) fn get_output(&self) -> Option<Output> {
        self.space.outputs().next().cloned()
    }

    pub(super) fn full_output_bounds(&self) -> (f64, f64) {
        self.space.outputs()
            .filter_map(|o| self.space.output_geometry(o))
            .fold((0.0, 0.0), |(w, h), geo| {
                (
                    ((geo.loc.x + geo.size.w) as f64).max(w),
                    ((geo.loc.y + geo.size.h) as f64).max(h),
                )
            })
    }

    pub(super) fn monitor_at(&self, pos: smithay::utils::Point<f64, smithay::utils::Logical>) -> Option<MonitorId> {
        for (&monitor_id, output) in &self.monitor_outputs {
            if let Some(geo) = self.space.output_geometry(output) {
                if geo.to_f64().contains(pos) {
                    return Some(monitor_id);
                }
            }
        }
        None
    }

    pub(super) fn update_usable_area(&mut self) {
        for (&monitor_id, output) in &self.monitor_outputs {
            let zone = layer_map_for_output(output).non_exclusive_zone();
            self.hub.update_monitor(monitor_id, Dimension::new(
                Length::new(zone.loc.x as f32),
                Length::new(zone.loc.y as f32),
                Length::new(zone.size.w as f32),
                Length::new(zone.size.h as f32),
            ), 1.0f32);
        }
        self.sync_window_positions();
    }

    pub(super) fn sync_window_positions(&mut self) {
        let placements = self.hub.get_visible_placements();

        let b = self.config.border_size;

        // Flatten placements into per-window info
        struct WinInfo {
            id: WindowId,
            monitor_id: MonitorId,
            frame: Dimension,
            is_float: bool,
            is_fullscreen: bool,
            is_focused: bool,
        }
        let focused_window = placements.focused_window;
        let mut wins: Vec<WinInfo> = Vec::new();
        let mut visible = HashSet::new();
        for mp in &placements.monitors {
            match &mp.layout {
                crate::core::MonitorLayout::Normal { tiling_windows, float_windows, containers: _ } => {
                    for wp in tiling_windows {
                        visible.insert(wp.id);
                        wins.push(WinInfo {
                            id: wp.id,
                            monitor_id: mp.monitor_id,
                            frame: wp.frame,
                            is_float: false,
                            is_fullscreen: false,
                            is_focused: Some(wp.id) == focused_window,
                        });
                    }
                    for wp in float_windows {
                        visible.insert(wp.id);
                        wins.push(WinInfo {
                            id: wp.id,
                            monitor_id: mp.monitor_id,
                            frame: wp.frame,
                            is_float: true,
                            is_fullscreen: false,
                            is_focused: Some(wp.id) == focused_window,
                        });
                    }
                }
                crate::core::MonitorLayout::Fullscreen(window_id) => {
                    visible.insert(*window_id);
                    wins.push(WinInfo {
                        id: *window_id,
                        monitor_id: mp.monitor_id,
                        frame: Dimension::default(),
                        is_float: false,
                        is_fullscreen: true,
                        is_focused: true,
                    });
                }
            }
        }
        self.visible_windows = visible;

        // Z-order: tiled (0), float (1), fullscreen (2)
        wins.sort_by_key(|w| if w.is_fullscreen { 2u8 } else if w.is_float { 1 } else { 0 });

        for wi in &wins {
            let Some(window) = self.window_map.get(&wi.id) else { continue };
            let output_pos = self.monitor_outputs.get(&wi.monitor_id)
                .and_then(|o| self.space.output_geometry(o))
                .map(|g| g.loc)
                .unwrap_or_default();

            let (content_x, content_y, content_w, content_h) = if wi.is_fullscreen {
                let output_size = self.monitor_outputs.get(&wi.monitor_id)
                    .and_then(|o| o.current_mode().map(|m| m.size))
                    .unwrap_or((1280, 720).into());
                (output_pos.x, output_pos.y, output_size.w, output_size.h)
            } else {
                (
                    output_pos.x + (wi.frame.x + Length::new(b)).value() as i32,
                    output_pos.y + (wi.frame.y + Length::new(b)).value() as i32,
                    (wi.frame.width - Length::new(2.0 * b)).max(Length::ZERO).value() as i32,
                    (wi.frame.height - Length::new(2.0 * b)).max(Length::ZERO).value() as i32,
                )
            };

            let output_size = self.monitor_outputs.get(&wi.monitor_id)
                .and_then(|o| o.current_mode().map(|m| m.size))
                .unwrap_or((1280, 720).into());
            let bounds: smithay::utils::Size<i32, smithay::utils::Logical> =
                (output_size.w, output_size.h).into();

            window.set_activated(wi.is_focused);
            if let Some(toplevel) = window.toplevel() {
                toplevel.with_pending_state(|state| {
                    state.size = Some((content_w, content_h).into());
                    state.bounds = Some(bounds);
                    set_toplevel_layout_states(&mut state.states, wi.is_fullscreen, wi.is_float);
                });
                toplevel.send_configure();
            }
            self.space.map_element(window.clone(), (content_x, content_y), false);
        }

        // Unmap windows not in any visible placement
        for (&window_id, window) in &self.window_map {
            if !self.visible_windows.contains(&window_id) {
                self.space.unmap_elem(window);
            }
        }

        self.space.refresh();
        self.sync_keyboard_focus(focused_window);
    }

    pub(super) fn sync_keyboard_focus(&mut self, focused: Option<WindowId>) {
        use super::focus::KeyboardFocusTarget;

        // By design, when a container is focused (via focus_parent), keyboard
        // focus is cleared. The user must focus a specific window to resume
        // keyboard input. The hub already collapses container focus to None in
        // VisiblePlacements.focused_window, so no Child::Container arm is needed here.
        let target = match focused {
            Some(id) => self.window_map.get(&id),
            None => None,
        }
        .and_then(|window| window.toplevel())
        .map(|toplevel| KeyboardFocusTarget::Surface(toplevel.wl_surface().clone()));

        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        keyboard.set_focus(self, target, serial);
    }
}

/// Sets xdg_toplevel layout states based on the window's display mode.
///
/// These states tell clients how the WM is managing the window, so they can
/// adjust their rendering:
/// - Tiled (Left/Right/Top/Bottom): the edge is adjacent to another window or
///   the screen edge. Clients should not draw shadows or rounded corners on
///   tiled edges. In a tiling WM, all 4 edges are tiled.
/// - Fullscreen: the window covers the entire output. Clients hide decorations.
/// - Float: no tiled/fullscreen states. Clients draw full decorations.
fn set_toplevel_layout_states(
    states: &mut smithay::wayland::shell::xdg::ToplevelStateSet,
    is_fullscreen: bool,
    is_float: bool,
) {
    use smithay::reexports::wayland_protocols::xdg::shell::server::xdg_toplevel::State;

    let tiled = !is_fullscreen && !is_float;
    if is_fullscreen { states.set(State::Fullscreen); } else { states.unset(State::Fullscreen); }
    if tiled { states.set(State::Maximized); } else { states.unset(State::Maximized); }
    if tiled { states.set(State::TiledLeft); } else { states.unset(State::TiledLeft); }
    if tiled { states.set(State::TiledRight); } else { states.unset(State::TiledRight); }
    if tiled { states.set(State::TiledTop); } else { states.unset(State::TiledTop); }
    if tiled { states.set(State::TiledBottom); } else { states.unset(State::TiledBottom); }
}

#[derive(Default)]
pub(super) struct ClientState {
    pub(super) compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}
