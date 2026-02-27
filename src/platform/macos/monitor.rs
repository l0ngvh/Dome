use std::collections::{HashMap, HashSet};

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID, CGWindowID};
use objc2_foundation::{NSNumber, NSString};

use crate::config::Config;
use crate::core::{
    Child, Container, ContainerPlacement, Dimension, Hub, MonitorId, MonitorLayout,
    MonitorPlacements, WindowPlacement,
};

use super::overlay::Overlays;
use super::registry::Registry;
use super::rendering::{ContainerBorder, build_tab_bar, compute_container_border};
use super::window::FullscreenState;

#[derive(Clone)]
pub(super) struct MonitorInfo {
    pub(super) display_id: CGDirectDisplayID,
    pub(super) name: String,
    pub(super) dimension: Dimension,
    pub(super) full_height: f32,
    pub(super) is_primary: bool,
    pub(super) scale: f64,
}

impl std::fmt::Display for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, dim={:?}, scale={})",
            self.name, self.display_id, self.dimension, self.scale
        )
    }
}

type DisplayId = u32;

pub(super) struct MonitorEntry {
    pub(super) id: MonitorId,
    pub(super) screen: MonitorInfo,
    pub(super) displayed_windows: HashSet<CGWindowID>,
}

impl MonitorEntry {
    pub(super) fn apply_placements(
        &mut self,
        mp: &MonitorPlacements,
        is_focused_monitor: bool,
        hub: &Hub,
        registry: &mut Registry,
        config: &Config,
        primary_full_height: f32,
    ) -> Overlays {
        match &mp.layout {
            MonitorLayout::Fullscreen(window_id) => {
                self.apply_fullscreen(*window_id, is_focused_monitor, registry);
                Overlays::default()
            }
            MonitorLayout::Normal {
                windows,
                containers,
            } => self.apply_normal(
                windows,
                containers,
                hub,
                registry,
                config,
                primary_full_height,
            ),
        }
    }

    fn apply_fullscreen(
        &mut self,
        window_id: crate::core::WindowId,
        is_focused_monitor: bool,
        registry: &mut Registry,
    ) {
        let new_windows: HashSet<_> = registry
            .by_id(window_id)
            .map(|w| w.cg_id())
            .into_iter()
            .collect();

        for cg_id in self.displayed_windows.difference(&new_windows) {
            if let Some(w) = registry.get_mut(*cg_id)
                && let Err(e) = w.hide()
            {
                tracing::trace!("Failed to hide window: {e:#}");
            }
        }
        self.displayed_windows = new_windows;

        let Some(w) = registry.by_id_mut(window_id) else {
            return;
        };
        if w.fullscreen() == FullscreenState::Native {
            if is_focused_monitor {
                w.focus().ok();
            }
        } else {
            w.set_fullscreen(self.screen.dimension);
        }
    }

    fn apply_normal(
        &mut self,
        windows: &[WindowPlacement],
        containers: &[ContainerPlacement],
        hub: &Hub,
        registry: &mut Registry,
        config: &Config,
        primary_full_height: f32,
    ) -> Overlays {
        let new_windows: HashSet<_> = windows
            .iter()
            .filter_map(|p| registry.by_id(p.id).map(|w| w.cg_id()))
            .collect();

        let leaving_native_fs = self
            .displayed_windows
            .difference(&new_windows)
            .any(|cg_id| {
                registry
                    .get(*cg_id)
                    .is_some_and(|w| w.fullscreen() == FullscreenState::Native)
            });

        for cg_id in self.displayed_windows.difference(&new_windows) {
            if let Some(w) = registry.get_mut(*cg_id)
                && let Err(e) = w.hide()
            {
                tracing::trace!("Failed to hide window: {e:#}");
            }
        }
        self.displayed_windows = new_windows;

        for wp in windows {
            let Some(w) = registry.by_id_mut(wp.id) else {
                continue;
            };
            if w.fullscreen() == FullscreenState::Native {
                continue;
            }
            if let Err(e) = w.show(wp, config) {
                tracing::trace!("Failed to set position for window: {e:#}");
            }
        }

        if leaving_native_fs && !windows.iter().any(|wp| wp.is_focused) {
            std::process::Command::new("osascript")
                .arg("-e")
                .arg("tell application \"System Events\" to key code 111 using {command down, control down, shift down, option down}")
                .spawn()
                .ok();
        }

        let mut container_borders = Vec::new();
        let mut tab_bars = Vec::new();

        for cp in containers {
            if cp.is_tabbed {
                let container = hub.get_container(cp.id);
                let titles = collect_tab_titles(container, registry);
                if let Some(tab_bar) = build_tab_bar(
                    cp.visible_frame,
                    cp.id,
                    &titles,
                    cp.active_tab_index,
                    config,
                    primary_full_height,
                ) {
                    tab_bars.push(tab_bar);
                }
            }

            if cp.is_focused
                && let Some(border) = compute_container_border(
                    cp.frame,
                    cp.visible_frame,
                    cp.spawn_mode,
                    config,
                    primary_full_height,
                )
            {
                container_borders.push(ContainerBorder {
                    key: cp.id,
                    frame: border.frame,
                    edges: border.edges,
                });
            }
        }

        Overlays {
            container_borders,
            tab_bars,
        }
    }
}

