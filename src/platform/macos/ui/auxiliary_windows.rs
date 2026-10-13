use std::cell::OnceCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use dispatch2::{DispatchQueue, DispatchRetained};
use objc2::MainThreadMarker;
use objc2_core_graphics::CGWindowID;

use super::internal_windows::{ContainerShow, FloatShow, InternalWindows, MonitorTilingData};
use super::mirror::{WindowCapture, create_captures_async};
use super::overlay::{FloatOverlay, TabBarOverlay, TilingOverlay};
use super::{CaptureMessage, CaptureSender};
use crate::config::Appearance;
use crate::core::Length;
use crate::core::{ContainerId, Dimension, MonitorId};
use crate::platform::macos::dome::events::{HubEvent, MonitorScene};
use crate::platform::macos::objc2_wrapper::dimension_to_ns_rect_cocoa;
use crate::platform::render::WgpuContext;

pub(super) struct AuxiliaryWindows {
    capture_sender: Rc<OnceCell<CaptureSender>>,
    capture_queue: DispatchRetained<DispatchQueue>,
    tiling_overlays: HashMap<MonitorId, TilingOverlay>,
    tab_bar_overlays: HashMap<ContainerId, TabBarOverlay>,
    float_overlays: HashMap<CGWindowID, FloatOverlay>,
    captures: HashMap<CGWindowID, WindowCapture>,
    capture_targets: HashMap<CGWindowID, (Dimension, f64, bool)>,
    gpu: Rc<WgpuContext>,
    appearance: Appearance,
    hub_sender: calloop::channel::Sender<HubEvent>,
}

impl AuxiliaryWindows {
    pub(super) fn new(
        gpu: Rc<WgpuContext>,
        appearance: Appearance,
        hub_sender: calloop::channel::Sender<HubEvent>,
        capture_sender: Rc<OnceCell<CaptureSender>>,
    ) -> Self {
        Self {
            capture_sender,
            capture_queue: DispatchQueue::new("dome.capture", None),
            tiling_overlays: HashMap::new(),
            tab_bar_overlays: HashMap::new(),
            float_overlays: HashMap::new(),
            captures: HashMap::new(),
            capture_targets: HashMap::new(),
            gpu,
            appearance,
            hub_sender,
        }
    }
}

impl InternalWindows for AuxiliaryWindows {
    fn set_monitors(&mut self, monitors: &[MonitorScene]) {
        let mtm = MainThreadMarker::new().expect("internal windows run on UI");
        let height = monitors
            .iter()
            .find(|m| m.is_primary)
            .map_or(0.0, |m| m.full_height);
        for monitor in monitors {
            self.tiling_overlays.entry(monitor.id).or_insert_with(|| {
                TilingOverlay::new(
                    mtm,
                    &self.gpu,
                    self.appearance.clone(),
                    dimension_to_ns_rect_cocoa(
                        Length::new(height),
                        monitor.work_area.to_dimension(),
                    ),
                    monitor.scale,
                )
            });
        }
        self.tiling_overlays
            .retain(|id, _| monitors.iter().any(|m| m.id == *id));
    }

    fn show_tiling(&mut self, data: &MonitorTilingData) {
        let Some(overlay) = self.tiling_overlays.get_mut(&data.monitor_id) else {
            return;
        };
        overlay.set_border_thickness(data.border_thickness);
        if data.windows.is_empty() && data.containers.is_empty() {
            overlay.clear();
        } else {
            overlay.render(
                data.cocoa_frame,
                data.scale,
                data.monitor_dim,
                &data.windows,
                &data.containers,
            );
        }
    }

    fn show_floats(&mut self, floats: &[FloatShow], focused: Option<CGWindowID>) {
        let mtm = MainThreadMarker::new().expect("internal windows run on UI");
        let mut capture_pairs = Vec::new();
        for show in floats {
            let is_new = !self.float_overlays.contains_key(&show.cg_id);
            let overlay = self.float_overlays.entry(show.cg_id).or_insert_with(|| {
                FloatOverlay::new(
                    mtm,
                    show.cocoa_frame,
                    show.cg_id,
                    self.hub_sender.clone(),
                    &self.gpu,
                    self.appearance.theme,
                    &self.appearance.font,
                )
            });
            let is_focused = focused == Some(show.cg_id);
            overlay.render(show, is_focused);
            self.capture_targets
                .insert(show.cg_id, (show.content_dim, show.scale, is_focused));
            if is_new {
                capture_pairs.push(show.cg_id);
            }
            if let Some(capture) = self.captures.get_mut(&show.cg_id) {
                if is_focused {
                    capture.stop();
                } else {
                    capture.start(show.cg_id, show.content_dim, show.scale);
                }
            }
        }
        if !capture_pairs.is_empty() {
            create_captures_async(
                capture_pairs,
                self.capture_queue.clone(),
                self.capture_sender
                    .get()
                    .expect("capture sender bound before scenes")
                    .clone(),
            );
        }
        let active: HashSet<_> = floats.iter().map(|s| s.cg_id).collect();
        self.float_overlays.retain(|key, _| active.contains(key));
        self.captures.retain(|key, _| active.contains(key));
        self.capture_targets.retain(|key, _| active.contains(key));
    }

    fn show_tab_bars(&mut self, containers: &[ContainerShow]) {
        let mtm = MainThreadMarker::new().expect("internal windows run on UI");
        let mut active = HashSet::new();
        for container in containers {
            if !container.placement.is_tabbed || container.placement.titles.is_empty() {
                continue;
            }
            let overlay = self
                .tab_bar_overlays
                .entry(container.placement.id)
                .or_insert_with(|| {
                    TabBarOverlay::new(
                        mtm,
                        &self.gpu,
                        self.appearance.clone(),
                        container.placement.id,
                        container.tab_bar_cocoa_frame,
                        container.scale,
                        self.hub_sender.clone(),
                    )
                });
            overlay.render(container, container.scale, container.border_thickness);
            active.insert(container.placement.id);
        }
        self.tab_bar_overlays.retain(|key, _| active.contains(key));
    }

    fn apply_capture_messages(&mut self, messages: Vec<CaptureMessage>) {
        for message in messages {
            match message {
                CaptureMessage::Ready { cg_id, mut capture } => {
                    if self.float_overlays.contains_key(&cg_id) {
                        if let Some(&(dimension, scale, false)) = self.capture_targets.get(&cg_id) {
                            capture.start(cg_id, dimension, scale);
                        }
                        self.captures.insert(cg_id, capture);
                    }
                }
                CaptureMessage::Frame { cg_id, surface } => {
                    if let Some(overlay) = self.float_overlays.get_mut(&cg_id) {
                        overlay.apply_frame(&surface);
                    }
                }
            }
        }
    }

    fn focus_tiling_overlay(&mut self, monitor: MonitorId) {
        if let Some(overlay) = self.tiling_overlays.get(&monitor) {
            overlay.focus(MainThreadMarker::new().expect("internal focus runs on UI"));
        }
    }

    fn set_appearance(&mut self, appearance: &Appearance) {
        self.appearance = appearance.clone();
        for overlay in self.float_overlays.values_mut() {
            overlay.set_appearance(appearance);
        }
        for overlay in self.tiling_overlays.values_mut() {
            overlay.set_appearance(appearance);
        }
        for overlay in self.tab_bar_overlays.values() {
            overlay.set_appearance(appearance);
        }
    }
}
