use std::collections::{HashMap, HashSet};

use crate::config::Appearance;
use crate::platform::macos::objc2_wrapper::dimension_to_ns_rect_cocoa;
use crate::platform::macos::ui::window::WindowLevel;
use crate::platform::macos::ui::{
    ContainerShow, FloatBorderPlacement, MirrorPlacement, OverlayPlacements,
    TilingOverlayPlacement, TilingWindowShow,
};
use dome_core::{
    Dimension, FractionalCoordinate, FractionalLength, MonitorId, MonitorLayout, MonitorPlacements,
    PixelRect, TilingWindowPlacement, VisiblePlacements, WindowId,
};

use super::Dome;

impl Dome {
    pub(in crate::platform::macos) fn apply_appearance(&mut self, appearance: &Appearance) {
        self.appearance = appearance.clone();
    }

    /// Fullscreen transitions must be resolved before applying placements.
    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn flush_layout(&mut self) {
        let result = self.hub.get_visible_placements();
        self.apply_visible_placements(&result);
    }

    pub(super) fn apply_visible_placements(&mut self, visible: &VisiblePlacements) {
        self.place_application_windows(visible);
        let mut placements = self.overlay_placements(visible);
        let target = self.focus_target(visible);
        placements.focus_tiling = match target {
            Some(FocusTarget::TilingOverlay(monitor)) => Some(monitor),
            _ => None,
        };
        if matches!(target, Some(FocusTarget::TilingOverlay(_))) {
            self.window_server.focus_overlay();
        }
        self.overlays.update(placements);
        if let Some(FocusTarget::Window(window)) = target {
            self.apply_native_focus(window);
        }
    }

    fn reconcile_visibility(&mut self, monitors: &[MonitorPlacements]) {
        let visible_windows: HashSet<WindowId> = monitors
            .iter()
            .flat_map(|monitor| match &monitor.layout {
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => tiling_windows
                    .iter()
                    .map(|placement| placement.id)
                    .chain(float_windows.iter().map(|placement| placement.id))
                    .collect::<Vec<_>>(),
                MonitorLayout::Fullscreen(id) => vec![*id],
            })
            .collect();
        let to_hide: Vec<_> = self
            .displayed_windows
            .difference(&visible_windows)
            .copied()
            .collect();
        for id in to_hide {
            self.hide_window(id);
        }
        self.displayed_windows = visible_windows;
    }

    fn place_application_windows(&mut self, visible: &VisiblePlacements) {
        self.reconcile_visibility(&visible.monitors);
        for monitor in &visible.monitors {
            match &monitor.layout {
                MonitorLayout::Fullscreen(id) => {
                    self.place_fullscreen_window(*id, monitor.work_area)
                }
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => {
                    for placement in tiling_windows {
                        if placement.visible_content_box.is_empty() {
                            self.move_window_offscreen(placement.id);
                        } else if shows_through_mirror(placement) {
                            self.park_mirrored_window(placement.id, placement.content_box);
                        } else {
                            // macOS rejects some partially off-screen frames, notably above the menu bar.
                            self.show_tiling(placement.id, placement.visible_content_box);
                        }
                    }
                    for placement in float_windows {
                        if !placement.content_box.is_empty()
                            && visible.focused_window == Some(placement.id)
                        {
                            self.show_float(placement.id, placement.content_box);
                        } else {
                            self.move_window_offscreen(placement.id);
                        }
                    }
                }
            }
        }
    }

