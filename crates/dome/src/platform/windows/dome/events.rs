use crate::config::Keystroke;
use crate::platform::windows::dome::Dome;
use crate::platform::windows::window_server::{PlacementObservation, WindowNotification};

pub(in crate::platform::windows) enum HubEvent {
    RunBinding {
        keymap: String,
        keystroke: Keystroke,
    },
}

impl Dome {
    pub(super) fn handle_window_notification(&mut self, notification: WindowNotification) {
        match notification {
            WindowNotification::WindowObserved {
                window,
                rect,
                monitor,
                dragged_at,
            } => {
                if let Some(observed_at) = dragged_at {
                    self.moving_windows.remove(&window.id);
                    if self.is_managed(window.id) {
                        if !window.state.minimized {
                            if self.handle_window_moved(window.id, rect, monitor, observed_at) {
                                self.set_constraints_for(window.id, window.constraints);
                            }
                            self.apply_layout();
                        }
                        return;
                    }
                }
                if window.is_manageable() {
                    self.add_window(window, rect, monitor);
                } else {
                    crate::trace_once!(key: (window.id, window.metadata.title.clone()), window_id = %window.id, "Native window is not manageable");
                }
            }
            WindowNotification::WindowRemoved(window_id) => self.window_destroyed(window_id),
            WindowNotification::WindowMinimized(window_id) => self.window_minimized(window_id),
            WindowNotification::FocusChanged(window_id) => self.handle_focus(window_id),
            WindowNotification::FocusSettled(window_id) => self.handle_focus(window_id),
            WindowNotification::MovementStarted(window_id) => {
                if self.is_managed(window_id) {
                    self.moving_windows.insert(window_id);
                }
            }
            WindowNotification::PlacementChanged {
                window_id,
                observed_at,
                placement,
                constraints,
            } => {
                self.moving_windows.remove(&window_id);
                if let Some(PlacementObservation::Visible { rect, monitor }) = placement
                    && self.is_managed(window_id)
                {
                    if self.handle_window_moved(window_id, rect, monitor, observed_at)
                        && let Some(constraints) = constraints
                    {
                        self.set_constraints_for(window_id, constraints);
                    }
                    self.apply_layout();
                }
            }
            WindowNotification::TitleChanged { window_id, title } => {
                self.update_titles(vec![(window_id, title)]);
            }
            WindowNotification::ConstraintsChanged {
                window_id,
                constraints,
            } => {
                self.set_constraints_for(window_id, constraints);
                self.apply_layout();
            }
            WindowNotification::PeriodicTick => self.retry_drifted_windows(),
        }
    }
}
