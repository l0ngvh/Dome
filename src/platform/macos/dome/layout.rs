use std::collections::HashSet;

use crate::core::{Dimension, Length, Logical, MonitorLayout, MonitorPlacements, WindowId};
use crate::platform::macos::objc2_wrapper::dimension_to_ns_rect_cocoa;

use super::Dome;
use super::events::{ContainerShow, FloatShow, HubMessage, MonitorTilingData, RenderFrame};
use super::window::{apply_inset, clip_to_bounds};

/// Top `tab_bar_height` strip of a tabbed container's frame, in logical points.
fn tab_bar_dimension(
    container_frame: Dimension<Logical>,
    tab_bar_height: Length<Logical>,
) -> Dimension<Logical> {
    Dimension::new(
        container_frame.x,
        container_frame.y,
        container_frame.width,
        tab_bar_height,
    )
}

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
                let entry = self.monitor_registry.monitor(mp.monitor_id);
                entry
                    .displayed()
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
                .set_displayed_windows(mp.monitor_id, displayed);
            let (t, f) = self.apply_monitor_placements(&mp, focused_window);
            tiling.push(t);
            float_shows.extend(f);
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
            tab_bar_height: self.config.partition_tree.tab_bar_height,
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
                let monitor = self.monitor_registry.monitor(mp.monitor_id);
                let dim = monitor.work_area();
                (
                    MonitorTilingData {
                        monitor_id: mp.monitor_id,
                        monitor_dim: dim,
                        cocoa_frame: dimension_to_ns_rect_cocoa(
                            Length::new(self.primary_full_height),
                            dim,
                        ),
                        scale: monitor.egui_scale(),
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
                let border_size = self
                    .monitor_registry
                    .physical_border(mp.monitor_id, Length::new(self.config.border_size));
                let monitor = self.monitor_registry.monitor(mp.monitor_id);
                let monitor_dim = monitor.work_area();
                let scale = monitor.egui_scale();

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
                    self.show_tiling(wp.id, target);
                    placed_tiling.push(*wp);
                }

                for wp in float_windows {
                    // Float dimensions are screen-absolute. The OS clips at screen
                    // edges, so we use wp.frame for everything (no visible_frame).
                    let content_dim = apply_inset(wp.frame, border_size);
                    if focused_window != Some(wp.id) {
                        self.move_window_offscreen(wp.id);
                    } else {
                        self.show_float(wp.id, content_dim);
                    }
                    let Some(entry) = self.registry.by_id(wp.id) else {
                        continue;
                    };
                    float_shows.push(FloatShow {
                        cg_id: entry.cg_id,
                        placement: *wp,
                        cocoa_frame: dimension_to_ns_rect_cocoa(
                            Length::new(self.primary_full_height),
                            wp.frame,
                        ),
                        scale,
                        content_dim,
                    });
                }

                let mut container_data = Vec::with_capacity(containers.len());
                for cp in containers {
                    let tab_bar_dim =
                        tab_bar_dimension(cp.frame, self.config.partition_tree.tab_bar_height);
                    let tab_bar_cocoa_frame = dimension_to_ns_rect_cocoa(
                        Length::new(self.primary_full_height),
                        tab_bar_dim,
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
                        windows: placed_tiling,
                        containers: container_data,
                    },
                    float_shows,
                )
            }
        }
    }
}
