//! Overlay binary. One click-through window draws a border around every
//! managed window on the primary display, minus the parts that windows in
//! front cover. `--mode` picks when it calls CGWindowList. Stdin takes
//! `manage <pid> <window number> <title>`, `unmanage <window number>`, and
//! `stats`, which logs the stats line at once and starts a new period.
//!
//! Primary display only. The coordinate flip assumes one display.

use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::io::BufRead;
use std::process::exit;
use std::ptr::{NonNull, null};
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::{MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSBackingStoreType, NSColor, NSRunningApplication,
    NSScreen, NSView, NSWindow, NSWindowCollectionBehavior, NSWindowStyleMask,
};
use objc2_application_services::{AXIsProcessTrusted, AXUIElement};
use objc2_core_foundation::{
    CFAbsoluteTimeGetCurrent, CFRetained, CFRunLoop, CFRunLoopTimer, CGPoint, CGRect, CGSize,
    kCFRunLoopCommonModes,
};
use objc2_core_graphics::{CGColor, CGMutablePath, CGPath};
use objc2_foundation::NSTimer;
use objc2_quartz_core::{CALayer, CAShapeLayer, CATransaction};
use spike::ax;
use spike::geometry::{Rect, border_ring, subtract_all};
use spike::moving::MovingSet;
use spike::now_ms;
use spike::throttle::{Submit, Throttle};
use spike::windows::{
    ChangeKind, WinInfo, changes, query_window, query_windows, query_windows_by_id,
};

mod triggers;
use triggers::TriggerKind;

/// Border thickness in points.
const THICKNESS: f64 = 6.0;
const FRAME_SECS: f64 = 1.0 / 60.0;
/// Windows smaller than this on either side are helper or shadow windows.
const MIN_SIDE: f64 = 40.0;
const STATS_SECS: f64 = 5.0;
/// At most one z-order CGWindowList call per interval in the AX modes.
const ZORDER_INTERVAL: Duration = Duration::from_millis(100);
/// The delay from a z-order call to its second call.
const SECOND_CALL_DELAY: Duration = Duration::from_millis(50);
/// The period of the reads of a moving window.
const READ_SECS: f64 = 0.016;
/// How long the reads of a window go on after its last move notification. The
/// notifications of a drag come about 110 ms apart.
const READ_TAIL: Duration = Duration::from_millis(250);
/// Bounds how long a busy app can stall the main thread on one AX read.
const AX_TIMEOUT_SECS: f32 = 0.05;

macro_rules! log {
    ($($arg:tt)*) => {
        eprintln!("ts_ms={} {}", now_ms(), format_args!($($arg)*))
    };
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Mode {
    Poll,
    PollDiff,
    Events,
    EventsFocused,
    /// The overlay calls CGWindowList after a z-order event and once per
    /// safety period. It reads a managed window's frame from AX after each move
    /// or resize notification of that window.
    AxGeometry,
    /// `ax-geometry`, plus a timer that reads a moving window's frame from AX
    /// every `READ_SECS`, a second CGWindowList call `SECOND_CALL_DELAY` after
    /// each z-order call, and a safety call that never skips.
    AxPositionReads,
    /// `ax-position-reads`, except that a moving window's bounds come from one
    /// CGWindowList call every `READ_SECS` that reads all the moving windows,
    /// instead of from an AX read timer per window.
    MovingWindowList,
}

impl Mode {
    fn parse(s: &str) -> Option<Mode> {
        match s {
            "poll" => Some(Mode::Poll),
            "poll-diff" => Some(Mode::PollDiff),
            "events" => Some(Mode::Events),
            "events-focused" => Some(Mode::EventsFocused),
            "ax-geometry" => Some(Mode::AxGeometry),
            "ax-position-reads" => Some(Mode::AxPositionReads),
            "moving-window-list" => Some(Mode::MovingWindowList),
            _ => None,
        }
    }

    fn name(self) -> &'static str {
        match self {
            Mode::Poll => "poll",
            Mode::PollDiff => "poll-diff",
            Mode::Events => "events",
            Mode::EventsFocused => "events-focused",
            Mode::AxGeometry => "ax-geometry",
            Mode::AxPositionReads => "ax-position-reads",
            Mode::MovingWindowList => "moving-window-list",
        }
    }

    /// Whether frames come from AX reads and a full CGWindowList call runs
    /// only for z-order events and the safety period.
    fn ax(self) -> bool {
        matches!(
            self,
            Mode::AxGeometry | Mode::AxPositionReads | Mode::MovingWindowList
        )
    }

    /// Whether each z-order call gets a second call `SECOND_CALL_DELAY` later.
    fn second_call(self) -> bool {
        matches!(self, Mode::AxPositionReads | Mode::MovingWindowList)
    }
}

