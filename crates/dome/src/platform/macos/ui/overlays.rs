use super::ContainerShow;
use super::borders::{FloatBorderOverlay, TilingOverlay, TilingWindowShow};
use super::capture_stream::NativeCaptures;
use super::drawing::WgpuContext;
use super::mirror_overlays::MirrorOverlay;
use super::tab_bar::TabBarOverlay;
use super::window::WindowLevel;
use crate::config::Appearance;
use dome_core::{
    ContainerId, Dimension, FloatWindowPlacement, FractionalLength, Logical, MonitorId, WindowId,
};
use objc2_foundation::NSRect;
use std::collections::HashMap;
use std::rc::Rc;

pub(in crate::platform::macos) trait Overlays {
    fn update(&mut self, placements: OverlayPlacements);
}

pub(in crate::platform::macos) struct OverlayPlacements {
    pub tiling: HashMap<MonitorId, TilingOverlayPlacement>,
    pub tabs: HashMap<ContainerId, (MonitorId, ContainerShow)>,
    pub floats: HashMap<WindowId, FloatBorderPlacement>,
    pub mirrors: HashMap<WindowId, MirrorPlacement>,
    pub appearance: Appearance,
    pub focus_tiling: Option<MonitorId>,
}

pub(in crate::platform::macos) struct TilingOverlayPlacement {
    pub cocoa_frame: NSRect,
    pub monitor: Dimension,
    pub scale: f64,
    pub border_thickness: FractionalLength<Logical>,
    pub windows: Vec<TilingWindowShow>,
    pub containers: Vec<ContainerShow>,
}

#[derive(Clone)]
pub(in crate::platform::macos) struct FloatBorderPlacement {
    pub placement: FloatWindowPlacement,
    pub cocoa_frame: NSRect,
    pub scale: f64,
    pub border_thickness: FractionalLength<Logical>,
    pub corner_radius: FractionalLength<Logical>,
}

pub(in crate::platform::macos) struct MirrorPlacement {
    pub cocoa_frame: NSRect,
    pub source: Dimension,
    pub scale: f64,
    pub level: WindowLevel,
}

pub(in crate::platform::macos) struct NativeOverlays {
    tiling: HashMap<MonitorId, TilingOverlay>,
    tabs: HashMap<ContainerId, TabBarOverlay>,
    floats: HashMap<WindowId, FloatBorderOverlay>,
    mirrors: HashMap<WindowId, MirrorOverlay>,
    gpu: Rc<WgpuContext>,
    captures: Rc<NativeCaptures>,
}

impl NativeOverlays {
    pub(in crate::platform::macos) fn new(
        gpu: Rc<WgpuContext>,
        captures: Rc<NativeCaptures>,
    ) -> Self {
        Self {
            tiling: HashMap::new(),
            tabs: HashMap::new(),
            floats: HashMap::new(),
            mirrors: HashMap::new(),
            gpu,
            captures,
        }
    }
}

impl Overlays for NativeOverlays {
    fn update(&mut self, placements: OverlayPlacements) {
        self.tiling
            .retain(|id, _| placements.tiling.contains_key(id));
        self.tabs.retain(|id, _| placements.tabs.contains_key(id));
        self.floats
            .retain(|id, _| placements.floats.contains_key(id));
        self.mirrors
            .retain(|id, _| placements.mirrors.contains_key(id));
        for (id, placement) in &placements.tiling {
            let overlay = self.tiling.entry(*id).or_insert_with(|| {
                TilingOverlay::new(&self.gpu, &placements.appearance, placement)
            });
            overlay.update(placement, &placements.appearance);
        }
        for (id, (monitor, placement)) in &placements.tabs {
            let tiling = &placements.tiling[monitor];
            let overlay = self.tabs.entry(*id).or_insert_with(|| {
                TabBarOverlay::new(
                    &self.gpu,
                    placements.appearance.clone(),
                    placement,
                    tiling.scale,
                )
            });
            overlay.set_appearance(&placements.appearance);
            overlay.render(placement, tiling.scale, tiling.border_thickness);
        }
        for (id, placement) in &placements.floats {
            let overlay = self.floats.entry(*id).or_insert_with(|| {
                FloatBorderOverlay::new(&self.gpu, &placements.appearance, placement)
            });
            overlay.update(placement, &placements.appearance);
            overlay.paint();
            overlay.set_visible(true);
        }
        for (id, placement) in &placements.mirrors {
            let overlay = self
                .mirrors
                .entry(*id)
                .or_insert_with(|| MirrorOverlay::new(*id, placement, self.captures.clone()));
            overlay.update(placement);
        }
        if let Some(id) = placements.focus_tiling
            && let Some(overlay) = self.tiling.get(&id)
        {
            overlay.focus();
        }
    }
}
