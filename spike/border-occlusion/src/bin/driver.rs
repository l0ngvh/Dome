//! Runs every overlay mode through every scenario and writes
//! `<out>/results.md`. The operator does nothing after starting it.

#[path = "driver/check.rs"]
mod check;
#[path = "driver/cpu.rs"]
mod cpu;
#[path = "driver/dome.rs"]
mod dome;
#[path = "driver/fidelity.rs"]
mod fidelity;
#[path = "driver/frames.rs"]
mod frames;
#[path = "driver/input.rs"]
mod input;
#[path = "driver/lag.rs"]
mod lag;
#[path = "driver/motion.rs"]
mod motion;
#[path = "driver/procs.rs"]
mod procs;
#[path = "driver/stage.rs"]
mod stage;

use std::fmt::Write as _;
use std::fs::File;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};
use std::time::{Duration, Instant};

use objc2_core_graphics::{CGDisplayBounds, CGEventTapLocation, CGMainDisplayID};
use spike::now_ms;
use spike::windows::query_windows_named;

use cpu::Measurement;
use dome::Dome;
use motion::Motion;
use procs::Overlay;
use stage::Stage;

const GATE_IDLE_SECS: f64 = 30.0;
const GATE_GIVE_UP: Duration = Duration::from_secs(30 * 60);
const SETTLE: Duration = Duration::from_secs(2);

struct Args {
    out: PathBuf,
    modes: Vec<String>,
    parts: Vec<String>,
    baseline: bool,
    no_idle_wait: bool,
}

fn parse_args() -> Args {
    let mut args = Args {
        out: PathBuf::new(),
        modes: ["events", "ax-position-reads", "moving-window-list"]
            .map(String::from)
            .to_vec(),
        parts: ["cpu", "fidelity", "lag"].map(String::from).to_vec(),
        baseline: true,
        no_idle_wait: false,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        match flag.as_str() {
            "dump" => {
                dump();
                exit(0);
            }
            "--no-baseline" => args.baseline = false,
            "--no-idle-wait" => args.no_idle_wait = true,
            "--out" | "--modes" | "--parts" => {
                let Some(value) = it.next() else {
                    usage(&format!("{flag} needs a value"));
                };
                let list = || value.split(',').map(String::from).collect();
                match flag.as_str() {
                    "--out" => args.out = PathBuf::from(&value),
                    "--modes" => args.modes = list(),
                    _ => args.parts = list(),
                }
            }
            _ => usage(&format!("unknown flag {flag}")),
        }
    }
    if args.out.as_os_str().is_empty() {
        usage("--out is required");
    }
    args
}

fn usage(message: &str) -> ! {
    eprintln!("{message}");
    eprintln!(
        "usage: driver --out <dir> [--modes poll,poll-diff,events,events-focused,ax-geometry,ax-position-reads,moving-window-list] [--parts cpu,fidelity,motion,lag] [--no-baseline] [--no-idle-wait]\n       driver dump"
    );
    exit(2);
}

fn dump() {
    for (w, owner) in query_windows_named(None) {
        println!(
            "id={} pid={} layer={} rect={},{},{},{} owner={owner}",
            w.id, w.pid, w.layer, w.rect.x, w.rect.y, w.rect.w, w.rect.h
        );
    }
}

pub struct Ctx {
    pub out: PathBuf,
    pub bin_dir: PathBuf,
    pub ws_pid: i32,
    log: File,
    gate_each: bool,
    gated_once: bool,
    /// When true, a gate waits only for a large system window to clear, not for
    /// idle input. User input during a scenario can then disturb its numbers.
    no_idle_wait: bool,
    pub report: Report,
}

impl Ctx {
    pub fn log(&mut self, message: &str) {
        let line = format!("ts_ms={} {message}", now_ms());
        println!("{line}");
        let _ = writeln!(self.log, "{line}");
    }

