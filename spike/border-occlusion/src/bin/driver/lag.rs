//! Drag lag, measured on a screen recording of a constant-speed drag.

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use spike::geometry::Rect;
use spike::now_ms;

use crate::check::is_magenta;
use crate::cpu::field;
use crate::procs::{self, Overlay};
use crate::stage::{self, Owner, Stage};
use crate::{Ctx, input, overlay_args};

const SPEED_PT_PER_SEC: f64 = 400.0;
const DRAG: Duration = Duration::from_secs(2);
const RECORD_SECS: u64 = 4;
/// The run of window pixels that marks the window's left edge.
const MIN_WINDOW_RUN: usize = 4;
/// A gap this wide between the window's right edge and the ring means the
/// ring leads. The window's own edge line can leave one non-blue pixel.
const LEAD_GAP: usize = 2;

#[derive(Clone, Debug, Default)]
pub struct LagResult {
    pub mode: String,
    pub frames: usize,
    pub median: Option<f64>,
    pub p95: Option<f64>,
    pub moved_per_sec: Option<f64>,
    pub reads_per_sec: Option<f64>,
    /// AX reads whose rect differed from the previous read of the window.
    pub changed_per_sec: Option<f64>,
    pub note: Option<String>,
}

/// Per second from `start_ms` to `end_ms`: move notifications, AX reads, and
/// AX reads with a new rect.
pub fn drag_rates(log: &str, start_ms: u64, end_ms: u64) -> (f64, f64, f64) {
    let mut counts = (0.0, 0.0, 0.0);
    for line in log.lines() {
        let Some(ts) = field(line, "ts_ms").and_then(|v| v.parse::<u64>().ok()) else {
            continue;
        };
        if ts < start_ms || ts > end_ms {
            continue;
        }
        match field(line, "event") {
            Some("trigger") if field(line, "kind") == Some("moved") => counts.0 += 1.0,
            Some("axread") => {
                counts.1 += 1.0;
                if field(line, "changed") == Some("1") {
                    counts.2 += 1.0;
                }
            }
            _ => {}
        }
    }
    let secs = end_ms.saturating_sub(start_ms) as f64 / 1000.0;
    if secs <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    (counts.0 / secs, counts.1 / secs, counts.2 / secs)
}

fn is_window_blue((r, g, b): (u8, u8, u8)) -> bool {
    r < 110 && g < 150 && b > 140 && b as i32 > r as i32 + 60
}

/// Signed distance in pixels from the ring to the window along one row, and
/// the window's first visible column. Positive when the ring trails a window
/// moving right. A leading ring covers the window's left edge, so a lead is
/// read on the right side, as the gap between the window and the ring.
pub fn row_lag(row: &[u8]) -> Option<(f64, usize)> {
    let px = |x: usize| (row[x * 3], row[x * 3 + 1], row[x * 3 + 2]);
    let width = row.len() / 3;
    let window_left = (0..width.saturating_sub(MIN_WINDOW_RUN))
        .find(|&x| (x..x + MIN_WINDOW_RUN).all(|i| is_window_blue(px(i))))?;
    let mut rings = Vec::new();
    let mut x = 0;
    while x < width {
        if is_magenta(px(x)) {
            let start = x;
            while x < width && is_magenta(px(x)) {
                x += 1;
            }
            if x - start >= 2 {
                rings.push((start, x));
            }
        } else {
            x += 1;
        }
    }
    let left = rings.iter().rposition(|&(_, end)| end <= window_left + 2)?;
    let ring_end = rings[left].1;
    if let Some(&(right_start, _)) = rings.get(left + 1) {
        let window_end = (ring_end..right_start)
            .rev()
            .find(|&x| is_window_blue(px(x)))
            .map(|x| x + 1);
        if let Some(end) = window_end
            && end + LEAD_GAP <= right_start
        {
            return Some((end as f64 - right_start as f64, window_left));
        }
    }
    Some((window_left as f64 - ring_end as f64, window_left))
}

fn percentile(sorted: &[f64], p: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let i = (p * (sorted.len() - 1) as f64).round() as usize;
    Some(sorted[i])
}

/// Lags of the frames where the window moved since the frame before.
pub fn moving_lags(rows: &[Vec<u8>]) -> Vec<f64> {
    let mut lags = Vec::new();
    let mut previous_left = None;
    for row in rows {
        let Some((lag, left)) = row_lag(row) else {
            previous_left = None;
            continue;
        };
        if previous_left.is_some_and(|p| p != left) {
            lags.push(lag);
        }
        previous_left = Some(left);
    }
    lags
}

