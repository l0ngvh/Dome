use std::os::unix::process::CommandExt;

use smithay::backend::input::{
    Axis, ButtonState, Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
    PointerAxisEvent, PointerButtonEvent, PointerMotionAbsoluteEvent, PointerMotionEvent,
};
use smithay::input::keyboard::{FilterResult, xkb};
use smithay::utils::{Logical, Point};
use smithay::wayland::compositor::with_states;

use crate::action::Action;
use crate::config::{Keymap, Modifiers};

use super::focus::PointerFocusTarget;
use super::state::DomeState;

impl DomeState {
    pub(super) fn process_input_event<I: InputBackend>(&mut self, event: InputEvent<I>) {
        match event {
            InputEvent::Keyboard { event } => self.handle_keyboard(event),
            InputEvent::PointerMotion { event } => self.handle_pointer_motion(event),
            InputEvent::PointerMotionAbsolute { event } => {
                self.handle_pointer_motion_absolute(event)
            }
            InputEvent::PointerButton { event } => self.handle_pointer_button(event),
            InputEvent::PointerAxis { event } => self.handle_pointer_axis(event),
            _ => {}
        }
    }

    fn handle_keyboard<I: InputBackend>(&mut self, event: impl KeyboardKeyEvent<I>) {
        let keyboard = self.seat.get_keyboard().unwrap();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let time = Event::time_msec(&event);

        keyboard.input::<(), _>(
            self,
            event.key_code(),
            event.state(),
            serial,
            time,
            |state, modifiers, keysym| {
                if event.state() == KeyState::Pressed {
                    let mut mods = Modifiers::empty();
                    if modifiers.ctrl {
                        mods |= Modifiers::CTRL;
                    }
                    if modifiers.alt {
                        mods |= Modifiers::ALT;
                    }
                    if modifiers.shift {
                        mods |= Modifiers::SHIFT;
                    }
                    if modifiers.logo {
                        mods |= Modifiers::META;
                    }

                    let key_name = xkb::keysym_get_name(keysym.modified_sym());
                    let keymap = Keymap {
                        key: key_name.to_lowercase(),
                        modifiers: mods,
                    };

                    let actions = state
                        .config
                        .keymaps
                        .default
                        .get(&keymap)
                        .cloned()
                        .unwrap_or_default();
                    if !actions.is_empty() {
                        for action in &actions {
                            state.handle_action(action);
                        }
                        return FilterResult::Intercept(());
                    }
                }
                FilterResult::Forward
            },
        );
    }