    /// Waits until the user has been idle for `GATE_IDLE_SECS`. Once posted
    /// input proves to reset the idle counter, a later call passes at once when
    /// the counter still matches the time since the driver's own last post.
    pub fn gate(&mut self, label: &str) -> Result<(), String> {
        let entry = input::idle_seconds();
        self.report.gate_entries.push((label.to_string(), entry));
        if self.no_idle_wait {
            return self.wait_for_no_blocker(label, entry);
        }
        if self.gated_once && !self.gate_each {
            let own = input::seconds_since_own_post();
            let blocker = stage_blocker();
            if blocker.is_none() && own.is_none_or(|own| entry + 1.0 >= own) {
                self.log(&format!(
                    "event=gate label=\"{label}\" entry_idle_s={entry:.1} since_own_post_s={own:?} passed=own_input_only"
                ));
                return Ok(());
            }
            self.log(&format!(
                "event=gate label=\"{label}\" entry_idle_s={entry:.1} since_own_post_s={own:?} foreign_input=1 blocker={blocker:?}"
            ));
        }
        let start = Instant::now();
        let mut logged_blocker = false;
        loop {
            let idle = input::idle_seconds();
            let blocker = stage_blocker();
            if idle >= GATE_IDLE_SECS && blocker.is_none() {
                self.log(&format!(
                    "event=gate label=\"{label}\" entry_idle_s={entry:.1} waited_s={:.1}",
                    start.elapsed().as_secs_f64()
                ));
                self.gated_once = true;
                return Ok(());
            }
            if let Some(b) = &blocker
                && !logged_blocker
            {
                self.log(&format!("event=gate label=\"{label}\" waiting_for=\"{b}\""));
                logged_blocker = true;
            }
            if start.elapsed() > GATE_GIVE_UP {
                self.log(&format!("event=gate label=\"{label}\" gave_up=1"));
                return Err(format!(
                    "idle gate gave up at \"{label}\" after 30 minutes of user input"
                ));
            }
            let wait = (GATE_IDLE_SECS - idle).clamp(0.5, 5.0);
            std::thread::sleep(Duration::from_secs_f64(wait));
        }
    }

