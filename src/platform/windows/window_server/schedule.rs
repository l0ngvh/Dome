use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use super::{HwndId, InspectionKind};

const QUIET_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_INTERVAL: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct InspectionProgress {
    pub pending: bool,
    pub in_flight: bool,
}

impl InspectionProgress {
    pub fn request(&mut self) {
        self.pending = true;
    }

    fn take(&mut self, eligible: bool) -> bool {
        if !eligible || !self.pending || self.in_flight {
            return false;
        }
        self.pending = false;
        self.in_flight = true;
        true
    }
}

pub(super) struct MovementWait {
    pub observed_at: Instant,
    pub quiet_until: Instant,
}

#[derive(Default)]
pub(super) struct WindowInspectionState {
    pub movement: Option<MovementWait>,
    pub drag_started_at: Option<Instant>,
    pub settle_pending: Option<Instant>,
    pub discovery: InspectionProgress,
    pub title: InspectionProgress,
    pub size_limits: InspectionProgress,
    pub placement: InspectionProgress,
    settle_in_flight: bool,
}

impl WindowInspectionState {
    pub fn may_read_geometry(&self, now: Instant) -> bool {
        self.movement
            .as_ref()
            .is_none_or(|wait| now >= wait.quiet_until)
    }

    pub fn moved(&mut self, observed_at: Instant) {
        if self
            .movement
            .as_ref()
            .is_none_or(|wait| observed_at >= wait.observed_at)
        {
            self.movement = Some(MovementWait {
                observed_at,
                quiet_until: observed_at + QUIET_INTERVAL,
            });
        }
    }

    pub fn progress(&mut self, kind: InspectionKind) -> Option<&mut InspectionProgress> {
        match kind {
            InspectionKind::Discovery => Some(&mut self.discovery),
            InspectionKind::Title => Some(&mut self.title),
            InspectionKind::SizeLimits => Some(&mut self.size_limits),
            InspectionKind::Placement => Some(&mut self.placement),
            InspectionKind::Settle { .. } => None,
        }
    }

    pub fn complete(&mut self, kind: InspectionKind, renew: bool) {
        if let Some(progress) = self.progress(kind) {
            progress.in_flight = false;
            progress.pending |= renew;
        } else if let InspectionKind::Settle { observed_at } = kind {
            self.settle_in_flight = false;
            if renew {
                self.settle_pending = Some(
                    self.settle_pending
                        .map_or(observed_at, |at| at.max(observed_at)),
                );
            }
        }
    }

    pub fn take_ready(&mut self, now: Instant) -> Vec<InspectionKind> {
        if self
            .movement
            .as_ref()
            .is_some_and(|wait| now >= wait.quiet_until)
        {
            let wait = self.movement.take().unwrap();
            self.settle_pending = Some(
                self.settle_pending
                    .map_or(wait.observed_at, |at| at.max(wait.observed_at)),
            );
        }
        if self
            .drag_started_at
            .is_some_and(|at| now >= at + DRAG_SAFETY_INTERVAL)
        {
            let at = self.drag_started_at.take().unwrap();
            self.settle_pending = Some(self.settle_pending.map_or(at, |pending| pending.max(at)));
        }
        let may_read = self.may_read_geometry(now);
        let mut jobs = Vec::new();
        for kind in [
            InspectionKind::Discovery,
            InspectionKind::Title,
            InspectionKind::SizeLimits,
            InspectionKind::Placement,
        ] {
            if self
                .progress(kind)
                .unwrap()
                .take(kind == InspectionKind::Title || may_read)
            {
                jobs.push(kind);
            }
        }
        if may_read
            && !self.settle_in_flight
            && let Some(observed_at) = self.settle_pending.take()
        {
            self.settle_in_flight = true;
            jobs.push(InspectionKind::Settle { observed_at });
        }
        jobs
    }

    fn next_deadline(&self) -> Option<Instant> {
        self.movement
            .as_ref()
            .map(|wait| wait.quiet_until)
            .into_iter()
            .chain(self.drag_started_at.map(|at| at + DRAG_SAFETY_INTERVAL))
            .min()
    }
}

#[derive(Default)]
pub(super) struct InspectionSchedule {
    pub windows: HashMap<HwndId, WindowInspectionState>,
    pub stopped: bool,
}

#[derive(Clone, Default)]
pub(super) struct SharedSchedule(Arc<Mutex<InspectionSchedule>>);

impl SharedSchedule {
    pub fn lock(&self) -> MutexGuard<'_, InspectionSchedule> {
        match self.0.lock() {
            Ok(schedule) => schedule,
            Err(poisoned) => {
                let mut schedule = poisoned.into_inner();
                schedule.windows.clear();
                schedule.stopped = true;
                schedule
            }
        }
    }

    pub fn may_read(&self, window: HwndId, now: Instant) -> Option<bool> {
        let schedule = self.lock();
        if schedule.stopped {
            return None;
        }
        schedule
            .windows
            .get(&window)
            .map(|state| state.may_read_geometry(now))
    }

    pub fn next_deadline(&self) -> Option<Instant> {
        self.lock()
            .windows
            .values()
            .filter_map(WindowInspectionState::next_deadline)
            .min()
    }
}