    fn overlay_placements(&self, visible: &VisiblePlacements) -> OverlayPlacements {
        let mut result = OverlayPlacements {
            tiling: HashMap::new(),
            tabs: HashMap::new(),
            floats: HashMap::new(),
            mirrors: HashMap::new(),
            appearance: self.appearance.clone(),
            focus_tiling: None,
        };
        let cocoa_frame = |dimension| {
            dimension_to_ns_rect_cocoa(FractionalLength::new(self.primary_full_height), dimension)
        };
        for monitor in &visible.monitors {
            let scale = self
                .monitor_registry
                .monitor(monitor.monitor_id)
                .backing_scale;
            let border_thickness = FractionalLength::from_pixels(monitor.border_thickness);
            let monitor_dimension = monitor.work_area.to_dimension();
            let mut tiling = TilingOverlayPlacement {
                cocoa_frame: cocoa_frame(monitor_dimension),
                monitor: monitor_dimension,
                scale,
                border_thickness,
                windows: Vec::new(),
                containers: Vec::new(),
            };
            if let MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } = &monitor.layout
            {
                for placement in tiling_windows {
                    if placement.visible_content_box.is_empty() {
                        continue;
                    }
                    let Some(entry) = self.registry.get(placement.id) else {
                        continue;
                    };
                    tiling.windows.push(TilingWindowShow {
                        placement: *placement,
                        corner_radius: entry.corner_radius,
                    });
                    if shows_through_mirror(placement) {
                        result.mirrors.insert(
                            placement.id,
                            MirrorPlacement {
                                cocoa_frame: cocoa_frame(
                                    placement.visible_content_box.to_dimension(),
                                ),
                                source: on_screen_part(placement),
                                scale,
                                level: WindowLevel::Bottom,
                            },
                        );
                    }
                }
                for placement in float_windows {
                    if placement.content_box.is_empty() {
                        continue;
                    }
                    let Some(entry) = self.registry.get(placement.id) else {
                        continue;
                    };
                    result.floats.insert(
                        placement.id,
                        FloatBorderPlacement {
                            placement: *placement,
                            cocoa_frame: cocoa_frame(placement.border_box.to_dimension()),
                            scale,
                            border_thickness,
                            corner_radius: entry.corner_radius,
                        },
                    );
                    if visible.focused_window != Some(placement.id) {
                        let content = placement.content_box.to_dimension();
                        result.mirrors.insert(
                            placement.id,
                            MirrorPlacement {
                                cocoa_frame: cocoa_frame(content),
                                source: Dimension::new(
                                    FractionalCoordinate::ZERO,
                                    FractionalCoordinate::ZERO,
                                    content.width,
                                    content.height,
                                ),
                                scale,
                                level: WindowLevel::Floating,
                            },
                        );
                    }
                }
                for placement in containers {
                    let show = ContainerShow {
                        placement: placement.clone(),
                        tab_bar_dim: placement.tab_bar_band.to_dimension(),
                        tab_bar_cocoa_frame: cocoa_frame(
                            placement.visible_tab_bar_band.to_dimension(),
                        ),
                    };
                    if placement.is_tabbed && !placement.titles.is_empty() {
                        result
                            .tabs
                            .insert(placement.id, (monitor.monitor_id, show.clone()));
                    }
                    tiling.containers.push(show);
                }
            }
            result.tiling.insert(monitor.monitor_id, tiling);
        }
        result
    }

    fn focus_target(&mut self, visible: &VisiblePlacements) -> Option<FocusTarget> {
        let focused_window = visible.focused_window;
        let focused_monitor = visible.focused_monitor;
        let focus_empty = focused_window.is_none()
            && (focused_window != self.last_focused
                || self
                    .last_focused_monitor
                    .is_some_and(|monitor| monitor != focused_monitor));
        self.last_focused_monitor = Some(focused_monitor);
        let changed = focused_window != self.last_focused;
        self.last_focused = focused_window;
        if changed
            && let Some(id) = focused_window
            && self.registry.contains(id)
        {
            return Some(FocusTarget::Window(id));
        }
        focus_empty.then_some(FocusTarget::TilingOverlay(focused_monitor))
    }

    fn apply_native_focus(&self, id: WindowId) {
        if self.registry.contains(id)
            && let Err(error) = self.window_server.focus(id)
        {
            tracing::trace!(?error, "Failed to focus window");
        }
    }
}

#[derive(Clone, Copy)]
enum FocusTarget {
    Window(WindowId),
    TilingOverlay(MonitorId),
}

/// Highlighted tiles use native windows because mirrors do not forward keyboard input.
fn shows_through_mirror(placement: &TilingWindowPlacement) -> bool {
    !placement.is_highlighted && placement.is_partially_off_screen()
}

/// The visible part of the content, relative to the window's top-left corner.
fn on_screen_part(placement: &TilingWindowPlacement) -> Dimension {
    let visible = placement.visible_content_box;
    let content = placement.content_box;
    PixelRect::from_pixels(
        (visible.x() - content.x()).coordinate(),
        (visible.y() - content.y()).coordinate(),
        visible.width(),
        visible.height(),
    )
    .to_dimension()
}