    pub(super) fn handle_action(&mut self, action: &Action) {
        use crate::action::{FocusTarget, MoveTarget, TabDirection, ToggleTarget};
        use crate::core::{Direction, TilingAction};

        match action {
            Action::Focus { target } => {
                match target {
                    FocusTarget::Up => {
                        self.hub.handle_tiling_action(TilingAction::FocusDirection {
                            direction: Direction::Vertical,
                            forward: false,
                        })
                    }
                    FocusTarget::Down => {
                        self.hub.handle_tiling_action(TilingAction::FocusDirection {
                            direction: Direction::Vertical,
                            forward: true,
                        })
                    }
                    FocusTarget::Left => {
                        self.hub.handle_tiling_action(TilingAction::FocusDirection {
                            direction: Direction::Horizontal,
                            forward: false,
                        })
                    }
                    FocusTarget::Right => {
                        self.hub.handle_tiling_action(TilingAction::FocusDirection {
                            direction: Direction::Horizontal,
                            forward: true,
                        })
                    }
                    FocusTarget::Parent => self.hub.handle_tiling_action(TilingAction::FocusParent),
                    FocusTarget::Tab { direction } => {
                        self.hub.handle_tiling_action(TilingAction::FocusTab {
                            forward: matches!(direction, TabDirection::Next),
                        })
                    }
                    FocusTarget::Workspace { name, monitor } => {
                        self.hub.focus_workspace(name, monitor.as_deref())
                    }
                    FocusTarget::Monitor { target } => self.hub.focus_monitor(target),
                }
                self.sync_window_positions();
            }
            Action::Move { target } => {
                match target {
                    MoveTarget::Up => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                        direction: Direction::Vertical,
                        forward: false,
                    }),
                    MoveTarget::Down => {
                        self.hub.handle_tiling_action(TilingAction::MoveDirection {
                            direction: Direction::Vertical,
                            forward: true,
                        })
                    }
                    MoveTarget::Left => {
                        self.hub.handle_tiling_action(TilingAction::MoveDirection {
                            direction: Direction::Horizontal,
                            forward: false,
                        })
                    }
                    MoveTarget::Right => {
                        self.hub.handle_tiling_action(TilingAction::MoveDirection {
                            direction: Direction::Horizontal,
                            forward: true,
                        })
                    }
                    MoveTarget::Workspace { name, monitor } => {
                        self.hub.move_focused_to_workspace(name, monitor.as_deref())
                    }
                    MoveTarget::Monitor { target } => self.hub.move_focused_to_monitor(target),
                }
                self.sync_window_positions();
            }
            Action::Toggle { target } => {
                match target {
                    ToggleTarget::Spawn => {
                        self.hub.handle_tiling_action(TilingAction::ToggleSpawnMode)
                    }
                    ToggleTarget::Direction => {
                        self.hub.handle_tiling_action(TilingAction::ToggleDirection)
                    }
                    ToggleTarget::Layout => self
                        .hub
                        .handle_tiling_action(TilingAction::ToggleContainerLayout),
                    ToggleTarget::Float => self.hub.toggle_float(),
                    ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
                }
                self.sync_window_positions();
            }
            Action::Exec { command } => {
                let mut cmd = std::process::Command::new("sh");
                cmd.arg("-c").arg(command).process_group(0);
                cmd.env("WAYLAND_DISPLAY", &self.wayland_socket_name);
                if let Some(ref display) = self.x11_display {
                    cmd.env("DISPLAY", display);
                }
                match cmd.spawn() {
                    Ok(child) => self.exec_children.push(child),
                    Err(e) => tracing::warn!("Failed to exec '{command}': {e}"),
                }
            }
            Action::Exit => {
                self.loop_signal.stop();
            }
            Action::Close => {
                if let Some(window_id) = self.hub.get_visible_placements().focused_window {
                    if let Some(toplevel) =
                        self.window_map.get(&window_id).and_then(|w| w.toplevel())
                    {
                        toplevel.send_close();
                    }
                }
            }
            Action::Master { .. } | Action::UnminimizeWindow { .. } | Action::Mode { .. } => {}
        }
    }

    fn handle_pointer_motion<I: InputBackend>(&mut self, event: impl PointerMotionEvent<I>) {
        let pointer = self.seat.get_pointer().unwrap();
        let mut pos = pointer.current_location();
        pos += event.delta();
        let pos = self.clamp_pointer(pos);

        self.pointer_motion_common(pos, event.time_msec());
    }

    fn handle_pointer_motion_absolute<I: InputBackend>(
        &mut self,
        event: impl PointerMotionAbsoluteEvent<I>,
    ) {
        let (max_w, max_h) = self.full_output_bounds();
        let pos = (
            event.x_transformed(max_w as i32) as f64,
            event.y_transformed(max_h as i32) as f64,
        )
            .into();
        let pos = self.clamp_pointer(pos);

        self.pointer_motion_common(pos, event.time_msec());
    }

    fn pointer_motion_common(&mut self, pos: Point<f64, Logical>, time: u32) {
        // Switch hub focus when pointer crosses monitor boundary
        if let Some(mid) = self.monitor_at(pos) {
            if mid != self.hub.focused_monitor() {
                let name = self
                    .monitor_outputs
                    .get(&mid)
                    .map(|o| o.name())
                    .unwrap_or_default();
                self.hub
                    .focus_monitor(&crate::action::MonitorTarget::Name(name));
                self.sync_window_positions();
            }
        }

        let pointer = self.seat.get_pointer().unwrap();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();
        let under = self.surface_under(pos);

        pointer.motion(
            self,
            under,
            &smithay::input::pointer::MotionEvent {
                location: pos,
                serial,
                time,
            },
        );
        pointer.frame(self);
    }

    fn handle_pointer_button<I: InputBackend>(&mut self, event: impl PointerButtonEvent<I>) {
        let pointer = self.seat.get_pointer().unwrap();
        let serial = smithay::utils::SERIAL_COUNTER.next_serial();

        let mut tab_bar_consumed = false;
        if event.state() == ButtonState::Pressed {
            let pos = pointer.current_location();

            if self.handle_tab_bar_click(pos) {
                tab_bar_consumed = true;
            } else {
                let focused = self.space.element_under(pos).and_then(|(window, _)| {
                    window.toplevel().and_then(|toplevel| {
                        with_states(toplevel.wl_surface(), |states| {
                            states.data_map.get::<crate::core::WindowId>().copied()
                        })
                    })
                });
                if let Some(window_id) = focused {
                    self.hub.set_focus(window_id);
                    self.sync_window_positions();
                }
            }
        }

        if !tab_bar_consumed {
            pointer.button(
                self,
                &smithay::input::pointer::ButtonEvent {
                    button: event.button_code(),
                    state: event.state(),
                    serial,
                    time: event.time_msec(),
                },
            );
            pointer.frame(self);
        }
    }

    fn handle_pointer_axis<I: InputBackend>(&mut self, event: impl PointerAxisEvent<I>) {
        let pointer = self.seat.get_pointer().unwrap();
        let source = event.source();
        let mut frame = smithay::input::pointer::AxisFrame::new(event.time_msec()).source(source);
        for axis in [Axis::Horizontal, Axis::Vertical] {
            if let Some(amount) = event.amount(axis) {
                frame = frame.value(axis, amount);
            } else if let Some(amount) = event.amount_v120(axis) {
                frame = frame.v120(axis, amount as i32);
            }
        }
        pointer.axis(self, frame);
        pointer.frame(self);
    }

    pub(super) fn surface_under(
        &self,
        pos: Point<f64, Logical>,
    ) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
        let output = self
            .monitor_at(pos)
            .and_then(|mid| self.monitor_outputs.get(&mid))
            .or_else(|| self.space.outputs().next())
            .cloned()?;
        let layer_map = smithay::desktop::layer_map_for_output(&output);

        // Check Overlay and Top layers first (above windows)
        for layer_type in [
            smithay::wayland::shell::wlr_layer::Layer::Overlay,
            smithay::wayland::shell::wlr_layer::Layer::Top,
        ] {
            if let Some(layer) = layer_map.layer_under(layer_type, pos) {
                let layer_geo = layer_map.layer_geometry(layer)?;
                if let Some((surface, surface_pos)) = layer.surface_under(
                    pos - layer_geo.loc.to_f64(),
                    smithay::desktop::WindowSurfaceType::ALL,
                ) {
                    return Some((PointerFocusTarget::Surface(surface), surface_pos.to_f64()));
                }
            }
        }

        // Check windows
        if let Some(result) = self.space.element_under(pos).and_then(|(window, loc)| {
            window
                .surface_under(pos - loc.to_f64(), smithay::desktop::WindowSurfaceType::ALL)
                .map(|(s, p)| (PointerFocusTarget::Surface(s), p.to_f64()))
        }) {
            return Some(result);
        }

        // Check Bottom and Background layers (below windows)
        for layer_type in [
            smithay::wayland::shell::wlr_layer::Layer::Bottom,
            smithay::wayland::shell::wlr_layer::Layer::Background,
        ] {
            if let Some(layer) = layer_map.layer_under(layer_type, pos) {
                let layer_geo = layer_map.layer_geometry(layer)?;
                if let Some((surface, surface_pos)) = layer.surface_under(
                    pos - layer_geo.loc.to_f64(),
                    smithay::desktop::WindowSurfaceType::ALL,
                ) {
                    return Some((PointerFocusTarget::Surface(surface), surface_pos.to_f64()));
                }
            }
        }

        None
    }

    fn handle_tab_bar_click(&mut self, pos: Point<f64, Logical>) -> bool {
        let th = crate::core::Length::from_pixels(self.config.partition_tree.tab_bar_height)
            .logical() as f64;
        let placements = self.hub.get_visible_placements();
        for mp in &placements.monitors {
            let crate::core::MonitorLayout::Normal { containers, .. } = &mp.layout else {
                continue;
            };
            let output_pos = self
                .monitor_outputs
                .get(&mp.monitor_id)
                .and_then(|o| self.space.output_geometry(o))
                .map(|g| g.loc)
                .unwrap_or_default();
            for cp in containers {
                if !cp.is_tabbed {
                    continue;
                }
                let f = cp.border_box.to_dimension();
                // Convert global pointer pos to monitor-local
                let px = pos.x as f32 - output_pos.x as f32;
                let py = pos.y as f32 - output_pos.y as f32;
                let Some(index) = tab_index_at(
                    px,
                    py,
                    f.x.value(),
                    f.y.value(),
                    f.width.value(),
                    th as f32,
                    cp.titles.len(),
                ) else {
                    continue;
                };
                self.hub.focus_tab_index(cp.id, index);
                self.sync_window_positions();
                return true;
            }
        }
        false
    }
}

