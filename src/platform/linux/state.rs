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
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::shm::ShmState;
use smithay::wayland::socket::ListeningSocketSource;

use crate::config::Config;
use crate::core::{Child, Dimension, Hub, WindowId};

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
    pub(super) on_open_done: HashSet<WindowId>,

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

    // Backend-specific
    pub(super) winit_data: Option<WinitBackendData>,
    pub(super) udev_data: Option<UdevData>,

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

        let mut seat: Seat<Self> = seat_state.new_wl_seat(&dh, "dome");
        seat.add_keyboard(Default::default(), 200, 25)?;
        seat.add_pointer();

        let space = Space::default();
        Self::init_wayland_listener(display, event_loop)?;
        let loop_signal = event_loop.get_signal();
        let loop_handle = event_loop.handle();

        let screen = Dimension {
            x: 0.0,
            y: 0.0,
            width: 1280.0,
            height: 720.0,
        };
        let hub = Hub::new(screen, config.clone().into());

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
            on_open_done: HashSet::new(),
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
            winit_data: None,
            udev_data: None,
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

    pub(super) fn full_output_size(&self) -> (f64, f64) {
        self.get_output()
            .and_then(|o| o.current_mode().map(|m| (m.size.w as f64, m.size.h as f64)))
            .unwrap_or((1280.0, 720.0))
    }

    pub(super) fn update_usable_area(&mut self) {
        let Some(output) = self.get_output() else { return };
        let zone = layer_map_for_output(&output).non_exclusive_zone();
        let monitor_id = self.hub.focused_monitor();
        self.hub.update_monitor_dimension(monitor_id, Dimension {
            x: zone.loc.x as f32,
            y: zone.loc.y as f32,
            width: zone.size.w as f32,
            height: zone.size.h as f32,
        });
        self.sync_window_positions();
    }

    pub(super) fn sync_window_positions(&mut self) {
        let placements = self.hub.get_visible_placements();

        let mut visible = HashSet::new();
        let mut float_ids = HashSet::new();
        let mut fullscreen_ids = HashSet::new();
        let mut focused_id = None;
        for mp in &placements {
            match &mp.layout {
                crate::core::MonitorLayout::Normal { windows, .. } => {
                    for wp in windows {
                        visible.insert(wp.id);
                        if wp.is_float {
                            float_ids.insert(wp.id);
                        }
                        if wp.is_focused {
                            focused_id = Some(wp.id);
                        }
                    }
                }
                crate::core::MonitorLayout::Fullscreen(window_id) => {
                    visible.insert(*window_id);
                    fullscreen_ids.insert(*window_id);
                    focused_id = Some(*window_id);
                }
            }
        }
        self.visible_windows = visible;

        let b = self.config.border_size;
        let screen = self.hub.get_monitor(self.hub.focused_monitor()).dimension();
        let bounds: smithay::utils::Size<i32, smithay::utils::Logical> =
            (screen.width as i32, screen.height as i32).into();

        // Get full output size for fullscreen (includes area behind layer surfaces)
        let output_size = self.get_output()
            .and_then(|o| o.current_mode().map(|m| m.size))
            .unwrap_or((screen.width as i32, screen.height as i32).into());

        // Map tiled windows first, then floats, then fullscreen on top
        for pass in 0..3 {
            for (&window_id, window) in &self.window_map {
                let is_float = float_ids.contains(&window_id);
                let is_fullscreen = fullscreen_ids.contains(&window_id);
                let target_pass = if is_fullscreen { 2 } else if is_float { 1 } else { 0 };
                if pass != target_pass {
                    continue;
                }
                if self.visible_windows.contains(&window_id) {
                    let (content_x, content_y, content_w, content_h) = if is_fullscreen {
                        (0, 0, output_size.w, output_size.h)
                    } else {
                        let dim = self.hub.get_window(window_id).dimension();
                        (
                            (dim.x + b) as i32,
                            (dim.y + b) as i32,
                            (dim.width - 2.0 * b).max(0.0) as i32,
                            (dim.height - 2.0 * b).max(0.0) as i32,
                        )
                    };
                    let is_focused = focused_id == Some(window_id);
                    window.set_activated(is_focused);
                    if let Some(toplevel) = window.toplevel() {
                        toplevel.with_pending_state(|state| {
                            state.size = Some((content_w, content_h).into());
                            state.bounds = Some(bounds);
                            set_toplevel_layout_states(&mut state.states, is_fullscreen, is_float);
                        });
                        toplevel.send_configure();
                    }
                    self.space.map_element(window.clone(), (content_x, content_y), false);
                } else {
                    self.space.unmap_elem(window);
                }
            }
        }

        self.space.refresh();
        self.sync_keyboard_focus();
    }

    pub(super) fn sync_keyboard_focus(&mut self) {
        use super::focus::KeyboardFocusTarget;

        // By design, when a container is focused (via focus_parent), keyboard
        // focus is cleared. The user must focus a specific window to resume
        // keyboard input.
        let ws = self.hub.get_workspace(self.hub.current_workspace());
        let target = match ws.focused() {
            Some(Child::Window(id)) => self.window_map.get(&id),
            _ => None,
        }
        .and_then(|window| window.toplevel())
        .map(|toplevel| KeyboardFocusTarget::Surface(toplevel.wl_surface().clone()));

        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        keyboard.set_focus(self, target, serial);
    }

    /// Runs on_open config rules for a toplevel surface (one-shot per window).
    /// Called from both new_toplevel and commit, since clients may set
    /// app_id/title after the initial map.
    pub(super) fn try_on_open_rules(&mut self, surface: &WlSurface) {
        if self.config.linux.on_open.is_empty() {
            return;
        }
        let Some(window_id) = with_states(surface, |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        }) else {
            return;
        };
        if self.on_open_done.contains(&window_id) {
            return;
        }
        let (app_id, title) = with_states(surface, |states| {
            let data = states
                .data_map
                .get::<smithay::wayland::shell::xdg::XdgToplevelSurfaceData>()
                .unwrap();
            let guard = data.lock().unwrap();
            (guard.app_id.clone(), guard.title.clone())
        });
        // Wait until at least one identifier is available
        if app_id.is_none() && title.is_none() {
            return;
        }
        self.on_open_done.insert(window_id);
        if let Some(rule) = self.config.linux.on_open.iter().find(|r| {
            r.window.matches(app_id.as_deref(), title.as_deref())
        }) {
            tracing::debug!(%window_id, ?app_id, ?title, actions = %rule.run, "Running on_open actions");
            let actions = rule.run.clone();
            for action in &actions {
                self.handle_action(action);
            }
        }
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
