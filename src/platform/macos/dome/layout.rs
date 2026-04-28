use std::collections::HashSet;

use objc2_foundation::{NSPoint, NSRect, NSSize};

use crate::core::{Dimension, MonitorLayout, MonitorPlacements, WindowId};

use super::Dome;
use super::events::{FloatShow, HubMessage, MonitorTilingData, RenderFrame};
use super::window::{apply_inset, clip_to_bounds};

impl Dome {
    /// All fullscreen -> normal and normal -> fullscreen must be resolved before this step
    #[tracing::instrument(skip_all)]
    pub(in crate::platform::macos) fn flush_layout(&mut self) {
        let mut tiling = Vec::new();
        let mut float_shows = Vec::new();
        let result = self.hub.get_visible_placements();
        let all_displayed_windows: HashSet<WindowId> = result
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
        let to_hide: Vec<_> = result
            .monitors
            .iter()
            .flat_map(|mp| {
                let entry = self.monitor_registry.get_entry(mp.monitor_id);
                entry
                    .displayed_windows
                    .difference(&all_displayed_windows)
                    .copied()
                    .collect::<Vec<_>>()
            })
            .collect();
        for wid in to_hide {
            self.hide_window(wid);
        }
        let focused_window = result.focused_window;
        let focused_monitor = result.focused_monitor;
        for mp in result.monitors {
            let displayed: HashSet<WindowId> = match &mp.layout {
                MonitorLayout::Fullscreen(window_id) => HashSet::from([*window_id]),
                MonitorLayout::Normal {
                    tiling_windows,
                    float_windows,
                    ..
                } => tiling_windows
                    .iter()
                    .map(|p| p.id)
                    .chain(float_windows.iter().map(|p| p.id))
                    .collect(),
            };
            self.monitor_registry
                .get_entry_mut(mp.monitor_id)
                .displayed_windows = displayed;
            let (t, f) = self.apply_monitor_placements(&mp, focused_window);
            tiling.push(t);
            float_shows.extend(f);
        }

        if focused_window != self.last_focused {
            self.last_focused = focused_window;
            if let Some(id) = focused_window
                && let Some(window) = self.registry.by_id(id)
                && let Err(err) = window.ax.focus()
            {
                tracing::trace!("Failed to focus window: {err:#}");
            }
        }
        let created = std::mem::take(&mut self.pending_created);
        let deleted = std::mem::take(&mut self.pending_deleted);

        for &wid in &created {
            if !deleted.contains(&wid) && !all_displayed_windows.contains(&wid) {
                self.hide_window(wid);
            }
        }

        for &wid in &deleted {
            let Some(entry) = self.registry.by_id(wid) else {
                continue;
            };
            let cg_id = entry.cg_id;
            self.recovery.untrack(cg_id);
            self.monitor_registry.remove_displayed_window(wid);
            self.registry.remove(cg_id);
        }

        self.sender.send(HubMessage::Frame(RenderFrame {
            tiling,
            float_shows,
            focused_window,
            focused_monitor_id: focused_monitor,
        }));
    }

    fn apply_monitor_placements(
        &mut self,
        mp: &MonitorPlacements,
        focused_window: Option<WindowId>,
    ) -> (MonitorTilingData, Vec<FloatShow>) {
        match &mp.layout {
            MonitorLayout::Fullscreen(window_id) => {
                self.place_fullscreen_window(*window_id, mp.monitor_id);
                let screen = &self.monitor_registry.get_entry(mp.monitor_id).screen;
                (
                    MonitorTilingData {
                        monitor_id: mp.monitor_id,
                        monitor_dim: screen.dimension,
                        cocoa_frame: to_ns_rect(self.primary_full_height, screen.dimension),
                        scale: screen.scale,
                        windows: Vec::new(),
                        containers: Vec::new(),
                    },
                    Vec::new(),
                )
            }
            MonitorLayout::Normal {
                tiling_windows,
                float_windows,
                containers,
            } => {
                let border_size = self.config.border_size;
                let screen = &self.monitor_registry.get_entry(mp.monitor_id).screen;
                let monitor_dim = screen.dimension;
                let scale = screen.scale;

                let mut placed_tiling = Vec::new();
                let mut float_shows = Vec::new();

                for wp in tiling_windows {
                    let content_dim = apply_inset(wp.frame, border_size);
                    // Clip to visible_frame bounds -- macOS doesn't reliably allow
                    // placing windows partially off-screen (especially above menu bar)
                    let visible_content = clip_to_bounds(content_dim, wp.visible_frame);
                    let Some(target) = visible_content else {
                        let _span = tracing::debug_span!("empty_visible_content", ?content_dim, visible_frame = ?wp.visible_frame).entered();
                        self.move_window_offscreen(wp.id);
                        continue;
                    };
                    self.place_window(wp.id, target);
                    placed_tiling.push(*wp);
                }

                for wp in float_windows {
                    // Float dimensions are screen-absolute. The OS clips at screen
                    // edges, so we use wp.frame for everything (no visible_frame).
                    let content_dim = apply_inset(wp.frame, border_size);
                    if focused_window != Some(wp.id) {
                        self.move_window_offscreen(wp.id);
                    } else {
                        self.place_window(wp.id, content_dim);
                    }
                    let Some(entry) = self.registry.by_id(wp.id) else {
                        continue;
                    };
                    float_shows.push(FloatShow {
                        cg_id: entry.cg_id,
                        placement: *wp,
                        cocoa_frame: to_ns_rect(self.primary_full_height, wp.frame),
                        scale,
                        content_dim,
                    });
                }

                let mut container_data = Vec::new();
                for cp in containers {
                    let tab_titles = cp.titles.clone();
                    container_data.push((cp.clone(), tab_titles));
                }

                (
                    MonitorTilingData {
                        monitor_id: mp.monitor_id,
                        monitor_dim,
                        cocoa_frame: to_ns_rect(self.primary_full_height, monitor_dim),
                        scale,
                        windows: placed_tiling,
                        containers: container_data,
                    },
                    float_shows,
                )
            }
        }
    }
}

// Quartz uses top-left origin, Cocoa uses bottom-left origin
pub(super) fn to_ns_rect(primary_full_height: f32, dim: Dimension) -> NSRect {
    NSRect::new(
        NSPoint::new(
            dim.x as f64,
            (primary_full_height - dim.y - dim.height) as f64,
        ),
        NSSize::new(dim.width as f64, dim.height as f64),
    )
}
