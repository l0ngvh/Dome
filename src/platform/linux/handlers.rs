use smithay::delegate_compositor;
use smithay::delegate_data_device;
use smithay::delegate_layer_shell;
use smithay::delegate_output;
use smithay::delegate_primary_selection;
use smithay::delegate_seat;
use smithay::delegate_shm;
use smithay::delegate_xdg_shell;
use smithay::delegate_xdg_decoration;
use smithay::desktop::{LayerSurface, PopupKind, PopupUngrabStrategy, Window, find_popup_root_surface, layer_map_for_output};
use smithay::input::{Seat, SeatHandler, SeatState};
use smithay::input::pointer::Focus;
use smithay::output::Output;
use smithay::reexports::wayland_server::protocol::wl_buffer::WlBuffer;
use smithay::reexports::wayland_server::protocol::wl_output::WlOutput;
use smithay::reexports::wayland_server::protocol::wl_seat::WlSeat;
use smithay::reexports::wayland_server::protocol::wl_surface::WlSurface;
use smithay::utils::Serial;
use smithay::wayland::buffer::BufferHandler;
use smithay::wayland::compositor::{CompositorClientState, CompositorHandler, CompositorState};
use smithay::wayland::output::OutputHandler;
use smithay::wayland::selection::data_device::{
    ClientDndGrabHandler, DataDeviceHandler, DataDeviceState, ServerDndGrabHandler,
};
use smithay::wayland::selection::primary_selection::{PrimarySelectionHandler, PrimarySelectionState};
use smithay::wayland::selection::SelectionHandler;
use smithay::wayland::shell::xdg::{
    PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
};
use smithay::wayland::shell::xdg::decoration::{XdgDecorationHandler, XdgDecorationState};
use smithay::reexports::wayland_protocols::xdg::decoration::zv1::server::zxdg_toplevel_decoration_v1::Mode as DecorationMode;
use smithay::wayland::shell::wlr_layer::{self, Layer, WlrLayerShellHandler, WlrLayerShellState};
use smithay::wayland::shm::{ShmHandler, ShmState};

use smithay::wayland::compositor::with_states;

use super::focus::{KeyboardFocusTarget, PointerFocusTarget};
use super::state::{ClientState, DomeState};

impl CompositorHandler for DomeState {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a smithay::reexports::wayland_server::Client) -> &'a CompositorClientState {
        &client.get_data::<ClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        smithay::backend::renderer::utils::on_commit_buffer_handler::<Self>(surface);

        // Ensure initial configure is sent for toplevels
        if let Some(toplevel) = self.xdg_shell_state.toplevel_surfaces().iter().find(|t| t.wl_surface() == surface).cloned() {
            if !toplevel.is_initial_configure_sent() {
                let dim = with_states(surface, |states| {
                    states.data_map.get::<crate::core::WindowId>().copied()
                })
                .map(|id| self.hub.get_window(id).dimension());

                if let Some(dim) = dim {
                    let b = self.config.border_size;
                    toplevel.with_pending_state(|state| {
                        state.size = Some((
                            (dim.width - 2.0 * b).max(0.0) as i32,
                            (dim.height - 2.0 * b).max(0.0) as i32,
                        ).into());
                    });
                }
                toplevel.send_configure();
            }
        }

        // Ensure initial configure is sent for popups
        if let Some(PopupKind::Xdg(ref popup_surface)) = self.popups.find_popup(surface) {
            if !popup_surface.is_initial_configure_sent() {
                popup_surface.send_configure().ok();
            }
        }

        if let Some(window) = self.space.elements().find(|w| w.toplevel().map(|t| t.wl_surface() == surface).unwrap_or(false)).cloned() {
            window.on_commit();
        }

        // Read client-requested size constraints from xdg_toplevel cached state
        if let Some(window_id) = with_states(surface, |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        }) {
            use smithay::wayland::shell::xdg::SurfaceCachedState;
            let (min, max) = with_states(surface, |states| {
                let cached = states.cached_state.get::<SurfaceCachedState>().current().clone();
                (cached.min_size, cached.max_size)
            });
            // Convert content size to frame size; protocol 0 = unconstrained → hub 0.0
            let b = self.config.border_size;
            let to_frame = |v: i32| if v > 0 { v as f32 + 2.0 * b } else { 0.0 };
            let (new_min_w, new_min_h) = (to_frame(min.w), to_frame(min.h));
            let (new_max_w, new_max_h) = (to_frame(max.w), to_frame(max.h));

            let win = self.hub.get_window(window_id);
            let (cur_min_w, cur_min_h) = win.min_size();
            let (cur_max_w, cur_max_h) = win.max_size();
            if new_min_w != cur_min_w || new_min_h != cur_min_h
                || new_max_w != cur_max_w || new_max_h != cur_max_h
            {
                self.hub.set_window_constraint(
                    window_id,
                    Some(new_min_w), Some(new_min_h),
                    Some(new_max_w), Some(new_max_h),
                );
                self.sync_window_positions();
            }
        }

        self.try_on_open_rules(surface);

        self.popups.commit(surface);

        // Handle layer surface commits
        if let Some(output) = self.get_output() {
            let mut layer_map = layer_map_for_output(&output);
            let layer = layer_map.layer_for_surface(surface, smithay::desktop::WindowSurfaceType::TOPLEVEL).cloned();
            if let Some(layer) = layer {
                let changed = layer_map.arrange();
                drop(layer_map);
                // Send initial configure (arrange() won't do it per spec)
                layer.layer_surface().send_pending_configure();
                if changed {
                    self.update_usable_area();
                }
            }
        }
    }
}

