use std::collections::{HashMap, HashSet};

use crate::platform::windows::handle::OFFSCREEN_POS;
use crate::platform::windows::ui::{
    FloatBorderPlacement, FloatBorderRequest, FloatBorderState, OverlayPlacements,
    ThumbnailPlacement, TilingOverlayPlacement, TilingWindowShow,
};
use crate::platform::windows::window_server::WindowOrder;
use dome_core::{
    MonitorId, MonitorLayout, MonitorPlacements, PixelRect, TilingWindowPlacement,
    VisiblePlacements, WindowId,
};

use super::{Dome, WindowState};

impl Dome {
    #[tracing::instrument(level = "trace", skip_all)]
    pub(crate) fn apply_layout(&mut self) {
        let visible = self.hub.get_visible_placements();
        self.apply_visible_placements(&visible);
    }

    pub(super) fn apply_visible_placements(&mut self, visible: &VisiblePlacements) {
        self.place_application_windows(visible);
        let mut placements = self.overlay_placements(visible);
        let target = self.focus_target(visible);
        placements.focus_tiling = match target {
            Some(FocusTarget::TilingOverlay(monitor)) => Some(monitor),
            _ => None,
        };
        self.overlays.update(placements);
        if let Some(FocusTarget::Window(window)) = target {
            self.window_server.set_foreground_window(window);
        }
    }

    fn place_application_windows(&mut self, visible: &VisiblePlacements) {
        let tabs_to_add = self.reconcile_visibility(&visible.monitors);
        let focus_changed = visible.focused_window != self.last_focused;
        for monitor in &visible.monitors {
            match &monitor.layout {
                MonitorLayout::Fullscreen(id) => {
                    self.show_fullscreen_window(*id, monitor.work_area, monitor.monitor_id)
                }
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => {
                    for placement in float_windows {
                        if !self.registry.contains(placement.id) {
                            continue;
                        }
                        if placement.content_box.is_empty() {
                            self.hide_window(placement.id);
                        } else if !self.moving_windows.contains(&placement.id) {
                            self.show_float(
                                placement.id,
                                placement,
                                focus_changed,
                                visible.focused_window == Some(placement.id),
                                monitor.monitor_id,
                            );
                        }
                    }
                    for placement in tiling_windows {
                        if self.registry.contains(placement.id) && placement.content_box.is_empty()
                        {
                            self.hide_window(placement.id);
                        }
                    }
                    let windows = self.painted_windows(tiling_windows);
                    let reference = windows.iter().find(|show| show.placement.is_highlighted);
                    if let Some(reference) = reference {
                        self.position_tiling_window(
                            &reference.placement,
                            monitor.monitor_id,
                            WindowOrder::Unchanged,
                        );
                    }
                    let order = reference.map_or(WindowOrder::Unchanged, |show| {
                        WindowOrder::After(show.placement.id)
                    });
                    for show in windows.iter().filter(|show| !show.placement.is_highlighted) {
                        self.position_tiling_window(&show.placement, monitor.monitor_id, order);
                    }
                }
            }
        }
        for id in tabs_to_add {
            if self.registry.contains(id) {
                self.window_server.add_tab(id);
            }
        }
    }

