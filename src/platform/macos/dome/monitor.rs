use std::collections::{HashMap, HashSet};

use anyhow::Context;
use objc2::MainThreadMarker;
use objc2_app_kit::NSScreen;
use objc2_core_graphics::{CGDirectDisplayID, CGDisplayBounds, CGMainDisplayID};
use objc2_foundation::{NSNumber, NSString};

use crate::core::{Dimension, Hub, Length, MonitorId, PixelRect, Pixels, ReportedMonitor};

use super::Dome;

#[derive(Clone, Debug)]
pub(in crate::platform::macos) struct MonitorInfo {
    pub(in crate::platform::macos) display_id: CGDirectDisplayID,
    pub(in crate::platform::macos) name: String,
    /// Visible area: `bounds` minus the menu bar and dock insets. Rounded
    /// inward at construction, so a window can never be placed onto a fraction
    /// of a pixel the menu bar or the dock reserved.
    pub(in crate::platform::macos) work_area: PixelRect,
    /// Full physical bounds reported by `CGDisplayBounds`, used for monitor
    /// lookup against raw window coordinates (e.g. borderless fullscreen).
    pub(in crate::platform::macos) bounds: Dimension,
    pub(in crate::platform::macos) full_height: f32,
    pub(in crate::platform::macos) is_primary: bool,
    /// `NSScreen.backingScaleFactor`, for egui render density only.
    pub(in crate::platform::macos) backing_scale: f64,
}

impl From<&MonitorInfo> for ReportedMonitor {
    fn from(info: &MonitorInfo) -> Self {
        ReportedMonitor {
            device_name: info.name.clone(),
            work_area: info.work_area,
            scale: 1.0,
            cg_display_id: Some(info.display_id),
            gdi_device: None,
        }
    }
}

impl std::fmt::Display for MonitorInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} (id={}, work_area={:?}, backing_scale={})",
            self.name, self.display_id, self.work_area, self.backing_scale
        )
    }
}

pub(in crate::platform::macos) fn get_all_monitors(
    mtm: MainThreadMarker,
) -> anyhow::Result<Vec<MonitorInfo>> {
    let primary_id = CGMainDisplayID();

    NSScreen::screens(mtm)
        .iter()
        .map(|screen| {
            let display_id = get_display_id(&screen)?;
            let name = screen.localizedName().to_string();
            let bounds = CGDisplayBounds(display_id);
            let frame = screen.frame();
            let visible = screen.visibleFrame();

            let top_inset =
                (frame.origin.y + frame.size.height) - (visible.origin.y + visible.size.height);
            let bottom_inset = visible.origin.y - frame.origin.y;

            Ok(MonitorInfo {
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
                backing_scale: screen.backingScaleFactor(),
            })
        })
        .collect()
}

fn get_display_id(screen: &NSScreen) -> anyhow::Result<CGDirectDisplayID> {
    let desc = screen.deviceDescription();
    let key = NSString::from_str("NSScreenNumber");
    let object = desc
        .objectForKey(&key)
        .context("NSScreen deviceDescription is missing NSScreenNumber")?;
    let number: &NSNumber = object
        .downcast_ref()
        .context("NSScreenNumber is not an NSNumber")?;
    Ok(number.unsignedIntValue())
}

type DisplayId = u32;

pub(super) struct MonitorRegistry {
    monitors: HashMap<MonitorId, MonitorInfo>,
    /// Never changes. A primary change overwrites the `monitors` entry under
    /// this id with the new display.
    primary_id: MonitorId,
}

impl MonitorRegistry {
    pub(super) fn new(primary: &MonitorInfo, primary_monitor_id: MonitorId) -> Self {
        Self {
            monitors: HashMap::from([(primary_monitor_id, primary.clone())]),
            primary_id: primary_monitor_id,
        }
    }

    fn id_for_display(&self, display_id: DisplayId) -> Option<MonitorId> {
        self.monitors
            .iter()
            .find(|(_, info)| info.display_id == display_id)
            .map(|(&id, _)| id)
    }

    pub(in crate::platform::macos) fn monitor(&self, monitor_id: MonitorId) -> &MonitorInfo {
        self.monitors
            .get(&monitor_id)
            .expect("monitor not found in registry")
    }

    pub(super) fn primary_monitor(&self) -> &MonitorInfo {
        self.monitors
            .get(&self.primary_id)
            .expect("primary monitor present")
    }

    pub(in crate::platform::macos) fn primary_full_height(&self) -> f32 {
        self.primary_monitor().full_height
    }

