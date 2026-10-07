//! Stills of the stage and the border check on them.

use std::path::Path;
use std::process::{Command, Stdio};

use spike::geometry::Rect;
use spike::windows::WinInfo;

/// Border thickness in points. Must match the overlay.
pub const THICKNESS: f64 = 6.0;
/// Must match the overlay's default `--level`. A window at this layer or above
/// is composited over the border, so its pixels decide nothing here.
pub const OVERLAY_LEVEL: i64 = 1000;
const STEP: f64 = 4.0;
const MARGIN: f64 = 3.0;

pub struct Image {
    pub width: usize,
    pub height: usize,
    /// Three bytes per pixel, row-major.
    pub rgb: Vec<u8>,
}

impl Image {
    pub fn load(path: &Path) -> Result<Image, String> {
        let file = std::fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
        let mut decoder = png::Decoder::new(std::io::BufReader::new(file));
        decoder.set_transformations(png::Transformations::normalize_to_color8());
        let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
        let size = reader
            .output_buffer_size()
            .ok_or_else(|| String::from("png too large"))?;
        let mut buf = vec![0; size];
        let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
        let channels = info.color_type.samples();
        let (width, height) = (info.width as usize, info.height as usize);
        let mut rgb = Vec::with_capacity(width * height * 3);
        for row in 0..height {
            let line = &buf[row * info.line_size..];
            for x in 0..width {
                let px = &line[x * channels..];
                match channels {
                    1 | 2 => rgb.extend_from_slice(&[px[0], px[0], px[0]]),
                    _ => rgb.extend_from_slice(&px[..3]),
                }
            }
        }
        Ok(Image { width, height, rgb })
    }

    pub fn pixel(&self, x: usize, y: usize) -> Option<(u8, u8, u8)> {
        if x >= self.width || y >= self.height {
            return None;
        }
        let i = (y * self.width + x) * 3;
        Some((self.rgb[i], self.rgb[i + 1], self.rgb[i + 2]))
    }
}

/// The capture is color managed, so pure sRGB magenta arrives shifted.
pub fn is_magenta((r, g, b): (u8, u8, u8)) -> bool {
    r > 170 && b > 170 && g < 110
}

pub fn capture(path: &Path, stage: Rect) -> Result<(), String> {
    let region = format!("-R{},{},{},{}", stage.x, stage.y, stage.w, stage.h);
    let status = Command::new("screencapture")
        .args(["-x", &region])
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|e| e.to_string())?;
    if !status.success() {
        return Err(format!("screencapture exited with {status}"));
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub checked: usize,
    pub missing: usize,
    pub over: usize,
}

impl Counts {
    pub fn failed(&self) -> bool {
        self.missing > 0 || self.over > 0
    }
}

fn contains(r: &Rect, (x, y): (f64, f64)) -> bool {
    r.x <= x && x < r.right() && r.y <= y && y < r.bottom()
}

/// Whether a point on the ring midline of one window should be magenta.
/// `front` holds every window in front of that window. `None` skips a point
/// too close to a ring corner or to an occluder edge for a pixel to decide.
pub fn expect_border(p: (f64, f64), midline: &Rect, front: &[Rect]) -> Option<bool> {
    let corners = [
        (midline.x, midline.y),
        (midline.right(), midline.y),
        (midline.x, midline.bottom()),
        (midline.right(), midline.bottom()),
    ];
    if corners
        .iter()
        .any(|c| (p.0 - c.0).abs() < MARGIN && (p.1 - c.1).abs() < MARGIN)
    {
        return None;
    }
    if front
        .iter()
        .any(|r| contains(&r.outset(MARGIN), p) && !contains(&r.outset(-MARGIN), p))
    {
        return None;
    }
    Some(!front.iter().any(|r| contains(r, p)))
}

/// Points every `STEP` along the rectangle halfway through the ring.
pub fn midline_points(frame: &Rect) -> (Rect, Vec<(f64, f64)>) {
    let m = frame.outset(THICKNESS / 2.0);
    let mut points = Vec::new();
    let mut x = m.x;
    while x <= m.right() {
        points.push((x, m.y));
        points.push((x, m.bottom()));
        x += STEP;
    }
    let mut y = m.y + STEP;
    while y < m.bottom() {
        points.push((m.x, y));
        points.push((m.right(), y));
        y += STEP;
    }
    (m, points)
}

/// Whether `p` lies in the ring band of `frame`, widened by `MARGIN` on both
/// sides.
fn on_ring(frame: &Rect, p: (f64, f64)) -> bool {
    contains(&frame.outset(THICKNESS + MARGIN), p) && !contains(&frame.outset(-MARGIN), p)
}

