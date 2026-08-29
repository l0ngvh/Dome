pub(in crate::platform::windows) mod overlay;

use std::collections::{HashMap, HashSet};

use self::overlay::{FloatOverlayApi, TabBarOverlayApi, TilingOverlayApi};
use crate::action::WorkspaceInfo;
use crate::config::Config;
use crate::core::{ContainerId, MonitorId, WindowId};
use crate::platform::windows::dome::CreateOverlay;
use crate::platform::windows::dome::app_window::AppWindowApi;
use crate::platform::windows::dome::events::{
    FloatOverlayAction, HubMessage, MonitorSetChange, PendingPlacement, PlacementAction,
    RenderScene, SceneSender,
};
use crate::platform::windows::external::ZOrder;
use crate::platform::windows::handle::ManageZOrder;

/// Owns every Dome-created window and the only code that touches one. It never reaches
/// the registry, the placement tracker, or the hub, which is what lets it move to its own
/// thread.
pub(in crate::platform::windows) struct WindowThread {
    config: Config,
    overlay_factory: Box<dyn CreateOverlay>,
    tiling_overlays: HashMap<MonitorId, Box<dyn TilingOverlayApi>>,
    tab_bars: HashMap<ContainerId, Box<dyn TabBarOverlayApi>>,
    float_overlays: HashMap<WindowId, Box<dyn FloatOverlayApi>>,
    app_window: Box<dyn AppWindowApi>,
    z_order: Box<dyn ManageZOrder>,
}

impl WindowThread {
    pub(in crate::platform::windows) fn new(
        config: Config,
        overlay_factory: Box<dyn CreateOverlay>,
        app_window: Box<dyn AppWindowApi>,
        z_order: Box<dyn ManageZOrder>,
    ) -> Self {
        Self {
            config,
            overlay_factory,
            tiling_overlays: HashMap::new(),
            tab_bars: HashMap::new(),
            float_overlays: HashMap::new(),
            app_window,
            z_order,
        }
    }

    pub(in crate::platform::windows) fn apply_monitor_change(&mut self, change: MonitorSetChange) {
        for spec in change.added {
            if let Ok(overlay) = self.overlay_factory.create_tiling_overlay(
                self.config.clone(),
                spec.work_area,
                spec.scale,
            ) {
                self.tiling_overlays.insert(spec.monitor_id, overlay);
            }
        }
        for id in change.removed {
            self.tiling_overlays.remove(&id);
        }
    }

    pub(in crate::platform::windows) fn apply_config(&mut self, config: &Config) {
        self.config = config.clone();
        for overlay in self.tiling_overlays.values_mut() {
            overlay.set_config(config);
        }
        for overlay in self.float_overlays.values_mut() {
            overlay.set_config(config);
        }
        for overlay in self.tab_bars.values_mut() {
            overlay.set_config(config);
        }
    }

    pub(in crate::platform::windows) fn apply_placements(
        &mut self,
        placements: &[PendingPlacement],
    ) {
        for placement in placements {
            let ext = &placement.ext;
            match &placement.action {
                PlacementAction::SetPosition { z_order, rect } => ext.set_position(*z_order, *rect),
                PlacementAction::AnchorAboveOverlay {
                    monitor_id,
                    rect,
                    escape_topmost,
                } => {
                    let Some(overlay) = self.tiling_overlays.get(monitor_id) else {
                        continue;
                    };
                    let overlay_hwnd = overlay.id();
                    // Read the reference before the escape write. NotTopmost lands the window at
                    // the top of the normal band, so a later read would return the window itself.
                    let above = self.z_order.window_above(overlay_hwnd);
                    if *escape_topmost {
                        ext.set_position(ZOrder::NotTopmost, *rect);
                    }
                    match above {
                        Some(prev) => ext.set_position(ZOrder::After(prev), *rect),
                        None => {
                            if !*escape_topmost {
                                ext.set_position(ZOrder::Unchanged, *rect);
                            }
                            self.z_order.demote_below(overlay_hwnd, ext.id());
                        }
                    }
                }
                PlacementAction::MoveOffscreen => ext.move_offscreen(),
                PlacementAction::ShowCmd(cmd) => ext.show_cmd(*cmd),
                PlacementAction::SetForegroundWindow => ext.set_foreground_window(),
            }
        }
    }

    #[tracing::instrument(level = "trace", skip_all)]
    pub(in crate::platform::windows) fn apply_scene(&mut self, scene: RenderScene) {
        self.apply_placements(&scene.placements);
        for action in &scene.float_overlays {
            match action {
                FloatOverlayAction::Update {
                    window_id,
                    placement,
                    z_order,
                    scale,
                    border_thickness,
                } => {
                    if !self.float_overlays.contains_key(window_id) {
                        match self.overlay_factory.create_float_overlay(
                            self.config.clone(),
                            *scale,
                            placement.visible_border_box,
                        ) {
                            Ok(o) => {
                                self.float_overlays.insert(*window_id, o);
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
                        .update(placement, *z_order, *scale, *border_thickness);
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
                let rect = placement.tab_bar_band;
                let tab_bar = match self.tab_bars.entry(placement.id) {
                    std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                    std::collections::hash_map::Entry::Vacant(e) => {
                        match self.overlay_factory.create_tab_bar(
                            self.config.clone(),
                            placement.id,
                            rect,
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
                tab_bar.update(
                    rect,
                    placement.titles.clone(),
                    placement.active_tab_index,
                    placement.is_highlighted,
                    data.scale,
                    data.border_thickness,
                );
            }
        }

        let active: HashSet<ContainerId> = scene
            .monitors
            .iter()
            .flat_map(|d| d.containers.iter().filter(|p| p.is_tabbed).map(|p| p.id))
            .collect();
        self.tab_bars.retain(|id, _| active.contains(id));

        if let Some(monitor_id) = scene.focus_monitor
            && let Some(overlay) = self.tiling_overlays.get(&monitor_id)
        {
            overlay.focus();
        }

        self.refresh_tray(&scene.workspaces);
    }

    fn refresh_tray(&self, workspaces: &[WorkspaceInfo]) {
        self.app_window.update_tray(workspaces);
    }
}

impl SceneSender for WindowThread {
    fn send(&mut self, msg: HubMessage) {
        match msg {
            HubMessage::Scene(scene) => self.apply_scene(scene),
            HubMessage::MonitorsChanged(change) => self.apply_monitor_change(change),
            HubMessage::ConfigChanged(config) => self.apply_config(&config),
            HubMessage::Placements(placements) => self.apply_placements(&placements),
        }
    }
}
