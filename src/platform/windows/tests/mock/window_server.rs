use super::MockExternalHwnd;
use crate::core::{Physical, PixelRect};
use crate::platform::windows::Wake;
use crate::platform::windows::dome::MonitorInfo;
use crate::platform::windows::external::{HwndId, ZOrder};
use crate::platform::windows::window_server::{
    NativeEvent, Observation, RecoveryEntry, SizeLimitRead, WindowServer,
};
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Instant;

#[derive(Default)]
pub(crate) struct MockWindowServer {
    pub trace: Rc<RefCell<Vec<&'static str>>>,
    pub frames: Vec<(HwndId, PixelRect<Physical>)>,
    pub parked: Vec<HwndId>,
    pub minimized: Vec<HwndId>,
    pub restored: Vec<HwndId>,
    pub orders: Vec<(HwndId, ZOrder)>,
    pub stacking: Rc<RefCell<Vec<(HwndId, ZOrder)>>>,
    pub display_refreshes: usize,
    pub wakes: Rc<RefCell<VecDeque<Wake>>>,
    pub windows: HashMap<HwndId, Arc<MockExternalHwnd>>,
    pub observations: Vec<Observation>,
    pub limits: Vec<HwndId>,
    pub displays: Vec<MonitorInfo>,
    pub native: Vec<NativeEvent>,
    pub shutdown: Vec<RecoveryEntry>,
    pub focus: Vec<HwndId>,
    pub closed: Vec<HwndId>,
    pub shutdown_calls: usize,
    pub read_passes: usize,
}
impl WindowServer for Rc<RefCell<MockWindowServer>> {
    fn ingest_native_events(&mut self, _: Instant) {
        self.borrow().trace.borrow_mut().push("native intake");
        self.borrow_mut().native.clear();
    }
    fn ingest_read_results(&mut self, _: Instant) {
        self.borrow().trace.borrow_mut().push("result intake");
    }
    fn read_settled_windows(&mut self, _: Instant) {
        self.borrow().trace.borrow_mut().push("read pass");
        self.borrow_mut().read_passes += 1;
    }
    fn has_observations(&self) -> bool {
        !self.borrow().observations.is_empty()
    }
    fn take_observations(&mut self) -> Vec<Observation> {
        self.borrow().trace.borrow_mut().push("observations");
        std::mem::take(&mut self.borrow_mut().observations)
    }
    fn next_deadline(&self) -> Option<Instant> {
        None
    }
    fn observe_displays(&mut self, monitors: &[MonitorInfo]) {
        self.borrow().trace.borrow_mut().push("displays");
        self.borrow_mut().display_refreshes += 1;
        self.borrow_mut().displays = monitors.to_vec();
    }
    fn request_size_limits(&mut self, demand: Vec<SizeLimitRead>) {
        if !demand.is_empty() {
            self.borrow().trace.borrow_mut().push("limits demand");
            self.borrow()
                .wakes
                .borrow_mut()
                .push_back(Wake::WindowServer);
        }
        self.borrow_mut()
            .limits
            .extend(demand.into_iter().map(|d| d.window));
    }
    fn read_corner_radii(&mut self, keys: &[HwndId]) -> Vec<Observation> {
        let server = self.borrow();
        keys.iter()
            .filter_map(|&window| {
                server
                    .windows
                    .get(&window)
                    .map(|w| Observation::CornerRadius {
                        window,
                        radius: w.corner_radius(),
                    })
            })
            .collect()
    }
    fn invalidate_foreground(&mut self) {}
    fn set_frame(&mut self, key: HwndId, frame: PixelRect<Physical>) {
        self.borrow_mut().frames.push((key, frame));
        self.borrow().trace.borrow_mut().push("frame");
        if let Some(window) = self.borrow().windows.get(&key) {
            window.set_frame(frame);
        }
    }
    fn set_z_order(&mut self, key: HwndId, order: ZOrder) {
        self.borrow_mut().orders.push((key, order));
        self.borrow().stacking.borrow_mut().push((key, order));
        self.borrow().trace.borrow_mut().push("external stack");
        if let Some(window) = self.borrow().windows.get(&key) {
            window.set_z_order(order);
        }
    }
    fn move_offscreen(&mut self, key: HwndId) {
        self.borrow_mut().parked.push(key);
        self.borrow().trace.borrow_mut().push("park");
        if let Some(window) = self.borrow().windows.get(&key) {
            window.move_offscreen();
        }
    }
    fn minimize(&mut self, key: HwndId) {
        self.borrow_mut().minimized.push(key);
        self.borrow().trace.borrow_mut().push("minimize");
        if let Some(window) = self.borrow().windows.get(&key) {
            window.minimize();
        }
    }
    fn unminimize(&mut self, key: HwndId) {
        self.borrow_mut().restored.push(key);
        self.borrow().trace.borrow_mut().push("restore");
        if let Some(window) = self.borrow().windows.get(&key) {
            window.unminimize();
        }
    }
    fn close(&mut self, key: HwndId) {
        self.borrow().trace.borrow_mut().push("close");
        self.borrow_mut().closed.push(key);
        if let Some(window) = self.borrow().windows.get(&key) {
            window.close();
        }
    }
    fn focus(&mut self, key: HwndId) {
        self.borrow().trace.borrow_mut().push("external focus");
        self.borrow_mut().focus.push(key);
        if let Some(window) = self.borrow().windows.get(&key) {
            window.set_foreground_window();
        }
    }
    fn recover(&mut self, key: HwndId, maximized: bool) {
        if let Some(window) = self.borrow().windows.get(&key) {
            window.recover(maximized);
        }
    }
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        self.borrow_mut().shutdown_calls += 1;
        for entry in &recovery {
            self.recover(entry.window, entry.is_maximized);
        }
        self.borrow_mut().shutdown.extend(recovery);
    }
}