struct Args {
    mode: Mode,
    level: isize,
    tail: Duration,
    safety: Duration,
    frame: Duration,
}

fn parse_args() -> Args {
    let mut args = Args {
        mode: Mode::Poll,
        level: 1000,
        tail: Duration::from_millis(300),
        safety: Duration::from_millis(1000),
        frame: Duration::from_secs_f64(FRAME_SECS),
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let value = it
            .next()
            .unwrap_or_else(|| usage(&format!("{flag} needs a value")));
        match flag.as_str() {
            "--mode" => {
                args.mode = Mode::parse(&value)
                    .unwrap_or_else(|| usage(&format!("bad value for {flag}: {value}")))
            }
            "--level" => args.level = number(&flag, &value),
            "--tail-ms" => args.tail = Duration::from_millis(number(&flag, &value)),
            "--safety-ms" => args.safety = Duration::from_millis(number(&flag, &value)),
            "--frame-ms" => args.frame = Duration::from_millis(number(&flag, &value)),
            _ => usage(&format!("unknown flag {flag}")),
        }
    }
    args
}

fn number<T: std::str::FromStr>(flag: &str, value: &str) -> T {
    value
        .parse()
        .unwrap_or_else(|_| usage(&format!("bad value for {flag}: {value}")))
}

fn usage(message: &str) -> ! {
    eprintln!("{message}");
    eprintln!(
        "usage: border-occlusion-spike [--mode poll|poll-diff|events|events-focused|ax-geometry|ax-position-reads|moving-window-list] [--level N] [--tail-ms N] [--safety-ms N]"
    );
    exit(2);
}

#[derive(Clone, Copy)]
enum Cause {
    Trigger,
    Frame,
    Safety,
    Tick,
}

impl Cause {
    fn name(self) -> &'static str {
        match self {
            Cause::Trigger => "trigger",
            Cause::Frame => "frame",
            Cause::Safety => "safety",
            Cause::Tick => "tick",
        }
    }
}

/// Why an AX mode called CGWindowList.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ListCause {
    Zorder,
    ZorderSecond,
    Safety,
}

impl ListCause {
    fn name(self) -> &'static str {
        match self {
            ListCause::Zorder => "zorder",
            ListCause::ZorderSecond => "zorder-second",
            ListCause::Safety => "safety",
        }
    }
}

#[derive(Clone, Copy)]
enum ReadSource {
    Notification,
    Timer,
}

impl ReadSource {
    fn name(self) -> &'static str {
        match self {
            ReadSource::Notification => "notification",
            ReadSource::Timer => "timer",
        }
    }
}

#[derive(Default)]
struct Stats {
    polls: [u64; 4],
    focused_polls: u64,
    cglist: [u64; 3],
    /// Second calls that found a change the first call missed.
    second_found: u64,
    ax_reads: u64,
    /// Reads whose rect differed from the previous read of that window.
    ax_changed: u64,
    ax_failures: u64,
    ax_ns_total: u128,
    /// Calls that read the bounds of the moving windows only.
    cglist_moving: u64,
    /// Moving calls that returned a rect the cache did not hold.
    cglist_moving_changed: u64,
    redraws: u64,
    poll_ns_total: u128,
    poll_ns_max: u128,
    windows: usize,
    /// The start of the period the next stats line covers.
    since: Option<Instant>,
}

impl Stats {
    fn new() -> Stats {
        Stats {
            since: Some(Instant::now()),
            ..Stats::default()
        }
    }
    fn record_poll(&mut self, cause: Cause, elapsed: Duration, windows: usize) {
        self.polls[cause as usize] += 1;
        self.windows = windows;
        self.record_time(elapsed);
    }

    fn record_cglist(&mut self, cause: ListCause, elapsed: Duration, windows: usize) {
        self.cglist[cause as usize] += 1;
        self.windows = windows;
        self.record_time(elapsed);
    }

    fn record_ax_read(&mut self, elapsed: Duration, ok: bool, changed: bool) {
        self.ax_reads += 1;
        self.ax_changed += u64::from(changed);
        self.ax_failures += u64::from(!ok);
        self.ax_ns_total += elapsed.as_nanos();
    }

    fn record_moving(&mut self, elapsed: Duration, changed: bool) {
        self.cglist_moving += 1;
        self.cglist_moving_changed += u64::from(changed);
        self.record_time(elapsed);
    }

    fn record_focused_poll(&mut self, elapsed: Duration) {
        self.focused_polls += 1;
        self.record_time(elapsed);
    }

