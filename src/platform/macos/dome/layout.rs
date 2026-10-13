use std::collections::HashMap;

use crate::core::{Length, MonitorLayout};

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

    pub(in crate::platform::macos) fn flush_layout(&mut self) {
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
            let info = self.monitor_registry.monitor(placements.monitor_id);
            let mut monitor = MonitorScene {
                id: placements.monitor_id,
                native: info.display_id,
                work_area: placements.work_area,
                system_work_area: info.work_area,
                bounds: info.bounds,
                is_primary: info.is_primary,
                full_height: info.full_height,
                scale: info.backing_scale,
                border_thickness: Length::from_pixels(placements.border_thickness),
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
        self.sender.send(HubMessage::Scene(Scene {
            windows,
            monitors,
            focused_window,
            focus_request: self.focus_request,
            workspaces: self.hub.query_workspaces(),
            displays_changed_at: self.displays_changed_at,
        }));
    }
}