    fn wait_for_no_blocker(&mut self, label: &str, entry: f64) -> Result<(), String> {
        let start = Instant::now();
        let mut logged_blocker = false;
        loop {
            match stage_blocker() {
                None => {
                    self.log(&format!(
                        "event=gate label=\"{label}\" entry_idle_s={entry:.1} waited_s={:.1} passed=no_idle_wait",
                        start.elapsed().as_secs_f64()
                    ));
                    self.gated_once = true;
                    return Ok(());
                }
                Some(b) => {
                    if !logged_blocker {
                        self.log(&format!("event=gate label=\"{label}\" waiting_for=\"{b}\""));
                        logged_blocker = true;
                    }
                }
            }
            if start.elapsed() > GATE_GIVE_UP {
                self.log(&format!("event=gate label=\"{label}\" gave_up=1"));
                return Err(format!("a system window blocked \"{label}\" for 30 minutes"));
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    /// Must run right after a gate passes, while the idle counter is high
    /// enough to show a reset.
    fn probe_posted_events(&mut self) {
        let mut resets = Vec::new();
        for tap in [
            CGEventTapLocation::SessionEventTap,
            CGEventTapLocation::HIDEventTap,
        ] {
            std::thread::sleep(Duration::from_secs(3));
            input::set_tap(tap);
            let before = input::idle_seconds();
            input::move_to(input::mouse_location());
            std::thread::sleep(Duration::from_millis(300));
            let after = input::idle_seconds();
            let reset = after < 1.0 && before > after + 1.0;
            self.log(&format!(
                "event=probe tap={} before_s={before:.2} after_s={after:.2} reset={}",
                input::tap_name(tap),
                u8::from(reset)
            ));
            self.report.probe.push((input::tap_name(tap), before, after, reset));
            resets.push((tap, reset));
        }
        let quiet = resets.iter().find(|(_, reset)| !reset).map(|(tap, _)| *tap);
        match quiet {
            Some(tap) => {
                input::set_tap(tap);
                self.gate_each = true;
                self.report.gate_policy = format!(
                    "Posted events go through the {} tap, which left the idle counter alone. The gate runs before every scenario.",
                    input::tap_name(tap)
                );
            }
            None => {
                input::set_tap(CGEventTapLocation::SessionEventTap);
                self.gate_each = false;
                self.report.gate_policy = String::from(
                    "Posted events reset the idle counter through both taps. The full 30 s wait runs at the start of the run. Before each later scenario, the gate passes at once when the counter still matches the time since the driver's own last posted event, and waits the full 30 s otherwise.",
                );
            }
        }
        let policy = self.report.gate_policy.clone();
        self.log(&format!("event=gate_policy text=\"{policy}\""));
    }
}

#[derive(Default)]
pub struct Report {
    pub started: String,
    pub modes: Vec<String>,
    pub probe: Vec<(&'static str, f64, f64, bool)>,
    pub gate_policy: String,
    pub gate_entries: Vec<(String, f64)>,
    pub cpu: Vec<CpuRow>,
    pub fidelity: Vec<fidelity::StepResult>,
    pub frames: Vec<FrameRow>,
    pub lag: Vec<lag::LagResult>,
    pub notes: Vec<String>,
}

pub struct FrameRow {
    pub case: String,
    pub mode: String,
    pub check: frames::MovieCheck,
    pub outcome: motion::Outcome,
}

pub struct CpuRow {
    pub mode: String,
    pub scenario: String,
    pub run: u32,
    /// The 1 minute load average at the start and at the end.
    pub load: (String, String),
    pub m: Measurement,
}

fn opt(v: Option<f64>, digits: usize) -> String {
    v.map_or_else(|| String::from("n/a"), |v| format!("{v:.digits$}"))
}

impl Report {
    fn render(&self) -> String {
        let mut s = String::new();
        let _ = writeln!(s, "# Border occlusion spike results\n");
        let _ = writeln!(s, "Started {}. Modes: {}.\n", self.started, self.modes.join(", "));

        let _ = writeln!(s, "## Run safety\n");
        for (tap, before, after, reset) in &self.probe {
            let _ = writeln!(
                s,
                "- Probe through the {tap} tap: idle counter {before:.2} s before a posted mouse move, {after:.2} s after. Reset: {}.",
                if *reset { "yes" } else { "no" }
            );
        }
        if !self.gate_policy.is_empty() {
            let _ = writeln!(s, "- {}", self.gate_policy);
        }
        let entries: Vec<String> = self
            .gate_entries
            .iter()
            .map(|(label, idle)| format!("{label} {idle:.1} s"))
            .collect();
        if !entries.is_empty() {
            let _ = writeln!(s, "- Idle counter at each gate entry: {}.", entries.join(", "));
        }
        for note in &self.notes {
            let _ = writeln!(s, "- {note}");
        }

        let _ = writeln!(s, "\n## CPU per mode and scenario\n");
        let _ = writeln!(
            s,
            "| Mode | Scenario | Run | Overlay CPU % | Overlay idle wakeups/s | Overlay interrupt wakeups/s | CGWindowList calls/s | AX reads/s | AX reads with a new frame/s | Second z-order calls (found a change) | Redraws/s | Triggers/s | Mean CGWindowList µs | Fixture CPU % | WindowServer CPU % | WindowServer idle wakeups/s | WindowServer power | Load start / end |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|");
        for row in &self.cpu {
            let o = row.m.overlay.clone().unwrap_or_default();
            let l = row.m.overlay_log.clone();
            let ws = &row.m.window_server;
            let ax_reads = match &l {
                Some(l) if l.ax_read_failures > 0.0 => {
                    format!("{:.1} ({} failed)", l.ax_reads_per_sec, l.ax_read_failures)
                }
                Some(l) => format!("{:.1}", l.ax_reads_per_sec),
                None => String::from("n/a"),
            };
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} | {} / {} |",
                row.mode,
                row.scenario,
                row.run,
                opt(o.cpu_percent, 2),
                opt(o.idle_wakeups_per_sec, 2),
                opt(o.interrupt_wakeups_per_sec, 1),
                opt(l.as_ref().map(|l| l.cglist_per_sec), 1),
                ax_reads,
                opt(l.as_ref().map(|l| l.ax_changed_per_sec), 1),
                l.as_ref().map_or_else(
                    || String::from("n/a"),
                    |l| format!("{} ({})", l.second_calls, l.second_found)
                ),
                opt(l.as_ref().map(|l| l.redraws_per_sec), 1),
                opt(l.as_ref().map(|l| l.triggers_per_sec), 1),
                opt(l.as_ref().map(|l| l.cglist_mean_us), 0),
                opt(row.m.fixture_cpu_percent, 2),
                opt(ws.cpu_percent, 1),
                opt(ws.idle_wakeups_per_sec, 2),
                opt(ws.power, 1),
                row.load.0,
                row.load.1,
            );
        }

        let _ = writeln!(s, "\n## Fidelity per case and mode\n");
        let _ = writeln!(
            s,
            "Counts are missing / over content, out of the midline points checked. The early still is 100 ms after the action, the late one 1500 ms after. Staleness is the time from the action to the first redraw that changed the path.\n"
        );
        let _ = writeln!(
            s,
            "| Case | Mode | Early | Late | Staleness ms | Unreported changes | Note |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|");
        let counts = |c: Option<check::Counts>| {
            c.map_or_else(
                || String::from("-"),
                |c| format!("{} / {} of {}", c.missing, c.over, c.checked),
            )
        };
        for f in &self.fidelity {
            let (early, late) = match f.magenta {
                Some((a, b)) => (format!("{a} magenta px"), format!("{b} magenta px")),
                None => (counts(f.early), counts(f.late)),
            };
            let unreported = if f.unreported.is_empty() {
                String::from("none")
            } else {
                f.unreported.join(", ")
            };
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} |",
                f.case,
                f.mode,
                early,
                late,
                f.staleness_ms
                    .map_or_else(|| String::from("-"), |v| v.to_string()),
                unreported,
                f.note.clone().unwrap_or_default(),
            );
        }
        let failing: Vec<&String> = self
            .fidelity
            .iter()
            .flat_map(|f| f.failing_stills.iter())
            .collect();
        if !failing.is_empty() {
            let _ = writeln!(s, "\nStills with a failure:\n");
            for path in failing {
                let _ = writeln!(s, "- `{path}`");
            }
        }

        let _ = writeln!(s, "\n## Per-frame check of moving windows\n");
        let _ = writeln!(
            s,
            "A frame fails when a magenta run across a ring has one window's content color on both sides for at least {} consecutive scan lines. The longest failing run lasts from its first frame to the next frame that passes.\n",
            frames::MIN_STREAK
        );
        let _ = writeln!(
            s,
            "| Variant | Mode | Frames | Failing frames | Share failing | Longest failing run ms | AX writes (failed) | Focus changes (failed) | Image | Note |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|---|---|");
        for f in &self.frames {
            let c = &f.check;
            let share = if c.frames > 0 {
                format!("{:.1}%", c.failing as f64 * 100.0 / c.frames as f64)
            } else {
                String::from("n/a")
            };
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {:.0} | {} ({}) | {} ({}) | {} | {} |",
                f.case,
                f.mode,
                c.frames,
                c.failing,
                share,
                c.longest_ms,
                f.outcome.writes,
                f.outcome.failed_writes,
                f.outcome.focuses,
                f.outcome.failed_focuses,
                c.image
                    .as_deref()
                    .map_or_else(|| String::from("-"), |p| format!("`{p}`")),
                c.note.clone().unwrap_or_default(),
            );
        }