    fn overlay_placements(&self, visible: &VisiblePlacements) -> OverlayPlacements {
        let mut result = OverlayPlacements {
            tiling: HashMap::new(),
            tabs: HashMap::new(),
            floats: HashMap::new(),
            thumbnails: HashMap::new(),
            appearance: self.appearance.clone(),
            focus_tiling: None,
        };
        for monitor in &visible.monitors {
            let scale = self.monitors.monitor(monitor.monitor_id).scale;
            let mut tiling = TilingOverlayPlacement {
                monitor: monitor.work_area,
                windows: Vec::new(),
                containers: Vec::new(),
                scale,
                border_thickness: monitor.border_thickness,
                order: WindowOrder::Unchanged,
            };
            if let MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } = &monitor.layout
            {
                tiling.windows = self.painted_windows(tiling_windows);
                tiling.order = tiling
                    .windows
                    .iter()
                    .find(|show| show.placement.is_highlighted)
                    .map_or(WindowOrder::Unchanged, |show| {
                        WindowOrder::After(show.placement.id)
                    });
                tiling.containers = containers
                    .iter()
                    .filter(|placement| placement.is_tabbed || placement.is_highlighted)
                    .cloned()
                    .collect();
                for container in containers.iter().filter(|placement| placement.is_tabbed) {
                    result
                        .tabs
                        .insert(container.id, (monitor.monitor_id, container.clone()));
                }
                for show in &tiling.windows {
                    let placement = &show.placement;
                    if shows_through_thumbnail(placement)
                        && !placement.visible_content_box.is_empty()
                    {
                        result.thumbnails.insert(
                            placement.id,
                            ThumbnailPlacement {
                                frame: placement.visible_content_box,
                                source_rect: on_screen_part(placement),
                            },
                        );
                    }
                }
                for placement in float_windows {
                    if !self.registry.contains(placement.id) || placement.content_box.is_empty() {
                        continue;
                    }
                    let request = if self.moving_windows.contains(&placement.id) {
                        FloatBorderRequest::PreserveExisting
                    } else {
                        let border = FloatBorderPlacement {
                            placement: *placement,
                            scale,
                            border_thickness: monitor.border_thickness,
                            corner_radius: self.window_server.corner_radius(placement.id),
                            order: WindowOrder::After(placement.id),
                        };
                        FloatBorderRequest::Present(if placement.visible_border_box.is_empty() {
                            FloatBorderState::Hidden(border)
                        } else {
                            FloatBorderState::Visible(border)
                        })
                    };
                    result.floats.insert(placement.id, request);
                }
            }
            result.tiling.insert(monitor.monitor_id, tiling);
        }
        result
    }

    fn painted_windows(&self, placements: &[TilingWindowPlacement]) -> Vec<TilingWindowShow> {
        placements
            .iter()
            .filter(|placement| {
                self.registry.contains(placement.id) && !placement.content_box.is_empty()
            })
            .map(|placement| TilingWindowShow {
                placement: *placement,
                corner_radius: self.window_server.corner_radius(placement.id),
            })
            .collect()
    }

    fn reconcile_visibility(&mut self, monitors: &[MonitorPlacements]) -> Vec<WindowId> {
        let visible: HashSet<WindowId> = monitors
            .iter()
            .flat_map(|monitor| match &monitor.layout {
                MonitorLayout::Fullscreen(id) => vec![*id],
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => tiling_windows
                    .iter()
                    .map(|window| window.id)
                    .chain(float_windows.iter().map(|window| window.id))
                    .collect(),
            })
            .collect();
        let to_hide: Vec<_> = self
            .displayed_windows
            .difference(&visible)
            .copied()
            .collect();
        let tabs_to_add = visible
            .difference(&self.displayed_windows)
            .copied()
            .collect();
        self.displayed_windows = visible;
        for id in to_hide {
            if let Some(entry) = self.registry.get(id)
                && !entry.is_minimized
            {
                self.window_server.delete_tab(id);
            }
            self.hide_window(id);
        }
        tabs_to_add
    }

    fn focus_target(&mut self, visible: &VisiblePlacements) -> Option<FocusTarget> {
        let focused = visible.focused_window;
        let focused_monitor = visible.focused_monitor;
        let focus_changed = focused != self.last_focused;
        let monitor_changed = self
            .last_focused_monitor
            .is_some_and(|monitor| monitor != focused_monitor);
        self.last_focused = focused;
        self.last_focused_monitor = Some(focused_monitor);
        if !focus_changed && !monitor_changed {
            return None;
        }
        match focused {
            Some(id) => self
                .registry
                .get(id)
                .filter(|entry| !matches!(entry.state, WindowState::ExclusiveFullscreen))
                .map(|_| FocusTarget::Window(id)),
            None => Some(FocusTarget::TilingOverlay(focused_monitor)),
        }
    }

    fn position_tiling_window(
        &mut self,
        placement: &TilingWindowPlacement,
        monitor: MonitorId,
        order: WindowOrder,
    ) {
        if !self.registry.contains(placement.id) || self.moving_windows.contains(&placement.id) {
            return;
        }
        let target = if shows_through_thumbnail(placement) {
            parked_at_full_size(placement.content_box)
        } else {
            placement.content_box
        };
        self.show_tiling(placement.id, target, monitor, order);
    }
}

#[derive(Clone, Copy)]
enum FocusTarget {
    Window(WindowId),
    TilingOverlay(MonitorId),
}

fn shows_through_thumbnail(placement: &TilingWindowPlacement) -> bool {
    !placement.is_highlighted && placement.is_partially_off_screen()
}

fn parked_at_full_size(content_box: PixelRect) -> PixelRect {
    PixelRect::from_pixels(
        OFFSCREEN_POS,
        OFFSCREEN_POS,
        content_box.width(),
        content_box.height(),
    )
}

fn on_screen_part(placement: &TilingWindowPlacement) -> PixelRect {
    let visible = placement.visible_content_box;
    let content = placement.content_box;
    PixelRect::from_pixels(
        (visible.x() - content.x()).coordinate(),
        (visible.y() - content.y()).coordinate(),
        visible.width(),
        visible.height(),
    )
}
