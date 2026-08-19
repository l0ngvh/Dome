use std::collections::{HashMap, HashSet};

use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID};
use objc2_foundation::{NSNumber, NSString};

use crate::core::{Dimension, Hub, Length, MonitorId, PixelRect, Pixels};
use crate::platform::reserve_for_bar;

use super::Dome;

#[derive(Clone, Debug)]
pub(in crate::platform::macos) struct MonitorInfo {
    pub(in crate::platform::macos) display_id: CGDirectDisplayID,
    pub(in crate::platform::macos) name: String,
    /// Visible area: `bounds` minus the menu bar and dock insets. Rounded
    /// inward at construction, so a window can never be placed onto a fraction
    /// of a pixel the menu bar, the dock or a status bar reserved.
    pub(in crate::platform::macos) work_area: PixelRect,
    /// Full physical bounds reported by `CGDisplayBounds`, used for monitor
    /// lookup against raw window coordinates (e.g. borderless fullscreen).
    pub(in crate::platform::macos) bounds: Dimension,
    pub(in crate::platform::macos) full_height: f32,
    pub(in crate::platform::macos) is_primary: bool,
    /// NSScreen.backingScaleFactor — used for egui render density only.
    /// This is NOT core Monitor.scale (which is always 1.0 on macOS because
    /// AppKit already reports points, so no DPI conversion is needed).
    pub(in crate::platform::macos) scale: f64,
}

impl std::fmt::Display for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, work_area={:?}, scale={})",
            self.name, self.display_id, self.work_area, self.scale
        )
    }
}

pub(in crate::platform::macos) fn get_all_monitors(mtm: MainThreadMarker) -> Vec<MonitorInfo> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen);
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            MonitorInfo {
                display_id,
                name,
                work_area: PixelRect::from_dimension_inward(Dimension::new(
                    Length::new(bounds.origin.x as f32),
                    Length::new((bounds.origin.y + top_inset) as f32),
                    Length::new(bounds.size.width as f32),
                    Length::new((bounds.size.height - top_inset - bottom_inset) as f32),
                )),
                bounds: Dimension::new(
                    Length::new(bounds.origin.x as f32),
                    Length::new(bounds.origin.y as f32),
                    Length::new(bounds.size.width as f32),
                    Length::new(bounds.size.height as f32),
                ),
                full_height: bounds.size.height as f32,
                is_primary: display_id == primary_id,
                scale: screen.backingScaleFactor(),
            }
        })
        .collect()
}

fn get_display_id(screen: &NSScreen) -> CGDirectDisplayID {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    desc.objectForKey(&key)
        .and_then(|obj| {
            let num: Option<&NSNumber> = obj.downcast_ref();
            num.map(|n| n.unsignedIntValue())
        })
        .unwrap_or(0)
}

/// Maps the `kCGNullDirectDisplay` sentinel `get_display_id` falls back to onto
/// an absent id, so core never stores a zero that looks real.
pub(in crate::platform::macos) fn publishable_display_id(
    display_id: CGDirectDisplayID,
) -> Option<u32> {
    (display_id != 0).then_some(display_id)
}

type DisplayId = u32;

pub(in crate::platform::macos) struct Monitor {
    id: MonitorId,
    info: MonitorInfo,
}

impl Monitor {
    pub(in crate::platform::macos) fn id(&self) -> MonitorId {
        self.id
    }

    pub(in crate::platform::macos) fn work_area(&self) -> PixelRect {
        self.info.work_area
    }

    pub(in crate::platform::macos) fn egui_scale(&self) -> f64 {
        self.info.scale
    }
}

pub(super) struct MonitorRegistry {
    map: HashMap<DisplayId, Monitor>,
    reverse: HashMap<MonitorId, DisplayId>,
    primary_display_id: DisplayId,
}

impl MonitorRegistry {
    pub(super) fn new(primary: &MonitorInfo, primary_monitor_id: MonitorId) -> Self {
        let mut map = HashMap::new();
        let mut reverse = HashMap::new();
        map.insert(
            primary.display_id,
            Monitor {
                id: primary_monitor_id,
                info: primary.clone(),
            },
        );
        reverse.insert(primary_monitor_id, primary.display_id);
        Self {
            map,
            reverse,
            primary_display_id: primary.display_id,
        }
    }

