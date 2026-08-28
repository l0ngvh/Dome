use std::collections::HashMap;
use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use anyhow::Context;
use objc2_core_graphics::CGDirectDisplayID;

use crate::core::{Dimension, Length};

use super::MonitorInfo;

const PROBE_TIMEOUT: Duration = Duration::from_millis(500);
const PROBE_POLL_STEP: Duration = Duration::from_millis(20);

pub(in crate::platform::macos) struct ExternalBarProbe;

impl ExternalBarProbe {
    /// `Err` means the bar could not be asked. A hidden bar is `Ok` with zero
    /// height.
    ///
    /// Spawns rather than runs to completion because a deadline needs a handle
    /// to kill, and `Command::output` consumes the child.
    pub(in crate::platform::macos) fn query() -> anyhow::Result<BarGeometry> {
        let mut child = Command::new("sketchybar")
            .args(["--query", "bar"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .context("spawn sketchybar --query bar")?;

        let deadline = Instant::now() + PROBE_TIMEOUT;
        let status = loop {
            if let Some(status) = child.try_wait().context("wait on sketchybar")? {
                break status;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                anyhow::bail!("sketchybar --query bar did not answer within {PROBE_TIMEOUT:?}");
            }
            std::thread::sleep(PROBE_POLL_STEP);
        };
        if !status.success() {
            anyhow::bail!("sketchybar --query bar exited with {status}");
        }

        // Draining only after exit would deadlock a child whose output exceeds
        // the pipe buffer. One small JSON object stays far under it.
        let mut stdout = String::new();
        child
            .stdout
            .take()
            .context("sketchybar stdout was not captured")?
            .read_to_string(&mut stdout)
            .context("read sketchybar bar query")?;
        let query: BarQuery = serde_json::from_str(&stdout).context("parse bar query")?;
        query.into_geometry()
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(in crate::platform::macos) struct BarGeometry {
    height: f64,
    position: Option<String>,
    y_offset: f64,
    margin: f64,
}

impl BarGeometry {
    pub(in crate::platform::macos) fn new(
        height: f64,
        position: Option<String>,
        y_offset: f64,
        margin: f64,
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
    let h = Length::new((geo.height + geo.y_offset + geo.margin) as f32);
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
    // BarQuery.
    error: Option<String>,
}

impl BarQuery {
    fn into_geometry(self) -> anyhow::Result<BarGeometry> {
        if let Some(error) = self.error {
            anyhow::bail!("bar query answered an error envelope: {error}");
        }
        if self.hidden.as_deref() == Some("on") {
            // Dropping the position keeps every zero geometry equal, so moving
            // a hidden bar cannot trigger a re-apply.
            return Ok(BarGeometry::new(0.0, None, 0.0, 0.0));
        }
        let height = self.height.context("bar query carried no height")?;
        Ok(BarGeometry::new(
            height,
            self.position,
            self.y_offset.unwrap_or(0.0),
            self.margin.unwrap_or(0.0),
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
        assert_eq!(geo.height, 25.0);
        assert_eq!(geo.position.as_deref(), Some("top"));
        assert_eq!(geo.y_offset, 2.0);
        assert_eq!(geo.margin, 3.0);
    }

    #[test]
    fn hidden_bar_yields_zero_geometry() {
        let query = BarQuery {
            height: Some(25.0),
            position: Some("top".into()),
            y_offset: None,
            margin: None,
            hidden: Some("on".into()),
            error: None,
        };
        let geo = query.into_geometry().expect("hidden bar yields geometry");
        assert_eq!(geo, BarGeometry::new(0.0, None, 0.0, 0.0));
    }

    #[test]
    fn visible_bar_with_drawing_off_still_reserves() {
        let query: BarQuery = serde_json::from_str(SAMPLE).unwrap();
        let geo = query
            .into_geometry()
            .expect("visible bar with drawing off yields geometry");
        assert_eq!(geo.height, 25.0);
        assert_eq!(geo.position.as_deref(), Some("top"));
    }

    #[test]
    fn missing_height_is_an_error() {
        let query = BarQuery {
            height: None,
            position: Some("top".into()),
            y_offset: None,
            margin: None,
            hidden: Some("off".into()),
            error: None,
        };
        assert!(query.into_geometry().is_err());
    }

    #[test]
    fn error_envelope_is_an_error() {
        let query: BarQuery = serde_json::from_str(r#"{"error":"query timed out"}"#).unwrap();
        assert!(query.into_geometry().is_err());
    }

    #[test]
    fn top_bar_reserves_same_strip_on_every_monitor() {
        let a = monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let b = monitor(2, 1920.0, 0.0, 2560.0, 1440.0);
        let geo = BarGeometry::new(20.0, Some("top".into()), 5.0, 5.0);

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
        let geo = BarGeometry::new(30.0, Some("bottom".into()), 0.0, 0.0);

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
    fn hidden_bar_shrinks_nothing() {
        let a = monitor(1, 0.0, 0.0, 1920.0, 1080.0);
        let geo = BarGeometry::new(0.0, None, 0.0, 0.0);

        let rects = reserved_rects(&geo, std::slice::from_ref(&a));

        let bar = rects.get(&1).copied().expect("zero geometry still maps");
        assert_eq!(
            reserve_for_bar(a.bounds, a.work_area.to_dimension(), bar),
            a.work_area.to_dimension()
        );
    }
}
