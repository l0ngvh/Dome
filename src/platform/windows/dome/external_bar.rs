use std::collections::HashMap;
use std::sync::LazyLock;

use crate::config::WindowMatcher;
use crate::core::{MonitorId, Physical, PixelRect, WindowMetadata as _};
use crate::platform::reserve_for_bar;
use crate::platform::windows::external::HwndId;

use super::monitor::{MonitorInfo, MonitorRegistry};
use super::window::WindowsMetadata;

#[derive(Default)]
pub(super) struct StatusBars {
    rects: HashMap<MonitorId, PixelRect<Physical>>,
    hwnd_monitor: HashMap<HwndId, MonitorId>,
}

impl StatusBars {
    pub(super) fn is_known_bar(metadata: &WindowsMetadata) -> bool {
        known_bars()
            .iter()
            .any(|m| metadata.matches_window_matcher(m))
    }

    pub(super) fn capture(
        &mut self,
        hwnd_id: HwndId,
        monitor: MonitorId,
        rect: PixelRect<Physical>,
    ) {
        self.rects.insert(monitor, rect);
        self.hwnd_monitor.insert(hwnd_id, monitor);
    }

    pub(super) fn move_to(
        &mut self,
        hwnd_id: HwndId,
        monitor: MonitorId,
        rect: PixelRect<Physical>,
    ) {
        let Some(&old) = self.hwnd_monitor.get(&hwnd_id) else {
            return;
        };
        if old != monitor {
            self.rects.remove(&old);
            self.hwnd_monitor.insert(hwnd_id, monitor);
        }
        self.rects.insert(monitor, rect);
    }

    pub(super) fn remove(&mut self, hwnd_id: HwndId) -> Option<MonitorId> {
        let monitor = self.hwnd_monitor.remove(&hwnd_id)?;
        self.rects.remove(&monitor);
        Some(monitor)
    }

    pub(super) fn is_tracked(&self, hwnd_id: HwndId) -> bool {
        self.hwnd_monitor.contains_key(&hwnd_id)
    }

    pub(super) fn reserve(&self, monitors: &mut [MonitorInfo], reg: &MonitorRegistry) {
        if self.rects.is_empty() {
            return;
        }
        for m in monitors.iter_mut() {
            let Some(mid) = reg.id_for_handle(m.handle) else {
                continue;
            };
            if let Some(bar) = self.rects.get(&mid).copied() {
                // Bar-edge math is f32 and shared with macOS, so the work area leaves
                // pixel space and comes back. Inward on the way back, so the
                // reservation is never handed back.
                let reserved =
                    reserve_for_bar(m.bounds, m.work_area.to_dimension(), bar.to_dimension());
                m.work_area = PixelRect::from_dimension_inward(reserved);
            }
        }
    }
}

fn known_bars() -> &'static [WindowMatcher] {
    static KNOWN_BARS: LazyLock<[WindowMatcher; 1]> = LazyLock::new(|| {
        [WindowMatcher {
            // Match Zebar by process only, never by class. It shares a
            // generic WebView2 window class with unrelated apps.
            process: Some("zebar.exe".into()),
            title: Some("/^Zebar -.*/".into()),
            class: Some("Tauri Window".into()),
            app: Some("Zebar".into()),
            ..Default::default()
        }]
    });
    &*KNOWN_BARS
}