    pub(super) fn contains(&self, display_id: DisplayId) -> bool {
        self.map.contains_key(&display_id)
    }

    pub(super) fn get(&self, display_id: DisplayId) -> Option<MonitorId> {
        self.map.get(&display_id).map(|e| e.id)
    }

    pub(in crate::platform::macos) fn monitor(&self, monitor_id: MonitorId) -> &Monitor {
        self.reverse
            .get(&monitor_id)
            .and_then(|d| self.map.get(d))
            .expect("monitor not found in registry")
    }

    pub(super) fn primary_monitor(&self) -> &Monitor {
        self.map
            .get(&self.primary_display_id)
            .expect("primary monitor present")
    }

    pub(super) fn primary_monitor_id(&self) -> MonitorId {
        self.get(self.primary_display_id).unwrap()
    }

    pub(in crate::platform::macos) fn primary_full_height(&self) -> f32 {
        self.map
            .get(&self.primary_display_id)
            .expect("primary monitor present")
            .info
            .full_height
    }

    pub(super) fn set_primary_display_id(&mut self, display_id: DisplayId) {
        self.primary_display_id = display_id;
    }

    pub(super) fn replace_primary(&mut self, new_info: &MonitorInfo) {
        debug_assert!(!self.map.contains_key(&new_info.display_id));
        if let Some(mut entry) = self.map.remove(&self.primary_display_id) {
            let old = self.primary_display_id;
            let monitor_id = entry.id;
            entry.info = new_info.clone();
            self.map.insert(new_info.display_id, entry);
            self.reverse.insert(monitor_id, new_info.display_id);
            self.primary_display_id = new_info.display_id;
            tracing::info!(old, new = new_info.display_id, "Primary monitor replaced");
        }
    }

    pub(super) fn insert(&mut self, monitor: &MonitorInfo, monitor_id: MonitorId) {
        self.map.insert(
            monitor.display_id,
            Monitor {
                id: monitor_id,
                info: monitor.clone(),
            },
        );
        self.reverse.insert(monitor_id, monitor.display_id);
    }

    fn remove_by_id(&mut self, monitor_id: MonitorId) {
        if let Some(display_id) = self.reverse.remove(&monitor_id) {
            self.map.remove(&display_id);
        }
    }

    pub(super) fn remove_stale(&mut self, current: &HashSet<DisplayId>) -> Vec<MonitorId> {
        let stale: Vec<_> = self
            .map
            .iter()
            .filter(|(key, _)| !current.contains(key))
            .map(|(_, e)| e.id)
            .collect();
        for &id in &stale {
            self.remove_by_id(id);
        }
        stale
    }

    pub(super) fn all_monitors(&self) -> Vec<MonitorInfo> {
        self.map.values().map(|e| e.info.clone()).collect()
    }

    /// Returns the `Monitor` whose full display bounds overlap `dim` the most by
    /// intersection area. The intersection is pure Rust over the cached
    /// `MonitorInfo.bounds` rather than a CoreGraphics call, so it is safe to hit
    /// from test contexts where CGS is not initialized.
    pub(super) fn find_closest_monitor(&self, dim: Dimension) -> Option<&Monitor> {
        let mut best: Option<(&Monitor, f32)> = None;
        for monitor in self.map.values() {
            let area = intersection_area(dim, monitor.info.bounds);
            if area <= 0.0 {
                continue;
            }
            if best.map(|(_, b)| area > b).unwrap_or(true) {
                best = Some((monitor, area));
            }
        }
        best.map(|(m, _)| m)
    }

    pub(super) fn is_borderless_fullscreen_at(&self, rect: PixelRect) -> bool {
        let point = Dimension::new(
            Length::from_pixels(rect.x()),
            Length::from_pixels(rect.y()),
            Length::new(1.0),
            Length::new(1.0),
        );
        let monitor = self.find_closest_monitor(point);
        monitor.is_some_and(|m| {
            let mon = m.info.work_area;
            let tolerance = Pixels::new(2);
            (rect.x() - mon.x()).abs() <= tolerance
                && (rect.y() - mon.y()).abs() <= tolerance
                && (rect.width() - mon.width()).abs() <= tolerance
                && (rect.height() - mon.height()).abs() <= tolerance
        })
    }