        let _ = writeln!(s, "\n## Drag lag per mode\n");
        let _ = writeln!(
            s,
            "The rates cover the 2 s from the first dragged mouse event to the release.\n"
        );
        let _ = writeln!(
            s,
            "| Mode | Moving frames | Median pt | 95th percentile pt | Move notifications/s | AX reads/s | AX reads with a new position/s | Note |"
        );
        let _ = writeln!(s, "|---|---|---|---|---|---|---|---|");
        for l in &self.lag {
            let _ = writeln!(
                s,
                "| {} | {} | {} | {} | {} | {} | {} | {} |",
                l.mode,
                l.frames,
                opt(l.median, 1),
                opt(l.p95, 1),
                opt(l.moved_per_sec, 1),
                opt(l.reads_per_sec, 1),
                opt(l.changed_per_sec, 1),
                l.note.clone().unwrap_or_default()
            );
        }
        s
    }

    pub fn write(&self, out: &Path) {
        let _ = std::fs::write(out.join("results.md"), self.render());
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CpuScenario {
    Idle,
    Drag,
    Resize,
    Switch,
    DomeLayout,
    FocusSpam,
    LayoutJumps,
    SmoothMoves,
}

impl CpuScenario {
    const ALL: [CpuScenario; 8] = [
        CpuScenario::Idle,
        CpuScenario::Drag,
        CpuScenario::Resize,
        CpuScenario::Switch,
        CpuScenario::DomeLayout,
        CpuScenario::FocusSpam,
        CpuScenario::LayoutJumps,
        CpuScenario::SmoothMoves,
    ];

    fn name(self) -> &'static str {
        match self {
            CpuScenario::Idle => "idle",
            CpuScenario::Drag => "drag",
            CpuScenario::Resize => "resize",
            CpuScenario::Switch => "app switch",
            CpuScenario::DomeLayout => "Dome layout",
            CpuScenario::FocusSpam => "focus spam",
            CpuScenario::LayoutJumps => Motion::Jumps.name(),
            CpuScenario::SmoothMoves => Motion::Smooth.name(),
        }
    }

    fn slug(self) -> &'static str {
        match self {
            CpuScenario::Idle => "idle",
            CpuScenario::Drag => "drag",
            CpuScenario::Resize => "resize",
            CpuScenario::Switch => "switch",
            CpuScenario::DomeLayout => "dome-layout",
            CpuScenario::FocusSpam => "focus-spam",
            CpuScenario::LayoutJumps => Motion::Jumps.slug(),
            CpuScenario::SmoothMoves => Motion::Smooth.slug(),
        }
    }

    fn motion(self) -> Option<Motion> {
        match self {
            CpuScenario::LayoutJumps => Some(Motion::Jumps),
            CpuScenario::SmoothMoves => Some(Motion::Smooth),
            _ => None,
        }
    }

    fn duration(self) -> Duration {
        match self {
            CpuScenario::Idle => Duration::from_secs(60),
            CpuScenario::FocusSpam => SPAM_INTERVAL * SPAM_COUNT,
            _ => Duration::from_secs(20),
        }
    }
}