    /// Returns the monitor displaced from the incoming primary display, if this
    /// registry already tracked one there.
    pub(super) fn replace_primary(&mut self, new_info: &MonitorInfo) -> Option<MonitorId> {
        let displaced = self.id_for_display(new_info.display_id);
        if let Some(id) = displaced {
            self.monitors.remove(&id);
        }
        let old = self.primary_monitor().display_id;
        self.monitors.insert(self.primary_id, new_info.clone());
        tracing::info!(old, new = new_info.display_id, "Primary monitor replaced");
        displaced
    }

    pub(super) fn insert(&mut self, monitor: &MonitorInfo, monitor_id: MonitorId) {
        self.monitors.insert(monitor_id, monitor.clone());
    }

    pub(super) fn remove_stale(&mut self, current: &HashSet<DisplayId>) -> Vec<MonitorId> {
        let stale: Vec<_> = self
            .monitors
            .iter()
            .filter(|(_, info)| !current.contains(&info.display_id))
            .map(|(&id, _)| id)
            .collect();
        for id in &stale {
            self.monitors.remove(id);
        }
        stale
    }

    pub(super) fn all_monitors(&self) -> Vec<MonitorInfo> {
        self.monitors.values().cloned().collect()
    }

    /// Returns the monitor whose `bounds` overlap `dim` the most by intersection
    /// area. Reads only the cached `bounds`, so it works in tests where
    /// CoreGraphics is not initialized.
    pub(super) fn find_closest_monitor(&self, dim: Dimension) -> Option<(MonitorId, &MonitorInfo)> {
        let mut best: Option<(MonitorId, &MonitorInfo, f32)> = None;
        for (&id, info) in &self.monitors {
            let area = intersection_area(dim, info.bounds);
            if area <= 0.0 {
                continue;
            }
            if best.map(|(_, _, b)| area > b).unwrap_or(true) {
                best = Some((id, info, area));
            }
        }
        best.map(|(id, info, _)| (id, info))
    }

    /// A rect that overlaps no monitor on either side counts as no crossing.
    pub(super) fn crosses_monitor(&self, from: PixelRect, to: PixelRect) -> bool {
        let id = |rect: PixelRect| {
            self.find_closest_monitor(rect.to_dimension())
                .map(|(id, _)| id)
        };
        match (id(from), id(to)) {
            (Some(a), Some(b)) => a != b,
            _ => false,
        }
    }

    pub(super) fn is_borderless_fullscreen_at(&self, rect: PixelRect) -> bool {
        let point = Dimension::new(
            Length::from_pixels(rect.x()),
            Length::from_pixels(rect.y()),
            Length::new(1.0),
            Length::new(1.0),
        );
        let monitor = self.find_closest_monitor(point);
        monitor.is_some_and(|(_, info)| {
            let mon = info.work_area;
            let tolerance = Pixels::new(2);
            rect.x() <= mon.x() + tolerance
                && rect.y() <= mon.y() + tolerance
                && rect.right() >= mon.right() - tolerance
                && rect.bottom() >= mon.bottom() - tolerance
        })
    }

    pub(super) fn update_monitor(
        &mut self,
        monitor: &MonitorInfo,
    ) -> Option<(MonitorId, PixelRect)> {
        let (&id, entry) = self
            .monitors
            .iter_mut()
            .find(|(_, info)| info.display_id == monitor.display_id)?;
        let old_work_area = entry.work_area;
        *entry = monitor.clone();
        Some((id, old_work_area))
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

        // Mirroring moves the primary role between displays, and the workspaces
        // follow the role instead of parking.
        if let Some(new_primary) = monitors.iter().find(|s| s.is_primary)
            && new_primary.display_id != self.primary_monitor().display_id
        {
            let occupant = self.replace_primary(new_primary);
            let primary_monitor_id = hub.primary_monitor();
            hub.update_monitor(primary_monitor_id, new_primary.into(), occupant);
        }

        // Add new monitors first to prevent exhausting all monitors
        for monitor in monitors {
            if self.id_for_display(monitor.display_id).is_none() {
                let id = hub.add_monitor(monitor.into());
                self.insert(monitor, id);
                tracing::info!(%monitor, "Monitor added");
            }
        }

        let removed = self.remove_stale(&current_keys);
        if !removed.is_empty() {
            for monitor_id in &removed {
                hub.remove_monitor(*monitor_id);
            }
            let primary = hub.primary_monitor();
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
                hub.update_monitor(monitor_id, monitor.into(), None);
            }
        }
    }
}

impl Dome {
    pub(super) fn reconcile_monitors(&mut self) {
        self.monitor_registry
            .reconcile(&mut self.hub, &self.monitors);
        self.primary_full_height = self.monitor_registry.primary_full_height();
    }
}
