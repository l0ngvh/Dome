use std::collections::HashMap;
use std::process::Command;

use objc2_core_graphics::CGDirectDisplayID;

use crate::core::{Dimension, Length};

use super::MonitorInfo;

pub(in crate::platform::macos) struct ExternalBarProbe;

impl ExternalBarProbe {
    /// `Ok(None)` means no known bar is running or the probe failed, which the
    /// caller treats as reserve nothing.
    pub(in crate::platform::macos) fn query() -> anyhow::Result<Option<BarGeometry>> {
        if let Some(geo) = Self::query_sketchybar()? {
            return Ok(Some(geo));
        }
        Ok(None)
    }

    /// No per-call timeout: the caller runs this off the dome thread on the GCD
    /// worker, so a slow bar cannot block the AppKit loop.
    fn query_sketchybar() -> anyhow::Result<Option<BarGeometry>> {
        let output = match Command::new("sketchybar").args(["--query", "bar"]).output() {
            Ok(output) => output,
            Err(e) => {
                crate::log_dedup::warn_once!(
                    key: "sketchybar-probe",
                    "SketchyBar probe failed, reserving no space: {e}"
                );
                return Ok(None);
            }
        };
        if !output.status.success() {
            crate::log_dedup::warn_once!(
                key: "sketchybar-probe",
                "SketchyBar probe returned non-success status, reserving no space"
            );
            return Ok(None);
        }
        let Ok(stdout) = std::str::from_utf8(&output.stdout) else {
            crate::log_dedup::warn_once!(
                key: "sketchybar-probe",
                "SketchyBar probe returned non-UTF8 output, reserving no space"
            );
            return Ok(None);
        };
        let query: BarQuery = match serde_json::from_str(stdout) {
            Ok(query) => query,
            Err(e) => {
                crate::log_dedup::warn_once!(
                    key: "sketchybar-probe",
                    "SketchyBar probe output did not parse, reserving no space: {e}"
                );
                return Ok(None);
            }
        };
        Ok(query.into_geometry())
    }
}

#[derive(Debug)]
pub(in crate::platform::macos) struct BarGeometry {
    height: Option<f64>,
    position: Option<String>,
    y_offset: Option<f64>,
    margin: Option<f64>,
}

impl BarGeometry {
    pub(in crate::platform::macos) fn new(
        height: Option<f64>,
        position: Option<String>,
        y_offset: Option<f64>,
        margin: Option<f64>,
    ) -> Self {
        Self {
            height,
            position,
            y_offset,
            margin,
        }
    }
}

/// Reserves the same strip on every monitor because `sketchybar --query bar`
/// does not report the bar's target-display set.
pub(in crate::platform::macos) fn reserved_rects(
    geo: &BarGeometry,
    monitors: &[MonitorInfo],
) -> HashMap<CGDirectDisplayID, Dimension> {
    let mut rects = HashMap::new();
    let Some(height) = geo.height else {
        return rects;
    };
    let h = Length::new((height + geo.y_offset.unwrap_or(0.0) + geo.margin.unwrap_or(0.0)) as f32);
    let bottom = geo.position.as_deref() == Some("bottom");
    for m in monitors {
        let y = if bottom {
            m.bounds.y + m.bounds.height - h
        } else {
            m.bounds.y
        };
        rects.insert(
            m.display_id,
            Dimension::new(m.bounds.x, y, m.bounds.width, h),
        );
    }
    rects
}

#[derive(serde::Deserialize)]
struct BarQuery {
    height: Option<f64>,
    position: Option<String>,
    y_offset: Option<f64>,
    margin: Option<f64>,
    // SketchyBar reports hidden as the strings "on"/"off", not JSON bools.
    hidden: Option<String>,
    // A wedged query answers {"error":...}, which parses as an otherwise-empty
    // BarQuery. Guard on it so an error envelope reserves nothing.
    error: Option<String>,
}