const LAYOUT_PERIOD: Duration = Duration::from_millis(500);
const SPAM_INTERVAL: Duration = Duration::from_millis(10);
const SPAM_COUNT: u32 = 500;

/// One ellipse every 4 s. It starts and ends at `from` and stays left of it.
pub fn drag_loop(from: (f64, f64), t: f64) -> (f64, f64) {
    let angle = t * std::f64::consts::TAU / 4.0;
    (
        from.0 + 150.0 * (angle.cos() - 1.0),
        from.1 + 100.0 * angle.sin(),
    )
}

fn run_cpu_body(scenario: CpuScenario, stage: &mut Stage, dome: Option<&Dome>, overlay: Option<i32>) {
    let fixtures = stage.pids();
    let target = stage.window("a2").rect;
    match scenario {
        CpuScenario::Idle => {}
        CpuScenario::Drag => {
            let grab = (target.x + 300.0, target.y + 12.0);
            let front = input::front_window_at(grab, overlay).map(|w| w.id);
            if front != Some(stage.window("a2").id) {
                eprintln!("drag: the window under the grab point is {front:?}, not a2");
            }
            input::drag(grab, scenario.duration(), &fixtures, overlay, |t| {
                drag_loop(grab, t)
            });
        }
        CpuScenario::Resize => {
            let corner = (target.right() - 3.0, target.bottom() - 3.0);
            input::drag(corner, scenario.duration(), &fixtures, overlay, |t| {
                drag_loop(corner, t)
            });
        }
        CpuScenario::Switch => {
            let start = Instant::now();
            for i in 0..scenario.duration().as_secs() {
                let pid = if i % 2 == 0 { stage.b.pid } else { stage.a.pid };
                input::activate(pid);
                let next = start + Duration::from_secs(i + 1);
                if let Some(wait) = next.checked_duration_since(Instant::now()) {
                    std::thread::sleep(wait);
                }
            }
        }
        CpuScenario::DomeLayout => {
            let Some(dome) = dome else { return };
            let tilings = dome::tilings();
            let start = Instant::now();
            let steps = scenario.duration().as_millis() / LAYOUT_PERIOD.as_millis();
            for i in 0..steps as u32 {
                let failed = dome.apply(&tilings[i as usize % 2]);
                if !failed.is_empty() {
                    eprintln!("Dome layout: AX write failed for {failed:?}");
                }
                let next = start + LAYOUT_PERIOD * (i + 1);
                if let Some(wait) = next.checked_duration_since(Instant::now()) {
                    std::thread::sleep(wait);
                }
            }
        }
        CpuScenario::FocusSpam => {
            let interval = SPAM_INTERVAL.as_millis() as u64;
            if let Err(e) = stage.spam("a1", "a2", interval, u64::from(SPAM_COUNT)) {
                eprintln!("focus spam: {e}");
            }
        }
        CpuScenario::LayoutJumps | CpuScenario::SmoothMoves => {
            let (Some(dome), Some(motion)) = (dome, scenario.motion()) else {
                return;
            };
            let duration = scenario.duration();
            let outcome = motion::run(motion, dome, |t| t >= duration);
            if outcome.failed_writes > 0 || outcome.failed_focuses > 0 {
                eprintln!("{}: {outcome:?}", scenario.name());
            }
        }
    }
}

