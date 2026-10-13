mod inspection_dispatcher;
mod schedule;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use super::dome::{MonitorInfo, WindowsMetadata};
use super::external::{HwndId, ZOrder};
use super::handle;
use crate::core::{Length, LimitObservation, Logical, Physical, PixelRect, Pixels};
use inspection_dispatcher::InspectionDispatcher;
use schedule::SharedSchedule;

#[derive(Clone, Debug)]
pub(super) struct OwnedWindowRead {
    pub window: HwndId,
    pub offset: Option<(Pixels<Physical>, Pixels<Physical>)>,
}

#[derive(Clone, Debug)]
pub(super) struct PlacementRead {
    pub invisible_border: (
        Pixels<Physical>,
        Pixels<Physical>,
        Pixels<Physical>,
        Pixels<Physical>,
    ),
    pub unaware: bool,
    pub owned_windows: Vec<OwnedWindowRead>,
    pub deferred_children: Vec<HwndId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FullscreenKind {
    Borderless,
    Exclusive,
}

#[derive(Debug)]
pub(super) struct DiscoveredWindow {
    pub key: HwndId,
    pub process_id: u32,
    pub is_maximized: bool,
    pub metadata: WindowsMetadata,
    pub frame: PixelRect<Physical>,
    pub monitor: isize,
    pub limits: Option<LimitObservation>,
    pub fullscreen: Option<FullscreenKind>,
}

impl std::fmt::Display for DiscoveredWindow {
    fn fmt(&self, output: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            output,
            "[pid={}|hwnd={}] {}",
            self.process_id, self.key, self.metadata
        )
    }
}

#[derive(Debug)]
pub(super) struct SettleRead {
    pub observed_at: Instant,
    pub minimized: bool,
    pub is_maximized: bool,
    pub placement: Option<(PixelRect<Physical>, isize)>,
}

#[derive(Debug)]
pub(super) enum Observation {
    Discovered(DiscoveredWindow),
    Gone(HwndId),
    Minimized(HwndId),
    Foreground {
        window: HwndId,
    },
    ExclusiveFullscreen(Option<HwndId>),
    Title {
        window: HwndId,
        title: Option<String>,
    },
    Settled {
        window: HwndId,
        read: SettleRead,
    },
    SizeLimits {
        window: HwndId,
        limits: LimitObservation,
    },
    CornerRadius {
        window: HwndId,
        radius: Length<Logical>,
    },
}

