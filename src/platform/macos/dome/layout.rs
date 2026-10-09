use std::collections::HashSet;

use crate::core::{
    Dimension, Length, MonitorLayout, MonitorPlacements, PixelRect, TilingWindowPlacement, WindowId,
};
use crate::platform::macos::objc2_wrapper::dimension_to_ns_rect_cocoa;

use super::Dome;
use super::events::{
    ContainerShow, FloatShow, HubMessage, MirrorShow, MonitorTilingData, RenderScene,
    TilingWindowShow,
};

impl Dome {
    /// All fullscreen -> normal and normal -> fullscreen must be resolved before this step
    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn flush_layout(&mut self) {
        let mut tiling = Vec::new();
        let mut float_shows = Vec::new();
        let mut mirror_shows = Vec::new();
        let result = self.hub.get_visible_placements();
        let visible_windows: HashSet<WindowId> = result
            .monitors
            .iter()
            .flat_map(|mp| match &mp.layout {
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => tiling_windows
                    .iter()
                    .map(|p| p.id)
                    .chain(float_windows.iter().map(|p| p.id))
                    .collect::<Vec<_>>(),
                MonitorLayout::Fullscreen(wid) => vec![*wid],
            })
            .collect();
        let to_hide: Vec<_> = self
            .displayed_windows
            .difference(&visible_windows)
            .copied()
            .collect();
        for wid in to_hide {
            self.hide_window(wid);
        }
        self.displayed_windows = visible_windows;
        let focused_window = result.focused_window;
        let focused_monitor = result.focused_monitor;
        for mp in result.monitors {
            let (t, f, m) = self.apply_monitor_placements(&mp, focused_window);
            tiling.push(t);
            float_shows.extend(f);
            mirror_shows.extend(m);
        }

        if focused_window != self.last_focused {
            self.last_focused = focused_window;
            if let Some(id) = focused_window
                && let Some(window) = self.registry.by_id(id)
                && let Err(err) = window.ext.focus()
            {
                tracing::trace!("Failed to focus window: {err:#}");
            }
        }
        let created = std::mem::take(&mut self.pending_created);
        let deleted = std::mem::take(&mut self.pending_deleted);

        for &wid in &created {
            if !deleted.contains(&wid) && !self.displayed_windows.contains(&wid) {
                self.hide_window(wid);
            }
        }

        for &wid in &deleted {
            let Some(entry) = self.registry.by_id(wid) else {
                continue;
            };
            let cg_id = entry.cg_id;
            self.recovery.untrack(cg_id);
            self.displayed_windows.remove(&wid);
            self.registry.remove(cg_id);
        }

        self.sender.send(HubMessage::Scene(RenderScene {
            tiling,
            float_shows,
            mirror_shows,
            focused_window,
            focused_monitor_id: focused_monitor,
            workspaces: self.hub.query_workspaces(),
        }));
    }