    fn record_time(&mut self, elapsed: Duration) {
        let ns = elapsed.as_nanos();
        self.poll_ns_total += ns;
        self.poll_ns_max = self.poll_ns_max.max(ns);
    }

    fn flush(&mut self) {
        let polls: u64 = self.polls.iter().sum::<u64>()
            + self.focused_polls
            + self.cglist.iter().sum::<u64>()
            + self.cglist_moving;
        let mean_us = self.poll_ns_total.checked_div(polls as u128 * 1000).unwrap_or(0);
        let ax_mean_us = self
            .ax_ns_total
            .checked_div(self.ax_reads as u128 * 1000)
            .unwrap_or(0);
        log!(
            "event=stats polls_trigger={} polls_frame={} polls_safety={} polls_tick={} polls_focused={} cglist_zorder={} cglist_zorder_second={} second_found={} cglist_safety={} cglist_moving={} cglist_moving_changed={} axreads={} axreads_changed={} axread_failures={} axread_mean_us={} redraws={} poll_mean_us={} poll_max_us={} windows={} period_ms={}",
            self.polls[Cause::Trigger as usize],
            self.polls[Cause::Frame as usize],
            self.polls[Cause::Safety as usize],
            self.polls[Cause::Tick as usize],
            self.focused_polls,
            self.cglist[ListCause::Zorder as usize],
            self.cglist[ListCause::ZorderSecond as usize],
            self.second_found,
            self.cglist[ListCause::Safety as usize],
            self.cglist_moving,
            self.cglist_moving_changed,
            self.ax_reads,
            self.ax_changed,
            self.ax_failures,
            ax_mean_us,
            self.redraws,
            mean_us,
            self.poll_ns_max / 1000,
            self.windows,
            self.since.map_or(0, |since| since.elapsed().as_millis()),
        );
        *self = Stats::new();
    }
}

/// A window named by a `manage` command. Only these get a border.
struct Managed {
    pid: i32,
    title: String,
    /// Found by title in the AX modes only. `None` until the match succeeds.
    element: Option<CFRetained<AXUIElement>>,
    /// The rect of the last successful AX read of this window.
    last_read: Option<Rect>,
    last_moved: Option<Instant>,
    /// In `ax-position-reads`, runs from a move notification until `READ_TAIL`
    /// after the last one.
    read_timer: Option<Retained<NSTimer>>,
}

impl Managed {
    fn stop_reads(&mut self) {
        if let Some(timer) = self.read_timer.take() {
            timer.invalidate();
        }
    }
}

struct Overlay {
    mode: Mode,
    shape: Retained<CAShapeLayer>,
    screen_height: f64,
    own_pid: i64,
    level: i64,
    last: Option<Vec<WinInfo>>,
    managed: HashMap<u32, Managed>,
    commands: Receiver<String>,
    stats: Stats,
    tail: Duration,
    safety: Duration,
    /// The period of the burst timer in `events`.
    frame: Duration,
    last_trigger: Instant,
    frame_timer: Option<Retained<NSTimer>>,
    /// In `events-focused`, the pid of the current burst while every trigger
    /// in it so far is a move or a resize from that pid.
    focus: Option<i32>,
    zorder: Throttle,
    trailing_timer: Option<Retained<NSTimer>>,
    second_timer: Option<Retained<NSTimer>>,
    /// The list the last `zorder` call returned, which its second call
    /// compares against.
    first_list: Option<Vec<WinInfo>>,
    /// Z-order events since the last `zorder` call.
    events_since_first: u32,
    moving: MovingSet,
    /// Runs while `moving` is not empty.
    moving_timer: Option<Retained<NSTimer>>,
    /// The pieces of the path on screen.
    drawn: Option<Vec<Rect>>,
}

impl Overlay {
    fn trigger(&mut self, kind: TriggerKind, pid: i32, element: Option<&AXUIElement>) {
        if self.mode.ax() {
            self.ax_trigger(kind, pid, element);
            return;
        }
        log!("event=trigger kind={} pid={pid}", kind.name());
        self.last_trigger = Instant::now();
        let movement = matches!(kind, TriggerKind::Moved | TriggerKind::Resized);
        if self.frame_timer.is_none() {
            self.focus = (self.mode == Mode::EventsFocused && movement).then_some(pid);
            self.poll_and_draw(Cause::Trigger);
            self.frame_timer = Some(repeating_timer(self.frame.as_secs_f64(), || {
                with_overlay(Overlay::on_frame);
            }));
        } else if !movement || self.focus != Some(pid) {
            self.focus = None;
        }
    }