/// Decodes the recording scaled to one pixel per point and keeps one row.
fn decode_rows(movie: &std::path::Path, stage: &Rect, row: usize) -> Result<Vec<Vec<u8>>, String> {
    let (w, h) = (stage.w as usize, stage.h as usize);
    let mut child = Command::new("ffmpeg")
        .args(["-v", "error", "-i"])
        .arg(movie)
        .args([
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
    let mut frame = vec![0u8; w * h * 3];
    let mut rows = Vec::new();
    while out.read_exact(&mut frame).is_ok() {
        rows.push(frame[row * w * 3..(row + 1) * w * 3].to_vec());
    }
    let _ = child.wait();
    procs::unregister(child.id());
    Ok(rows)
}

pub fn run_mode(ctx: &mut Ctx, mode: &str) -> Result<LagResult, String> {
    let dir = ctx.out.join("lag");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut stage = Stage::start(&ctx.bin_dir)?;
    stage.reset(&[(
        Owner::A,
        "lag",
        Rect::new(150.0, 300.0, 400.0, 300.0),
        stage::BLUE,
    )])?;
    let overlay = Overlay::start(
        &ctx.bin_dir.join("border-occlusion-spike"),
        &overlay_args(mode, None),
        &dir.join(format!("{mode}.overlay.log")),
    )?;
    stage.attach(overlay.control.clone());
    std::thread::sleep(Duration::from_millis(1500));

    let movie = dir.join(format!("{mode}.mov"));
    let _ = std::fs::remove_file(&movie);
    let b = stage.backdrop;
    let target = stage.window("lag").rect;
    let grab = (target.x + 200.0, target.y + 12.0);
    // While `screencapture -v` records, its own full-screen window at layer
    // 1499 is the frontmost window at every point, so `input::press` would
    // refuse the press.
    input::press(grab, &stage.pids(), Some(overlay.pid));
    let mut recorder = procs::spawn(
        Command::new("screencapture")
            .args([
                "-x",
                "-v",
                &format!("-V{RECORD_SECS}"),
                &format!("-R{},{},{},{}", b.x, b.y, b.w, b.h),
            ])
            .arg(&movie)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )?;
    std::thread::sleep(Duration::from_millis(1000));
    let drag_start = now_ms();
    let release = input::drag_pressed(grab, DRAG, |t| (grab.0 + SPEED_PT_PER_SEC * t, grab.1));

    let deadline = Instant::now() + Duration::from_secs(RECORD_SECS + 15);
    loop {
        if let Ok(Some(_)) = recorder.try_wait() {
            procs::unregister(recorder.id());
            break;
        }
        if Instant::now() > deadline {
            procs::reap(recorder);
            return Err(String::from("screencapture -v did not finish"));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    drop(overlay);
    drop(stage);

    let log = std::fs::read_to_string(dir.join(format!("{mode}.overlay.log"))).unwrap_or_default();
    let (moved, reads, changed) = drag_rates(&log, drag_start, release);
    let row = (target.y + target.h / 2.0 - b.y) as usize;
    let rows = decode_rows(&movie, &b, row)?;
    let mut lags = moving_lags(&rows);
    lags.sort_by(f64::total_cmp);
    Ok(LagResult {
        mode: mode.to_string(),
        frames: lags.len(),
        median: percentile(&lags, 0.5),
        p95: percentile(&lags, 0.95),
        moved_per_sec: Some(moved),
        reads_per_sec: Some(reads),
        changed_per_sec: Some(changed),
        note: Some(format!("{} frames decoded", rows.len())),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(spans: &[(usize, usize, (u8, u8, u8))], width: usize) -> Vec<u8> {
        let mut out = vec![128u8; width * 3];
        for &(start, end, (r, g, b)) in spans {
            for x in start..end {
                out[x * 3..x * 3 + 3].copy_from_slice(&[r, g, b]);
            }
        }
        out
    }

    const MAGENTA: (u8, u8, u8) = (235, 51, 247);
    const BLUE: (u8, u8, u8) = (48, 80, 200);

    #[test]
    fn drag_rates_count_moves_and_reads_inside_the_drag() {
        let log = "ts_ms=900 event=trigger kind=moved pid=1\n\
                   ts_ms=1000 event=trigger kind=moved pid=1\n\
                   ts_ms=1010 event=axread window=5 ok=1 us=300 changed=1 source=notification\n\
                   ts_ms=1026 event=axread window=5 ok=1 us=300 changed=0 source=timer\n\
                   ts_ms=1500 event=trigger kind=focused_window pid=1\n\
                   ts_ms=2100 event=axread window=5 ok=1 us=300 changed=1 source=timer\n";
        assert_eq!(drag_rates(log, 1000, 2000), (1.0, 2.0, 1.0));
    }

    #[test]
    fn a_trailing_ring_gives_a_positive_lag() {
        let r = row(&[(20, 26, MAGENTA), (40, 80, BLUE)], 100);
        assert_eq!(row_lag(&r), Some((14.0, 40)));
    }

    #[test]
    fn a_ring_flush_with_the_window_gives_zero() {
        let r = row(&[(34, 40, MAGENTA), (40, 80, BLUE), (80, 86, MAGENTA)], 100);
        assert_eq!(row_lag(&r), Some((0.0, 40)));
    }

    #[test]
    fn a_trailing_ring_over_the_right_edge_still_reads_from_the_left() {
        let r = row(&[(20, 26, MAGENTA), (40, 80, BLUE), (66, 72, MAGENTA)], 100);
        assert_eq!(row_lag(&r), Some((14.0, 40)));
    }

    #[test]
    fn a_leading_ring_gives_a_negative_lag() {
        // The overlay paints over the window, so the left ring hides the
        // window's first 4 columns.
        let r = row(&[(36, 76, BLUE), (34, 40, MAGENTA), (80, 86, MAGENTA)], 100);
        assert_eq!(row_lag(&r), Some((-4.0, 40)));
    }

    #[test]
    fn only_frames_after_a_move_count() {
        let still = row(&[(34, 40, MAGENTA), (40, 80, BLUE)], 100);
        let moved = row(&[(34, 40, MAGENTA), (50, 90, BLUE)], 100);
        let lags = moving_lags(&[still.clone(), still, moved]);
        assert_eq!(lags, vec![10.0]);
    }
}
