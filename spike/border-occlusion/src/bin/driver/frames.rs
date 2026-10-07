//! The per-frame check on a screen recording of moving windows. A correct ring
//! always has a different color on one side, either its own window's edge or
//! whatever lies outside the window. So a ring with one window's content color
//! on both sides lies across that window's content, and the frame fails.

use std::io::Read;
use std::path::Path;
use std::process::{Child, Command, Stdio};

use spike::geometry::Rect;

use crate::check::is_magenta;
use crate::procs;

/// The four window content colors as a decoded `screencapture -v` frame
/// shows them.
pub const PALETTE: [(u8, u8, u8); 4] = [(40, 69, 192), (40, 147, 68), (201, 115, 40), (38, 165, 181)];
/// The squared distance from a palette color within which a pixel takes that
/// color. The decode shifts and blurs colors near an edge.
const MAX_DISTANCE_SQ: i32 = 45 * 45;
/// A ring is 6 pt thick, so a run this long lies across its thickness. The
/// decode blurs each end of a run by up to about a point.
const RUN: std::ops::RangeInclusive<usize> = 2..=10;
/// Where the color on each side of a run is sampled, in pixels past the run's
/// last magenta pixel. Both samples must agree, so a window's 1 pt edge line or
/// one blurred pixel decides nothing.
const OFFSETS: [usize; 2] = [3, 6];
/// Consecutive failing scan lines that fail a frame. A window's rounded corner
/// shows the window behind it next to a correct ring for fewer lines.
pub const MIN_STREAK: usize = 12;
/// How far a streak's run may shift from one scan line to the next.
const STREAK_SLACK: usize = 3;

pub fn window_color((r, g, b): (u8, u8, u8)) -> Option<usize> {
    PALETTE
        .iter()
        .map(|&(pr, pg, pb)| {
            let d = |a: u8, b: u8| i32::from(a) - i32::from(b);
            d(r, pr).pow(2) + d(g, pg).pow(2) + d(b, pb).pow(2)
        })
        .enumerate()
        .filter(|&(_, d)| d <= MAX_DISTANCE_SQ)
        .min_by_key(|&(_, d)| d)
        .map(|(i, _)| i)
}

pub struct Frame<'a> {
    pub width: usize,
    pub height: usize,
    /// Three bytes per pixel, row-major.
    pub rgb: &'a [u8],
}

impl Frame<'_> {
    fn pixel(&self, x: usize, y: usize) -> (u8, u8, u8) {
        let i = (y * self.width + x) * 3;
        (self.rgb[i], self.rgb[i + 1], self.rgb[i + 2])
    }
}

/// Consecutive scan lines whose magenta run has one window's color on both
/// sides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Streak {
    /// True for runs along columns, which cross a horizontal ring piece.
    pub columns: bool,
    /// The palette index.
    pub color: usize,
    /// The frame pixel at the middle of the first run.
    pub at: (usize, usize),
    pub lines: usize,
    /// The middle of the latest run, along the scan line.
    center: usize,
}

/// The longest streak in either direction, or `None` when no scan line fails.
pub fn longest_streak(frame: &Frame) -> Option<Streak> {
    let rows = scan(frame.height, frame.width, |line, pos| frame.pixel(pos, line), false);
    let columns = scan(frame.width, frame.height, |line, pos| frame.pixel(line, pos), true);
    [rows, columns].into_iter().flatten().max_by_key(|s| s.lines)
}

pub fn fails(frame: &Frame) -> Option<Streak> {
    longest_streak(frame).filter(|s| s.lines >= MIN_STREAK)
}

fn scan(
    lines: usize,
    len: usize,
    px: impl Fn(usize, usize) -> (u8, u8, u8),
    columns: bool,
) -> Option<Streak> {
    let mut best: Option<Streak> = None;
    let mut active: Vec<Streak> = Vec::new();
    for line in 0..lines {
        let mut next = Vec::new();
        let mut pos = 0;
        while pos < len {
            if !is_magenta(px(line, pos)) {
                pos += 1;
                continue;
            }
            let start = pos;
            while pos < len && is_magenta(px(line, pos)) {
                pos += 1;
            }
            if !RUN.contains(&(pos - start)) {
                continue;
            }
            let last = pos - 1;
            let side = |samples: [Option<usize>; 2]| -> Option<usize> {
                let colors: Vec<Option<usize>> = samples
                    .iter()
                    .map(|s| s.filter(|&p| p < len).and_then(|p| window_color(px(line, p))))
                    .collect();
                colors[0].filter(|_| colors.iter().all(|c| *c == colors[0]))
            };
            let before = side(OFFSETS.map(|o| start.checked_sub(o)));
            let after = side(OFFSETS.map(|o| Some(last + o)));
            let (Some(color), Some(other)) = (before, after) else {
                continue;
            };
            if color != other {
                continue;
            }
            let center = (start + last) / 2;
            let found = active
                .iter()
                .position(|s| s.color == color && s.center.abs_diff(center) <= STREAK_SLACK);
            let streak = match found {
                Some(i) => {
                    let mut s = active.swap_remove(i);
                    s.lines += 1;
                    s.center = center;
                    s
                }
                None => Streak {
                    columns,
                    color,
                    at: if columns { (line, center) } else { (center, line) },
                    lines: 1,
                    center,
                },
            };
            if best.is_none_or(|b| streak.lines > b.lines) {
                best = Some(streak);
            }
            next.push(streak);
        }
        active = next;
    }
    best
}

