use crate::core::{
    Hub, WindowId,
    hub::RestrictedAction,
    node::{DisplayMode, MonitorId, PixelRect, WindowHost},
};

impl Hub {
    pub(super) fn focus_sticky(&mut self, monitor_id: MonitorId, window_id: WindowId) {
        let monitor = self.access.monitors.get_mut(monitor_id);
        if let Some(pos) = monitor
            .sticky_windows
            .iter()
            .position(|&id| id == window_id)
        {
            monitor.sticky_windows.remove(pos);
            monitor.sticky_windows.push(window_id);
        }
        monitor.is_sticky_focused = true;
    }

    pub(super) fn attach_sticky_to_monitor(
        &mut self,
        monitor_id: MonitorId,
        id: WindowId,
        border_box: PixelRect,
    ) {
        let window = self.access.windows.get_mut(id);
        window.mode = DisplayMode::Float { border_box };
        window.set_host(WindowHost::Monitor(monitor_id));
        self.access
            .monitors
            .get_mut(monitor_id)
            .sticky_windows
            .push(id);
        self.focus_sticky(monitor_id, id);
    }

    /// Returns the window's border box. Clears the monitor's sticky focus when this
    /// window held it, rather than passing it to the next sticky window.
    pub(super) fn detach_sticky_from_monitor(&mut self, id: WindowId) -> PixelRect {
        let window = self.access.windows.get(id);
        let DisplayMode::Float { border_box, .. } = window.mode else {
            panic!("detach_sticky_from_monitor: {id} is not Float");
        };
        let monitor_id = match window.host {
            WindowHost::Monitor(monitor_id) => monitor_id,
            WindowHost::Minimized { .. } | WindowHost::Workspace(_) => {
                panic!("detach_sticky_from_monitor: {id} is not sticky")
            }
        };
        let monitor = self.access.monitors.get_mut(monitor_id);

        let was_focused =
            monitor.is_sticky_focused && monitor.sticky_windows.last().copied() == Some(id);

        let pos = monitor
            .sticky_windows
            .iter()
            .position(|&sid| sid == id)
            .expect("detach_sticky_from_monitor: window not in sticky_windows");
        monitor.sticky_windows.remove(pos);

        if was_focused {
            monitor.is_sticky_focused = false;
        }

        border_box
    }

    /// Makes the focused window sticky on the focused monitor, or makes a focused sticky
    /// window a float on the active workspace of its monitor. Does nothing on a fullscreen
    /// window.
    #[tracing::instrument(skip(self))]
    pub(crate) fn toggle_sticky(&mut self) {
        if self.is_restricted(RestrictedAction::DisplayModeChange) {
            return;
        }
        let current_ws = self.current_workspace();
        let Some(window_id) = self.focused_window(current_ws) else {
            return;
        };
        let window = self.access.windows.get(window_id);
        let (host, mode) = (window.host, window.mode);
        let monitor_id = self.access.workspaces.get(current_ws).monitor;

        match host {
            WindowHost::Monitor(sticky_monitor) => {
                let border_box = self.detach_sticky_from_monitor(window_id);
                let ws = self.access.monitors.get(sticky_monitor).active_workspace;
                self.attach_float_to_workspace(ws, window_id, border_box, None);
                tracing::debug!(%window_id, "Window is now floating");
            }
            WindowHost::Minimized { .. } | WindowHost::Workspace(_) => match mode {
                DisplayMode::Fullscreen { .. } => (),
                DisplayMode::Tiling => {
                    let strategy = self.strategies.for_workspace_mut(current_ws);
                    let border_box = strategy.detach_window(&mut self.access, window_id);
                    if strategy.tiling_window_count(&self.access, current_ws) == 0 {
                        let ws = self.access.workspaces.get_mut(current_ws);
                        if ws.fullscreen_windows.is_empty() {
                            ws.is_float_focused = !ws.float_windows.is_empty();
                        }
                    }
                    self.attach_sticky_to_monitor(monitor_id, window_id, border_box);
                    tracing::debug!(%window_id, "Window is now sticky");
                }
                DisplayMode::Float { .. } => {
                    let border_box = self.detach_float_from_workspace(window_id);
                    self.attach_sticky_to_monitor(monitor_id, window_id, border_box);
                    tracing::debug!(%window_id, "Window is now sticky");
                }
            },
        }
    }
}
