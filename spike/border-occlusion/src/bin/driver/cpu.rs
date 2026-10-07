//! CPU and wakeup sampling around one scenario, for the overlay and for
//! WindowServer.

use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use spike::now_ms;

use crate::procs::{self, Overlay};

const TOP_INTERVAL_SECS: u64 = 5;

#[derive(Clone, Debug, Default)]
pub struct ProcStats {
    pub cpu_percent: Option<f64>,
    pub idle_wakeups_per_sec: Option<f64>,
    /// From `proc_pid_rusage`, which only works on the driver's own children.
    pub interrupt_wakeups_per_sec: Option<f64>,
    pub power: Option<f64>,
}

#[derive(Clone, Debug, Default)]
pub struct OverlayStats {
    /// Every CGWindowList call. A full call and a call that reads only the
    /// moving windows each count as one.
    pub cglist_per_sec: f64,
    pub ax_reads_per_sec: f64,
    /// AX reads whose rect differed from the previous read of that window.
    pub ax_changed_per_sec: f64,
    pub ax_read_failures: f64,
    pub second_calls: f64,
    /// Second z-order calls that found a change the first call missed.
    pub second_found: f64,
    pub redraws_per_sec: f64,
    pub cglist_mean_us: f64,
    pub triggers_per_sec: f64,
}

#[derive(Clone, Debug, Default)]
pub struct Measurement {
    pub overlay: Option<ProcStats>,
    pub window_server: ProcStats,
    pub overlay_log: Option<OverlayStats>,
    /// The fixture processes summed, from the `ps` CPU time delta.
    pub fixture_cpu_percent: Option<f64>,
    pub start_ms: u64,
    pub end_ms: u64,
}

/// Samples while `body` runs. When `body` returns early, the sampling still
/// lasts `duration`.
pub fn measure(
    prefix: &Path,
    overlay: Option<&Overlay>,
    fixture_pids: &[i32],
    ws_pid: i32,
    duration: Duration,
    body: impl FnOnce(),
) -> Measurement {
    let overlay_pid = overlay.map(|o| o.pid);
    let samples = duration.as_secs() / TOP_INTERVAL_SECS + 1;
    let top_path = prefix.with_extension("top.txt");
    let top_file = std::fs::File::create(&top_path).expect("top output file");
    let mut top_args = vec![
        "-l".to_string(),
        samples.to_string(),
        "-s".to_string(),
        TOP_INTERVAL_SECS.to_string(),
        "-stats".to_string(),
        "pid,command,cpu,time,idlew,power".to_string(),
    ];
    for pid in overlay_pid.iter().chain(std::iter::once(&ws_pid)) {
        top_args.push("-pid".to_string());
        top_args.push(pid.to_string());
    }
    let top = procs::spawn(
        Command::new("top")
            .args(&top_args)
            .stdin(Stdio::null())
            .stdout(top_file)
            .stderr(Stdio::null()),
    )
    .expect("spawn top");

    if let Some(o) = overlay {
        o.control.send("stats");
        std::thread::sleep(Duration::from_millis(50));
    }
    let start = Instant::now();
    let start_ms = now_ms();
    let ws_cpu0 = ps_cpu_seconds(ws_pid);
    let overlay_cpu0 = overlay_pid.and_then(ps_cpu_seconds);
    let overlay_ru0 = overlay_pid.and_then(rusage);
    let fixture_cpu0 = sum_cpu_seconds(fixture_pids);

    body();
    if let Some(rest) = duration.checked_sub(start.elapsed()) {
        std::thread::sleep(rest);
    }

    let wall = start.elapsed().as_secs_f64();
    let end_ms = now_ms();
    let ws_cpu1 = ps_cpu_seconds(ws_pid);
    let overlay_cpu1 = overlay_pid.and_then(ps_cpu_seconds);
    let overlay_ru1 = overlay_pid.and_then(rusage);
    let fixture_cpu1 = sum_cpu_seconds(fixture_pids);
    if let Some(o) = overlay {
        o.control.send("stats");
    }

    wait_for(top, Duration::from_secs(TOP_INTERVAL_SECS + 5));
    let top_text = std::fs::read_to_string(&top_path).unwrap_or_default();
    let top_rows = parse_top(&top_text);

    let cpu = |a: Option<f64>, b: Option<f64>| Some((b? - a?) / wall * 100.0);
    let mut window_server = top_stats(top_rows.get(&ws_pid));
    window_server.cpu_percent = cpu(ws_cpu0, ws_cpu1);

    let overlay_stats = overlay_pid.map(|pid| {
        let mut stats = top_stats(top_rows.get(&pid));
        stats.cpu_percent = cpu(overlay_cpu0, overlay_cpu1);
        stats.interrupt_wakeups_per_sec = match (overlay_ru0, overlay_ru1) {
            (Some(a), Some(b)) => Some((b.1 - a.1) as f64 / wall),
            _ => None,
        };
        stats
    });

    Measurement {
        overlay: overlay_stats,
        window_server,
        overlay_log: overlay.map(|o| overlay_log_stats(&o.log, start_ms, end_ms)),
        fixture_cpu_percent: cpu(fixture_cpu0, fixture_cpu1),
        start_ms,
        end_ms,
    }
}

