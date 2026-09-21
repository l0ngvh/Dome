use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::Arc;

use super::MockWiring;
use crate::config::Appearance;
use crate::core::{MonitorId, WindowId};
use crate::platform::windows::dome::events::{
    FloatOverlayAction, HubMessage, MonitorSetChange, RenderScene, SceneSender,
};
use crate::platform::windows::external::{HwndId, ManageOverlay, ZOrder};
use crate::platform::windows::tests::env::FocusTarget;

/// Overlay handles that the window thread has reported but the domain has
/// not received yet.
pub(crate) enum OverlayReport {
    Tiling(MonitorId, Arc<dyn ManageOverlay>),
    Float(WindowId, Arc<dyn ManageOverlay>),
}

/// Stands in for `WindowThread`. It records every message the domain sends, and it creates an
/// overlay window for every overlay a message asks for.
pub(crate) struct MockSceneSender {
    log: SceneLog,
    overlays: OverlayRegistry,
}

#[derive(Default)]
struct SceneLog {
    latest: Option<RenderScene>,
    appearance: Option<Appearance>,
}

struct OverlayRegistry {
    tiling: HashMap<MonitorId, HwndId>,
    float: HashMap<WindowId, HwndId>,
    next_tiling_id: isize,
    next_float_id: isize,
    pending: Vec<OverlayReport>,
    wiring: MockWiring,
}

struct MockOverlayHandle {
    overlay_id: HwndId,
    wiring: MockWiring,
}

impl MockSceneSender {
    pub(crate) fn new(wiring: MockWiring) -> Self {
        Self {
            log: SceneLog::default(),
            overlays: OverlayRegistry::new(wiring),
        }
    }

    pub(crate) fn latest_scene(&self) -> &RenderScene {
        self.log
            .latest
            .as_ref()
            .expect("the domain has sent a scene")
    }

    pub(crate) fn appearance(&self) -> Option<Appearance> {
        self.log.appearance.clone()
    }

    pub(crate) fn take_pending_reports(&mut self) -> Vec<OverlayReport> {
        std::mem::take(&mut self.overlays.pending)
    }

    pub(crate) fn tiling_overlay_ids(&self) -> Vec<HwndId> {
        self.overlays.tiling.values().copied().collect()
    }

    pub(crate) fn tiling_overlay_for(&self, monitor: MonitorId) -> Option<HwndId> {
        self.overlays.tiling.get(&monitor).copied()
    }

    pub(crate) fn float_overlay_for(&self, window: WindowId) -> Option<HwndId> {
        self.overlays.float.get(&window).copied()
    }

    pub(crate) fn float_overlay_ids(&self) -> Vec<HwndId> {
        self.overlays.float.values().copied().collect()
    }
}

impl OverlayRegistry {
    fn new(wiring: MockWiring) -> Self {
        Self {
            tiling: HashMap::new(),
            float: HashMap::new(),
            next_tiling_id: 9900,
            next_float_id: 9000,
            pending: Vec::new(),
            wiring,
        }
    }

    fn apply_monitor_change(&mut self, change: MonitorSetChange) {
        for spec in change.added {
            let overlay = HwndId::test(self.next_tiling_id);
            self.next_tiling_id += 1;
            self.wiring.z_stack.simulate_create(overlay);
            self.wiring.z_stack.move_to_bottom(overlay);
            self.tiling.insert(spec.monitor_id, overlay);
            self.pending
                .push(OverlayReport::Tiling(spec.monitor_id, self.handle(overlay)));
        }
        for monitor in change.removed {
            if let Some(overlay) = self.tiling.remove(&monitor) {
                self.wiring.z_stack.remove(overlay);
            }
        }
    }

    fn apply_float_actions(&mut self, scene: &RenderScene) {
        for action in &scene.float_overlays {
            match action {
                FloatOverlayAction::Create {
                    window_id, z_order, ..
                } => self.create_float(*window_id, *z_order),
                FloatOverlayAction::Update { .. } => {}
                FloatOverlayAction::Hide(window_id) => {
                    if let Some(&overlay) = self.float.get(window_id) {
                        self.wiring.z_stack.remove(overlay);
                    }
                }
            }
        }

        let live: HashSet<WindowId> = scene
            .monitors
            .iter()
            .flat_map(|m| m.float_windows.iter().map(|wp| wp.id))
            .collect();
        let z_stack = self.wiring.z_stack.clone();
        self.float.retain(|window, &mut overlay| {
            let keep = live.contains(window);
            if !keep {
                z_stack.remove(overlay);
            }
            keep
        });
    }

    fn create_float(&mut self, window_id: WindowId, z_order: ZOrder) {
        if self.float.contains_key(&window_id) {
            return;
        }
        let overlay = HwndId::test(self.next_float_id);
        self.next_float_id += 1;
        self.wiring.z_stack.simulate_create(overlay);
        self.wiring.z_stack.apply(overlay, z_order);
        self.float.insert(window_id, overlay);
        self.pending
            .push(OverlayReport::Float(window_id, self.handle(overlay)));
    }

    fn handle(&self, overlay: HwndId) -> Arc<dyn ManageOverlay> {
        Arc::new(MockOverlayHandle {
            overlay_id: overlay,
            wiring: self.wiring.clone(),
        })
    }
}

impl SceneSender for Rc<RefCell<MockSceneSender>> {
    fn send(&mut self, msg: HubMessage) {
        let mut sender = self.borrow_mut();
        match msg {
            HubMessage::Scene(scene) => {
                sender.overlays.apply_float_actions(&scene);
                sender.log.latest = Some(scene);
            }
            HubMessage::MonitorsChanged(change) => sender.overlays.apply_monitor_change(change),
            HubMessage::AppearanceChanged(appearance) => {
                sender.log.appearance = Some(appearance);
            }
        }
    }
}

impl ManageOverlay for MockOverlayHandle {
    fn set_z_order(&self, z: ZOrder) {
        self.wiring.z_stack.apply(self.overlay_id, z);
    }

    fn focus(&self) {
        *self.wiring.focus_target.lock().unwrap() = FocusTarget::Overlay;
    }
}