impl Observation {
    fn window(&self) -> Option<HwndId> {
        match self {
            Self::Discovered(read) => Some(read.key),
            Self::Gone(window) | Self::Minimized(window) => Some(*window),
            Self::Foreground { window }
            | Self::Title { window, .. }
            | Self::Settled { window, .. }
            | Self::SizeLimits { window, .. }
            | Self::CornerRadius { window, .. } => Some(*window),
            Self::ExclusiveFullscreen(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum InspectionKind {
    Discovery,
    Title,
    SizeLimits,
    Placement,
    Settle { observed_at: Instant },
}

#[derive(Debug)]
enum InspectionOutcome {
    Completed {
        observation: Option<Box<Observation>>,
        placement: Option<PlacementRead>,
        owner: Option<HwndId>,
    },
    Failed,
    Deferred,
    Cancelled,
}

#[derive(Debug)]
struct InspectionResult {
    window: HwndId,
    kind: InspectionKind,
    outcome: InspectionOutcome,
}

pub(super) struct SizeLimitRead {
    pub window: HwndId,
}

pub(super) struct RecoveryEntry {
    pub window: HwndId,
    pub is_maximized: bool,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum NativeEvent {
    Discover(HwndId),
    Gone(HwndId),
    Title(HwndId),
    Minimized(HwndId),
    Restored {
        window: HwndId,
        observed_at: Instant,
    },
    MoveStarted {
        window: HwndId,
        observed_at: Instant,
    },
    Moved {
        window: HwndId,
        observed_at: Instant,
    },
    MoveEnded {
        window: HwndId,
        observed_at: Instant,
    },
    Foreground {
        window: HwndId,
        internal: bool,
        exclusive: Option<HwndId>,
    },
}

#[derive(Clone)]
pub(super) struct NativeEventSender {
    sender: mpsc::Sender<NativeEvent>,
    schedule: SharedSchedule,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl NativeEventSender {
    pub fn send(&self, event: NativeEvent) {
        {
            let mut schedule = self.schedule.lock();
            if schedule.stopped {
                return;
            }
            match event {
                NativeEvent::Moved {
                    window,
                    observed_at,
                }
                | NativeEvent::MoveStarted {
                    window,
                    observed_at,
                } => {
                    let state = schedule.windows.entry(window).or_default();
                    state.moved(observed_at);
                    if matches!(event, NativeEvent::MoveStarted { .. }) {
                        state.drag_started_at = Some(observed_at);
                    }
                }
                _ => {}
            }
        }
        if self.sender.send(event).is_ok() {
            (self.wake)();
        }
    }

    pub fn observe_foreground(&self, notified: Option<HwndId>) {
        if let Some((window, internal, exclusive)) = handle::read_foreground(notified) {
            self.send(NativeEvent::Foreground {
                window,
                internal,
                exclusive,
            });
        }
    }
}

pub(super) trait WindowServer {
    fn ingest_native_events(&mut self, now: Instant);
    fn ingest_read_results(&mut self, now: Instant);
    fn read_settled_windows(&mut self, now: Instant);
    fn has_observations(&self) -> bool;
    fn take_observations(&mut self) -> Vec<Observation>;
    fn next_deadline(&self) -> Option<Instant>;
    fn observe_displays(&mut self, monitors: &[MonitorInfo]);
    fn request_size_limits(&mut self, demand: Vec<SizeLimitRead>);
    fn read_corner_radii(&mut self, windows: &[HwndId]) -> Vec<Observation>;
    fn invalidate_foreground(&mut self);
    fn set_frame(&mut self, window: HwndId, frame: PixelRect<Physical>);
    fn set_z_order(&mut self, window: HwndId, z: ZOrder);
    fn move_offscreen(&mut self, window: HwndId);
    fn minimize(&mut self, window: HwndId);
    fn unminimize(&mut self, window: HwndId);
    fn close(&mut self, window: HwndId);
    fn focus(&mut self, window: HwndId);
    fn recover(&mut self, window: HwndId, was_maximized: bool);
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>);
}

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) enum NativeTrace {
    Event(NativeEvent),
    Placement(HwndId, PlacementRead),
    Settled(HwndId),
}

pub(super) struct WindowsWindowServer {
    #[cfg(test)]
    native_trace: Option<mpsc::Sender<(Instant, NativeTrace)>>,
    native_sender: NativeEventSender,
    native_events: mpsc::Receiver<NativeEvent>,
    results: mpsc::Receiver<InspectionResult>,
    dispatcher: Option<InspectionDispatcher>,
    schedule: SharedSchedule,
    observations: Vec<Observation>,
    placement: HashMap<HwndId, PlacementRead>,
    discovered: HashSet<HwndId>,
    owners: HashMap<HwndId, HwndId>,
    retired: HashSet<HwndId>,
    displays: Vec<MonitorInfo>,
    foreground: Option<HwndId>,
    stopped: bool,
}

impl WindowsWindowServer {
    pub fn new(wake: Arc<dyn Fn() + Send + Sync>) -> anyhow::Result<Self> {
        let schedule = SharedSchedule::default();
        let (native_sender, native_events) = mpsc::channel();
        let (result_sender, results) = mpsc::channel();
        let dispatcher = InspectionDispatcher::new(result_sender, schedule.clone(), wake.clone())?;
        Ok(Self {
            #[cfg(test)]
            native_trace: None,
            native_sender: NativeEventSender {
                sender: native_sender,
                schedule: schedule.clone(),
                wake,
            },
            native_events,
            results,
            dispatcher: Some(dispatcher),
            schedule,
            observations: Vec::new(),
            placement: HashMap::new(),
            discovered: HashSet::new(),
            owners: HashMap::new(),
            retired: HashSet::new(),
            displays: Vec::new(),
            foreground: None,
            stopped: false,
        })
    }

    #[cfg(test)]
    pub fn trace_native_acceptance(&mut self, trace: mpsc::Sender<(Instant, NativeTrace)>) {
        self.native_trace = Some(trace);
    }

    #[cfg(test)]
    fn trace(&self, event: NativeTrace) {
        if let Some(trace) = &self.native_trace {
            let _ = trace.send((Instant::now(), event));
        }
    }

    pub fn native_sender(&self) -> NativeEventSender {
        self.native_sender.clone()
    }

    fn request_parent_placement(&mut self, child: HwndId) {
        if let Some(parent) = self.owners.get(&child)
            && let Some(state) = self.schedule.lock().windows.get_mut(parent)
        {
            state.placement.request();
        }
    }

    fn accept_result(&mut self, result: InspectionResult, now: Instant) {
        if self.retired.contains(&result.window) {
            return;
        }
        let renew = matches!(result.outcome, InspectionOutcome::Deferred)
            || (result.kind != InspectionKind::Title
                && self.schedule.may_read(result.window, now) == Some(false));
        {
            let mut schedule = self.schedule.lock();
            let Some(state) = schedule.windows.get_mut(&result.window) else {
                return;
            };
            state.complete(result.kind, renew);
        }
        if renew {
            return;
        }
        let InspectionOutcome::Completed {
            observation,
            placement,
            owner,
        } = result.outcome
        else {
            return;
        };
        if let Some(owner) = owner
            && self.schedule.may_read(owner, now).is_some()
        {
            self.owners.insert(result.window, owner);
            self.request_parent_placement(result.window);
        }
        if let Some(placement) = placement {
            self.accept_placement(result.window, placement, now);
        }
        if let Some(observation) = observation {
            let observation = *observation;
            if matches!(observation, Observation::Discovered(_)) {
                self.discovered.insert(result.window);
            }
            if !matches!(observation, Observation::Settled { .. })
                || self.discovered.contains(&result.window)
            {
                #[cfg(test)]
                if matches!(observation, Observation::Settled { .. }) {
                    self.trace(NativeTrace::Settled(result.window));
                }
                self.observations.push(observation);
            }
        }
    }

    fn accept_placement(&mut self, window: HwndId, mut read: PlacementRead, now: Instant) {
        let previous = self.placement.get(&window);
        let mut deferred = !read.deferred_children.is_empty();
        read.deferred_children.clear();
        read.owned_windows
            .retain(|child| !self.retired.contains(&child.window));
        for child in &mut read.owned_windows {
            self.owners.insert(child.window, window);
            let may_read = {
                let mut schedule = self.schedule.lock();
                schedule
                    .windows
                    .entry(child.window)
                    .or_default()
                    .may_read_geometry(now)
            };
            if !may_read {
                deferred = true;
                child.offset = None;
            }
            if child.offset.is_none() {
                child.offset = previous
                    .and_then(|prior| {
                        prior
                            .owned_windows
                            .iter()
                            .find(|old| old.window == child.window)
                    })
                    .and_then(|old| old.offset);
            }
        }
        self.owners.retain(|child, parent| {
            *parent != window
                || read
                    .owned_windows
                    .iter()
                    .any(|owned| owned.window == *child)
        });
        #[cfg(test)]
        self.trace(NativeTrace::Placement(window, read.clone()));
        self.placement.insert(window, read);
        if deferred && let Some(state) = self.schedule.lock().windows.get_mut(&window) {
            state.placement.request();
        }
    }
}

impl Drop for WindowsWindowServer {
    fn drop(&mut self) {
        let mut schedule = self.schedule.lock();
        schedule.stopped = true;
        schedule.windows.clear();
    }
}

impl WindowServer for WindowsWindowServer {
    fn ingest_native_events(&mut self, _now: Instant) {
        if self.stopped {
            return;
        }
        while let Ok(event) = self.native_events.try_recv() {
            #[cfg(test)]
            self.trace(NativeTrace::Event(event));
            match event {
                NativeEvent::Discover(window) => {
                    self.retired.remove(&window);
                    self.schedule
                        .lock()
                        .windows
                        .entry(window)
                        .or_default()
                        .discovery
                        .request();
                    self.request_parent_placement(window);
                }
                NativeEvent::Gone(window) => {
                    self.request_parent_placement(window);
                    self.schedule.lock().windows.remove(&window);
                    self.placement.remove(&window);
                    self.discovered.remove(&window);
                    self.retired.insert(window);
                    self.owners
                        .retain(|child, parent| *child != window && *parent != window);
                    for placement in self.placement.values_mut() {
                        placement
                            .owned_windows
                            .retain(|child| child.window != window);
                    }
                    self.observations
                        .retain(|observation| observation.window() != Some(window));
                    self.observations.push(Observation::Gone(window));
                    if self.foreground == Some(window) {
                        self.foreground = None;
                    }
                }
                NativeEvent::Title(window) => {
                    if let Some(state) = self.schedule.lock().windows.get_mut(&window) {
                        state.title.request();
                    }
                }
                NativeEvent::Minimized(window) => {
                    self.observations.push(Observation::Minimized(window));
                }
                NativeEvent::Restored {
                    window,
                    observed_at,
                }
                | NativeEvent::MoveEnded {
                    window,
                    observed_at,
                } => {
                    self.retired.remove(&window);
                    let mut schedule = self.schedule.lock();
                    let state = schedule.windows.entry(window).or_default();
                    state.drag_started_at = None;
                    if self.discovered.contains(&window) {
                        state.settle_pending = Some(observed_at);
                    } else {
                        state.discovery.request();
                    }
                    drop(schedule);
                    self.request_parent_placement(window);
                }
                NativeEvent::Moved { window, .. } | NativeEvent::MoveStarted { window, .. } => {
                    self.request_parent_placement(window);
                }
                NativeEvent::Foreground {
                    window,
                    internal,
                    exclusive,
                } => {
                    self.foreground = Some(window);
                    self.observations
                        .push(Observation::ExclusiveFullscreen(exclusive));
                    if !internal {
                        self.observations.push(Observation::Foreground { window });
                    }
                }
            }
        }
    }

    fn ingest_read_results(&mut self, now: Instant) {
        if self.stopped {
            return;
        }
        while let Ok(result) = self.results.try_recv() {
            self.accept_result(result, now);
        }
    }

    fn read_settled_windows(&mut self, now: Instant) {
        if self.stopped {
            return;
        }
        let jobs = {
            let mut schedule = self.schedule.lock();
            let blocked_parents: HashSet<_> = self
                .owners
                .iter()
                .filter_map(|(child, parent)| {
                    schedule
                        .windows
                        .get(child)
                        .filter(|state| !state.may_read_geometry(now))
                        .map(|_| *parent)
                })
                .collect();
            let mut jobs = Vec::new();
            for (&window, state) in &mut schedule.windows {
                for kind in state.take_ready(now) {
                    if kind == InspectionKind::Placement && blocked_parents.contains(&window) {
                        state.complete(kind, true);
                    } else if matches!(kind, InspectionKind::Settle { .. })
                        && !self.discovered.contains(&window)
                    {
                        state.complete(kind, false);
                    } else {
                        jobs.push((window, kind));
                    }
                }
            }
            jobs
        };
        if let Some(dispatcher) = &self.dispatcher {
            for (window, kind) in jobs {
                dispatcher.submit(window, kind);
            }
        }
    }

    fn has_observations(&self) -> bool {
        !self.observations.is_empty()
    }
    fn take_observations(&mut self) -> Vec<Observation> {
        std::mem::take(&mut self.observations)
    }
    fn next_deadline(&self) -> Option<Instant> {
        self.schedule.next_deadline()
    }

    fn observe_displays(&mut self, monitors: &[MonitorInfo]) {
        if !self.stopped && !monitors.is_empty() {
            self.displays = monitors.to_vec();
        }
    }

    fn request_size_limits(&mut self, demand: Vec<SizeLimitRead>) {
        if self.stopped {
            return;
        }
        let mut schedule = self.schedule.lock();
        let mut requested = false;
        for demand in demand {
            if let Some(state) = schedule.windows.get_mut(&demand.window) {
                state.size_limits.request();
                requested = true;
            }
        }
        drop(schedule);
        if requested {
            (self.native_sender.wake)();
        }
    }

    fn read_corner_radii(&mut self, windows: &[HwndId]) -> Vec<Observation> {
        if self.stopped {
            return Vec::new();
        }
        let read = || {
            windows
                .iter()
                .map(|&window| Observation::CornerRadius {
                    window,
                    radius: handle::read_corner_radius(window),
                })
                .collect()
        };
        #[cfg(test)]
        {
            handle::submission::scene_queries(read)
        }
        #[cfg(not(test))]
        {
            read()
        }
    }

    fn invalidate_foreground(&mut self) {
        self.foreground = None;
    }
    fn set_frame(&mut self, window: HwndId, frame: PixelRect<Physical>) {
        if !self.stopped
            && let Some(read) = self.placement.get(&window)
        {
            handle::submit_frame(window, frame, read, &self.displays);
        }
    }
    fn set_z_order(&mut self, window: HwndId, z: ZOrder) {
        if !self.stopped {
            handle::submit_z_order(window, z);
        }
    }
    fn move_offscreen(&mut self, window: HwndId) {
        if !self.stopped {
            handle::move_window_offscreen(window.into());
        }
    }
    fn minimize(&mut self, window: HwndId) {
        if !self.stopped {
            handle::submit_show(window, false);
        }
    }
    fn unminimize(&mut self, window: HwndId) {
        if !self.stopped {
            handle::submit_show(window, true);
        }
    }
    fn close(&mut self, window: HwndId) {
        if !self.stopped {
            handle::submit_close(window);
        }
    }
    fn focus(&mut self, window: HwndId) {
        if !self.stopped && self.foreground != Some(window) {
            handle::submit_foreground(window);
        }
    }
    fn recover(&mut self, window: HwndId, was_maximized: bool) {
        handle::submit_recovery(window, was_maximized);
    }
    fn shutdown(&mut self, recovery: Vec<RecoveryEntry>) {
        if self.stopped {
            return;
        }
        self.stopped = true;
        {
            let mut schedule = self.schedule.lock();
            schedule.stopped = true;
            schedule.windows.clear();
        }
        self.observations.clear();
        self.foreground = None;
        self.dispatcher.take();
        for entry in recovery {
            self.recover(entry.window, entry.is_maximized);
        }
    }
}