impl BarQuery {
    fn into_geometry(self) -> Option<BarGeometry> {
        if self.error.is_some() {
            return None;
        }
        if self.hidden.as_deref() == Some("on") {
            return None;
        }
        self.height?;
        Some(BarGeometry::new(
            self.height,
            self.position,
            self.y_offset,
            self.margin,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::PixelRect;
    use crate::platform::reserve_for_bar;

    const SAMPLE: &str = r#"{
        "position": "top",
        "topmost": "off",
        "sticky": "on",
        "hidden": "off",
        "shadow": "off",
        "font_smoothing": "off",
        "show_in_fullscreen": "off",
        "blur_radius": 0,
        "margin": 0,
        "drawing": "off",
        "color": "0x44000000",
        "border_color": "0xffff0000",
        "border_width": 0,
        "height": 25,
        "corner_radius": 0,
        "padding_left": 20,
        "padding_right": 20,
        "x_offset": 0,
        "y_offset": 0,
        "clip": 0.000000,
        "image": {
            "value": "(null)",
            "drawing": "off",
            "scale": 1.000000
        },
        "items": [
            "dome.ws.0",
            "dome.driver"
        ]
    }"#;

    fn monitor(display_id: CGDirectDisplayID, x: f32, y: f32, w: f32, h: f32) -> MonitorInfo {
        let dim = Dimension::new(
            Length::new(x),
            Length::new(y),
            Length::new(w),
            Length::new(h),
        );
        MonitorInfo {
            display_id,
            name: format!("Monitor {display_id}"),
            work_area: PixelRect::from_dimension_inward(dim),
            bounds: dim,
            full_height: h,
            is_primary: display_id == 1,
            scale: 2.0,
        }
    }

    #[test]
    fn parses_captured_sketchybar_bar_query() {
        let query: BarQuery = serde_json::from_str(SAMPLE).unwrap();
        assert_eq!(query.position.as_deref(), Some("top"));
        assert_eq!(query.height, Some(25.0));
        assert_eq!(query.margin, Some(0.0));
        assert_eq!(query.y_offset, Some(0.0));
        assert_eq!(query.hidden.as_deref(), Some("off"));
        assert!(query.error.is_none());
    }

    #[test]
    fn folds_visible_bar_into_geometry() {
        let query = BarQuery {
            height: Some(25.0),
            position: Some("top".into()),
            y_offset: Some(2.0),
            margin: Some(3.0),
            hidden: Some("off".into()),
            error: None,
        };
        let geo = query.into_geometry().expect("visible bar yields geometry");
        assert_eq!(geo.height, Some(25.0));
        assert_eq!(geo.position.as_deref(), Some("top"));
        assert_eq!(geo.y_offset, Some(2.0));
        assert_eq!(geo.margin, Some(3.0));
    }

    #[test]
    fn hidden_bar_yields_no_geometry() {
        let query = BarQuery {
            height: Some(25.0),
            position: Some("top".into()),
            y_offset: None,
            margin: None,
            hidden: Some("on".into()),
            error: None,
        };
        assert!(query.into_geometry().is_none());
    }

    #[test]
    fn visible_bar_with_drawing_off_still_reserves() {
        let query: BarQuery = serde_json::from_str(SAMPLE).unwrap();
        let geo = query
            .into_geometry()
            .expect("visible bar with drawing off yields geometry");
        assert_eq!(geo.height, Some(25.0));
        assert_eq!(geo.position.as_deref(), Some("top"));
    }

    #[test]
    fn missing_height_yields_no_geometry() {
        let query = BarQuery {
            height: None,
            position: Some("top".into()),
            y_offset: None,
            margin: None,
            hidden: Some("off".into()),
            error: None,
        };
        assert!(query.into_geometry().is_none());
    }

    #[test]
    fn error_envelope_yields_no_geometry() {
        let query: BarQuery = serde_json::from_str(r#"{"error":"query timed out"}"#).unwrap();
        assert!(query.into_geometry().is_none());
    }

    #[test]
    fn top_bar_reserves_same_strip_on_every_monitor() {
        let a = monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let b = monitor(2, 1920.0, 0.0, 2560.0, 1440.0);
        let geo = BarGeometry::new(Some(20.0), Some("top".into()), Some(5.0), Some(5.0));

        let rects = reserved_rects(&geo, &[a.clone(), b.clone()]);

        let h = Length::new(30.0);
        assert_eq!(
            rects.get(&1),
            Some(&Dimension::new(a.bounds.x, a.bounds.y, a.bounds.width, h))
        );
        assert_eq!(
            rects.get(&2),
            Some(&Dimension::new(b.bounds.x, b.bounds.y, b.bounds.width, h))
        );

        let bar = rects.get(&1).copied().unwrap();
        assert_eq!(
            reserve_for_bar(a.bounds, a.work_area.to_dimension(), bar),
            Dimension::new(
                Length::ZERO,
                Length::new(30.0),
                Length::new(1920.0),
                Length::new(1050.0),
            )
        );
    }

    #[test]
    fn bottom_bar_anchors_at_monitor_bottom() {
        let a = monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let geo = BarGeometry::new(Some(30.0), Some("bottom".into()), None, None);

        let rects = reserved_rects(&geo, std::slice::from_ref(&a));

        let h = Length::new(30.0);
        assert_eq!(
            rects.get(&1),
            Some(&Dimension::new(
                a.bounds.x,
                a.bounds.y + a.bounds.height - h,
                a.bounds.width,
                h,
            ))
        );
    }

    #[test]
    fn absent_height_reserves_nothing() {
        let a = monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let geo = BarGeometry::new(None, Some("top".into()), None, None);
        assert!(reserved_rects(&geo, &[a]).is_empty());
    }
}
