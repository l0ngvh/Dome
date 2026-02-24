use std::collections::{HashMap, HashSet};

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{MONITOR_DEFAULTTONULL, MonitorFromWindow};

use crate::config::{Color, Config};
use crate::core::{
    Child, ContainerId, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout, SpawnMode,
    WindowId, WindowPlacement,
};

use super::dome::{ContainerOverlay, TabBarInfo, TabInfo, WindowOverlay};
use super::window::{Registry, Taskbar, WindowHandle, WindowKey};

pub(super) struct MonitorEntry {
    pub(super) id: MonitorId,
    pub(super) dimension: Dimension,
    pub(super) displayed_windows: HashSet<WindowKey>,
}

impl MonitorEntry {
    pub(super) fn apply_placements(
        &mut self,
        layout: &MonitorLayout,
        registry: &Registry,
        taskbar: &mut Taskbar,
        hub: &mut Hub,
        config: &Config,
    ) -> (Vec<WindowOverlay>, Vec<ContainerOverlay>) {
        match layout {
            MonitorLayout::Fullscreen(window_id) => {
                self.apply_fullscreen(*window_id, registry, taskbar);
                (vec![], vec![])
            }
            MonitorLayout::Normal { windows, containers } => {
                self.apply_normal(windows, containers, registry, taskbar, hub, config)
            }
        }
    }

    fn apply_fullscreen(
        &mut self,
        window_id: WindowId,
        registry: &Registry,
        taskbar: &mut Taskbar,
    ) {
        let current_windows: HashSet<_> = registry
            .get_handle(window_id)
            .map(|h| WindowKey::from(&h))
            .into_iter()
            .collect();
        for key in self.displayed_windows.difference(&current_windows) {
            if let Some(handle) = registry.get_handle_by_key(*key) {
                handle.hide();
                taskbar.delete_tab(handle.hwnd()).ok();
            }
        }
        self.displayed_windows = current_windows;

        if let Some(mut handle) = registry.get_handle(window_id) {
            handle.set_fullscreen(&self.dimension);
            taskbar.add_tab(handle.hwnd()).ok();
        }
    }

    fn apply_normal(
        &mut self,
        windows: &[WindowPlacement],
        containers: &[ContainerPlacement],
        registry: &Registry,
        taskbar: &mut Taskbar,
        hub: &mut Hub,
        config: &Config,
    ) -> (Vec<WindowOverlay>, Vec<ContainerOverlay>) {
        let border = config.border_size;
        let current_windows: HashSet<_> = windows
            .iter()
            .filter_map(|p| registry.get_handle(p.id).map(|h| WindowKey::from(&h)))
            .collect();
        for key in self.displayed_windows.difference(&current_windows) {
            if let Some(handle) = registry.get_handle_by_key(*key) {
                handle.hide();
                taskbar.delete_tab(handle.hwnd()).ok();
            }
        }
        self.displayed_windows = current_windows;

        let mut window_overlays = Vec::new();
        for wp in windows {
            if let Some(mut handle) = registry.get_handle(wp.id) {
                if let Some([min_w, min_h, max_w, max_h]) = handle.get_constraints(&wp.frame, border)
                    && let Some(id) = registry.get_id(&handle)
                {
                    hub.set_window_constraint(id, min_w, min_h, max_w, max_h);
                }
                handle.show(&wp.frame, border, wp.is_float);
                taskbar.add_tab(handle.hwnd()).ok();

                let colors = if wp.is_focused {
                    if wp.is_float {
                        [config.focused_color; 4]
                    } else {
                        spawn_colors(wp.spawn_mode, config)
                    }
                } else {
                    [config.border_color; 4]
                };
                window_overlays.push(WindowOverlay {
                    window_id: wp.id,
                    frame: wp.visible_frame,
                    edges: border_edges(wp.visible_frame, border, colors),
                    is_float: wp.is_float,
                });
            }
        }

        let mut container_overlays = Vec::new();
        for cp in containers {
            let tab_bar = if cp.is_tabbed {
                Some(build_tab_info(
                    hub,
                    registry,
                    cp.id,
                    cp.visible_frame,
                    config,
                    cp.is_focused,
                ))
            } else {
                None
            };

            let edges = if cp.is_focused {
                let colors = spawn_colors(cp.spawn_mode, config);
                border_edges(cp.visible_frame, border, colors)
            } else {
                vec![]
            };

            if cp.is_tabbed || cp.is_focused {
                container_overlays.push(ContainerOverlay {
                    container_id: cp.id,
                    frame: cp.visible_frame,
                    edges,
                    tab_bar,
                });
            }
        }

        (window_overlays, container_overlays)
    }
}