/// Checks the ring of every window in `windows` that `targets` names. Also
/// returns one line per failed point, naming the target and the kind.
pub fn check(
    image: &Image,
    stage: &Rect,
    windows: &[WinInfo],
    targets: &[u32],
) -> (Counts, Vec<String>) {
    let scale = image.width as f64 / stage.w;
    let mut counts = Counts::default();
    let mut failures = Vec::new();
    for (i, w) in windows.iter().enumerate() {
        if !targets.contains(&w.id) {
            continue;
        }
        let front: Vec<Rect> = windows[..i]
            .iter()
            .filter(|o| o.layer < OVERLAY_LEVEL)
            .map(|o| o.rect)
            .collect();
        // Another bordered window in front may draw its own ring across a
        // covered point, so magenta there proves nothing.
        let front_rings: Vec<Rect> = windows[..i]
            .iter()
            .filter(|o| targets.contains(&o.id))
            .map(|o| o.rect)
            .collect();
        let (midline, points) = midline_points(&w.rect);
        for p in points {
            if !contains(&stage.outset(-1.0), p) {
                continue;
            }
            let Some(expected) = expect_border(p, &midline, &front) else {
                continue;
            };
            if !expected && front_rings.iter().any(|r| on_ring(r, p)) {
                continue;
            }
            let px = ((p.0 - stage.x) * scale) as usize;
            let py = ((p.1 - stage.y) * scale) as usize;
            let Some(color) = image.pixel(px, py) else {
                continue;
            };
            let magenta = is_magenta(color);
            counts.checked += 1;
            if expected && !magenta {
                counts.missing += 1;
                failures.push(format!("missing id={} at {},{} {color:?}", w.id, p.0, p.1));
            }
            if !expected && magenta {
                counts.over += 1;
                failures.push(format!("over id={} at {},{} {color:?}", w.id, p.0, p.1));
            }
        }
    }
    (counts, failures)
}

pub fn count_magenta(image: &Image) -> usize {
    image
        .rgb
        .chunks_exact(3)
        .filter(|p| is_magenta((p[0], p[1], p[2])))
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_uncovered_midline_point_expects_magenta() {
        let frame = Rect::new(100.0, 100.0, 200.0, 100.0);
        let (m, _) = midline_points(&frame);
        assert_eq!(expect_border((200.0, 97.0), &m, &[]), Some(true));
    }

    #[test]
    fn a_covered_point_expects_no_magenta() {
        let frame = Rect::new(100.0, 100.0, 200.0, 100.0);
        let (m, _) = midline_points(&frame);
        let cover = Rect::new(150.0, 50.0, 100.0, 100.0);
        assert_eq!(expect_border((200.0, 97.0), &m, &[cover]), Some(false));
    }

    #[test]
    fn points_near_an_occluder_edge_or_a_corner_are_skipped() {
        let frame = Rect::new(100.0, 100.0, 200.0, 100.0);
        let (m, _) = midline_points(&frame);
        let cover = Rect::new(150.0, 50.0, 100.0, 100.0);
        assert_eq!(expect_border((151.0, 97.0), &m, &[cover]), None);
        assert_eq!(expect_border((98.0, 98.0), &m, &[]), None);
    }

    #[test]
    fn a_front_ring_crossing_a_covered_point_is_not_over_content() {
        let front = Rect::new(400.0, 280.0, 500.0, 380.0);
        assert!(on_ring(&front, (395.0, 533.0)));
        assert!(!on_ring(&front, (600.0, 500.0)));
        assert!(!on_ring(&front, (380.0, 533.0)));
    }

    #[test]
    fn midline_points_lie_on_the_ring_midline() {
        let frame = Rect::new(10.0, 10.0, 40.0, 20.0);
        let (m, points) = midline_points(&frame);
        assert_eq!(m, Rect::new(7.0, 7.0, 46.0, 26.0));
        assert!(
            points
                .iter()
                .all(|&(x, y)| { x == m.x || x == m.right() || y == m.y || y == m.bottom() })
        );
        assert!(points.len() > 20);
    }

    #[test]
    fn magenta_classification_rejects_fixture_colors() {
        assert!(is_magenta((235, 51, 247)));
        for color in [
            (48, 80, 200),
            (48, 160, 80),
            (208, 128, 48),
            (48, 176, 192),
            (128, 128, 128),
        ] {
            assert!(!is_magenta(color), "{color:?}");
        }
    }
}