    fn on_frame(&mut self) {
        match self.focus {
            Some(pid) => self.focused_poll(pid),
            None => {
                self.poll_and_draw(Cause::Frame);
            }
        }
        if self.last_trigger.elapsed() > self.tail
            && let Some(timer) = self.frame_timer.take()
        {
            timer.invalidate();
            if self.focus.take().is_some() {
                self.poll_and_draw(Cause::Frame);
            }
        }
    }

    /// Reads only the frontmost cached window of `pid` and patches its rect
    /// into the cached list. Falls back to a full poll when that window is
    /// not in the cache or no longer exists.
    fn focused_poll(&mut self, pid: i32) {
        let index = self.last.as_ref().and_then(|last| {
            last.iter()
                .position(|w| w.pid == i64::from(pid) && w.layer == 0)
        });
        let Some(index) = index else {
            self.focus = None;
            self.poll_and_draw(Cause::Frame);
            return;
        };
        let Some(last) = self.last.as_mut() else {
            return;
        };
        let start = Instant::now();
        let fresh = query_window(last[index].id);
        self.stats.record_focused_poll(start.elapsed());
        match fresh {
            Some(w) if w.rect != last[index].rect => {
                last[index].rect = w.rect;
                let changed = draw(
                    &self.shape,
                    self.screen_height,
                    self.level,
                    last,
                    &self.managed,
                    &mut self.drawn,
                );
                self.stats.redraws += 1;
                log!(
                    "event=redraw cause=frame changed={} focused=1",
                    u8::from(changed)
                );
            }
            Some(_) => {}
            None => {
                self.focus = None;
                self.poll_and_draw(Cause::Frame);
            }
        }
    }

    fn on_safety(&mut self) {
        if self.frame_timer.is_some() {
            return;
        }
        let Some(old) = self.poll_and_draw(Cause::Safety) else {
            return;
        };
        let new = self.last.clone().unwrap_or_default();
        for (w, change) in changes(&old, &new) {
            log!(
                "event=unreported app={} pid={} id={} layer={} change={}",
                app_name(w.pid),
                w.pid,
                w.id,
                w.layer,
                change.name()
            );
        }
        self.trigger(TriggerKind::Safety, 0, None);
    }

    /// Returns the previous list when the new list differs from it. `poll`
    /// mode redraws on every call, the other modes only on a change.
    fn poll_and_draw(&mut self, cause: Cause) -> Option<Vec<WinInfo>> {
        let start = Instant::now();
        let windows = query_windows(Some(self.own_pid));
        self.stats.record_poll(cause, start.elapsed(), windows.len());
        let changed = self.last.as_ref() != Some(&windows);
        if changed || self.mode == Mode::Poll {
            let path_changed = draw(
                &self.shape,
                self.screen_height,
                self.level,
                &windows,
                &self.managed,
                &mut self.drawn,
            );
            self.stats.redraws += 1;
            log!(
                "event=redraw cause={} changed={}",
                cause.name(),
                u8::from(path_changed)
            );
        }
        if changed {
            Some(self.last.replace(windows).unwrap_or_default())
        } else {
            None
        }
    }

    fn drain_commands(&mut self) {
        while let Ok(line) = self.commands.try_recv() {
            self.command(&line);
        }
    }

    fn command(&mut self, line: &str) {
        let words: Vec<&str> = line.splitn(4, ' ').collect();
        match words.as_slice() {
            ["manage", pid, id, title] => {
                let (Ok(pid), Ok(id)) = (pid.parse::<i32>(), id.parse::<u32>()) else {
                    log!("event=error what=bad_command line=\"{line}\"");
                    return;
                };
                let element = self
                    .mode
                    .ax()
                    .then(|| ax::window_with_title(pid, title, AX_TIMEOUT_SECS))
                    .flatten();
                log!(
                    "event=manage window={id} pid={pid} title={title} handle={}",
                    u8::from(element.is_some())
                );
                let old = self.managed.insert(
                    id,
                    Managed {
                        pid,
                        title: title.to_string(),
                        element,
                        last_read: None,
                        last_moved: None,
                        read_timer: None,
                    },
                );
                if let Some(mut old) = old {
                    old.stop_reads();
                }
                self.redraw_cached("manage");
            }
            ["stats"] => self.stats.flush(),
            ["unmanage", id] => {
                let Ok(id) = id.parse::<u32>() else {
                    log!("event=error what=bad_command line=\"{line}\"");
                    return;
                };
                if let Some(mut m) = self.managed.remove(&id) {
                    m.stop_reads();
                    self.moving.remove(id);
                    log!("event=unmanage window={id} pid={} title={}", m.pid, m.title);
                    self.redraw_cached("unmanage");
                }
            }
            _ => log!("event=error what=bad_command line=\"{line}\""),
        }
    }