    fn apply_monitor_placements(
        &mut self,
        mp: &MonitorPlacements,
        focused_window: Option<WindowId>,
    ) -> (MonitorTilingData, Vec<FloatShow>, Vec<MirrorShow>) {
        match &mp.layout {
            MonitorLayout::Fullscreen(window_id) => {
                self.place_fullscreen_window(*window_id, mp.work_area);
                let scale = self.monitor_registry.monitor(mp.monitor_id).backing_scale;
                let dim = mp.work_area.to_dimension();
                (
                    MonitorTilingData {
                        monitor_id: mp.monitor_id,
                        monitor_dim: dim,
                        cocoa_frame: dimension_to_ns_rect_cocoa(
                            Length::new(self.primary_full_height),
                            dim,
                        ),
                        scale,
                        border_thickness: Length::from_pixels(mp.border_thickness),
                        windows: Vec::new(),
                        containers: Vec::new(),
                    },
                    Vec::new(),
                    Vec::new(),
                )
            }
            MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } => {
                let monitor_dim = mp.work_area.to_dimension();
                let scale = self.monitor_registry.monitor(mp.monitor_id).backing_scale;

                let mut placed_tiling = Vec::new();
                let mut float_shows = Vec::new();
                let mut mirror_shows = Vec::new();

                for wp in tiling_windows {
                    // Tiling placements are always Positioned, so parking is legal here.
                    if wp.visible_content_box.is_empty() {
                        tracing::debug!(
                            window_id = %wp.id,
                            border_box = ?wp.border_box,
                            content_box = ?wp.content_box,
                            "No visible content box, parking window"
                        );
                        self.move_window_offscreen(wp.id);
                        continue;
                    }
                    let mirrored = shows_through_mirror(wp);
                    if mirrored {
                        self.park_mirrored_window(wp.id, wp.content_box);
                    } else {
                        // macOS doesn't reliably allow placing windows partially off-screen
                        // (especially above the menu bar), so place the trimmed rect.
                        self.show_tiling(wp.id, wp.visible_content_box);
                    }
                    let Some(entry) = self.registry.by_id(wp.id) else {
                        continue;
                    };
                    if mirrored {
                        mirror_shows.push(MirrorShow {
                            cg_id: entry.cg_id,
                            cocoa_frame: dimension_to_ns_rect_cocoa(
                                Length::new(self.primary_full_height),
                                wp.visible_content_box.to_dimension(),
                            ),
                            source: on_screen_part(wp),
                            scale,
                        });
                    }
                    placed_tiling.push(TilingWindowShow {
                        placement: *wp,
                        corner_radius: entry.corner_radius,
                    });
                }

                for wp in float_windows {
                    // Float dimensions are screen-absolute. The OS clips at screen
                    // edges, so we use wp.border_box for everything (no visible_border_box).
                    if wp.content_box.is_empty() {
                        tracing::debug!(window_id = %wp.id, "Float content box entirely border, parking window");
                        self.move_window_offscreen(wp.id);
                        continue;
                    }
                    if focused_window != Some(wp.id) {
                        self.move_window_offscreen(wp.id);
                    } else {
                        self.show_float(wp.id, wp.content_box);
                    }
                    let Some(entry) = self.registry.by_id(wp.id) else {
                        continue;
                    };
                    float_shows.push(FloatShow {
                        cg_id: entry.cg_id,
                        placement: *wp,
                        cocoa_frame: dimension_to_ns_rect_cocoa(
                            Length::new(self.primary_full_height),
                            wp.border_box.to_dimension(),
                        ),
                        scale,
                        border_thickness: Length::from_pixels(mp.border_thickness),
                        content_dim: wp.content_box.to_dimension(),
                        corner_radius: entry.corner_radius,
                    });
                }

                let mut container_data = Vec::with_capacity(containers.len());
                for cp in containers {
                    let tab_bar_dim = cp.tab_bar_band.to_dimension();
                    let tab_bar_cocoa_frame = dimension_to_ns_rect_cocoa(
                        Length::new(self.primary_full_height),
                        cp.visible_tab_bar_band.to_dimension(),
                    );
                    container_data.push(ContainerShow {
                        placement: cp.clone(),
                        tab_bar_dim,
                        tab_bar_cocoa_frame,
                    });
                }

                (
                    MonitorTilingData {
                        monitor_id: mp.monitor_id,
                        monitor_dim,
                        cocoa_frame: dimension_to_ns_rect_cocoa(
                            Length::new(self.primary_full_height),
                            monitor_dim,
                        ),
                        scale,
                        border_thickness: Length::from_pixels(mp.border_thickness),
                        windows: placed_tiling,
                        containers: container_data,
                    },
                    float_shows,
                    mirror_shows,
                )
            }
        }
    }
}

/// Whether a tile shows through a mirror instead of as the window itself, which holds for a
/// tile that is partly off screen and not highlighted. A highlighted tile has the tiling focus,
/// where typing goes, and a mirror passes no input to its window, so that tile stays real.
fn shows_through_mirror(wp: &TilingWindowPlacement) -> bool {
    !wp.is_highlighted && wp.is_partially_off_screen()
}

/// The part of the window that `visible_content_box` shows, relative to the window's top-left
/// corner.
fn on_screen_part(wp: &TilingWindowPlacement) -> Dimension {
    let visible = wp.visible_content_box;
    let content = wp.content_box;
    PixelRect::from_pixels(
        visible.x() - content.x(),
        visible.y() - content.y(),
        visible.width(),
        visible.height(),
    )
    .to_dimension()
}