/// A mode name may carry a burst period in ms after `@`, as in `events@50`.
pub fn overlay_args(mode: &str, safety_ms: Option<u64>) -> Vec<String> {
    let (mode, frame_ms) = match mode.split_once('@') {
        Some((mode, ms)) => (mode, Some(ms)),
        None => (mode, None),
    };
    let mut args = vec![String::from("--mode"), mode.to_string()];
    if let Some(ms) = frame_ms {
        args.push(String::from("--frame-ms"));
        args.push(ms.to_string());
    }
    if let Some(ms) = safety_ms {
        args.push(String::from("--safety-ms"));
        args.push(ms.to_string());
    }
    args
}

fn cpu_part(ctx: &mut Ctx, modes: &[String], baseline: bool) -> Result<(), String> {
    let mut runs: Vec<Option<String>> = modes.iter().cloned().map(Some).collect();
    if baseline {
        runs.push(None);
    }
    for run in 1..=CPU_RUNS {
        for mode in &runs {
            cpu_mode(ctx, mode.as_deref(), run)?;
        }
    }
    Ok(())
}

const CPU_RUNS: u32 = 2;

fn cpu_mode(ctx: &mut Ctx, mode: Option<&str>, run: u32) -> Result<(), String> {
    let label = mode.unwrap_or("no overlay").to_string();
    let dir = ctx
        .out
        .join("cpu")
        .join(label.replace(' ', "-"))
        .join(format!("run{run}"));
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut stage = Stage::start(&ctx.bin_dir)?;
    let b = stage.backdrop;
    let message = format!(
        "event=stage backdrop_id={} backdrop={},{},{},{}",
        stage.backdrop_id, b.x, b.y, b.w, b.h
    );
    ctx.log(&message);

    for scenario in CpuScenario::ALL {
        let scenario_label = scenario.name().to_string();
        ctx.gate(&format!("cpu {label} {scenario_label} run {run}"))?;
        let dome = match scenario.motion() {
            Some(_) => {
                stage.reset(&motion::stage_layout())?;
                Some(Dome::new(&stage, &motion::LABELS)?)
            }
            None => {
                stage.reset(&stage::cpu_layout())?;
                match scenario {
                    CpuScenario::DomeLayout => Some(Dome::new(&stage, &stage::CPU_LABELS)?),
                    _ => None,
                }
            }
        };
        let prefix = dir.join(scenario.slug());
        let overlay = match mode {
            Some(m) => Some(Overlay::start(
                &ctx.bin_dir.join("border-occlusion-spike"),
                &overlay_args(m, None),
                &prefix.with_extension("overlay.log"),
            )?),
            None => None,
        };
        if let Some(o) = &overlay {
            stage.attach(o.control.clone());
        }
        std::thread::sleep(SETTLE);
        let load_start = load_one_minute();
        ctx.log(&format!(
            "event=cpu_start mode=\"{label}\" scenario=\"{scenario_label}\" run={run} load={load_start}"
        ));
        let overlay_pid = overlay.as_ref().map(|o| o.pid);
        let fixture_pids = [stage.a.pid, stage.b.pid];
        let m = cpu::measure(
            &prefix,
            overlay.as_ref(),
            &fixture_pids,
            ctx.ws_pid,
            scenario.duration(),
            || run_cpu_body(scenario, &mut stage, dome.as_ref(), overlay_pid),
        );
        let load_end = load_one_minute();
        drop(overlay);
        stage.detach();
        ctx.log(&format!(
            "event=cpu_end mode=\"{label}\" scenario=\"{scenario_label}\" run={run} load={load_end} overlay_cpu={:?} ws_cpu={:?}",
            m.overlay.as_ref().and_then(|o| o.cpu_percent),
            m.window_server.cpu_percent
        ));
        ctx.report.cpu.push(CpuRow {
            mode: label.clone(),
            scenario: scenario_label,
            run,
            load: (load_start, load_end),
            m,
        });
        ctx.report.write(&ctx.out);
    }
    Ok(())
}

