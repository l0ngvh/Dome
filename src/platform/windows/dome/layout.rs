use crate::platform::windows::taskbar::ManageTaskbar;
use std::collections::HashMap;

use crate::core::MonitorLayout;

use super::Dome;
use super::events::{
    ExpectedWindow, FocusTarget, HubMessage, MonitorScene, Scene, WindowPlacement,
};

impl Dome {
    pub(super) fn current_focus_target(&self) -> FocusTarget {
        let visible = self.hub.get_visible_placements();
        visible
            .focused_window
            .and_then(|id| self.native_keys.get(&id).copied())
            .map(FocusTarget::Window)
            .unwrap_or(FocusTarget::Monitor(visible.focused_monitor))
    }

    pub(in crate::platform::windows) fn apply_layout(&mut self) {
        let mut windows: HashMap<_, _> = self
            .window_ids
            .iter()
            .map(|(&key, &id)| {
                let window = self.hub.window(id);
                (
                    key,
                    ExpectedWindow {
                        id,
                        placement: if window.is_minimized() {
                            WindowPlacement::Minimized
                        } else {
                            WindowPlacement::Hidden
                        },
                    },
                )
            })
            .collect();
        let visible = self.hub.get_visible_placements();
        let focused_window = visible
            .focused_window
            .and_then(|id| self.native_keys.get(&id).copied());
        let target = focused_window
            .map(FocusTarget::Window)
            .unwrap_or(FocusTarget::Monitor(visible.focused_monitor));
        if self.last_focus_target != Some(target) {
            self.focus_request = Some(target);
        }
        self.last_focus_target = Some(target);

        let mut monitors = Vec::with_capacity(visible.monitors.len());
        for placements in visible.monitors {
            let info = self.monitors.monitor(placements.monitor_id);
            let mut monitor = MonitorScene {
                id: placements.monitor_id,
                native: info.handle,
                work_area: placements.work_area,
                system_work_area: info.work_area,
                scale: info.scale,
                border_thickness: placements.border_thickness,
                tiling_windows: Vec::new(),
                float_windows: Vec::new(),
                containers: Vec::new(),
            };
            match placements.layout {
                MonitorLayout::Fullscreen(id) => {
                    if let Some(key) = self.native_keys.get(&id)
                        && let Some(window) = windows.get_mut(key)
                    {
                        window.placement = WindowPlacement::Fullscreen {
                            monitor: monitor.id,
                        };
                    }
                }
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    containers,
                } => {
                    monitor.containers = containers;
                    monitor.tiling_windows = tiling_windows.clone();
                    monitor.float_windows = float_windows.clone();
                    for placement in tiling_windows {
                        if let Some(key) = self.native_keys.get(&placement.id)
                            && let Some(window) = windows.get_mut(key)
                        {
                            window.placement = WindowPlacement::Tiling {
                                monitor: monitor.id,
                                placement,
                            };
                        }
                    }
                    for placement in float_windows {
                        if let Some(key) = self.native_keys.get(&placement.id)
                            && let Some(window) = windows.get_mut(key)
                        {
                            window.placement = WindowPlacement::Float {
                                monitor: monitor.id,
                                placement,
                            };
                        }
                    }
                }
            }
            monitors.push(monitor);
        }
        let displayed: std::collections::HashSet<_> = windows
            .values()
            .filter(|w| {
                !matches!(
                    w.placement,
                    WindowPlacement::Hidden | WindowPlacement::Minimized
                )
            })
            .map(|w| w.id)
            .collect();
        for id in self.displayed_windows.difference(&displayed) {
            if !self.hub.window(*id).is_minimized()
                && let Some(&key) = self.native_keys.get(id)
            {
                self.taskbar.delete_tab(key);
            }
        }
        for id in displayed.difference(&self.displayed_windows) {
            if let Some(&key) = self.native_keys.get(id) {
                self.taskbar.add_tab(key);
            }
        }
        for (&key, expected) in &windows {
            if matches!(expected.placement, WindowPlacement::Hidden) {
                self.taskbar.delete_tab(key);
            }
        }
        self.displayed_windows = displayed;
        self.window.send(HubMessage::Scene(Scene {
            windows,
            monitors,
            focused_window,
            focus_request: self.focus_request,
            workspaces: self.hub.query_workspaces(),
        }));
    }
}