    /// The managed set changed but the window list did not, so no
    /// CGWindowList call is needed.
    fn redraw_cached(&mut self, cause: &str) {
        if self.mode.ax() {
            self.redraw_if_changed(cause);
            return;
        }
        let Some(last) = self.last.as_ref() else {
            return;
        };
        let changed = draw(
            &self.shape,
            self.screen_height,
            self.level,
            last,
            &self.managed,
            &mut self.drawn,
        );
        self.stats.redraws += 1;
        log!("event=redraw cause={cause} changed={}", u8::from(changed));
    }

    fn ax_trigger(&mut self, kind: TriggerKind, pid: i32, element: Option<&AXUIElement>) {
        log!("event=trigger kind={} pid={pid}", kind.name());
        match kind {
            TriggerKind::Moved | TriggerKind::Resized => self.geometry_event(kind, pid, element),
            _ => {
                self.events_since_first += 1;
                match self.zorder.submit(Instant::now()) {
                    Submit::CallNow => self.zorder_call(),
                    Submit::Arm(delay) => {
                        let timer = one_shot_timer(delay.as_secs_f64(), || {
                            with_overlay(Overlay::on_trailing);
                        });
                        if let Some(old) = self.trailing_timer.replace(timer) {
                            old.invalidate();
                        }
                    }
                    Submit::Pending => {}
                }
            }
        }
    }

    fn zorder_call(&mut self) {
        self.cglist(ListCause::Zorder);
        if let Some(delay) = self.zorder.second_delay() {
            let timer = one_shot_timer(delay.as_secs_f64(), || {
                with_overlay(Overlay::on_second);
            });
            if let Some(old) = self.second_timer.replace(timer) {
                old.invalidate();
            }
        }
    }

    fn on_second(&mut self) {
        self.second_timer = None;
        if self.zorder.fire_second() {
            self.cglist(ListCause::ZorderSecond);
        }
    }

    /// Reads the frame of the managed window that `element` names. A move of
    /// any other window reads nothing, so its cached rect stays old until the
    /// next CGWindowList call.
    fn geometry_event(&mut self, kind: TriggerKind, pid: i32, element: Option<&AXUIElement>) {
        let Some(element) = element else { return };
        let id = self
            .managed
            .iter()
            .find(|(_, m)| m.pid == pid && m.element.as_deref() == Some(element))
            .map(|(id, _)| *id);
        let Some(id) = id else { return };
        self.read_frame(id, ReadSource::Notification);
        if kind == TriggerKind::Moved {
            match self.mode {
                Mode::AxPositionReads => self.keep_reading(id),
                Mode::MovingWindowList => self.keep_listing(id),
                _ => {}
            }
        }
    }

    fn keep_listing(&mut self, id: u32) {
        self.moving.moved(id, Instant::now());
        if self.moving_timer.is_none() {
            self.moving_timer = Some(repeating_timer(READ_SECS, || {
                with_overlay(Overlay::on_moving_tick);
            }));
        }
    }

    /// Patches the rect of each moving window into the cache and leaves the
    /// z-order and every other rect as the last full call left them.
    fn on_moving_tick(&mut self) {
        let ids = self.moving.current(Instant::now());
        if ids.is_empty() {
            if let Some(timer) = self.moving_timer.take() {
                timer.invalidate();
            }
            return;
        }
        let start = Instant::now();
        let fresh = query_windows_by_id(&ids);
        let elapsed = start.elapsed();
        let mut changed = false;
        if let Some(last) = self.last.as_mut() {
            for w in &fresh {
                if let Some(cached) = last.iter_mut().find(|c| c.id == w.id)
                    && cached.rect != w.rect
                {
                    cached.rect = w.rect;
                    changed = true;
                }
            }
        }
        self.stats.record_moving(elapsed, changed);
        log!(
            "event=cglist cause=moving count={} us={} changed={}",
            ids.len(),
            elapsed.as_micros(),
            u8::from(changed)
        );
        self.redraw_if_changed("moving");
    }

    fn keep_reading(&mut self, id: u32) {
        let Some(m) = self.managed.get_mut(&id) else {
            return;
        };
        m.last_moved = Some(Instant::now());
        if m.read_timer.is_none() {
            m.read_timer = Some(repeating_timer(READ_SECS, move || {
                with_overlay(|o| o.on_read_tick(id));
            }));
        }
    }

    fn on_read_tick(&mut self, id: u32) {
        self.read_frame(id, ReadSource::Timer);
        if let Some(m) = self.managed.get_mut(&id)
            && m.last_moved.is_none_or(|at| at.elapsed() >= READ_TAIL)
        {
            m.stop_reads();
        }
    }