/// The tab a monitor-local click selects in a tabbed container's tab bar, or `None`
/// when the click misses the bar. The bar spans the container's full width at its top
/// edge, and the tabs divide that width evenly.
fn tab_index_at(
    click_x: f32,
    click_y: f32,
    bar_x: f32,
    bar_y: f32,
    bar_width: f32,
    bar_height: f32,
    tab_count: usize,
) -> Option<usize> {
    if tab_count == 0 {
        return None;
    }
    if click_x < bar_x || click_x >= bar_x + bar_width {
        return None;
    }
    if click_y < bar_y || click_y >= bar_y + bar_height {
        return None;
    }
    let tab_width = bar_width / tab_count as f32;
    Some(((click_x - bar_x) / tab_width) as usize)
}

#[cfg(test)]
mod tests {
    use super::tab_index_at;

    // A 300 wide bar at (100, 50), 30 tall, holding 3 tabs of 100 each.
    fn three_tabs(click_x: f32, click_y: f32) -> Option<usize> {
        tab_index_at(click_x, click_y, 100.0, 50.0, 300.0, 30.0, 3)
    }

    #[test]
    fn each_third_of_the_bar_selects_its_own_tab() {
        assert_eq!(three_tabs(150.0, 60.0), Some(0));
        assert_eq!(three_tabs(250.0, 60.0), Some(1));
        assert_eq!(three_tabs(350.0, 60.0), Some(2));
    }