/// A window of another process above the normal layer that covers at least
/// half the screen, such as an open Notification Center. A posted click would
/// land on it.
fn stage_blocker() -> Option<String> {
    let screen = CGDisplayBounds(CGMainDisplayID());
    let half = screen.size.width * screen.size.height / 2.0;
    query_windows_named(None).into_iter().find_map(|(w, owner)| {
        let foreign = w.layer > 0 && w.layer < 1000 && !procs::is_child(w.pid as i32);
        (foreign && w.rect.w * w.rect.h >= half).then(|| format!("{owner} layer {}", w.layer))
    })
}

fn load_average() -> String {
    Command::new("sysctl")
        .args(["-n", "vm.loadavg"])
        .output()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .trim()
                .trim_matches(['{', '}', ' '])
                .to_string()
        })
        .unwrap_or_default()
}

fn window_server_pid() -> Result<i32, String> {
    let out = Command::new("pgrep")
        .args(["-x", "WindowServer"])
        .output()
        .map_err(|e| e.to_string())?;
    String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .and_then(|l| l.trim().parse().ok())
        .ok_or_else(|| String::from("no WindowServer pid"))
}

fn run(ctx: &mut Ctx, args: &Args) -> Result<(), String> {
    ctx.gate("run start")?;
    ctx.report
        .notes
        .push(format!("Load average (1, 5, 15 min) at the start: {}.", load_average()));
    if ctx.no_idle_wait {
        ctx.report.notes.push(String::from(
            "The run used --no-idle-wait. The user may have used the machine during a scenario.",
        ));
    } else {
        ctx.probe_posted_events();
    }
    let has = |part: &str| args.parts.iter().any(|p| p == part);
    if has("cpu") {
        cpu_part(ctx, &args.modes, args.baseline)?;
    }
    let checked: Vec<&String> = args.modes.iter().filter(|m| *m != "poll-diff").collect();
    let motion_only = !has("fidelity") && has("motion");
    if has("fidelity") || motion_only {
        for mode in checked.iter().filter(|m| **m != "events-focused") {
            let load_start = load_one_minute();
            let results = fidelity::run_mode(ctx, mode, motion_only)?;
            ctx.report.fidelity.extend(results);
            ctx.report.notes.push(format!(
                "Load average (1 min) at the start and the end of the {mode} accuracy cases: {load_start} and {}.",
                load_one_minute()
            ));
            ctx.report.write(&ctx.out);
        }
    }
    if has("lag") {
        for mode in &checked {
            ctx.gate(&format!("lag {mode}"))?;
            let load_start = load_one_minute();
            let result = lag::run_mode(ctx, mode)?;
            ctx.report.lag.push(result);
            ctx.report.notes.push(format!(
                "Load average (1 min) at the start and the end of the {mode} drag lag: {load_start} and {}.",
                load_one_minute()
            ));
            ctx.report.write(&ctx.out);
        }
    }
    Ok(())
}

fn load_one_minute() -> String {
    load_average()
        .split_whitespace()
        .next()
        .unwrap_or("?")
        .to_string()
}

fn main() {
    let args = parse_args();
    procs::install_cleanup();
    std::fs::create_dir_all(&args.out).expect("create the out directory");
    let bin_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
        .expect("driver directory");

    let log = File::create(args.out.join("driver.log")).expect("driver log");
    let mut ctx = Ctx {
        out: args.out.clone(),
        bin_dir,
        ws_pid: window_server_pid().expect("WindowServer pid"),
        log,
        gate_each: true,
        gated_once: false,
        no_idle_wait: args.no_idle_wait,
        report: Report {
            started: format!("at ts_ms {}", now_ms()),
            modes: args.modes.clone(),
            ..Report::default()
        },
    };

    let caffeinate = procs::spawn(
        Command::new("caffeinate")
            .args(["-dimsu", "-w", &std::process::id().to_string()])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null()),
    );
    if let Err(e) = &caffeinate {
        ctx.log(&format!("event=error what=caffeinate detail=\"{e}\""));
    }

    let result = run(&mut ctx, &args);
    ctx.report.notes.push(format!(
        "Load average (1, 5, 15 min) at the end: {}.",
        load_average()
    ));
    if let Err(e) = &result {
        ctx.report.notes.push(format!("The run stopped early: {e}."));
        ctx.log(&format!("event=stopped reason=\"{e}\""));
    }
    ctx.report.write(&ctx.out);
    procs::cleanup();
    if result.is_err() {
        exit(3);
    }
}