#[derive(Clone, Debug, Default)]
pub struct MovieCheck {
    pub frames: usize,
    pub failing: usize,
    /// From the first frame of the longest failing run to the next frame that
    /// passes, or to the end of the recording.
    pub longest_ms: f64,
    /// A PNG of the first frame of the longest failing run.
    pub image: Option<String>,
    pub note: Option<String>,
}

pub fn start_recording(movie: &Path, stage: &Rect, secs: u64) -> Result<Child, String> {
    let _ = std::fs::remove_file(movie);
    procs::spawn(
        Command::new("screencapture")
            .args([
                "-x",
                "-v",
                &format!("-V{secs}"),
                &format!("-R{},{},{},{}", stage.x, stage.y, stage.w, stage.h),
            ])
            .arg(movie)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )
}

/// The presentation time of each frame, in seconds.
fn frame_times(movie: &Path) -> Result<Vec<f64>, String> {
    let out = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "frame=pts_time",
            "-of",
            "csv=p=0",
        ])
        .arg(movie)
        .output()
        .map_err(|e| format!("ffprobe: {e}"))?;
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.split(',').next()?.trim().parse().ok())
        .collect())
}

/// Checks every recorded frame, decoded at one pixel per point. Writes the
/// first frame of the longest failing run to `image` and one line per failing
/// frame beside it.
pub fn check_movie(movie: &Path, stage: &Rect, image: &Path) -> Result<MovieCheck, String> {
    let times = frame_times(movie)?;
    let (w, h) = (stage.w as usize, stage.h as usize);
    // `screencapture -v` records at a variable rate, and the default output
    // rate would repeat frames to fill a fixed one.
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(movie)
        .args([
            "-fps_mode",
            "passthrough",
            "-vf",
            &format!("scale={w}:{h}:flags=neighbor"),
            "-f",
            "rawvideo",
            "-pix_fmt",
            "rgb24",
            "-",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|e| format!("ffmpeg: {e}"))?;
    procs::register(child.id());
    let mut out = child.stdout.take().ok_or("ffmpeg stdout")?;
    let mut buf = vec![0u8; w * h * 3];
    let mut results: Vec<Option<Streak>> = Vec::new();
    // The first frame of the failing run in progress, and the longest run so
    // far as (first frame, milliseconds, pixels of the first frame).
    let mut current: Option<(usize, Vec<u8>)> = None;
    let mut longest: Option<(usize, f64, Vec<u8>)> = None;
    let mut close_run = |current: &mut Option<(usize, Vec<u8>)>, end: usize| {
        if let Some((start, rgb)) = current.take() {
            let ms = run_ms(&times, start, end);
            if longest.as_ref().is_none_or(|(_, best, _)| ms > *best) {
                longest = Some((start, ms, rgb));
            }
        }
    };
    while out.read_exact(&mut buf).is_ok() {
        let n = results.len();
        let result = fails(&Frame {
            width: w,
            height: h,
            rgb: &buf,
        });
        if result.is_some() && current.is_none() {
            current = Some((n, buf.clone()));
        }
        if result.is_none() {
            close_run(&mut current, n);
        }
        results.push(result);
    }
    let frames = results.len();
    close_run(&mut current, frames);
    let _ = child.wait();
    procs::unregister(child.id());
    let mut check = MovieCheck {
        frames,
        failing: results.iter().filter(|r| r.is_some()).count(),
        ..MovieCheck::default()
    };
    if times.len() != frames {
        check.note = Some(format!("{} frame times for {frames} frames", times.len()));
    }
    let mut lines = String::from("frame pts_s direction color x y lines\n");
    for (i, r) in results.iter().enumerate() {
        if let Some(s) = r {
            lines.push_str(&format!(
                "{i} {:.3} {} {} {} {} {}\n",
                times.get(i).copied().unwrap_or(f64::NAN),
                if s.columns { "columns" } else { "rows" },
                s.color,
                s.at.0,
                s.at.1,
                s.lines
            ));
        }
    }
    if let Some((_, ms, rgb)) = longest {
        check.longest_ms = ms;
        write_png(image, w, h, &rgb)?;
        let _ = std::fs::write(image.with_extension("txt"), lines);
        check.image = Some(image.display().to_string());
    }
    Ok(check)
}

/// From the start of frame `start` to the start of frame `end`. Past the last
/// frame, the recording ends one mean frame interval after the last frame.
fn run_ms(times: &[f64], start: usize, end: usize) -> f64 {
    let at = |i: usize| -> Option<f64> {
        if let Some(t) = times.get(i) {
            return Some(*t);
        }
        let (first, last) = (times.first()?, times.last()?);
        let step = (last - first) / (times.len() - 1).max(1) as f64;
        Some(last + step * (i + 1 - times.len()) as f64)
    };
    match (at(start), at(end)) {
        (Some(s), Some(e)) => (e - s) * 1000.0,
        _ => 0.0,
    }
}

fn write_png(path: &Path, w: usize, h: usize, rgb: &[u8]) -> Result<(), String> {
    let file = std::fs::File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), w as u32, h as u32);
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|e| e.to_string())?;
    writer.write_image_data(rgb).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GRAY: (u8, u8, u8) = (115, 115, 115);
    const MAGENTA: (u8, u8, u8) = (235, 51, 247);

    struct Canvas {
        width: usize,
        height: usize,
        rgb: Vec<u8>,
    }

    impl Canvas {
        fn new(width: usize, height: usize) -> Canvas {
            let mut rgb = Vec::with_capacity(width * height * 3);
            for _ in 0..width * height {
                rgb.extend_from_slice(&[GRAY.0, GRAY.1, GRAY.2]);
            }
            Canvas { width, height, rgb }
        }

        fn fill(&mut self, r: Rect, (cr, cg, cb): (u8, u8, u8)) {
            for y in r.y as usize..r.bottom() as usize {
                for x in r.x as usize..r.right() as usize {
                    let i = (y * self.width + x) * 3;
                    self.rgb[i..i + 3].copy_from_slice(&[cr, cg, cb]);
                }
            }
        }

        fn ring(&mut self, r: Rect) {
            for piece in spike::geometry::border_ring(&r, 6.0) {
                self.fill(piece, MAGENTA);
            }
        }

        fn frame(&self) -> Frame<'_> {
            Frame {
                width: self.width,
                height: self.height,
                rgb: &self.rgb,
            }
        }
    }

    /// A green window over a blue one. The green window's ring is correct.
    fn stage() -> (Canvas, Rect, Rect) {
        let mut c = Canvas::new(400, 300);
        let blue = Rect::new(40.0, 40.0, 200.0, 150.0);
        let green = Rect::new(150.0, 100.0, 200.0, 150.0);
        c.fill(blue, PALETTE[0]);
        c.fill(green, PALETTE[1]);
        c.ring(green);
        (c, blue, green)
    }

    #[test]
    fn a_correct_ring_passes() {
        let (c, _, _) = stage();
        assert_eq!(fails(&c.frame()), None);
    }

    #[test]
    fn a_ring_across_a_window_fails_where_it_crosses() {
        let (mut c, blue, _) = stage();
        // The blue window's ring drawn whole, as if the blue window were in
        // front. Its right and bottom pieces cross the green window.
        c.ring(blue);
        let streak = fails(&c.frame()).expect("the crossing ring fails the frame");
        assert_eq!(streak.color, 1, "{streak:?}");
        let (x, y) = streak.at;
        let crosses_right = !streak.columns && (240..246).contains(&x) && y >= 100;
        let crosses_bottom = streak.columns && (190..196).contains(&y) && x >= 150;
        assert!(crosses_right || crosses_bottom, "{streak:?}");
        assert!(streak.lines >= MIN_STREAK, "{streak:?}");
    }

    #[test]
    fn a_short_crossing_like_a_rounded_corner_passes() {
        let (mut c, _, green) = stage();
        let stub = Rect::new(green.x + 60.0, green.y + 20.0, 6.0, (MIN_STREAK - 1) as f64);
        c.fill(stub, MAGENTA);
        assert_eq!(fails(&c.frame()), None);
        assert_eq!(longest_streak(&c.frame()).map(|s| s.lines), Some(MIN_STREAK - 1));
    }

    #[test]
    fn palette_colors_classify_and_others_do_not() {
        for (i, color) in PALETTE.iter().enumerate() {
            assert_eq!(window_color(*color), Some(i));
        }
        for other in [GRAY, MAGENTA, (30, 30, 30), (128, 128, 128)] {
            assert_eq!(window_color(other), None, "{other:?}");
        }
    }
}