    #[test]
    fn a_tab_owns_its_left_edge() {
        assert_eq!(three_tabs(100.0, 50.0), Some(0));
        assert_eq!(three_tabs(200.0, 50.0), Some(1));
        assert_eq!(three_tabs(300.0, 50.0), Some(2));
    }

    #[test]
    fn the_last_tab_stops_short_of_the_right_edge() {
        assert_eq!(three_tabs(399.9, 60.0), Some(2));
        assert_eq!(three_tabs(400.0, 60.0), None);
    }

    #[test]
    fn a_click_outside_the_bar_selects_nothing() {
        assert_eq!(three_tabs(99.0, 60.0), None, "left of the bar");
        assert_eq!(three_tabs(150.0, 49.0), None, "above the bar");
        assert_eq!(three_tabs(150.0, 80.0), None, "below the bar");
        assert_eq!(three_tabs(150.0, 79.9), Some(0), "inside the bottom edge");
    }

    #[test]
    fn a_container_with_no_tabs_selects_nothing() {
        assert_eq!(tab_index_at(150.0, 60.0, 100.0, 50.0, 300.0, 30.0, 0), None);
    }

    #[test]
    fn a_single_tab_takes_the_whole_bar() {
        assert_eq!(
            tab_index_at(100.0, 60.0, 100.0, 50.0, 300.0, 30.0, 1),
            Some(0)
        );
        assert_eq!(
            tab_index_at(399.9, 60.0, 100.0, 50.0, 300.0, 30.0, 1),
            Some(0)
        );
    }
}
