use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::overlay::{FloatOverlay, TabBarOverlay, TilingOverlay, WgpuOverlayFactory};
use super::{FloatShow, InternalWindow, InternalWindows};
use crate::config::Appearance;
use crate::core::{ContainerId, ContainerPlacement, MonitorId};
use crate::platform::windows::dome::events::{MonitorScene, TilingWindowShow};
use crate::platform::windows::external::{HwndId, ManageOverlay, ZOrder};

pub(in crate::platform::windows) struct AuxiliaryWindows {
    appearance: Appearance,
    factory: WgpuOverlayFactory,
    monitors: HashMap<MonitorId, MonitorScene>,
    tiling: HashMap<MonitorId, (Box<TilingOverlay>, Arc<dyn ManageOverlay>)>,
    floats: HashMap<HwndId, (Box<FloatOverlay>, Arc<dyn ManageOverlay>)>,
    tabs: HashMap<ContainerId, Box<TabBarOverlay>>,
}

impl AuxiliaryWindows {
    pub(in crate::platform::windows) fn new(
        appearance: Appearance,
        factory: WgpuOverlayFactory,
    ) -> Self {
        Self {
            appearance,
            factory,
            monitors: HashMap::new(),
            tiling: HashMap::new(),
            floats: HashMap::new(),
            tabs: HashMap::new(),
        }
    }
}

impl InternalWindows for AuxiliaryWindows {
    fn set_monitors(&mut self, monitors: &[MonitorScene]) {
        self.monitors = monitors.iter().map(|m| (m.id, m.clone())).collect();
        self.tiling.retain(|id, _| self.monitors.contains_key(id));
        for monitor in monitors {
            if let std::collections::hash_map::Entry::Vacant(entry) = self.tiling.entry(monitor.id)
            {
                match self.factory.create_tiling_overlay(
                    self.appearance.clone(),
                    monitor.work_area,
                    monitor.scale,
                ) {
                    Ok(overlay) => {
                        entry.insert(overlay);
                    }
                    Err(error) => tracing::warn!(%error, "Failed to create tiling overlay"),
                }
            }
        }
    }

    fn show_tiling(
        &mut self,
        monitor: MonitorId,
        windows: &[TilingWindowShow],
        containers: &[ContainerPlacement],
    ) {
        let Some((overlay, _)) = self.tiling.get_mut(&monitor) else {
            return;
        };
        let Some(data) = self.monitors.get(&monitor) else {
            return;
        };
        if windows.is_empty() && containers.is_empty() {
            overlay.clear();
        } else {
            overlay.update(
                data.work_area,
                windows,
                containers,
                data.scale,
                data.border_thickness,
            );
        }
    }

    fn show_floats(&mut self, floats: &[FloatShow]) {
        let live: HashSet<_> = floats.iter().map(|f| f.window).collect();
        self.floats.retain(|key, _| live.contains(key));
        for float in floats {
            if let std::collections::hash_map::Entry::Vacant(entry) =
                self.floats.entry(float.window)
            {
                match self.factory.create_float_overlay(
                    self.appearance.clone(),
                    float.scale,
                    float.placement.visible_border_box,
                    ZOrder::Unchanged,
                ) {
                    Ok(overlay) => {
                        entry.insert(overlay);
                    }
                    Err(error) => tracing::warn!(%error, "Failed to create float overlay"),
                }
            }
            if let Some((overlay, _)) = self.floats.get_mut(&float.window) {
                overlay.update(
                    &float.placement,
                    float.scale,
                    float.border_thickness,
                    float.corner_radius,
                );
            }
        }
    }

    fn show_tab_bars(&mut self, monitors: &[MonitorScene]) {
        let mut live = HashSet::new();
        for data in monitors {
            for placement in data.containers.iter().filter(|p| p.is_tabbed) {
                live.insert(placement.id);
                if let std::collections::hash_map::Entry::Vacant(entry) =
                    self.tabs.entry(placement.id)
                {
                    match self.factory.create_tab_bar(
                        self.appearance.clone(),
                        placement.id,
                        placement.tab_bar_band,
                        data.scale,
                    ) {
                        Ok(overlay) => {
                            entry.insert(overlay);
                        }
                        Err(error) => tracing::warn!(%error, "Failed to create tab bar"),
                    }
                }
                if let Some(tab) = self.tabs.get_mut(&placement.id) {
                    tab.update(
                        placement.tab_bar_band,
                        placement.titles.clone(),
                        placement.active_tab_index,
                        placement.is_highlighted,
                        data.scale,
                        data.border_thickness,
                    );
                }
            }
        }
        self.tabs.retain(|key, _| live.contains(key));
    }

    fn stack(&mut self, window: InternalWindow, order: ZOrder) {
        let handle = match window {
            InternalWindow::TilingOverlay(id) => self.tiling.get(&id).map(|(_, handle)| handle),
            InternalWindow::FloatOverlay(key) => self.floats.get(&key).map(|(_, handle)| handle),
        };
        if let Some(handle) = handle {
            handle.set_z_order(order);
        }
    }

    fn focus_tiling_overlay(&mut self, monitor: MonitorId) {
        if let Some((_, handle)) = self.tiling.get(&monitor) {
            handle.focus();
        }
    }

    fn set_appearance(&mut self, appearance: &Appearance) {
        self.appearance = appearance.clone();
        for (overlay, _) in self.tiling.values_mut() {
            overlay.set_appearance(appearance);
        }
        for (overlay, _) in self.floats.values_mut() {
            overlay.set_appearance(appearance);
        }
        for overlay in self.tabs.values_mut() {
            overlay.set_appearance(appearance);
        }
    }
}