pub(super) struct MonitorRegistry {
    pub(super) map: HashMap<isize, MonitorEntry>,
    pub(super) reverse: HashMap<MonitorId, isize>,
    pub(super) primary_handle: isize,
}

impl MonitorRegistry {
    pub(super) fn new(primary_handle: isize, primary_id: MonitorId, dimension: Dimension) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary_handle,
            MonitorEntry {
                id: primary_id,
                dimension,
                displayed_windows: HashSet::new(),
            },
        );
        reverse.insert(primary_id, primary_handle);
        Self {
            map,
            reverse,
            primary_handle,
        }
    }

    pub(super) fn insert(&mut self, handle: isize, monitor_id: MonitorId, dimension: Dimension) {
        self.map.insert(
            handle,
            MonitorEntry {
                id: monitor_id,
                dimension,
                displayed_windows: HashSet::new(),
            },
        );
        self.reverse.insert(monitor_id, handle);
    }

    pub(super) fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(handle) = self.reverse.remove(&monitor_id) {
            self.map.remove(&handle);
        }
    }

    pub(super) fn get_entry_mut(&mut self, monitor_id: MonitorId) -> Option<&mut MonitorEntry> {
        self.reverse
            .get(&monitor_id)
            .and_then(|h| self.map.get_mut(h))
    }

    pub(super) fn find_monitor_dimension(&self, hwnd: HWND) -> Option<Dimension> {
        let hmonitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL) };
        self.map.get(&(hmonitor.0 as isize)).map(|e| e.dimension)
    }
}

fn build_tab_info(
    hub: &Hub,
    registry: &Registry,
    container_id: ContainerId,
    visible_frame: Dimension,
    config: &Config,
    is_focused: bool,
) -> TabBarInfo {
    let container = hub.get_container(container_id);
    let children = container.children();
    let tab_width = if children.is_empty() {
        visible_frame.width
    } else {
        visible_frame.width / children.len() as f32
    };
    let active_tab = container.active_tab_index();

    let tabs = children
        .iter()
        .enumerate()
        .map(|(i, c)| {
            let title = match c {
                Child::Window(wid) => registry
                    .get_handle(*wid)
                    .and_then(|h| h.title().map(|s| s.to_owned()))
                    .unwrap_or_else(|| "Unknown".to_owned()),
                Child::Container(_) => "Container".to_owned(),
            };
            TabInfo {
                title,
                x: visible_frame.x + i as f32 * tab_width,
                width: tab_width,
                is_active: i == active_tab,
            }
        })
        .collect();

    TabBarInfo {
        tabs,
        height: config.tab_bar_height,
        background_color: config.tab_bar_background_color,
        active_background_color: config.active_tab_background_color,
        border_color: if is_focused { config.focused_color } else { config.border_color },
        border: config.border_size,
    }
}

fn spawn_colors(spawn: SpawnMode, config: &Config) -> [Color; 4] {
    let f = config.focused_color;
    let s = config.spawn_indicator_color;
    [
        if spawn.is_tab() { s } else { f },
        if spawn.is_vertical() { s } else { f },
        f,
        if spawn.is_horizontal() { s } else { f },
    ]
}

fn border_edges(dim: Dimension, border: f32, colors: [Color; 4]) -> Vec<(Dimension, Color)> {
    vec![
        // Top
        (Dimension { x: dim.x, y: dim.y, width: dim.width, height: border }, colors[0]),
        // Bottom
        (Dimension { x: dim.x, y: dim.y + dim.height - border, width: dim.width, height: border }, colors[1]),
        // Left
        (Dimension { x: dim.x, y: dim.y + border, width: border, height: dim.height - 2.0 * border }, colors[2]),
        // Right
        (Dimension { x: dim.x + dim.width - border, y: dim.y + border, width: border, height: dim.height - 2.0 * border }, colors[3]),
    ]
}