    fn on_trailing(&mut self) {
        self.trailing_timer = None;
        if self.zorder.fire(Instant::now()) {
            self.zorder_call();
        }
    }

    fn on_ax_safety(&mut self) {
        if self.mode == Mode::AxGeometry
            && self
                .zorder
                .last_call()
                .is_some_and(|at| at.elapsed() < self.safety)
        {
            return;
        }
        self.cglist(ListCause::Safety);
    }

    /// Replaces the whole cache, including every rect an AX read patched.
    fn cglist(&mut self, cause: ListCause) {
        let start = Instant::now();
        let windows = query_windows(Some(self.own_pid));
        let elapsed = start.elapsed();
        self.stats.record_cglist(cause, elapsed, windows.len());
        match cause {
            ListCause::ZorderSecond => {
                // A moved rect is left out, because a moving window changes
                // its rect between any two calls.
                let missed: Vec<&str> = self
                    .first_list
                    .as_deref()
                    .map(|first| changes(first, &windows))
                    .unwrap_or_default()
                    .into_iter()
                    .filter(|(_, change)| *change != ChangeKind::Moved)
                    .map(|(_, change)| change.name())
                    .collect();
                self.stats.second_found += u64::from(!missed.is_empty());
                log!(
                    "event=cglist cause={} us={} found={} changes={} events={}",
                    cause.name(),
                    elapsed.as_micros(),
                    u8::from(!missed.is_empty()),
                    if missed.is_empty() {
                        String::from("none")
                    } else {
                        missed.join(",")
                    },
                    self.events_since_first
                );
            }
            _ => log!(
                "event=cglist cause={} us={}",
                cause.name(),
                elapsed.as_micros()
            ),
        }
        if cause == ListCause::Zorder && self.mode.second_call() {
            self.first_list = Some(windows.clone());
            self.events_since_first = 0;
        }
        let old = self.last.replace(windows);
        if cause == ListCause::Safety
            && let (Some(old), Some(new)) = (&old, &self.last)
        {
            for (w, change) in changes(old, new) {
                log!(
                    "event=unreported app={} pid={} id={} layer={} change={}",
                    app_name(w.pid),
                    w.pid,
                    w.id,
                    w.layer,
                    change.name()
                );
            }
        }
        for (id, m) in &mut self.managed {
            if m.element.is_none() {
                m.element = ax::window_with_title(m.pid, &m.title, AX_TIMEOUT_SECS);
                if m.element.is_some() {
                    log!("event=manage window={id} pid={} title={} handle=1", m.pid, m.title);
                }
            }
        }
        self.redraw_if_changed(cause.name());
    }

    /// A failed read keeps the cached rect.
    fn read_frame(&mut self, id: u32, source: ReadSource) {
        let Some(m) = self.managed.get(&id) else {
            return;
        };
        let Some(element) = m.element.clone() else {
            return;
        };
        let previous = m.last_read.or_else(|| {
            self.last
                .as_ref()
                .and_then(|last| last.iter().find(|w| w.id == id))
                .map(|w| w.rect)
        });
        let start = Instant::now();
        let frame = ax::frame(&element);
        let elapsed = start.elapsed();
        let changed = frame.is_some() && frame != previous;
        self.stats.record_ax_read(elapsed, frame.is_some(), changed);
        log!(
            "event=axread window={id} ok={} us={} changed={} source={}",
            u8::from(frame.is_some()),
            elapsed.as_micros(),
            u8::from(changed),
            source.name()
        );
        let Some(rect) = frame else { return };
        if let Some(m) = self.managed.get_mut(&id) {
            m.last_read = Some(rect);
        }
        let cached = self
            .last
            .as_mut()
            .and_then(|last| last.iter_mut().find(|w| w.id == id));
        if let Some(w) = cached {
            w.rect = rect;
        }
        self.redraw_if_changed("axread");
    }

    fn redraw_if_changed(&mut self, cause: &str) {
        let Some(last) = self.last.as_ref() else {
            return;
        };
        let pieces = visible_pieces(last, &self.managed, self.level);
        if self.drawn.as_ref() == Some(&pieces) {
            return;
        }
        set_path(&self.shape, self.screen_height, &pieces);
        self.drawn = Some(pieces);
        self.stats.redraws += 1;
        log!("event=redraw cause={cause} changed=1");
    }
}

thread_local! {
    static OVERLAY: RefCell<Option<Overlay>> = const { RefCell::new(None) };
}

