//! The fidelity cases. Each case resets the stage, runs one or more steps, and
//! checks two stills per step.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use spike::geometry::Rect;
use spike::now_ms;
use spike::windows::{WinInfo, query_windows, query_windows_named};

use crate::check::{self, Counts, Image};
use crate::cpu::field;
use crate::dome::{self, Dome};
use crate::frames;
use crate::motion::{self, Motion};
use crate::procs::{self, Overlay};
use crate::stage::{self, CPU_LABELS, Owner, Stage};
use crate::{Ctx, FrameRow, drag_loop, input, overlay_args};

const EARLY_MS: u64 = 100;
const OVERLAY_SAFETY_MS: u64 = 1000;
const OVERLAY_TAIL_MS: u64 = 300;
const LATE_MS: u64 = OVERLAY_SAFETY_MS + OVERLAY_TAIL_MS + 200;
const SETTLE: Duration = Duration::from_millis(1500);
const RECORD_SECS: u64 = 4;
/// The longest a moving-window case moves its windows while it waits for
/// `screencapture -v` to exit.
const RECORD_LIMIT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug, Default)]
pub struct StepResult {
    pub case: String,
    pub mode: String,
    pub early: Option<Counts>,
    pub late: Option<Counts>,
    /// Magenta pixel counts of the two stills, for Mission Control.
    pub magenta: Option<(usize, usize)>,
    pub staleness_ms: Option<u64>,
    pub unreported: Vec<String>,
    pub failing_stills: Vec<String>,
    pub note: Option<String>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    Borders,
    MagentaCount,
}

struct Runner<'a> {
    stage: &'a mut Stage,
    overlay: &'a Overlay,
    mode: String,
    dir: PathBuf,
    results: Vec<StepResult>,
}

fn base_layout() -> Vec<(Owner, &'static str, Rect, &'static str)> {
    vec![
        (Owner::A, "lo", Rect::new(150.0, 150.0, 500.0, 380.0), stage::BLUE),
        (Owner::A, "hi", Rect::new(400.0, 280.0, 500.0, 380.0), stage::GREEN),
        (Owner::B, "b1", Rect::new(950.0, 150.0, 420.0, 300.0), stage::ORANGE),
    ]
}

fn sleep_until(ts_ms: u64) {
    let now = now_ms();
    if ts_ms > now {
        std::thread::sleep(Duration::from_millis(ts_ms - now));
    }
}