    pub(super) fn update_monitor(
        &mut self,
        monitor: &MonitorInfo,
    ) -> Option<(MonitorId, PixelRect)> {
        let entry = self.map.get_mut(&monitor.display_id)?;
        let old_work_area = entry.info.work_area;
        entry.info = monitor.clone();
        Some((entry.id, old_work_area))
    }
}

fn intersection_area(a: Dimension, b: Dimension) -> f32 {
    let x1 = a.x.value().max(b.x.value());
    let y1 = a.y.value().max(b.y.value());
    let x2 = (a.x + a.width).value().min((b.x + b.width).value());
    let y2 = (a.y + a.height).value().min((b.y + b.height).value());
    let w = (x2 - x1).max(0.0);
    let h = (y2 - y1).max(0.0);
    w * h
}

impl MonitorRegistry {
    pub(super) fn reconcile(&mut self, hub: &mut Hub, monitors: &[MonitorInfo]) {
        let current_keys: HashSet<_> = monitors.iter().map(|s| s.display_id).collect();

        // Special handling for when the primary monitor got replaced, i.e. due to mirroring to prevent
        // disruption due to removal and addition of workspaces.
        if let Some(new_primary) = monitors.iter().find(|s| s.is_primary) {
            if !self.contains(new_primary.display_id) {
                self.replace_primary(new_primary);
                hub.update_monitor(self.primary_monitor_id(), new_primary.work_area, 1.0);
                // `replace_primary` rebinds this MonitorId onto a different
                // panel, so the previous stamp is now wrong.
                hub.set_monitor_cg_display_id(
                    self.primary_monitor_id(),
                    publishable_display_id(new_primary.display_id),
                );
            } else {
                self.set_primary_display_id(new_primary.display_id);
            }
        }

        // Add new monitors first to prevent exhausting all monitors
        for monitor in monitors {
            if !self.contains(monitor.display_id) {
                let id = hub.add_monitor(monitor.name.clone(), monitor.work_area, 1.0);
                hub.set_monitor_cg_display_id(id, publishable_display_id(monitor.display_id));
                self.insert(monitor, id);
                tracing::info!(%monitor, "Monitor added");
            }
        }

        let removed = self.remove_stale(&current_keys);
        if !removed.is_empty() {
            // Capture the primary once before the loop: it is never in `removed`
            // (the primary is always a survivor), so the parked workspaces rent
            // to a stable present id even as monitors are deleted per iteration.
            let primary = self.primary_monitor_id();
            for monitor_id in &removed {
                hub.remove_monitor(*monitor_id, primary);
            }
            tracing::info!(?removed, %primary, "Monitors removed");
        }

        for monitor in monitors {
            if let Some((monitor_id, old_work_area)) = self.update_monitor(monitor) {
                if old_work_area != monitor.work_area {
                    tracing::info!(
                        name = %monitor.name,
                        ?old_work_area,
                        new_work_area = ?monitor.work_area,
                        "Monitor work area changed"
                    );
                }
                hub.update_monitor(monitor_id, monitor.work_area, 1.0);
            }
        }
    }
}

impl Dome {
    pub(super) fn update_monitors(&mut self, monitors: &[MonitorInfo]) {
        // Cache the unshrunk list. Re-shrinking an already-shrunk cache would
        // compound the reservation on each call.
        self.monitors = monitors.to_vec();
        if self.status_bars.is_empty() {
            self.monitor_registry.reconcile(&mut self.hub, monitors);
        } else {
            let shrunk: Vec<MonitorInfo> = monitors
                .iter()
                .map(|m| {
                    // Bar-edge math is f32 and shared with Windows, so the work area
                    // leaves pixel space and comes back.
                    let work_area = match self.status_bars.rect_for(m.display_id) {
                        Some(bar) => PixelRect::from_dimension_inward(reserve_for_bar(
                            m.bounds,
                            m.work_area.to_dimension(),
                            bar,
                        )),
                        None => m.work_area,
                    };
                    MonitorInfo {
                        work_area,
                        ..m.clone()
                    }
                })
                .collect();
            self.monitor_registry.reconcile(&mut self.hub, &shrunk);
        }
        self.primary_full_height = self.monitor_registry.primary_full_height();
    }
}