fn app_name(pid: i64) -> String {
    let name = NSRunningApplication::runningApplicationWithProcessIdentifier(pid as i32)
        .and_then(|app| app.localizedName())
        .map(|name| name.to_string())
        .unwrap_or_else(|| process_name(pid as i32));
    name.replace(' ', "_")
}

/// The executable name, for a process that is not an app, such as
/// WindowServer.
fn process_name(pid: i32) -> String {
    let mut buf = [0u8; 256];
    let len = unsafe { libc::proc_name(pid, buf.as_mut_ptr().cast(), buf.len() as u32) };
    match usize::try_from(len) {
        Ok(len) if len > 0 => String::from_utf8_lossy(&buf[..len]).into_owned(),
        _ => format!("pid{pid}"),
    }
}

pub fn on_trigger(kind: TriggerKind, pid: i32, element: Option<&AXUIElement>) {
    with_overlay(|o| o.trigger(kind, pid, element));
}

fn with_overlay<R>(f: impl FnOnce(&mut Overlay) -> R) -> Option<R> {
    OVERLAY.with(|cell| match cell.try_borrow_mut() {
        Ok(mut guard) => guard.as_mut().map(f),
        Err(_) => {
            log!("event=error what=reentrant_borrow");
            None
        }
    })
}

fn repeating_timer(secs: f64, f: impl Fn() + 'static) -> Retained<NSTimer> {
    let block = RcBlock::new(move |_timer: NonNull<NSTimer>| f());
    unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(secs, true, &block) }
}

fn one_shot_timer(secs: f64, f: impl Fn() + 'static) -> Retained<NSTimer> {
    let block = RcBlock::new(move |_timer: NonNull<NSTimer>| f());
    unsafe { NSTimer::scheduledTimerWithTimeInterval_repeats_block(secs, false, &block) }
}

fn main() {
    let args = parse_args();
    let events = matches!(args.mode, Mode::Events | Mode::EventsFocused) || args.mode.ax();
    if events && !unsafe { AXIsProcessTrusted() } {
        eprintln!("events modes need the Accessibility permission for this process");
        exit(1);
    }

    let mtm = MainThreadMarker::new().expect("must run on the main thread");
    let app = NSApplication::sharedApplication(mtm);
    // An Accessory app still activates itself once when `run` starts, which
    // would take focus from the fixtures. A Prohibited app never activates.
    app.setActivationPolicy(NSApplicationActivationPolicy::Prohibited);

    let screens = NSScreen::screens(mtm);
    let screen = screens.firstObject().expect("at least one screen");
    let frame = screen.frame();
    let screen_height = frame.size.height;

    let window = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            NSWindow::alloc(mtm),
            frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::Buffered,
            false,
        )
    };
    window.setOpaque(false);
    window.setBackgroundColor(Some(&NSColor::clearColor()));
    window.setLevel(args.level);
    window.setIgnoresMouseEvents(true);
    window.setHasShadow(false);
    window.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::Stationary
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::IgnoresCycle,
    );

    let content: Retained<NSView> = window.contentView().expect("content view");
    let shape = CAShapeLayer::new();
    // Opaque magenta, so a screen capture can classify a border pixel exactly.
    let color = CGColor::new_srgb(1.0, 0.0, 1.0, 1.0);
    shape.setFillColor(Some(&color));
    // Layer-hosting view: install the shape layer, then turn on layer backing.
    // Its path is then in the view's own coordinates (origin bottom-left).
    let layer: &CALayer = &shape;
    content.setLayer(Some(layer));
    content.setWantsLayer(true);

    window.orderFrontRegardless();

    let (tx, commands) = channel::<String>();
    OVERLAY.with(|cell| {
        *cell.borrow_mut() = Some(Overlay {
            mode: args.mode,
            shape,
            screen_height,
            own_pid: std::process::id() as i64,
            level: args.level as i64,
            last: None,
            managed: HashMap::new(),
            commands,
            stats: Stats::new(),
            tail: args.tail,
            safety: args.safety,
            frame: args.frame,
            last_trigger: Instant::now(),
            frame_timer: None,
            focus: None,
            zorder: if args.mode.second_call() {
                Throttle::with_second_call(ZORDER_INTERVAL, SECOND_CALL_DELAY)
            } else {
                Throttle::new(ZORDER_INTERVAL)
            },
            trailing_timer: None,
            second_timer: None,
            first_list: None,
            events_since_first: 0,
            moving: MovingSet::new(READ_TAIL),
            moving_timer: None,
            drawn: None,
        })
    });
    read_commands(tx);

    let mut timers = Vec::new();
    let mut observed = (0, 0);
    if events {
        observed = triggers::install();
        on_trigger(TriggerKind::Start, 0, None);
        let safety: fn(&mut Overlay) = if args.mode.ax() {
            Overlay::on_ax_safety
        } else {
            Overlay::on_safety
        };
        timers.push(repeating_timer(args.safety.as_secs_f64(), move || {
            with_overlay(safety);
        }));
    } else {
        timers.push(repeating_timer(FRAME_SECS, || {
            with_overlay(|o| o.poll_and_draw(Cause::Tick));
        }));
    }
    timers.push(repeating_timer(STATS_SECS, || {
        with_overlay(|o| o.stats.flush());
    }));

    log!(
        "event=start mode={} level={} tail_ms={} safety_ms={} screen={:.0}x{:.0} pid={} observed_apps={} refused_apps={}",
        args.mode.name(),
        args.level,
        args.tail.as_millis(),
        args.safety.as_millis(),
        frame.size.width,
        screen_height,
        std::process::id(),
        observed.0,
        observed.1,
    );
    app.run();
}

