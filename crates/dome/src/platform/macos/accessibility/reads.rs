use std::collections::HashMap;

use anyhow::Result;
use dome_core::{Dimension, PixelRect};
use objc2_core_graphics::CGWindowID;

use super::AXWindow;
use super::application::NativeAccessibility;
use crate::platform::macos::corner_radius::corner_radius;
use crate::platform::macos::window_server::{
    NativeAppObservation, NativeFocusedWindow, NativeWindowObservation, WindowMetadata,
};

impl NativeAccessibility {
    pub(super) fn read_windows(
        &mut self,
        requested_ids: Vec<CGWindowID>,
    ) -> Result<Option<NativeAppObservation>> {
        self.application.refresh_enhanced_ui();
        let listed = self.application.clone().windows()?;
        let observed: HashMap<_, _> = self
            .windows
            .iter()
            .map(|(&id, window)| (id, observe(window)))
            .collect();
        let mut invalid: Vec<_> = observed
            .iter()
            .filter_map(|(&id, observation)| (!observation.valid).then_some(id))
            .collect();
        let mut observations = Vec::new();
        for mut window in listed {
            let id = window.cg_id();
            if let Some(previous) = self.windows.get(&id) {
                window.placement = previous.placement;
            }
            let observation = observe(&window);
            if observation.valid {
                self.windows.insert(id, window);
                observations.push(observation);
                invalid.retain(|invalid_id| *invalid_id != id);
            } else {
                invalid.push(id);
            }
        }
        for id in &invalid {
            self.windows.remove(id);
        }
        let requested = observed
            .into_iter()
            .filter(|(id, _)| requested_ids.contains(id))
            .collect();
        Ok(Some(NativeAppObservation {
            pid: self.application.pid(),
            listed: observations,
            requested,
            requested_ids,
            invalid,
        }))
    }

    pub(super) fn read_focus(&mut self) -> Result<Option<NativeFocusedWindow>> {
        let mut window = self.application.clone().focused_window()?;
        let id = window.cg_id();
        if let Some(previous) = self.windows.get(&id) {
            window.placement = previous.placement;
        }
        let observation = NativeFocusedWindow {
            cg_id: id,
            pid: window.pid(),
            metadata: metadata(&window),
            is_native_fullscreen: window.read_native_fullscreen(),
            position: window.get_position().ok(),
            size: window.get_size().ok(),
        };
        self.windows.insert(id, window);
        Ok(Some(observation))
    }
}

fn metadata(window: &AXWindow) -> WindowMetadata {
    WindowMetadata {
        title: window.title().map(str::to_owned),
        app_name: window.app_name().map(str::to_owned),
        bundle_id: window.bundle_id().map(str::to_owned),
    }
}

fn observe(window: &AXWindow) -> NativeWindowObservation {
    let position = window.get_position().ok();
    let size = window.get_size().ok();
    NativeWindowObservation {
        cg_id: window.cg_id(),
        pid: window.pid(),
        metadata: metadata(window),
        valid: window.is_valid(),
        minimized: window.is_minimized(),
        native_fullscreen: window.read_native_fullscreen(),
        role: window.read_role(),
        subrole: window.read_subrole(),
        is_root: window.is_root_window(),
        position_settable: window.is_position_settable(),
        size_settable: window.is_size_settable(),
        main_settable: window.is_main_settable(),
        rect: position
            .zip(size)
            .map(|((x, y), (w, h))| PixelRect::from_dimension(Dimension::new(x, y, w, h))),
        placement: window.placement,
        corner_radius: position
            .map(|origin| corner_radius(origin, window.read_close_button_frame())),
    }
}