fn sum_cpu_seconds(pids: &[i32]) -> Option<f64> {
    pids.iter().map(|pid| ps_cpu_seconds(*pid)).sum()
}

fn wait_for(mut child: std::process::Child, limit: Duration) {
    let deadline = Instant::now() + limit;
    while Instant::now() < deadline {
        if let Ok(Some(_)) = child.try_wait() {
            procs::unregister(child.id());
            return;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    procs::reap(child);
}

/// Accumulated CPU time from `ps`, which prints hundredths of a second. `top`
/// drops the hundredths once a process passes one hour of CPU time.
pub fn ps_cpu_seconds(pid: i32) -> Option<f64> {
    let out = Command::new("ps")
        .args(["-o", "time=", "-p", &pid.to_string()])
        .output()
        .ok()?;
    parse_cpu_time(String::from_utf8_lossy(&out.stdout).trim())
}

/// Parses `[[h:]m:]s[.cc]`, the `TIME` format of `ps` and `top`.
pub fn parse_cpu_time(s: &str) -> Option<f64> {
    if s.is_empty() {
        return None;
    }
    let mut total = 0.0;
    for part in s.split(':') {
        total = total * 60.0 + part.parse::<f64>().ok()?;
    }
    Some(total)
}

/// Returns (package idle wakeups, interrupt wakeups).
fn rusage(pid: i32) -> Option<(u64, u64)> {
    let mut info: libc::rusage_info_v2 = unsafe { std::mem::zeroed() };
    let ret = unsafe {
        libc::proc_pid_rusage(
            pid,
            libc::RUSAGE_INFO_V2,
            (&mut info as *mut libc::rusage_info_v2).cast::<libc::rusage_info_t>(),
        )
    };
    (ret == 0).then_some((info.ri_pkg_idle_wkups, info.ri_interrupt_wkups))
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TopRow {
    idle_wakeups: f64,
    power: f64,
}

/// Every sample row per pid, in order. Each row reads
/// `PID COMMAND %CPU TIME IDLEW POWER`, and the command may hold spaces, so
/// the numeric columns are taken from the right.
fn parse_top(text: &str) -> HashMap<i32, Vec<TopRow>> {
    let mut rows: HashMap<i32, Vec<TopRow>> = HashMap::new();
    for line in text.lines() {
        let words: Vec<&str> = line.split_whitespace().collect();
        if words.len() < 6 {
            continue;
        }
        let Ok(pid) = words[0].parse::<i32>() else {
            continue;
        };
        let n = words.len();
        let clean = |w: &str| w.trim_end_matches(['+', '-']).parse::<f64>().ok();
        let (Some(idle_wakeups), Some(power)) = (clean(words[n - 2]), clean(words[n - 1])) else {
            continue;
        };
        rows.entry(pid).or_default().push(TopRow {
            idle_wakeups,
            power,
        });
    }
    rows
}

fn top_stats(rows: Option<&Vec<TopRow>>) -> ProcStats {
    let mut stats = ProcStats::default();
    let Some(rows) = rows else { return stats };
    if rows.len() < 2 {
        return stats;
    }
    let span = ((rows.len() - 1) as u64 * TOP_INTERVAL_SECS) as f64;
    stats.idle_wakeups_per_sec =
        Some((rows[rows.len() - 1].idle_wakeups - rows[0].idle_wakeups) / span);
    // The first sample has no interval behind it, so its power is not a rate.
    let later = &rows[1..];
    stats.power = Some(later.iter().map(|r| r.power).sum::<f64>() / later.len() as f64);
    stats
}

pub fn field<'a>(line: &'a str, key: &str) -> Option<&'a str> {
    line.split_whitespace()
        .find_map(|kv| kv.strip_prefix(key)?.strip_prefix('='))
}

/// Sums the stats lines after `start_ms`, up to and including the first one
/// at or after `end_ms`. Each line carries the length of the period it covers.
fn overlay_log_stats(path: &Path, start_ms: u64, end_ms: u64) -> OverlayStats {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    parse_overlay_log(&text, start_ms, end_ms)
}

fn parse_overlay_log(text: &str, start_ms: u64, end_ms: u64) -> OverlayStats {
    let mut cglist = 0.0;
    let mut redraws = 0.0;
    let mut cglist_us_weighted = 0.0;
    let mut ax_reads = 0.0;
    let mut ax_changed = 0.0;
    let mut ax_failures = 0.0;
    let mut second_calls = 0.0;
    let mut second_found = 0.0;
    let mut covered_ms = 0.0;
    let mut triggers = 0.0;
    for line in text.lines() {
        let Some(ts) = field(line, "ts_ms").and_then(|v| v.parse::<u64>().ok()) else {
            continue;
        };
        if ts <= start_ms {
            continue;
        }
        let num = |k: &str| field(line, k).and_then(|v| v.parse::<f64>().ok()).unwrap_or(0.0);
        match field(line, "event") {
            Some("stats") => {
                let line_calls = [
                    "polls_trigger",
                    "polls_frame",
                    "polls_safety",
                    "polls_tick",
                    "polls_focused",
                    "cglist_zorder",
                    "cglist_zorder_second",
                    "cglist_safety",
                    "cglist_moving",
                ]
                .iter()
                .map(|k| num(k))
                .sum::<f64>();
                cglist += line_calls;
                redraws += num("redraws");
                cglist_us_weighted += num("poll_mean_us") * line_calls;
                ax_reads += num("axreads");
                ax_changed += num("axreads_changed");
                ax_failures += num("axread_failures");
                second_calls += num("cglist_zorder_second");
                second_found += num("second_found");
                covered_ms += num("period_ms");
                if ts >= end_ms {
                    break;
                }
            }
            Some("trigger") if ts <= end_ms => triggers += 1.0,
            _ => {}
        }
    }
    let covered = covered_ms / 1000.0;
    let wall = (end_ms - start_ms) as f64 / 1000.0;
    let rate = |n: f64| if covered > 0.0 { n / covered } else { 0.0 };
    OverlayStats {
        cglist_per_sec: rate(cglist),
        ax_reads_per_sec: rate(ax_reads),
        ax_changed_per_sec: rate(ax_changed),
        ax_read_failures: ax_failures,
        second_calls,
        second_found,
        redraws_per_sec: rate(redraws),
        cglist_mean_us: if cglist > 0.0 {
            cglist_us_weighted / cglist
        } else {
            0.0
        },
        triggers_per_sec: if wall > 0.0 { triggers / wall } else { 0.0 },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_stats_cover_the_lines_after_start_through_the_first_at_end() {
        let text = "ts_ms=100 event=stats cglist_zorder=50 period_ms=5000\n\
                    ts_ms=1200 event=trigger kind=moved\n\
                    ts_ms=5100 event=stats polls_frame=10 cglist_zorder=2 axreads=8 axread_failures=1 poll_mean_us=100 redraws=4 period_ms=4000\n\
                    ts_ms=7110 event=stats cglist_safety=1 cglist_zorder_second=2 cglist_moving=3 second_found=1 axreads=2 axreads_changed=1 poll_mean_us=400 redraws=1 period_ms=2000\n\
                    ts_ms=9000 event=stats cglist_safety=9 period_ms=1900\n";
        let s = parse_overlay_log(text, 1100, 7100);
        assert_eq!(s.cglist_per_sec, 18.0 / 6.0);
        assert_eq!(s.ax_reads_per_sec, 10.0 / 6.0);
        assert_eq!(s.ax_changed_per_sec, 1.0 / 6.0);
        assert_eq!(s.ax_read_failures, 1.0);
        assert_eq!((s.second_calls, s.second_found), (2.0, 1.0));
        assert_eq!(s.redraws_per_sec, 5.0 / 6.0);
        assert_eq!(s.cglist_mean_us, (12.0 * 100.0 + 6.0 * 400.0) / 18.0);
        assert_eq!(s.triggers_per_sec, 1.0 / 6.0);
    }

    #[test]
    fn cpu_time_formats() {
        assert_eq!(parse_cpu_time("0:01.50"), Some(1.5));
        assert_eq!(parse_cpu_time("376:29.00"), Some(376.0 * 60.0 + 29.0));
        assert_eq!(parse_cpu_time("06:16:28"), Some(6.0 * 3600.0 + 16.0 * 60.0 + 28.0));
        assert_eq!(parse_cpu_time(""), None);
    }

    #[test]
    fn top_rows_parse_from_the_right() {
        let text = "PID    COMMAND      %CPU TIME     IDLEW  POWER\n\
                    805    WindowServer 12.0 06:16:24 483692 12.0\n\
                    9      border-occlusi 1.0 00:00.10 10+ 0.5\n\
                    805    WindowServer 9.0 06:16:29 483702+ 9.5\n";
        let rows = parse_top(text);
        assert_eq!(rows[&805].len(), 2);
        assert_eq!(rows[&805][1].idle_wakeups, 483702.0);
        assert_eq!(rows[&9][0].power, 0.5);
        let stats = top_stats(rows.get(&805));
        assert_eq!(stats.idle_wakeups_per_sec, Some(2.0));
        assert_eq!(stats.power, Some(9.5));
    }

    #[test]
    fn field_reads_key_value() {
        let line = "ts_ms=12 event=redraw cause=frame changed=1";
        assert_eq!(field(line, "cause"), Some("frame"));
        assert_eq!(field(line, "ts_ms"), Some("12"));
        assert_eq!(field(line, "nothing"), None);
    }
}
