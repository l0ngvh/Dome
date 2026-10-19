use super::borders::{FloatBorderOverlay, TilingOverlay};
use super::drawing::WgpuContext;
use super::tab_bar::TabBarOverlay;
use super::thumbnail_overlays::ThumbnailOverlay;
use super::{FloatBorderRequest, FloatBorderState, OverlayPlacements, Overlays};
use crate::platform::windows::event_loop::native::CallbackPanic;
use crate::platform::windows::event_loop::{EventSender, NativeEvent};
use crate::platform::windows::window_server::NativeWindowServer;
use dome_core::{ContainerId, MonitorId, WindowId};
use std::collections::{HashMap, hash_map::Entry};
use std::rc::Rc;

pub(in crate::platform::windows) struct NativeOverlays {
    tiling: HashMap<MonitorId, TilingOverlay>,
    tabs: HashMap<ContainerId, TabBarOverlay>,
    floats: HashMap<WindowId, FloatBorderOverlay>,
    thumbnails: HashMap<WindowId, ThumbnailOverlay>,
    graphics: WgpuContext,
    server: Rc<NativeWindowServer>,
    callback_panic: Rc<CallbackPanic>,
    dpi_changes: EventSender<NativeEvent>,
}

impl NativeOverlays {
    pub(in crate::platform::windows) fn new(
        callback_panic: Rc<CallbackPanic>,
        dpi_changes: EventSender<NativeEvent>,
        graphics: WgpuContext,
        server: Rc<NativeWindowServer>,
    ) -> Self {
        Self {
            tiling: HashMap::new(),
            tabs: HashMap::new(),
            floats: HashMap::new(),
            thumbnails: HashMap::new(),
            graphics,
            server,
            callback_panic,
            dpi_changes,
        }
    }

    fn remove_absent_resources(&mut self, placements: &OverlayPlacements) {
        self.tiling
            .retain(|id, _| placements.tiling.contains_key(id));
        self.tabs.retain(|id, _| placements.tabs.contains_key(id));
        self.floats
            .retain(|id, _| placements.floats.contains_key(id));
        self.thumbnails
            .retain(|id, _| placements.thumbnails.contains_key(id));
    }

    fn reconcile_tiling_and_tabs(&mut self, placements: &OverlayPlacements) {
        for (id, placement) in &placements.tiling {
            let overlay = match self.tiling.entry(*id) {
                Entry::Vacant(entry) => match TilingOverlay::new(
                    self.callback_panic.clone(),
                    self.dpi_changes.clone(),
                    &self.graphics,
                    placements.appearance.clone(),
                    placement,
                ) {
                    Ok(overlay) => entry.insert(overlay),
                    Err(error) => {
                        tracing::warn!(?error, "failed to create tiling overlay");
                        continue;
                    }
                },
                Entry::Occupied(entry) => {
                    let overlay = entry.into_mut();
                    overlay.set_appearance(&placements.appearance);
                    overlay.update(
                        placement.monitor,
                        &placement.windows,
                        &placement.containers,
                        placement.scale,
                        placement.border_thickness,
                    );
                    overlay
                }
            };
            // The overlay must follow its managed reference even after a resize.
            overlay.set_z_order(self.server.native_order(placement.order));
        }
        self.tabs.retain(|id, _| {
            placements
                .tabs
                .get(id)
                .is_some_and(|(monitor, _)| self.tiling.contains_key(monitor))
        });
        for (id, (monitor, placement)) in &placements.tabs {
            if !self.tiling.contains_key(monitor) {
                continue;
            }
            let monitor = &placements.tiling[monitor];
            let overlay = match self.tabs.entry(*id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => match TabBarOverlay::new(
                    self.callback_panic.clone(),
                    &self.graphics,
                    placements.appearance.clone(),
                    *id,
                    placement.visible_tab_bar_band,
                    monitor.scale,
                ) {
                    Ok(overlay) => entry.insert(overlay),
                    Err(error) => {
                        tracing::warn!(?error, "failed to create tab bar");
                        continue;
                    }
                },
            };
            overlay.set_appearance(&placements.appearance);
            overlay.update(placement, monitor.scale, monitor.border_thickness);
        }
    }

    fn reconcile_floats(&mut self, placements: &OverlayPlacements) {
        for (id, request) in &placements.floats {
            let FloatBorderRequest::Present(state) = request else {
                continue;
            };
            let (FloatBorderState::Visible(placement) | FloatBorderState::Hidden(placement)) =
                state;
            let overlay = match self.floats.entry(*id) {
                Entry::Occupied(entry) => entry.into_mut(),
                Entry::Vacant(entry) => match FloatBorderOverlay::new(
                    self.callback_panic.clone(),
                    &self.graphics,
                    placements.appearance.clone(),
                    placement.placement.visible_border_box,
                ) {
                    Ok(overlay) => entry.insert(overlay),
                    Err(error) => {
                        tracing::warn!(?error, "failed to create float overlay");
                        continue;
                    }
                },
            };
            overlay.set_appearance(&placements.appearance);
            overlay.update(
                &placement.placement,
                placement.scale,
                placement.border_thickness,
                placement.corner_radius,
                matches!(state, FloatBorderState::Visible(_)),
            );
            overlay.set_z_order(self.server.native_order(placement.order));
        }
    }

    fn reconcile_thumbnails(&mut self, placements: &OverlayPlacements) {
        for (id, placement) in &placements.thumbnails {
            match self.thumbnails.entry(*id) {
                Entry::Occupied(mut entry) => entry.get_mut().update(placement),
                Entry::Vacant(entry) => match ThumbnailOverlay::new(
                    self.callback_panic.clone(),
                    &self.server,
                    *id,
                    placement,
                ) {
                    Ok(overlay) => {
                        entry.insert(overlay);
                    }
                    Err(error) => tracing::warn!(?error, "failed to create thumbnail"),
                },
            }
        }
    }
}

impl Overlays for NativeOverlays {
    fn update(&mut self, placements: OverlayPlacements) {
        self.remove_absent_resources(&placements);
        self.reconcile_tiling_and_tabs(&placements);
        self.reconcile_floats(&placements);
        self.reconcile_thumbnails(&placements);
        if let Some(monitor) = placements.focus_tiling
            && let Some(overlay) = self.tiling.get(&monitor)
        {
            overlay.focus();
        }
    }
}
