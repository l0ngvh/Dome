use std::collections::HashMap;
use std::sync::Arc;

use objc2_core_graphics::CGWindowID;

use super::window::{PlacementTarget, WindowState};
use crate::core::{Length, PixelRect, WindowId};
use crate::platform::macos::dome::events::{DiscoveredWindow, FullscreenKind};
use crate::platform::macos::window_server::ApplicationLifetime;

pub(super) struct ManagedWindow {
    pub(super) lifetime: Arc<ApplicationLifetime>,
    pub(super) discovery: DiscoveredWindow,
    pub(super) hidden: bool,
    pub(super) admitted: Option<WindowId>,
    pub(super) actual: Option<PixelRect>,
    pub(super) minimized: bool,
    pub(super) fullscreen: Option<FullscreenKind>,
    pub(super) corner_radius: Length,
    pub(super) target: Option<PlacementTarget>,
    pub(super) state: WindowState,
    pub(super) awaiting_response: bool,
}

#[derive(Default)]
pub(super) struct WindowRegistry {
    pub(super) windows: HashMap<CGWindowID, ManagedWindow>,
}

impl WindowRegistry {
    pub(super) fn get(&self, key: CGWindowID) -> Option<&ManagedWindow> {
        self.windows.get(&key)
    }

    pub(super) fn discover(
        &mut self,
        lifetime: &Arc<ApplicationLifetime>,
        discovery: &DiscoveredWindow,
    ) {
        self.windows
            .entry(discovery.key)
            .or_insert_with(|| ManagedWindow {
                lifetime: lifetime.clone(),
                discovery: discovery.clone(),
                hidden: false,
                admitted: None,
                actual: discovery.frame,
                minimized: discovery.minimized,
                fullscreen: discovery.fullscreen,
                corner_radius: discovery.corner_radius,
                target: None,
                state: WindowState::Unplaced,
                awaiting_response: false,
            });
    }
}