fn slug(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

impl Runner<'_> {
    fn overlay_pid(&self) -> Option<i32> {
        Some(self.overlay.pid)
    }

    fn reset(&mut self, layout: &[(Owner, &str, Rect, &str)]) -> Result<(), String> {
        self.stage.reset(layout)?;
        std::thread::sleep(SETTLE);
        Ok(())
    }

    fn targets(&self) -> Vec<u32> {
        self.stage
            .windows
            .values()
            .filter(|w| w.managed)
            .map(|w| w.id)
            .collect()
    }

    fn still(&self, name: &str, check: Check) -> Result<(Counts, usize, PathBuf), String> {
        let path = self.dir.join(format!("{name}.png"));
        let windows = query_windows(self.overlay_pid().map(i64::from));
        check::capture(&path, self.stage.backdrop)?;
        let image = Image::load(&path)?;
        let (counts, failures) =
            check::check(&image, &self.stage.backdrop, &windows, &self.targets());
        let magenta = check::count_magenta(&image);
        if check == Check::MagentaCount {
            // Mission Control shows every window on the screen, including
            // windows outside the fixtures, so its still is not kept.
            let _ = std::fs::remove_file(&path);
        } else if counts.failed() {
            let mut text = String::from("windows front to back:\n");
            for w in &windows {
                text.push_str(&format!("{w:?}\n"));
            }
            text.push_str("failed points:\n");
            text.push_str(&failures.join("\n"));
            let _ = std::fs::write(path.with_extension("txt"), text);
        }
        Ok((counts, magenta, path))
    }

    fn step(&mut self, case: &str, check: Check, action_ms: u64) -> Result<(), String> {
        self.step_after(case, check, action_ms, EARLY_MS)
    }

    fn step_after(
        &mut self,
        case: &str,
        check: Check,
        action_ms: u64,
        early_ms: u64,
    ) -> Result<(), String> {
        let name = slug(case);
        sleep_until(action_ms + early_ms);
        let (early, early_magenta, early_path) = self.still(&format!("{name}-1"), check)?;
        sleep_until(action_ms + LATE_MS);
        let (late, late_magenta, late_path) = self.still(&format!("{name}-2"), check)?;
        let end_ms = now_ms();

        let (staleness_ms, unreported) = read_log(&self.overlay.log, action_ms, end_ms);
        let mut result = StepResult {
            case: case.to_string(),
            mode: self.mode.clone(),
            staleness_ms,
            unreported,
            ..StepResult::default()
        };
        match check {
            Check::Borders => {
                for (counts, path) in [(early, &early_path), (late, &late_path)] {
                    if counts.failed() {
                        result.failing_stills.push(path.display().to_string());
                    }
                }
                result.early = Some(early);
                result.late = Some(late);
            }
            Check::MagentaCount => result.magenta = Some((early_magenta, late_magenta)),
        }
        self.results.push(result);
        Ok(())
    }

    fn not_exercised(&mut self, case: &str, why: &str) {
        self.results.push(StepResult {
            case: case.to_string(),
            mode: self.mode.clone(),
            note: Some(format!("not exercised: {why}")),
            ..StepResult::default()
        });
    }

    fn note_last(&mut self, note: String) {
        if let Some(last) = self.results.last_mut() {
            last.note = Some(note);
        }
    }

    /// Posts Escape only when a fixture or an app named in `also` is active,
    /// so the keystroke never lands in another app.
    fn escape(&self, also: &[&str]) -> Option<u64> {
        let pid = input::focused_app_pid()?;
        let name = input::app_name(pid);
        if !self.stage.pids().contains(&pid) && !also.contains(&name.as_str()) {
            eprintln!("escape skipped: {name} ({pid}) is active");
            return None;
        }
        let at = now_ms();
        input::escape();
        Some(at)
    }

    fn cmd(&mut self, label: &str, verb: &str, rest: &str) -> Result<u64, String> {
        let at = now_ms();
        self.stage.cmd(label, verb, rest)?;
        Ok(at)
    }

    fn visible_point(&self, label: &str) -> (f64, f64) {
        let r = self.stage.window(label).rect;
        (r.x + 100.0, r.y + 200.0)
    }
}

/// Returns the delay from `action_ms` to the first overlay log line with
/// `event=redraw changed=1`, and every unreported change logged up to
/// `end_ms`.
fn read_log(path: &Path, action_ms: u64, end_ms: u64) -> (Option<u64>, Vec<String>) {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let mut staleness = None;
    let mut unreported = Vec::new();
    for line in text.lines() {
        let Some(ts) = field(line, "ts_ms").and_then(|v| v.parse::<u64>().ok()) else {
            continue;
        };
        if ts < action_ms || ts > end_ms {
            continue;
        }
        match field(line, "event") {
            Some("redraw") if staleness.is_none() && field(line, "changed") == Some("1") => {
                staleness = Some(ts - action_ms);
            }
            Some("unreported") => unreported.push(format!(
                "{} {}",
                field(line, "app").unwrap_or("?"),
                field(line, "change").unwrap_or("?")
            )),
            _ => {}
        }
    }
    (staleness, unreported)
}