impl XdgShellHandler for DomeState {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.xdg_shell_state
    }

    // Tiling WM: maximize/fullscreen requests both enter fullscreen, since
    // tiled windows already fill their allocated space.
    fn maximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(window_id) = with_states(surface.wl_surface(), |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        }) {
            self.hub.set_focus(window_id);
            self.hub.set_fullscreen(window_id);
            self.sync_window_positions();
        }
    }

    fn fullscreen_request(&mut self, surface: ToplevelSurface, _output: Option<WlOutput>) {
        if let Some(window_id) = with_states(surface.wl_surface(), |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        }) {
            self.hub.set_focus(window_id);
            self.hub.set_fullscreen(window_id);
            self.sync_window_positions();
        }
    }

    fn unfullscreen_request(&mut self, surface: ToplevelSurface) {
        if let Some(window_id) = with_states(surface.wl_surface(), |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        }) {
            self.hub.unset_fullscreen(window_id);
            self.sync_window_positions();
        }
    }

    fn unmaximize_request(&mut self, surface: ToplevelSurface) {
        if let Some(window_id) = with_states(surface.wl_surface(), |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        }) {
            self.hub.unset_fullscreen(window_id);
            self.sync_window_positions();
        }
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface.clone());
        let window_id = self.hub.insert_tiling();

        with_states(surface.wl_surface(), |states| {
            states.data_map.insert_if_missing_threadsafe(|| window_id);
        });

        self.window_map.insert(window_id, window);
        self.sync_window_positions();
        self.try_on_open_rules(surface.wl_surface());
    }

    fn toplevel_destroyed(&mut self, surface: ToplevelSurface) {
        let window_id = with_states(surface.wl_surface(), |states| {
            states.data_map.get::<crate::core::WindowId>().copied()
        });

        if let Some(window_id) = window_id {
            if let Some(window) = self.window_map.remove(&window_id) {
                self.space.unmap_elem(&window);
            }
            self.on_open_done.remove(&window_id);
            self.hub.delete_window(window_id);
            self.sync_window_positions();
        }
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        surface.with_pending_state(|state| {
            state.geometry = _positioner.get_geometry();
        });
        surface.send_configure().ok();
        self.popups.track_popup(PopupKind::Xdg(surface)).ok();
    }

    fn grab(&mut self, surface: PopupSurface, seat: WlSeat, serial: Serial) {
        let seat: Seat<Self> = Seat::from_resource(&seat).unwrap();
        let popup_kind = PopupKind::Xdg(surface);

        if let Ok(root) = find_popup_root_surface(&popup_kind) {
            let root = KeyboardFocusTarget::Surface(root);
            if let Ok(mut grab) =
                self.popups.grab_popup(root, popup_kind, &seat, serial)
            {
                if let Some(keyboard) = seat.get_keyboard() {
                    if keyboard.is_grabbed() && !keyboard.has_grab(serial) {
                        grab.ungrab(PopupUngrabStrategy::All);
                        return;
                    }
                    keyboard.set_grab(
                        self,
                        smithay::desktop::PopupKeyboardGrab::new(&grab),
                        serial,
                    );
                }
                if let Some(pointer) = seat.get_pointer() {
                    if pointer.is_grabbed() && !pointer.has_grab(serial) {
                        grab.ungrab(PopupUngrabStrategy::All);
                        return;
                    }
                    pointer.set_grab(
                        self,
                        smithay::desktop::PopupPointerGrab::new(&grab),
                        serial,
                        Focus::Keep,
                    );
                }
            }
        }
    }

    fn reposition_request(&mut self, surface: PopupSurface, positioner: PositionerState, token: u32) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
        surface.send_configure().ok();
    }
}

