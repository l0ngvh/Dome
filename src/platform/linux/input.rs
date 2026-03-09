use std::os::unix::process::CommandExt;

use smithay::backend::input::{
    Event, InputBackend, InputEvent, KeyState, KeyboardKeyEvent,
    PointerMotionEvent, PointerMotionAbsoluteEvent, PointerButtonEvent, ButtonState,
    Axis, PointerAxisEvent,
};
use smithay::input::keyboard::{xkb, FilterResult};
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
            InputEvent::PointerMotionAbsolute { event } => self.handle_pointer_motion_absolute(event),
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
                        mods |= Modifiers::CMD;
                    }

                    let key_name = xkb::keysym_get_name(keysym.modified_sym());
                    let keymap = Keymap {
                        key: key_name.to_lowercase(),
                        modifiers: mods,
                    };

                    let actions = state.config.keymaps.get(&keymap).cloned().unwrap_or_default();
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
        use crate::action::{FocusTarget, MoveTarget, ToggleTarget};

        match action {
            Action::Focus { target } => {
                match target {
                    FocusTarget::Up => self.hub.focus_up(),
                    FocusTarget::Down => self.hub.focus_down(),
                    FocusTarget::Left => self.hub.focus_left(),
                    FocusTarget::Right => self.hub.focus_right(),
                    FocusTarget::Parent => self.hub.focus_parent(),
                    FocusTarget::NextTab => self.hub.focus_next_tab(),
                    FocusTarget::PrevTab => self.hub.focus_prev_tab(),
                    FocusTarget::Workspace { name } => self.hub.focus_workspace(name),
                    FocusTarget::Monitor { target } => self.hub.focus_monitor(target),
                }
                self.sync_window_positions();
            }
            Action::Move { target } => {
                match target {
                    MoveTarget::Up => self.hub.move_up(),
                    MoveTarget::Down => self.hub.move_down(),
                    MoveTarget::Left => self.hub.move_left(),
                    MoveTarget::Right => self.hub.move_right(),
                    MoveTarget::Workspace { name } => self.hub.move_focused_to_workspace(name),
                    MoveTarget::Monitor { target } => self.hub.move_focused_to_monitor(target),
                }
                self.sync_window_positions();
            }
            Action::Toggle { target } => {
                match target {
                    ToggleTarget::SpawnDirection => self.hub.toggle_spawn_mode(),
                    ToggleTarget::Direction => self.hub.toggle_direction(),
                    ToggleTarget::Layout => self.hub.toggle_container_layout(),
                    ToggleTarget::Float => self.hub.toggle_float(),
                    ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
                }
                self.sync_window_positions();
            }
            Action::Exec { command } => {
                std::process::Command::new("sh")
                    .arg("-c")
                    .arg(command)
                    .process_group(0)
                    .spawn()
                    .inspect_err(|e| tracing::warn!("Failed to exec '{command}': {e}"))
                    .ok();
            }
            Action::Exit => {
                self.loop_signal.stop();
            }
        }
    }

    fn handle_pointer_motion<I: InputBackend>(&mut self, event: impl PointerMotionEvent<I>) {
        let pointer = self.seat.get_pointer().unwrap();
        let mut pos = pointer.current_location();
        pos += event.delta();

        let (max_w, max_h) = self.full_output_size();
        pos.x = pos.x.clamp(0.0, max_w);
        pos.y = pos.y.clamp(0.0, max_h);

        self.pointer_motion_common(pos, event.time_msec());
    }

    fn handle_pointer_motion_absolute<I: InputBackend>(&mut self, event: impl PointerMotionAbsoluteEvent<I>) {
        let (max_w, max_h) = self.full_output_size();
        let pos = (
            event.x_transformed(max_w as i32) as f64,
            event.y_transformed(max_h as i32) as f64,
        ).into();

        self.pointer_motion_common(pos, event.time_msec());
    }

    fn pointer_motion_common(&mut self, pos: Point<f64, Logical>, time: u32) {
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
                let focused = self.space.element_under(pos)
                    .and_then(|(window, _)| {
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

    fn surface_under(&self, pos: Point<f64, Logical>) -> Option<(PointerFocusTarget, Point<f64, Logical>)> {
        let output = self.get_output()?;
        let layer_map = smithay::desktop::layer_map_for_output(&output);

        // Check Overlay and Top layers first (above windows)
        for layer_type in [smithay::wayland::shell::wlr_layer::Layer::Overlay, smithay::wayland::shell::wlr_layer::Layer::Top] {
            if let Some(layer) = layer_map.layer_under(layer_type, pos) {
                let layer_geo = layer_map.layer_geometry(layer)?;
                if let Some((surface, surface_pos)) = layer.surface_under(pos - layer_geo.loc.to_f64(), smithay::desktop::WindowSurfaceType::ALL) {
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
        for layer_type in [smithay::wayland::shell::wlr_layer::Layer::Bottom, smithay::wayland::shell::wlr_layer::Layer::Background] {
            if let Some(layer) = layer_map.layer_under(layer_type, pos) {
                let layer_geo = layer_map.layer_geometry(layer)?;
                if let Some((surface, surface_pos)) = layer.surface_under(pos - layer_geo.loc.to_f64(), smithay::desktop::WindowSurfaceType::ALL) {
                    return Some((PointerFocusTarget::Surface(surface), surface_pos.to_f64()));
                }
            }
        }

        None
    }

    fn handle_tab_bar_click(&mut self, pos: Point<f64, Logical>) -> bool {
        let th = self.config.tab_bar_height as f64;
        let placements = self.hub.get_visible_placements();
        for mp in &placements {
            let crate::core::MonitorLayout::Normal { containers, .. } = &mp.layout else {
                continue;
            };
            for cp in containers {
                if !cp.is_tabbed {
                    continue;
                }
                let f = cp.frame;
                let px = pos.x as f32;
                let py = pos.y as f32;
                if px >= f.x && px < f.x + f.width && py >= f.y && py < f.y + th as f32 {
                    let num_tabs = self.hub.get_container(cp.id).children().len();
                    if num_tabs == 0 {
                        continue;
                    }
                    let tab_width = f.width / num_tabs as f32;
                    let index = ((px - f.x) / tab_width) as usize;
                    self.hub.focus_tab_index(cp.id, index);
                    self.sync_window_positions();
                    return true;
                }
            }
        }
        false
    }
}
