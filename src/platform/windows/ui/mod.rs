pub(in crate::platform::windows) mod overlay;

use std::collections::{HashMap, HashSet};

use self::overlay::{FloatOverlay, TabBarOverlay, TilingOverlay, WgpuOverlayFactory};
use crate::config::Appearance;
use crate::core::{ContainerId, MonitorId, WindowId};
use crate::platform::windows::HubSender;
use crate::platform::windows::dome::events::{
    FloatOverlayAction, HubMessage, MonitorSetChange, RenderScene, SceneSender,
};

/// Owns every Dome-created window and the only code that touches one.
pub(in crate::platform::windows) struct WindowThread {
    appearance: Appearance,
    overlay_factory: WgpuOverlayFactory,
    tiling_overlays: HashMap<MonitorId, Box<TilingOverlay>>,
    tab_bars: HashMap<ContainerId, Box<TabBarOverlay>>,
    float_overlays: HashMap<WindowId, Box<FloatOverlay>>,
    report: HubSender,
}

impl WindowThread {
    pub(in crate::platform::windows) fn new(
        appearance: Appearance,
        overlay_factory: WgpuOverlayFactory,
        report: HubSender,
    ) -> Self {
        Self {
            appearance,
            overlay_factory,
            tiling_overlays: HashMap::new(),
            tab_bars: HashMap::new(),
            float_overlays: HashMap::new(),
            report,
        }
    }

    pub(in crate::platform::windows) fn apply_monitor_change(&mut self, change: MonitorSetChange) {
        for spec in change.added {
            if let Ok((overlay, handle)) = self.overlay_factory.create_tiling_overlay(
                self.appearance.clone(),
                spec.work_area,
                spec.scale,
            ) {
                self.report.tiling_overlay_ready(spec.monitor_id, handle);
                self.tiling_overlays.insert(spec.monitor_id, overlay);
            }
        }
        for id in change.removed {
            self.tiling_overlays.remove(&id);
        }
    }

    pub(in crate::platform::windows) fn apply_appearance(&mut self, appearance: &Appearance) {
        self.appearance = appearance.clone();
        for overlay in self.tiling_overlays.values_mut() {
            overlay.set_appearance(appearance);
        }
        for overlay in self.float_overlays.values_mut() {
            overlay.set_appearance(appearance);
        }
        for overlay in self.tab_bars.values_mut() {
            overlay.set_appearance(appearance);
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(in crate::platform::windows) fn apply_scene(&mut self, scene: RenderScene) {
        for action in &scene.float_overlays {
            match action {
                FloatOverlayAction::Create {
                    window_id,
                    placement,
                    z_order,
                    scale,
                    border_thickness,
                } => {
                    if !self.float_overlays.contains_key(window_id) {
                        match self.overlay_factory.create_float_overlay(
                            self.appearance.clone(),
                            *scale,
                            placement.visible_border_box,
                            *z_order,
                        ) {
                            Ok((overlay, handle)) => {
                                self.report.float_overlay_ready(*window_id, handle);
                                self.float_overlays.insert(*window_id, overlay);
                            }
                            Err(e) => {
                                tracing::warn!("Failed to create float overlay: {e:#}");
                                continue;
                            }
                        }
                    }
                    self.float_overlays
                        .get_mut(window_id)
                        .expect("float overlay inserted above")
                        .update(placement, *scale, *border_thickness);
                }
                FloatOverlayAction::Update {
                    window_id,
                    placement,
                    scale,
                    border_thickness,
                } => {
                    if let Some(overlay) = self.float_overlays.get_mut(window_id) {
                        overlay.update(placement, *scale, *border_thickness);
                    }
                }
                FloatOverlayAction::Hide(window_id) => {
                    if let Some(overlay) = self.float_overlays.get_mut(window_id) {
                        overlay.hide();
                    }
                }
            }
        }

        let live_float_windows: HashSet<WindowId> = scene
            .monitors
            .iter()
            .flat_map(|m| m.float_windows.iter().map(|wp| wp.id))
            .collect();
        self.float_overlays
            .retain(|id, _| live_float_windows.contains(id));

        for data in &scene.monitors {
            if !self.tiling_overlays.contains_key(&data.monitor_id) {
                continue;
            }
            if data.tiling_windows.is_empty() && data.containers.is_empty() {
                self.tiling_overlays
                    .get_mut(&data.monitor_id)
                    .unwrap()
                    .clear();
                continue;
            }
            self.tiling_overlays
                .get_mut(&data.monitor_id)
                .unwrap()
                .update(
                    data.work_area,
                    &data.tiling_windows,
                    &data.containers,
                    data.scale,
                    data.border_thickness,
                );
            for placement in data.containers.iter().filter(|p| p.is_tabbed) {
                let tab_bar = match self.tab_bars.entry(placement.id) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        match self.overlay_factory.create_tab_bar(
                            self.appearance.clone(),
                            placement.id,
                            placement.visible_tab_bar_band,
                            data.scale,
                        ) {
                            Ok(o) => e.insert(o),
                            Err(err) => {
                                tracing::warn!(?err, "failed to create tab bar");
                                continue;
                            }
                        }
                    }
                };
                tab_bar.update(placement, data.scale, data.border_thickness);
            }
        }

        let active: HashSet<ContainerId> = scene
            .monitors
            .iter()
            .flat_map(|d| d.containers.iter().filter(|p| p.is_tabbed).map(|p| p.id))
            .collect();
        self.tab_bars.retain(|id, _| active.contains(id));
    }
}

impl SceneSender for WindowThread {
    fn send(&mut self, msg: HubMessage) {
        match msg {
            HubMessage::Scene(scene) => self.apply_scene(scene),
            HubMessage::MonitorsChanged(change) => self.apply_monitor_change(change),
            HubMessage::AppearanceChanged(appearance) => self.apply_appearance(&appearance),
        }
    }
}