fn collect_tab_titles(container: &Container, registry: &Registry) -> Vec<String> {
    container
        .children()
        .iter()
        .map(|c| match c {
            Child::Window(wid) => registry
                .by_id(*wid)
                .and_then(|w| w.title())
                .unwrap_or("Unknown")
                .to_owned(),
            Child::Container(_) => "Container".to_owned(),
        })
        .collect()
}

pub(super) struct MonitorRegistry {
    map: HashMap<DisplayId, MonitorEntry>,
    reverse: HashMap<MonitorId, DisplayId>,
    primary_display_id: DisplayId,
}

impl MonitorRegistry {
    pub(super) fn new(primary: &MonitorInfo, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary.display_id,
            MonitorEntry {
                id: primary_monitor_id,
                screen: primary.clone(),
                displayed_windows: HashSet::new(),
            },
        );
        reverse.insert(primary_monitor_id, primary.display_id);
        Self {
            map,
            reverse,
            primary_display_id: primary.display_id,
        }
    }

    pub(super) fn contains(&self, display_id: DisplayId) -> bool {
        self.map.contains_key(&display_id)
    }

    pub(super) fn get(&self, display_id: DisplayId) -> Option<MonitorId> {
        self.map.get(&display_id).map(|e| e.id)
    }

    pub(super) fn get_entry_mut(&mut self, monitor_id: MonitorId) -> Option<&mut MonitorEntry> {
        self.reverse
            .get(&monitor_id)
            .and_then(|d| self.map.get_mut(d))
    }

    pub(super) fn primary_monitor_id(&self) -> MonitorId {
        self.get(self.primary_display_id).unwrap()
    }

    pub(super) fn set_primary_display_id(&mut self, display_id: DisplayId) {
        self.primary_display_id = display_id;
    }

    pub(super) fn replace_primary(&mut self, new_screen: &MonitorInfo) {
        debug_assert!(!self.map.contains_key(&new_screen.display_id));
        if let Some(mut entry) = self.map.remove(&self.primary_display_id) {
            let old = self.primary_display_id;
            let monitor_id = entry.id;
            entry.screen = new_screen.clone();
            self.map.insert(new_screen.display_id, entry);
            self.reverse.insert(monitor_id, new_screen.display_id);
            self.primary_display_id = new_screen.display_id;
            tracing::info!(old, new = new_screen.display_id, "Primary monitor replaced");
        }
    }

    pub(super) fn insert(&mut self, screen: &MonitorInfo, monitor_id: MonitorId) {
        self.map.insert(
            screen.display_id,
            MonitorEntry {
                id: monitor_id,
                screen: screen.clone(),
                displayed_windows: HashSet::new(),
            },
        );
        self.reverse.insert(monitor_id, screen.display_id);
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(display_id) = self.reverse.remove(&monitor_id) {
            self.map.remove(&display_id);
        }
    }

    pub(super) fn remove_stale(&mut self, current: &HashSet<DisplayId>) -> Vec<MonitorId> {
        let stale: Vec<_> = self
            .map
            .iter()
            .filter(|(key, _)| !current.contains(key))
            .map(|(_, e)| e.id)
            .collect();
        for &id in &stale {
            self.remove_by_id(id);
        }
        stale
    }

    pub(super) fn all_screens(&self) -> Vec<MonitorInfo> {
        self.map.values().map(|e| e.screen.clone()).collect()
    }

    pub(super) fn find_monitor_at(&self, x: f32, y: f32) -> Option<&MonitorInfo> {
        self.map
            .values()
            .find(|e| {
                let d = &e.screen.dimension;
                x >= d.x && x < d.x + d.width && y >= d.y && y < d.y + d.height
            })
            .map(|e| &e.screen)
    }

    pub(super) fn update_screen(&mut self, screen: &MonitorInfo) -> Option<(MonitorId, Dimension)> {
        let entry = self.map.get_mut(&screen.display_id)?;
        let old_dim = entry.screen.dimension;
        entry.screen = screen.clone();
        Some((entry.id, old_dim))
    }
}

fn get_display_id(screen: &NSScreen) -> CGDirectDisplayID {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    desc.objectForKey(&key)
        .and_then(|obj| {
            let num: Option<&NSNumber> = obj.downcast_ref();
            num.map(|n| n.unsignedIntValue())
        })
        .unwrap_or(0)
}

pub(super) fn get_all_screens(mtm: MainThreadMarker) -> Vec<MonitorInfo> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen);
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            MonitorInfo {
                display_id,
                name,
                dimension: Dimension {
                    x: bounds.origin.x as f32,
                    y: (bounds.origin.y + top_inset) as f32,
                    width: bounds.size.width as f32,
                    height: (bounds.size.height - top_inset - bottom_inset) as f32,
                },
                full_height: bounds.size.height as f32,
                is_primary: display_id == primary_id,
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}

pub(super) fn primary_full_height_from(monitors: &[MonitorInfo]) -> f32 {
    monitors.iter().find(|s| s.is_primary).unwrap().full_height
}
