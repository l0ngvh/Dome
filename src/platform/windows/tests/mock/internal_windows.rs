use super::MockWiring;
use crate::config::Appearance;
use crate::core::{ContainerPlacement, MonitorId, WindowId};
use crate::platform::windows::dome::events::{MonitorScene, TilingWindowShow};
use crate::platform::windows::external::{HwndId, ZOrder};
use crate::platform::windows::tests::env::FocusTarget;
use crate::platform::windows::ui::{FloatShow, InternalWindow, InternalWindows};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

pub(crate) struct PaintedMonitor {
    pub monitor: MonitorScene,
    pub windows: Vec<TilingWindowShow>,
}
pub(crate) struct MockInternalWindows {
    pub trace: Rc<RefCell<Vec<&'static str>>>,
    pub stacking: Rc<RefCell<Vec<(HwndId, ZOrder)>>>,
    pub tab_monitors: Vec<MonitorScene>,
    pub paint_calls: usize,
    pub stack_calls: usize,
    pub monitors: Vec<PaintedMonitor>,
    pub floats: Vec<FloatShow>,
    pub appearance: Option<Appearance>,
    tiling: HashMap<MonitorId, HwndId>,
    floating: HashMap<HwndId, (WindowId, HwndId)>,
    next_tiling: isize,
    next_float: isize,
    wiring: MockWiring,
}
impl MockInternalWindows {
    pub(crate) fn new(wiring: MockWiring) -> Self {
        Self {
            trace: Rc::default(),
            stacking: Rc::default(),
            tab_monitors: Vec::new(),
            paint_calls: 0,
            stack_calls: 0,
            monitors: Vec::new(),
            floats: Vec::new(),
            appearance: None,
            tiling: HashMap::new(),
            floating: HashMap::new(),
            next_tiling: 9900,
            next_float: 9000,
            wiring,
        }
    }
    pub(crate) fn tiling_overlay_ids(&self) -> Vec<HwndId> {
        self.tiling.values().copied().collect()
    }
    pub(crate) fn tiling_overlay_for(&self, monitor: MonitorId) -> Option<HwndId> {
        self.tiling.get(&monitor).copied()
    }
    pub(crate) fn float_overlay_for(&self, window: WindowId) -> Option<HwndId> {
        self.floating
            .values()
            .find(|(id, _)| *id == window)
            .map(|(_, key)| *key)
    }
    pub(crate) fn float_overlay_ids(&self) -> Vec<HwndId> {
        self.floating.values().map(|(_, key)| *key).collect()
    }
}
impl InternalWindows for Rc<RefCell<MockInternalWindows>> {
    fn set_monitors(&mut self, monitors: &[MonitorScene]) {
        let mut output = self.borrow_mut();
        output.trace.borrow_mut().push("content");
        output.paint_calls += 1;
        let stack = output.wiring.z_stack.clone();
        output.tiling.retain(|id, key| {
            let keep = monitors.iter().any(|m| m.id == *id);
            if !keep {
                stack.remove(*key);
            }
            keep
        });
        for monitor in monitors {
            if !output.tiling.contains_key(&monitor.id) {
                let key = HwndId::test(output.next_tiling);
                output.next_tiling += 1;
                output.tiling.insert(monitor.id, key);
                stack.simulate_create(key);
                stack.move_to_bottom(key);
            }
        }
        output.monitors = monitors
            .iter()
            .map(|monitor| PaintedMonitor {
                monitor: monitor.clone(),
                windows: Vec::new(),
            })
            .collect();
    }
    fn show_tiling(
        &mut self,
        monitor: MonitorId,
        windows: &[TilingWindowShow],
        containers: &[ContainerPlacement],
    ) {
        let mut output = self.borrow_mut();
        let painted = output
            .monitors
            .iter_mut()
            .find(|m| m.monitor.id == monitor)
            .unwrap();
        painted.windows = windows.to_vec();
        painted.monitor.containers = containers.to_vec();
    }
    fn show_floats(&mut self, floats: &[FloatShow]) {
        let mut output = self.borrow_mut();
        let stack = output.wiring.z_stack.clone();
        output.floating.retain(|key, (_, overlay)| {
            let keep = floats.iter().any(|f| f.window == *key);
            if !keep {
                stack.remove(*overlay);
            }
            keep
        });
        for float in floats {
            if !output.floating.contains_key(&float.window) {
                let overlay = HwndId::test(output.next_float);
                output.next_float += 1;
                output
                    .floating
                    .insert(float.window, (float.placement.id, overlay));
                stack.simulate_create(overlay);
            }
        }
        output.floats = floats
            .iter()
            .map(|f| FloatShow {
                window: f.window,
                placement: f.placement,
                scale: f.scale,
                border_thickness: f.border_thickness,
                corner_radius: f.corner_radius,
            })
            .collect();
    }
    fn show_tab_bars(&mut self, monitors: &[MonitorScene]) {
        self.borrow_mut().tab_monitors = monitors.to_vec();
    }
    fn stack(&mut self, window: InternalWindow, order: ZOrder) {
        let mut output = self.borrow_mut();
        output.trace.borrow_mut().push("internal stack");
        output.stack_calls += 1;
        let overlay = match window {
            InternalWindow::TilingOverlay(id) => output.tiling.get(&id).copied(),
            InternalWindow::FloatOverlay(key) => {
                output.floating.get(&key).map(|(_, overlay)| *overlay)
            }
        };
        if let Some(overlay) = overlay {
            output.stacking.borrow_mut().push((overlay, order));
            output.wiring.z_stack.apply(overlay, order);
        }
    }
    fn focus_tiling_overlay(&mut self, _: MonitorId) {
        self.borrow().trace.borrow_mut().push("overlay focus");
        *self.borrow().wiring.focus_target.lock().unwrap() = FocusTarget::Overlay;
    }
    fn set_appearance(&mut self, appearance: &Appearance) {
        self.borrow_mut().appearance = Some(appearance.clone());
    }
}
