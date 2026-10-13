use super::window::{PlacementProgress, PlacementTarget};
use crate::core::{Length, Logical, Physical, PixelRect, WindowId};
use crate::platform::windows::external::HwndId;
use crate::platform::windows::window_server::{DiscoveredWindow, FullscreenKind};
use std::collections::HashMap;

pub(super) struct ManagedWindow {
    pub admitted: Option<WindowId>,
    pub actual: PixelRect<Physical>,
    pub monitor: isize,
    pub is_maximized: bool,
    pub minimized: bool,
    pub fullscreen: Option<FullscreenKind>,
    pub dome_minimized: bool,
    pub corner_radius: Length<Logical>,
    pub target: Option<PlacementTarget>,
    pub placement: PlacementProgress,
    pub awaiting_response: bool,
}

#[derive(Default)]
pub(super) struct WindowRegistry {
    pub windows: HashMap<HwndId, ManagedWindow>,
}
impl WindowRegistry {
    pub(super) fn get(&self, key: HwndId) -> Option<&ManagedWindow> {
        self.windows.get(&key)
    }
    pub(super) fn by_id(&self, id: WindowId) -> Option<(HwndId, &ManagedWindow)> {
        self.windows
            .iter()
            .find(|(_, entry)| entry.admitted == Some(id))
            .map(|(&key, entry)| (key, entry))
    }
    pub(super) fn discover(&mut self, discovery: &DiscoveredWindow) {
        self.windows
            .entry(discovery.key)
            .or_insert_with(|| ManagedWindow {
                admitted: None,
                actual: discovery.frame,
                monitor: discovery.monitor,
                is_maximized: discovery.is_maximized,
                minimized: false,
                fullscreen: discovery.fullscreen,
                dome_minimized: false,
                corner_radius: Length::ZERO,
                target: None,
                placement: PlacementProgress::default(),
                awaiting_response: false,
            });
    }
}