/// Feeds stdin lines to the overlay on the main thread. A polling timer would
/// add wakeups to every idle measurement, so the timer sleeps until the
/// reader thread moves its fire date to now.
fn read_commands(tx: std::sync::mpsc::Sender<String>) {
    let never = CFAbsoluteTimeGetCurrent() + 1e9;
    let timer = unsafe {
        CFRunLoopTimer::new(
            None,
            never,
            1e9,
            0,
            0,
            Some(on_commands_timer),
            std::ptr::null_mut(),
        )
    }
    .expect("commands timer");
    if let Some(main) = CFRunLoop::main() {
        main.add_timer(Some(&timer), unsafe { kCFRunLoopCommonModes });
    }
    // The timer lives as long as the process, so the reader thread may keep
    // its address. CFRunLoopTimer calls are thread safe.
    let address = CFRetained::into_raw(timer).as_ptr() as usize;
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            let Ok(line) = line else { break };
            if tx.send(line).is_err() {
                return;
            }
            let timer = unsafe { &*(address as *const CFRunLoopTimer) };
            timer.set_next_fire_date(CFAbsoluteTimeGetCurrent());
            if let Some(main) = CFRunLoop::main() {
                main.wake_up();
            }
        }
    });
}

unsafe extern "C-unwind" fn on_commands_timer(_timer: *mut CFRunLoopTimer, _info: *mut c_void) {
    with_overlay(Overlay::drain_commands);
}

/// Returns whether the path differs from `drawn`, the path on screen before.
fn draw(
    shape: &CAShapeLayer,
    screen_height: f64,
    level: i64,
    windows: &[WinInfo],
    managed: &HashMap<u32, Managed>,
    drawn: &mut Option<Vec<Rect>>,
) -> bool {
    let pieces = visible_pieces(windows, managed, level);
    let changed = drawn.as_ref() != Some(&pieces);
    set_path(shape, screen_height, &pieces);
    *drawn = Some(pieces);
    changed
}

/// The visible border pieces of every managed window in `windows`, which is
/// front to back. A window at `level` or above is composited over the
/// overlay, so it hides the border under it without a cut.
fn visible_pieces(windows: &[WinInfo], managed: &HashMap<u32, Managed>, level: i64) -> Vec<Rect> {
    let mut pieces = Vec::new();
    for (i, win) in windows.iter().enumerate() {
        if !managed.contains_key(&win.id) {
            continue;
        }
        if win.rect.w < MIN_SIDE || win.rect.h < MIN_SIDE {
            continue;
        }

        let ring_bounds = win.rect.outset(THICKNESS);
        let occluders: Vec<Rect> = windows[..i]
            .iter()
            .filter(|other| other.layer < level)
            .map(|other| other.rect)
            .filter(|r| r.overlaps(&ring_bounds))
            .collect();

        pieces.extend(subtract_all(border_ring(&win.rect, THICKNESS).to_vec(), &occluders));
    }
    pieces
}

fn set_path(shape: &CAShapeLayer, screen_height: f64, pieces: &[Rect]) {
    let path = CGMutablePath::new();
    for piece in pieces {
        // CGWindowList is top-left, the view is bottom-left.
        let view_y = screen_height - piece.y - piece.h;
        let cg = CGRect {
            origin: CGPoint {
                x: piece.x,
                y: view_y,
            },
            size: CGSize {
                width: piece.w,
                height: piece.h,
            },
        };
        unsafe { CGMutablePath::add_rect(Some(&path), null(), cg) };
    }

    // No implicit animation, so the border tracks instantly.
    CATransaction::begin();
    CATransaction::setDisableActions(true);
    let p: &CGPath = &path;
    shape.setPath(Some(p));
    CATransaction::commit();
}
