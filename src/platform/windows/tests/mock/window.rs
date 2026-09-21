use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use super::MockWiring;
use crate::core::{Dimension, Length, LimitObservation, LimitUpdate, PixelRect};
use crate::platform::windows::external::{HwndId, ManageExternalWindow, ShowCmd, ZOrder};
use crate::platform::windows::tests::env::FocusTarget;
use crate::platform::windows::tests::fixtures::OFFSCREEN_POS;

pub(crate) struct MockExternalHwnd {
    pub(crate) hwnd_id: HwndId,
    pub(crate) manageable: bool,
    pub(crate) title: Option<String>,
    pub(crate) process: String,
    pub(crate) class: Option<String>,
    pub(crate) app_name: Option<String>,
    pub(crate) dimension: Mutex<Dimension>,
    pub(crate) minimized: AtomicBool,
    pub(crate) constraints: LimitObservation,
    override_position: Mutex<Option<(i32, i32, i32, i32)>>,
    wiring: MockWiring,
}

impl MockExternalHwnd {
    pub(crate) fn new(id: isize, title: &str, process: &str, wiring: MockWiring) -> Self {
        let hwnd_id = HwndId::test(id);
        wiring.z_stack.simulate_create(hwnd_id);
        Self {
            hwnd_id,
            manageable: true,
            title: Some(title.to_string()),
            process: process.to_string(),
            class: None,
            app_name: None,
            dimension: Mutex::new(Dimension::new(
                Length::ZERO,
                Length::ZERO,
                Length::new(800.0),
                Length::new(600.0),
            )),
            minimized: AtomicBool::new(false),
            // An app that sets no size limits reports each one as Cleared.
            constraints: LimitObservation {
                min_width: LimitUpdate::Cleared,
                min_height: LimitUpdate::Cleared,
                max_width: LimitUpdate::Cleared,
                max_height: LimitUpdate::Cleared,
            },
            override_position: Mutex::new(None),
            wiring,
        }
    }

    pub(crate) fn with_manageable(mut self, manageable: bool) -> Self {
        self.manageable = manageable;
        self
    }

    pub(crate) fn with_class(mut self, class: &str) -> Self {
        self.class = Some(class.to_string());
        self
    }

    pub(crate) fn with_app_name(mut self, app_name: &str) -> Self {
        self.app_name = Some(app_name.to_string());
        self
    }

    pub(crate) fn with_dimension(self, dim: Dimension) -> Self {
        *self.dimension.lock().unwrap() = dim;
        self
    }

    pub(crate) fn with_min_size(mut self, width: f32, height: f32) -> Self {
        let limit = |v: f32| {
            if v > 0.0 {
                LimitUpdate::Set(Length::new(v))
            } else {
                LimitUpdate::Cleared
            }
        };
        self.constraints.min_width = limit(width);
        self.constraints.min_height = limit(height);
        self
    }

    pub(crate) fn set_override_position(&self, pos: Option<(i32, i32, i32, i32)>) {
        *self.override_position.lock().unwrap() = pos;
    }

    pub(crate) fn get_dim(&self) -> Dimension {
        *self.dimension.lock().unwrap()
    }

    pub(crate) fn is_offscreen(&self) -> bool {
        let dim = self.get_dim();
        dim.x <= OFFSCREEN_POS || dim.y <= OFFSCREEN_POS
    }
}

impl ManageExternalWindow for MockExternalHwnd {
    fn id(&self) -> HwndId {
        self.hwnd_id
    }

    fn pid(&self) -> u32 {
        1
    }

    fn set_position(&self, z: ZOrder, rect: PixelRect) {
        self.minimized.store(false, Ordering::Relaxed);
        let dim = self
            .override_position
            .lock()
            .unwrap()
            .map_or(rect.to_dimension(), |pos| {
                Dimension::new(
                    Length::new(pos.0 as f32),
                    Length::new(pos.1 as f32),
                    Length::new(pos.2 as f32),
                    Length::new(pos.3 as f32),
                )
            });
        *self.dimension.lock().unwrap() = dim;
        self.wiring.z_stack.apply(self.hwnd_id, z);
        self.wiring.moves.record(self.hwnd_id, dim);
    }

    fn set_z_order(&self, z: ZOrder) {
        self.wiring.z_stack.apply(self.hwnd_id, z);
    }

    fn move_offscreen(&self) {
        let dim = if let Some((x, y, w, h)) = *self.override_position.lock().unwrap() {
            let d = Dimension::new(
                Length::new(x as f32),
                Length::new(y as f32),
                Length::new(w as f32),
                Length::new(h as f32),
            );
            *self.dimension.lock().unwrap() = d;
            d
        } else {
            let mut d = self.dimension.lock().unwrap();
            d.x = OFFSCREEN_POS;
            d.y = OFFSCREEN_POS;
            *d
        };
        self.wiring.z_stack.move_to_bottom(self.hwnd_id);
        self.wiring.moves.record(self.hwnd_id, dim);
    }

    fn show_cmd(&self, cmd: ShowCmd) {
        match cmd {
            ShowCmd::Minimize => {
                // SW_MINIMIZE parks the window at an iconic-cache rect no test can
                // observe, so the stored rect stays as it is.
                self.minimized.store(true, Ordering::Relaxed);
                let dim = *self.dimension.lock().unwrap();
                self.wiring.moves.record(self.hwnd_id, dim);
            }
            ShowCmd::Restore => {
                self.minimized.store(false, Ordering::Relaxed);
            }
        }
    }

    fn set_foreground_window(&self) {
        *self.wiring.focus_target.lock().unwrap() = FocusTarget::Window(self.hwnd_id);
    }

    fn close(&self) {}

    fn is_maximized(&self) -> bool {
        false
    }

    fn recover(&self, _was_maximized: bool) {
        let mut dim = self.dimension.lock().unwrap();
        dim.x = Length::new(100.0);
        dim.y = Length::new(100.0);
    }
}

impl Drop for MockExternalHwnd {
    fn drop(&mut self) {
        self.wiring.z_stack.remove(self.hwnd_id);
    }
}
