use std::collections::{HashMap, HashSet};
use std::io::Write;
use std::os::fd::{AsRawFd, RawFd};
use std::os::linux::net::SocketAddrExt;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::os::unix::net::{SocketAddr, UnixListener};
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

use anyhow::Result;
use calloop::generic::Generic;
use calloop::{EventLoop, LoopSignal};
use smithay::desktop::{PopupManager, Space, Window, layer_map_for_output};
use smithay::input::{Seat, SeatState};
use smithay::output::Output;
use smithay::reexports::wayland_server::backend::{ClientData, ClientId, DisconnectReason};
use smithay::reexports::wayland_server::{Display, DisplayHandle};
use smithay::utils::{Logical, Point, Rectangle};
use smithay::wayland::compositor::{CompositorClientState, CompositorState};
use smithay::wayland::dmabuf::{DmabufGlobal, DmabufState};
use smithay::wayland::output::OutputManagerState;
use smithay::wayland::selection::data_device::DataDeviceState;
use smithay::wayland::selection::primary_selection::PrimarySelectionState;
use smithay::wayland::selection::wlr_data_control::DataControlState;
use smithay::wayland::shell::wlr_layer::WlrLayerShellState;
use smithay::wayland::shell::xdg::XdgShellState;
use smithay::wayland::shell::xdg::decoration::XdgDecorationState;
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
    pub(super) data_control_state: DataControlState,
    pub(super) popups: PopupManager,
    pub(super) seat: Seat<Self>,
    pub(super) cursor_status: smithay::input::pointer::CursorImageStatus,
    pub(super) layer_shell_state: WlrLayerShellState,
    pub(super) dmabuf_state: DmabufState,
    pub(super) dmabuf_global: Option<DmabufGlobal>,

    // Backend-specific
    pub(super) winit_data: Option<WinitBackendData>,
    pub(super) udev_data: Option<UdevData>,

    // Children spawned by Action::Exec. Dome keeps each handle so it can reap the
    // process, because it leaves SIGCHLD at its default disposition.
    pub(super) exec_children: Vec<std::process::Child>,

    // XWayland via xwayland-satellite
    pub(super) x11_display: Option<String>,
    pub(super) xwayland_child: Option<std::process::Child>,
    // Dome owns the X11 lock file and filesystem socket, so it removes them on teardown.
    pub(super) x11_lock_path: Option<PathBuf>,
    pub(super) x11_socket_path: Option<PathBuf>,

    // Name of Dome's own Wayland socket (e.g. "wayland-1"). Dome does not export
    // this into its own process environment. It is injected per-spawn into
    // launched clients and xwayland-satellite to point them at Dome.
    pub(super) wayland_socket_name: String,

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
        let data_control_state =
            DataControlState::new::<Self, _>(&dh, Some(&primary_selection_state), |_| true);
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
        let wayland_socket_name = Self::init_wayland_listener(display, event_loop)?
            .to_string_lossy()
            .into_owned();
        let loop_signal = event_loop.get_signal();
        let loop_handle = event_loop.handle();

        let primary = crate::core::ReportedMonitor {
            device_name: "primary".to_string(),
            work_area: crate::core::PixelRect::new(0, 0, 1280, 720),
            scale: 1.0,
            cg_display_id: None,
            gdi_device: None,
        };
        let hub = Hub::new(primary, GlobalLayoutConfig::from(&config), Vec::new());

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
            data_control_state,
            popups,
            seat,
            cursor_status: smithay::input::pointer::CursorImageStatus::default_named(),
            layer_shell_state,
            dmabuf_state,
            dmabuf_global: None,
            winit_data: None,
            udev_data: None,
            exec_children: Vec::new(),
            x11_display: None,
            xwayland_child: None,
            x11_lock_path: None,
            x11_socket_path: None,
            wayland_socket_name,
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
        // xwayland-satellite 0.8.2 never reports its display back to the parent,
        // so Dome allocates the X11 display and its listening sockets itself and
        // hands them to satellite via -listenfd.
        let Some((display_name, abstract_listener, unix_listener, lock_path, socket_path)) =
            allocate_x11_sockets()
        else {
            tracing::warn!("could not allocate an X11 display. X11 apps won't work.");
            return;
        };

        // satellite must inherit the socket fds across exec.
        for fd in [abstract_listener.as_raw_fd(), unix_listener.as_raw_fd()] {
            if let Err(e) = clear_cloexec(fd) {
                tracing::warn!("failed to clear close-on-exec on X11 socket fd: {e}");
                remove_x11_file(&lock_path);
                remove_x11_file(&socket_path);
                return;
            }
        }

        let mut command = std::process::Command::new("xwayland-satellite");
        command.arg(&display_name);
        // Headless software GL can segfault Xwayland's default glamor path, so
        // DOME_XWAYLAND_GLAMOR (for example "none") lets a headless smoke force the
        // Xwayland renderer. Unset leaves satellite's default glamor GL.
        if let Ok(glamor) = std::env::var("DOME_XWAYLAND_GLAMOR") {
            if !glamor.is_empty() {
                command.arg("-glamor").arg(glamor);
            }
        }
        command
            .arg("-listenfd")
            .arg(abstract_listener.as_raw_fd().to_string())
            .arg("-listenfd")
            .arg(unix_listener.as_raw_fd().to_string())
            .env("WAYLAND_DISPLAY", &self.wayland_socket_name)
            .stderr(std::process::Stdio::inherit());
        // Xwayland reaps an xkbcomp child to build the keymap, so it needs SIGCHLD at
        // its default disposition rather than whatever the parent happens to hold.
        unsafe {
            command.pre_exec(|| {
                if libc::signal(libc::SIGCHLD, libc::SIG_DFL) == libc::SIG_ERR {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = match command.spawn() {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("xwayland-satellite not available: {e}. X11 apps won't work.");
                remove_x11_file(&lock_path);
                remove_x11_file(&socket_path);
                return;
            }
        };

        tracing::info!("xwayland-satellite started with DISPLAY={display_name}");
        self.x11_display = Some(display_name);
        self.xwayland_child = Some(child);
        self.x11_lock_path = Some(lock_path);
        self.x11_socket_path = Some(socket_path);
        // Dropping the listeners here closes Dome's copies of the fds, so later
        // spawned clients do not inherit them. satellite keeps its own copies.
    }

    pub(super) fn respawn_xwayland_satellite(&mut self) {
        if let Some(ref mut child) = self.xwayland_child {
            match child.try_wait() {
                Ok(None) => return, // still running
                Ok(Some(status)) => {
                    tracing::warn!("xwayland-satellite exited: {status}, respawning");
                }
                Err(_) => {
                    tracing::warn!("xwayland-satellite was reaped elsewhere, respawning");
                }
            }
            self.xwayland_child = None;
            self.x11_display = None;
            if let Some(lock) = self.x11_lock_path.take() {
                remove_x11_file(&lock);
            }
            if let Some(socket) = self.x11_socket_path.take() {
                remove_x11_file(&socket);
            }
            self.spawn_xwayland_satellite();
        }
    }

    /// Collect the Action::Exec children that have finished, so they do not linger as
    /// zombies for the compositor's lifetime.
    pub(super) fn reap_exec_children(&mut self) {
        if self.exec_children.is_empty() {
            return;
        }
        self.exec_children
            .retain_mut(|child| match child.try_wait() {
                Ok(None) => true,
                Ok(Some(_)) => false,
                Err(e) => {
                    tracing::warn!("failed to wait on an exec'd child: {e}");
                    false
                }
            });
    }

    pub(super) fn shutdown(&mut self) {
        self.kill_xwayland_satellite();
        crate::ipc::remove_socket_file();
    }

    pub(super) fn kill_xwayland_satellite(&mut self) {
        if let Some(ref mut child) = self.xwayland_child {
            if let Err(e) = child.kill() {
                tracing::warn!("failed to kill xwayland-satellite: {e:#}");
            }
        }
        self.xwayland_child = None;
        self.x11_display = None;
        if let Some(lock) = self.x11_lock_path.take() {
            remove_x11_file(&lock);
        }
        if let Some(socket) = self.x11_socket_path.take() {
            remove_x11_file(&socket);
        }
    }

    fn init_wayland_listener(
        display: Display<DomeState>,
        event_loop: &mut EventLoop<'static, CalloopData>,
    ) -> Result<std::ffi::OsString> {
        let listening_socket = ListeningSocketSource::new_auto()?;
        let socket_name = listening_socket.socket_name().to_os_string();

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
        Ok(socket_name)
    }

    pub(super) fn get_output(&self) -> Option<Output> {
        self.space.outputs().next().cloned()
    }

    pub(super) fn query_json(&self, query: crate::action::Query) -> String {
        use crate::action::{MinimizedWindow, Query};
        match query {
            Query::Workspaces => serde_json::to_string(&self.hub.query_workspaces())
                .expect("WorkspaceInfo is infallibly serializable"),
            Query::Monitors => serde_json::to_string(&self.hub.query_monitors())
                .expect("MonitorDetails is infallibly serializable"),
            Query::MinimizedWindows => {
                let entries: Vec<MinimizedWindow> = self
                    .hub
                    .minimized_window_entries()
                    .into_iter()
                    .map(|e| MinimizedWindow {
                        id: e.id,
                        title: e.title,
                        app_name: e.app_name,
                        bundle_id: e.bundle_id,
                        executable_path: e.executable_path,
                    })
                    .collect();
                serde_json::to_string(&entries).expect("MinimizedWindow is infallibly serializable")
            }
        }
    }

    pub(super) fn full_output_bounds(&self) -> (f64, f64) {
        self.space
            .outputs()
            .filter_map(|o| self.space.output_geometry(o))
            .fold((0.0, 0.0), |(w, h), geo| {
                (
                    ((geo.loc.x + geo.size.w) as f64).max(w),
                    ((geo.loc.y + geo.size.h) as f64).max(h),
                )
            })
    }

    /// Clamp a pointer position to the union of output rectangles. When the raw
    /// position lands in a gap with no output behind it (for example the empty
    /// strip below a shorter monitor in a mixed-height row), snap it to the
    /// nearest point on the nearest output instead of leaving it in dead space.
    pub(super) fn clamp_pointer(
        &self,
        pos: smithay::utils::Point<f64, smithay::utils::Logical>,
    ) -> smithay::utils::Point<f64, smithay::utils::Logical> {
        if self.monitor_at(pos).is_some() {
            return pos;
        }
        let rectangles = self
            .space
            .outputs()
            .filter_map(|output| self.space.output_geometry(output))
            .map(|geometry| geometry.to_f64());
        nearest_point_in_rectangles(pos, rectangles).unwrap_or(pos)
    }

    pub(super) fn monitor_at(
        &self,
        pos: smithay::utils::Point<f64, smithay::utils::Logical>,
    ) -> Option<MonitorId> {
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
            let reported = crate::core::ReportedMonitor {
                device_name: output.name(),
                work_area: crate::core::PixelRect::new(
                    zone.loc.x,
                    zone.loc.y,
                    zone.size.w,
                    zone.size.h,
                ),
                scale: 1.0,
                cg_display_id: None,
                gdi_device: None,
            };
            self.hub.update_monitor(monitor_id, reported, None);
        }
        self.sync_window_positions();
    }

    pub(super) fn sync_window_positions(&mut self) {
        let placements = self.hub.get_visible_placements();

        let b = Length::from_pixels(self.config.border_size).logical();

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
                crate::core::MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    containers: _,
                } => {
                    for wp in tiling_windows {
                        visible.insert(wp.id);
                        wins.push(WinInfo {
                            id: wp.id,
                            monitor_id: mp.monitor_id,
                            frame: wp.border_box.to_dimension(),
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
                            frame: wp.border_box.to_dimension(),
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
        wins.sort_by_key(|w| {
            if w.is_fullscreen {
                2u8
            } else if w.is_float {
                1
            } else {
                0
            }
        });

        for wi in &wins {
            let Some(window) = self.window_map.get(&wi.id) else {
                continue;
            };
            let output_pos = self
                .monitor_outputs
                .get(&wi.monitor_id)
                .and_then(|o| self.space.output_geometry(o))
                .map(|g| g.loc)
                .unwrap_or_default();

            let (content_x, content_y, content_w, content_h) = if wi.is_fullscreen {
                let output_size = self
                    .monitor_outputs
                    .get(&wi.monitor_id)
                    .and_then(|o| o.current_mode().map(|m| m.size))
                    .unwrap_or((1280, 720).into());
                (output_pos.x, output_pos.y, output_size.w, output_size.h)
            } else {
                (
                    output_pos.x + (wi.frame.x + Length::new(b)).value() as i32,
                    output_pos.y + (wi.frame.y + Length::new(b)).value() as i32,
                    (wi.frame.width - Length::new(2.0 * b))
                        .max(Length::ZERO)
                        .value() as i32,
                    (wi.frame.height - Length::new(2.0 * b))
                        .max(Length::ZERO)
                        .value() as i32,
                )
            };

            let output_size = self
                .monitor_outputs
                .get(&wi.monitor_id)
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
            self.space
                .map_element(window.clone(), (content_x, content_y), false);
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
    if is_fullscreen {
        states.set(State::Fullscreen);
    } else {
        states.unset(State::Fullscreen);
    }
    if tiled {
        states.set(State::Maximized);
    } else {
        states.unset(State::Maximized);
    }
    if tiled {
        states.set(State::TiledLeft);
    } else {
        states.unset(State::TiledLeft);
    }
    if tiled {
        states.set(State::TiledRight);
    } else {
        states.unset(State::TiledRight);
    }
    if tiled {
        states.set(State::TiledTop);
    } else {
        states.unset(State::TiledTop);
    }
    if tiled {
        states.set(State::TiledBottom);
    } else {
        states.unset(State::TiledBottom);
    }
}

/// Allocate an X11 display number and bind its listening sockets, following the
/// X11 display lock protocol: a `/tmp/.X<N>-lock` file plus the abstract and
/// filesystem sockets at `/tmp/.X11-unix/X<N>`. Returns the display name, both
/// listeners, and the lock and socket paths to remove on teardown.
fn allocate_x11_sockets() -> Option<(String, UnixListener, UnixListener, PathBuf, PathBuf)> {
    ensure_x11_unix_dir();

    for n in 0u32..64 {
        let lock_path = PathBuf::from(format!("/tmp/.X{n}-lock"));
        let mut lock_file = match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o444)
            .open(&lock_path)
        {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                tracing::warn!("failed to create X11 lock file {lock_path:?}: {e}");
                continue;
            }
        };
        // The lock records the owner PID in the historic 10-wide format.
        if let Err(e) = writeln!(lock_file, "{:>10}", std::process::id()) {
            tracing::warn!("failed to write PID to X11 lock file {lock_path:?}: {e}");
            remove_x11_file(&lock_path);
            continue;
        }

        let socket_name = format!("/tmp/.X11-unix/X{n}");
        let socket_path = PathBuf::from(&socket_name);
        // Remove a leftover socket from a crashed session, or bind fails.
        remove_x11_file(&socket_path);

        let abstract_addr = match SocketAddr::from_abstract_name(socket_name.as_bytes()) {
            Ok(addr) => addr,
            Err(e) => {
                tracing::warn!("failed to build abstract X11 socket address: {e}");
                remove_x11_file(&lock_path);
                continue;
            }
        };
        let abstract_listener = match UnixListener::bind_addr(&abstract_addr) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("failed to bind abstract X11 socket for :{n}: {e}");
                remove_x11_file(&lock_path);
                continue;
            }
        };
        let unix_listener = match UnixListener::bind(&socket_path) {
            Ok(l) => l,
            Err(e) => {
                tracing::warn!("failed to bind X11 socket {socket_path:?}: {e}");
                remove_x11_file(&lock_path);
                continue;
            }
        };

        return Some((
            format!(":{n}"),
            abstract_listener,
            unix_listener,
            lock_path,
            socket_path,
        ));
    }

    tracing::warn!("no free X11 display found in :0..:64");
    None
}

/// Ensure `/tmp/.X11-unix` exists with the sticky, world-writable perms X uses.
fn ensure_x11_unix_dir() {
    match std::fs::create_dir("/tmp/.X11-unix") {
        Ok(()) => {
            if let Err(e) =
                std::fs::set_permissions("/tmp/.X11-unix", std::fs::Permissions::from_mode(0o1777))
            {
                tracing::warn!("failed to set /tmp/.X11-unix permissions: {e}");
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => tracing::warn!("failed to create /tmp/.X11-unix: {e}"),
    }
}

/// The point inside the union of `rectangles` closest to `pos`, or `None` when there
/// are no rectangles. A `pos` already inside one of them comes back unchanged. Ties
/// go to the first rectangle in iteration order.
fn nearest_point_in_rectangles(
    pos: Point<f64, Logical>,
    rectangles: impl IntoIterator<Item = Rectangle<f64, Logical>>,
) -> Option<Point<f64, Logical>> {
    let mut best: Option<(f64, Point<f64, Logical>)> = None;
    for geometry in rectangles {
        let candidate: Point<f64, Logical> = (
            pos.x
                .clamp(geometry.loc.x, geometry.loc.x + geometry.size.w),
            pos.y
                .clamp(geometry.loc.y, geometry.loc.y + geometry.size.h),
        )
            .into();
        let distance = (candidate.x - pos.x).powi(2) + (candidate.y - pos.y).powi(2);
        if best.as_ref().is_none_or(|&(closest, _)| distance < closest) {
            best = Some((distance, candidate));
        }
    }
    best.map(|(_, point)| point)
}

/// Clear close-on-exec so a spawned child inherits the fd across exec.
fn clear_cloexec(fd: RawFd) -> std::io::Result<()> {
    // SAFETY: fd is a live socket fd owned by a UnixListener in scope.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
    if flags < 0 {
        return Err(std::io::Error::last_os_error());
    }
    // SAFETY: the same fd, written with close-on-exec cleared.
    let ret = unsafe { libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) };
    if ret < 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

fn remove_x11_file(path: &std::path::Path) {
    if let Err(e) = std::fs::remove_file(path) {
        if e.kind() != std::io::ErrorKind::NotFound {
            tracing::warn!("failed to remove X11 file {path:?}: {e}");
        }
    }
}

#[derive(Default)]
pub(super) struct ClientState {
    pub(super) compositor_state: CompositorClientState,
}

impl ClientData for ClientState {
    fn initialized(&self, _client_id: ClientId) {}
    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

#[cfg(test)]
mod tests {
    use super::nearest_point_in_rectangles;
    use smithay::utils::{Logical, Point, Rectangle};

    fn rectangle(x: f64, y: f64, w: f64, h: f64) -> Rectangle<f64, Logical> {
        Rectangle::new((x, y).into(), (w, h).into())
    }

    fn point(x: f64, y: f64) -> Point<f64, Logical> {
        (x, y).into()
    }

    #[test]
    fn no_rectangles_yields_nothing() {
        assert_eq!(nearest_point_in_rectangles(point(5.0, 5.0), []), None);
    }

    #[test]
    fn a_point_inside_comes_back_unchanged() {
        let inside = point(640.0, 400.0);
        assert_eq!(
            nearest_point_in_rectangles(inside, [rectangle(0.0, 0.0, 1280.0, 800.0)]),
            Some(inside)
        );
    }

    #[test]
    fn a_point_past_one_edge_keeps_its_other_axis() {
        assert_eq!(
            nearest_point_in_rectangles(point(-50.0, 400.0), [rectangle(0.0, 0.0, 1280.0, 800.0)]),
            Some(point(0.0, 400.0))
        );
    }

    #[test]
    fn a_point_past_a_corner_clamps_both_axes() {
        assert_eq!(
            nearest_point_in_rectangles(point(2000.0, 900.0), [rectangle(0.0, 0.0, 1280.0, 800.0)]),
            Some(point(1280.0, 800.0))
        );
    }

    // Two side-by-side outputs of different heights leave an L-shaped union. A pointer
    // below the shorter one is outside every output, and it must land on the nearest
    // one rather than stay where it is.
    #[test]
    fn a_point_below_the_shorter_output_lands_on_that_output() {
        let tall = rectangle(0.0, 0.0, 1000.0, 1000.0);
        let short = rectangle(1000.0, 0.0, 1000.0, 500.0);
        assert_eq!(
            nearest_point_in_rectangles(point(1500.0, 700.0), [tall, short]),
            Some(point(1500.0, 500.0))
        );
    }

    #[test]
    fn a_point_in_a_gap_picks_the_nearer_rectangle() {
        let left = rectangle(0.0, 0.0, 100.0, 100.0);
        let right = rectangle(300.0, 0.0, 100.0, 100.0);
        assert_eq!(
            nearest_point_in_rectangles(point(280.0, 50.0), [left, right]),
            Some(point(300.0, 50.0))
        );
        assert_eq!(
            nearest_point_in_rectangles(point(120.0, 50.0), [left, right]),
            Some(point(100.0, 50.0))
        );
    }

    #[test]
    fn an_equidistant_point_takes_the_first_rectangle() {
        let left = rectangle(0.0, 0.0, 100.0, 100.0);
        let right = rectangle(200.0, 0.0, 100.0, 100.0);
        assert_eq!(
            nearest_point_in_rectangles(point(150.0, 50.0), [left, right]),
            Some(point(100.0, 50.0))
        );
    }
}