fn new_window_of(before: &[WinInfo], owners: &[&str], limit: Duration) -> Option<(WinInfo, u64)> {
    let start = Instant::now();
    while start.elapsed() < limit {
        for (w, owner) in query_windows_named(None) {
            if owners.iter().any(|o| owner.contains(o))
                && !before.iter().any(|b| b.id == w.id)
                && w.rect.w > 100.0
                && w.rect.h > 40.0
            {
                return Some((w, now_ms()));
            }
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    None
}

fn wait_gone(owners: &[&str], limit: Duration) {
    let start = Instant::now();
    while start.elapsed() < limit {
        let any = query_windows_named(None)
            .iter()
            .any(|(w, owner)| owners.iter().any(|o| owner.contains(o)) && w.layer > 0);
        if !any {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
}

fn kill_child(child: Child) {
    procs::reap(child);
}

fn run_cases(r: &mut Runner, ctx: &mut Ctx) -> Result<(), String> {
    let fixtures = r.stage.pids();
    let overlay = r.overlay_pid();

    ctx.gate(&format!("fidelity {} raise", r.mode))?;
    r.reset(&base_layout())?;
    let at = input::click(r.visible_point("lo"), &fixtures, overlay);
    r.step("raise", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} drag", r.mode))?;
    r.reset(&base_layout())?;
    let hi = r.stage.window("hi").rect;
    let grab = (hi.x + 200.0, hi.y + 12.0);
    let at = input::drag(grab, Duration::from_secs(3), &fixtures, overlay, |t| {
        drag_loop(grab, t)
    });
    r.step("drag", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} resize", r.mode))?;
    r.reset(&base_layout())?;
    let hi = r.stage.window("hi").rect;
    let corner = (hi.right() - 3.0, hi.bottom() - 3.0);
    let at = input::drag(corner, Duration::from_secs(3), &fixtures, overlay, |t| {
        drag_loop(corner, t)
    });
    r.step("resize", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} overlap", r.mode))?;
    r.reset(&[
        (Owner::A, "left", Rect::new(150.0, 150.0, 500.0, 400.0), stage::BLUE),
        (Owner::B, "b1", Rect::new(950.0, 150.0, 420.0, 300.0), stage::ORANGE),
    ])?;
    let at = now_ms();
    r.stage.open(
        Owner::A,
        "right",
        Rect::new(600.0, 200.0, 500.0, 400.0),
        stage::GREEN,
    )?;
    r.step("overlap", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} context menu", r.mode))?;
    r.reset(&base_layout())?;
    let hi = r.stage.window("hi").rect;
    let at = input::right_click((hi.x + 250.0, hi.y + 190.0), &fixtures, overlay);
    r.step("context menu open", Check::Borders, at)?;
    match r.escape(&[]) {
        Some(at) => r.step("context menu Escape", Check::Borders, at)?,
        None => r.not_exercised("context menu Escape", "a fixture was not active"),
    }

    ctx.gate(&format!("fidelity {} pop-up menu", r.mode))?;
    r.reset(&base_layout())?;
    let at = r.cmd("hi", "popup", "")?;
    r.step("pop-up menu open", Check::Borders, at)?;
    match r.escape(&[]) {
        Some(at) => r.step("pop-up menu Escape", Check::Borders, at)?,
        None => r.not_exercised("pop-up menu Escape", "a fixture was not active"),
    }

    ctx.gate(&format!("fidelity {} launcher", r.mode))?;
    r.reset(&base_layout())?;
    if input::focused_app_pid().is_some_and(|p| fixtures.contains(&p)) {
        let before = query_windows(None);
        input::cmd_space();
        match new_window_of(&before, &["Raycast"], Duration::from_millis(1500)) {
            Some((_, seen)) => {
                r.step("launcher open", Check::Borders, seen)?;
                match r.escape(&["Raycast"]) {
                    Some(at) => r.step("launcher Escape", Check::Borders, at)?,
                    None => r.not_exercised("launcher Escape", "Raycast was not active"),
                }
            }
            None => {
                r.not_exercised("launcher open", "no Raycast window appeared");
                r.escape(&["Raycast"]);
            }
        }
    } else {
        r.not_exercised("launcher open", "a fixture was not active");
    }

    ctx.gate(&format!("fidelity {} QuickLook", r.mode))?;
    r.reset(&base_layout())?;
    let before = query_windows(None);
    let readme = Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md");
    let ql = procs::spawn(
        Command::new("qlmanage")
            .arg("-p")
            .arg(&readme)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    )?;
    match new_window_of(&before, &["qlmanage", "Quick Look"], Duration::from_secs(4)) {
        Some((_, seen)) => {
            r.step("QuickLook open", Check::Borders, seen)?;
            let at = now_ms();
            kill_child(ql);
            r.step("QuickLook killed", Check::Borders, at)?;
        }
        None => {
            kill_child(ql);
            r.not_exercised("QuickLook open", "no QuickLook window appeared");
        }
    }

    ctx.gate(&format!("fidelity {} floating window", r.mode))?;
    r.reset(&base_layout())?;
    r.cmd("hi", "float", "on")?;
    input::activate(r.stage.pid(Owner::B));
    std::thread::sleep(SETTLE);
    let at = input::click(r.visible_point("lo"), &fixtures, overlay);
    r.step("floating window, raise below it", Check::Borders, at)?;
    r.cmd("hi", "float", "off")?;

    ctx.gate(&format!("fidelity {} order front", r.mode))?;
    r.reset(&base_layout())?;
    let at = r.cmd("lo", "front", "")?;
    r.step("order front", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} minimize", r.mode))?;
    r.reset(&base_layout())?;
    let at = r.cmd("hi", "minimize", "")?;
    r.step("minimize", Check::Borders, at)?;
    let at = r.cmd("hi", "restore", "")?;
    r.step("restore", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} open and close", r.mode))?;
    r.reset(&base_layout())?;
    let at = now_ms();
    r.stage.open(
        Owner::A,
        "n1",
        Rect::new(500.0, 420.0, 400.0, 260.0),
        stage::YELLOW,
    )?;
    r.step("open", Check::Borders, at)?;
    let at = now_ms();
    r.stage.close("n1")?;
    r.step("close", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} app switch", r.mode))?;
    r.reset(&[
        (Owner::A, "lo", Rect::new(150.0, 150.0, 500.0, 380.0), stage::BLUE),
        (Owner::A, "hi", Rect::new(400.0, 280.0, 500.0, 380.0), stage::GREEN),
        (Owner::B, "b1", Rect::new(820.0, 360.0, 420.0, 360.0), stage::ORANGE),
    ])?;
    let b_pid = r.stage.pid(Owner::B);
    let at = now_ms();
    input::activate(b_pid);
    r.step("app switch", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} Dome layout", r.mode))?;
    r.reset(&stage::cpu_layout())?;
    let dome = Dome::new(r.stage, &CPU_LABELS)?;
    let at = now_ms();
    let failed = dome.apply(&dome::tilings()[0]);
    r.step("Dome layout", Check::Borders, at)?;
    if !failed.is_empty() {
        r.note_last(format!("AX write failed for {}", failed.join(", ")));
    }

    ctx.gate(&format!("fidelity {} minimum size", r.mode))?;
    r.reset(&stage::cpu_layout())?;
    r.cmd("a1", "minsize", "800 500")?;
    let dome = Dome::new(r.stage, &CPU_LABELS)?;
    let layout = dome::tilings()[0].clone();
    let requested = layout[0].1;
    let at = now_ms();
    dome.apply(&layout);
    r.step("minimum size", Check::Borders, at)?;
    let a1 = r.stage.window("a1").id;
    let real = query_windows(None).into_iter().find(|w| w.id == a1);
    r.note_last(format!(
        "a1 requested {}x{}, real {}",
        requested.w,
        requested.h,
        real.map_or_else(|| String::from("not listed"), |w| format!("{}x{}", w.rect.w, w.rect.h))
    ));

    ctx.gate(&format!("fidelity {} focus spam end", r.mode))?;
    r.reset(&base_layout())?;
    // An odd count ends with `lo` in front, where `hi` was before the spam.
    let end = r.stage.spam("lo", "hi", 10, 101)?;
    r.step_after("focus spam end", Check::Borders, end, 150)?;
    r.note_last(String::from("early still 150 ms after the last key change"));

    ctx.gate(&format!("fidelity {} unmanaged window moves", r.mode))?;
    r.reset(&base_layout())?;
    r.stage.open_unmanaged(
        Owner::B,
        "u",
        Rect::new(1100.0, 400.0, 300.0, 200.0),
        stage::YELLOW,
    )?;
    std::thread::sleep(SETTLE);
    let at = r.cmd("u", "move", "560 500")?;
    r.step("unmanaged window moves", Check::Borders, at)?;

    ctx.gate(&format!("fidelity {} notification banner", r.mode))?;
    let mut layout = base_layout();
    layout.push((
        Owner::B,
        "nb",
        Rect::new(1000.0, 60.0, 480.0, 300.0),
        stage::CYAN,
    ));
    r.reset(&layout)?;
    let before = query_windows(None);
    let _ = Command::new("osascript")
        .args([
            "-e",
            "display notification \"border occlusion spike\" with title \"spike\"",
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let owners = ["Notification Center", "NotificationCenter"];
    match new_window_of(&before, &owners, Duration::from_secs(2)) {
        Some((w, seen)) => {
            r.step("notification banner", Check::Borders, seen)?;
            r.note_last(format!(
                "banner window layer {} at {},{} {}x{}",
                w.layer, w.rect.x, w.rect.y, w.rect.w, w.rect.h
            ));
        }
        None => r.not_exercised("notification banner", "no banner window appeared within 2 s"),
    }
    wait_gone(&owners, Duration::from_secs(10));

    ctx.gate(&format!("fidelity {} Mission Control", r.mode))?;
    r.reset(&base_layout())?;
    let at = now_ms();
    let _ = Command::new("open")
        .args(["-a", "Mission Control"])
        .status();
    r.step("Mission Control", Check::MagentaCount, at)?;
    match r.escape(&["Dock"]) {
        Some(at) => r.step("Mission Control Escape", Check::Borders, at)?,
        None => {
            let at = now_ms();
            let _ = Command::new("open")
                .args(["-a", "Mission Control"])
                .status();
            r.step("Mission Control Escape", Check::Borders, at)?;
            r.note_last(String::from("closed by a second open -a, not Escape"));
        }
    }
    std::thread::sleep(SETTLE);
    motion_cases(r, ctx)
}

/// Records each moving-window variant for the per-frame check, then checks
/// two stills after the windows stop.
fn motion_cases(r: &mut Runner, ctx: &mut Ctx) -> Result<(), String> {
    for motion in Motion::ALL {
        ctx.gate(&format!("fidelity {} {}", r.mode, motion.name()))?;
        r.reset(&motion::stage_layout())?;
        let dome = Dome::new(r.stage, &motion::LABELS)?;
        let backdrop = r.stage.backdrop;
        let movie = r.dir.join(format!("{}.mov", motion.slug()));
        let mut recorder = frames::start_recording(&movie, &backdrop, RECORD_SECS)?;
        let mut recorded = false;
        // The windows move from before the recording starts until after it
        // ends, so every recorded frame shows them moving.
        let outcome = motion::run(motion, &dome, |t| {
            recorded = recorded || matches!(recorder.try_wait(), Ok(Some(_)));
            recorded || t > RECORD_LIMIT
        });
        if recorded {
            procs::unregister(recorder.id());
        } else {
            procs::reap(recorder);
        }
        let case = format!("{} stop", motion.name());
        r.step(&case, Check::Borders, outcome.end_ms)?;
        r.note_last(format!(
            "early still 100 ms after the last AX write, {} writes ({} failed), {} focus changes ({} failed)",
            outcome.writes, outcome.failed_writes, outcome.focuses, outcome.failed_focuses
        ));
        let image = r.dir.join(format!("{}-frame.png", motion.slug()));
        let check = if recorded {
            frames::check_movie(&movie, &backdrop, &image)?
        } else {
            frames::MovieCheck {
                note: Some(format!("screencapture -v did not finish within {RECORD_LIMIT:?}")),
                ..frames::MovieCheck::default()
            }
        };
        ctx.report.frames.push(FrameRow {
            case: motion.name().to_string(),
            mode: r.mode.clone(),
            check,
            outcome,
        });
        ctx.report.write(&ctx.out);
    }
    Ok(())
}

pub fn run_mode(ctx: &mut Ctx, mode: &str, motion_only: bool) -> Result<Vec<StepResult>, String> {
    let dir = ctx.out.join("fidelity").join(mode);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut stage = Stage::start(&ctx.bin_dir)?;
    let overlay = Overlay::start(
        &ctx.bin_dir.join("border-occlusion-spike"),
        &overlay_args(mode, None),
        &dir.join("overlay.log"),
    )?;
    stage.attach(overlay.control.clone());
    let mut runner = Runner {
        stage: &mut stage,
        overlay: &overlay,
        mode: mode.to_string(),
        dir,
        results: Vec::new(),
    };
    let outcome = if motion_only {
        motion_cases(&mut runner, ctx)
    } else {
        run_cases(&mut runner, ctx)
    };
    let results = std::mem::take(&mut runner.results);
    match outcome {
        Ok(()) => Ok(results),
        Err(e) => {
            ctx.report.fidelity.extend(results);
            Err(e)
        }
    }
}
