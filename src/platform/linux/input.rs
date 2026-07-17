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
            Action::Focus(target) => {
                match target {
                    FocusTarget::Up => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                        direction: Direction::Vertical,
                        forward: false,
                    }),
                    FocusTarget::Down => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                        direction: Direction::Vertical,
                        forward: true,
                    }),
                    FocusTarget::Left => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                        direction: Direction::Horizontal,
                        forward: false,
                    }),
                    FocusTarget::Right => self.hub.handle_tiling_action(TilingAction::FocusDirection {
                        direction: Direction::Horizontal,
                        forward: true,
                    }),
                    FocusTarget::Parent => self.hub.handle_tiling_action(TilingAction::FocusParent),
                    FocusTarget::Tab { direction } => {
                        self.hub.handle_tiling_action(TilingAction::FocusTab {
                            forward: matches!(direction, TabDirection::Next),
                        })
                    }
                    FocusTarget::Workspace { name } => self.hub.focus_workspace(name),
                    FocusTarget::Monitor { target } => self.hub.focus_monitor(target),
                }
                self.sync_window_positions();
            }
            Action::Move(target) => {
                match target {
                    MoveTarget::Up => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                        direction: Direction::Vertical,
                        forward: false,
                    }),
                    MoveTarget::Down => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                        direction: Direction::Vertical,
                        forward: true,
                    }),
                    MoveTarget::Left => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                        direction: Direction::Horizontal,
                        forward: false,
                    }),
                    MoveTarget::Right => self.hub.handle_tiling_action(TilingAction::MoveDirection {
                        direction: Direction::Horizontal,
                        forward: true,
                    }),
                    MoveTarget::Workspace { name } => self.hub.move_focused_to_workspace(name),
                    MoveTarget::Monitor { target } => self.hub.move_focused_to_monitor(target),
                }
                self.sync_window_positions();
            }
            Action::Toggle(target) => {
                match target {
                    ToggleTarget::Spawn => self.hub.handle_tiling_action(TilingAction::ToggleSpawnMode),
                    ToggleTarget::Direction => self.hub.handle_tiling_action(TilingAction::ToggleDirection),
                    ToggleTarget::Layout => self.hub.handle_tiling_action(TilingAction::ToggleContainerLayout),
                    ToggleTarget::Float => self.hub.toggle_float(),
                    ToggleTarget::Fullscreen => self.hub.toggle_fullscreen(),
                }
                self.sync_window_positions();
            }
            Action::Exec { command } => {
                let mut cmd = std::process::Command::new("sh");
                cmd.arg("-c")
                    .arg(command)
                    .process_group(0);
                if let Some(ref display) = self.x11_display {
                    cmd.env("DISPLAY", display);
                }
                cmd.spawn()
                    .inspect_err(|e| tracing::warn!("Failed to exec '{command}': {e}"))
                    .ok();
            }
            Action::Exit => {
                self.loop_signal.stop();
            }
            Action::Master(_)
            | Action::ToggleMinimized
            | Action::UnminimizeWindow(_)
            | Action::Mode { .. } => {}
        }
    }

    fn handle_pointer_motion<I: InputBackend>(&mut self, event: impl PointerMotionEvent<I>) {
        let pointer = self.seat.get_pointer().unwrap();
        let mut pos = pointer.current_location();
        pos += event.delta();

        let (max_w, max_h) = self.full_output_bounds();
        pos.x = pos.x.clamp(0.0, max_w);
        pos.y = pos.y.clamp(0.0, max_h);

        self.pointer_motion_common(pos, event.time_msec());
    }

    fn handle_pointer_motion_absolute<I: InputBackend>(&mut self, event: impl PointerMotionAbsoluteEvent<I>) {
        let (max_w, max_h) = self.full_output_bounds();
        let pos = (
            event.x_transformed(max_w as i32) as f64,
            event.y_transformed(max_h as i32) as f64,
        ).into();

        self.pointer_motion_common(pos, event.time_msec());
    }

    fn pointer_motion_common(&mut self, pos: Point<f64, Logical>, time: u32) {
        // Switch hub focus when pointer crosses monitor boundary
        if let Some(mid) = self.monitor_at(pos) {
            if mid != self.hub.focused_monitor() {
                let name = self.monitor_outputs.get(&mid).map(|o| o.name()).unwrap_or_default();
                self.hub.focus_monitor(&crate::action::MonitorTarget::Name(name));
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
        let output = self.monitor_at(pos)
            .and_then(|mid| self.monitor_outputs.get(&mid))
            .or_else(|| self.space.outputs().next())
            .cloned()?;
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
        let th = self.config.partition_tree.tab_bar_height.logical() as f64;
        let placements = self.hub.get_visible_placements();
        for mp in &placements.monitors {
            let crate::core::MonitorLayout::Normal { containers, .. } = &mp.layout else {
                continue;
            };
            let output_pos = self.monitor_outputs.get(&mp.monitor_id)
                .and_then(|o| self.space.output_geometry(o))
                .map(|g| g.loc)
                .unwrap_or_default();
            for cp in containers {
                if !cp.is_tabbed {
                    continue;
                }
                let f = cp.frame;
                // Convert global pointer pos to monitor-local
                let px = pos.x as f32 - output_pos.x as f32;
                let py = pos.y as f32 - output_pos.y as f32;
                if px >= f.x.value() && px < (f.x + f.width).value() && py >= f.y.value() && py < f.y.value() + th as f32 {
                    let num_tabs = cp.titles.len();
                    if num_tabs == 0 {
                        continue;
                    }
                    let tab_width = f.width.value() / num_tabs as f32;
                    let index = ((px - f.x.value()) / tab_width) as usize;
                    self.hub.focus_tab_index(cp.id, index);
                    self.sync_window_positions();
                    return true;
                }
            }
        }
        false
    }
}