impl XdgDecorationHandler for DomeState {
    fn new_decoration(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn request_mode(&mut self, toplevel: ToplevelSurface, _mode: DecorationMode) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_configure();
    }

    fn unset_mode(&mut self, toplevel: ToplevelSurface) {
        toplevel.with_pending_state(|state| {
            state.decoration_mode = Some(DecorationMode::ServerSide);
        });
        toplevel.send_configure();
    }
}

impl SeatHandler for DomeState {
    type KeyboardFocus = KeyboardFocusTarget;
    type PointerFocus = PointerFocusTarget;
    type TouchFocus = PointerFocusTarget;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.seat_state
    }

    fn focus_changed(&mut self, _seat: &smithay::input::Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {}
    fn cursor_image(&mut self, _seat: &smithay::input::Seat<Self>, image: smithay::input::pointer::CursorImageStatus) {
        self.cursor_status = image;
    }
}

impl ShmHandler for DomeState {
    fn shm_state(&self) -> &ShmState {
        &self.shm_state
    }
}

impl BufferHandler for DomeState {
    fn buffer_destroyed(&mut self, _buffer: &WlBuffer) {}
}

impl SelectionHandler for DomeState {
    type SelectionUserData = ();
}

impl OutputHandler for DomeState {}

impl DataDeviceHandler for DomeState {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.data_device_state
    }
}

impl ClientDndGrabHandler for DomeState {}
impl ServerDndGrabHandler for DomeState {}

impl PrimarySelectionHandler for DomeState {
    fn primary_selection_state(&self) -> &PrimarySelectionState {
        &self.primary_selection_state
    }
}

impl WlrLayerShellHandler for DomeState {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: wlr_layer::LayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        _namespace: String,
    ) {
        let smithay_output = output
            .as_ref()
            .and_then(|o| Output::from_resource(o))
            .or_else(|| self.get_output());

        let Some(smithay_output) = smithay_output else { return };

        let desktop_layer = LayerSurface::new(surface.clone(), _namespace);
        let wants_focus = desktop_layer.can_receive_keyboard_focus();
        layer_map_for_output(&smithay_output).map_layer(&desktop_layer).ok();

        // Notify the client which output this layer surface is on (for DPI, etc.)
        smithay_output.enter(surface.wl_surface());

        if wants_focus {
            let keyboard = self.seat.get_keyboard().unwrap();
            let serial = smithay::utils::SERIAL_COUNTER.next_serial();
            keyboard.set_focus(self, Some(KeyboardFocusTarget::Surface(surface.wl_surface().clone())), serial);
        }
    }

    fn layer_destroyed(&mut self, surface: wlr_layer::LayerSurface) {
        if let Some(output) = self.get_output() {
            let desktop_layer = layer_map_for_output(&output)
                .layers()
                .find(|l| l.layer_surface().wl_surface() == surface.wl_surface())
                .cloned();
            if let Some(layer) = desktop_layer {
                layer_map_for_output(&output).unmap_layer(&layer);
            }
        }
        self.update_usable_area();

        // Restore keyboard focus to hub's focused window
        let keyboard = self.seat.get_keyboard().unwrap();
        let should_restore = keyboard.current_focus()
            .and_then(|f| match f {
                KeyboardFocusTarget::Surface(ref s) => Some(s == surface.wl_surface()),
                _ => None,
            })
            .unwrap_or(false);
        if should_restore {
            self.sync_keyboard_focus();
        }
    }

    fn new_popup(&mut self, _parent: wlr_layer::LayerSurface, popup: smithay::wayland::shell::xdg::PopupSurface) {
        self.popups.track_popup(PopupKind::Xdg(popup)).ok();
    }
}

delegate_compositor!(DomeState);
delegate_xdg_shell!(DomeState);
delegate_xdg_decoration!(DomeState);
delegate_layer_shell!(DomeState);
delegate_shm!(DomeState);
delegate_seat!(DomeState);
delegate_data_device!(DomeState);
delegate_primary_selection!(DomeState);
delegate_output!(DomeState);
