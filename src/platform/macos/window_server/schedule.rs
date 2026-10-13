use std::collections::VecDeque;
use std::time::{Duration, Instant};

use super::{ApplicationEvent, DebounceBurst, InspectionKind, PositionReadOrigin};
use crate::platform::macos::dispatcher::{ApplicationCommand, ApplicationOperation};

pub(in crate::platform::macos) const QUIET_INTERVAL: Duration = Duration::from_millis(300);

pub(in crate::platform::macos) fn geometry_read(kind: InspectionKind) -> bool {
    matches!(
        kind,
        InspectionKind::AppWindows | InspectionKind::Positions(_) | InspectionKind::Space
    )
}

pub(in crate::platform::macos) fn record_movement(
    movement: &mut Option<DebounceBurst>,
    observed_at: Instant,
) {
    match movement {
        Some(burst) if observed_at <= burst.last + QUIET_INTERVAL => {
            burst.first = burst.first.min(observed_at);
            burst.last = burst.last.max(observed_at);
        }
        _ => {
            *movement = Some(DebounceBurst {
                first: observed_at,
                last: observed_at,
            })
        }
    }
}

pub(in crate::platform::macos) struct ApplicationReadSchedule {
    hidden: bool,
    suspended: bool,
    movement: Option<DebounceBurst>,
    quiet_until: Option<Instant>,
    pending: PendingOperations,
}

struct PendingOperations {
    accepting: bool,
    operations: VecDeque<ApplicationOperation>,
}

impl ApplicationReadSchedule {
    pub(in crate::platform::macos) fn new(hidden: bool, suspended: bool) -> Self {
        let mut schedule = Self {
            hidden,
            suspended,
            movement: None,
            quiet_until: None,
            pending: PendingOperations {
                accepting: true,
                operations: VecDeque::new(),
            },
        };
        schedule
            .pending
            .push(ApplicationOperation::RegisterObserver);
        schedule
    }

    pub(in crate::platform::macos) fn collect(&mut self, command: ApplicationCommand) {
        if !self.pending.accepting {
            return;
        }
        match command {
            ApplicationCommand::Read(kind) => self.read(kind),
            ApplicationCommand::Operation(operation) => self.pending.push(operation),
            ApplicationCommand::Shutdown(recovery) => {
                self.prune();
                self.pending.accepting = false;
                self.pending
                    .operations
                    .push_back(ApplicationOperation::Restore(recovery));
                self.pending
                    .operations
                    .push_back(ApplicationOperation::Terminate);
            }
            ApplicationCommand::Terminate => {
                self.prune();
                self.pending.accepting = false;
                self.pending
                    .operations
                    .push_back(ApplicationOperation::Terminate);
            }
            ApplicationCommand::Event(event) => self.event(event),
        }
    }

    fn event(&mut self, event: ApplicationEvent) {
        match event {
            ApplicationEvent::MovedOrResized { observed_at } => {
                record_movement(&mut self.movement, observed_at);
                self.quiet_until = self.movement.map(|burst| burst.last + QUIET_INTERVAL);
            }
            ApplicationEvent::Hidden => {
                self.hidden = true;
                self.prune();
            }
            ApplicationEvent::Shown => {
                self.hidden = false;
                self.refresh();
            }
            ApplicationEvent::Suspended(suspended) => {
                self.suspended = suspended;
                if !suspended {
                    self.refresh();
                }
            }
            ApplicationEvent::WindowsChanged => self.read(InspectionKind::AppWindows),
            ApplicationEvent::FocusChanged => self.read(InspectionKind::FocusedWindow),
            ApplicationEvent::TitleChanged(window) => self.read(InspectionKind::Title(window)),
            ApplicationEvent::TitleElementChanged(element) => self
                .pending
                .push(ApplicationOperation::ResolveTitle(element)),
            ApplicationEvent::SpaceChanged => self.read(InspectionKind::Space),
            ApplicationEvent::Refresh => self.refresh(),
        }
    }

    fn refresh(&mut self) {
        self.pending.push(ApplicationOperation::RegisterObserver);
        self.read(InspectionKind::AppWindows);
        self.read(InspectionKind::Positions(PositionReadOrigin::Periodic));
    }

    fn read(&mut self, kind: InspectionKind) {
        self.pending.push(ApplicationOperation::Read(kind));
    }

    fn prune(&mut self) {
        self.pending
            .operations
            .retain(|operation| matches!(operation, ApplicationOperation::RemoveObserver(_)));
        self.quiet_until = None;
        self.movement = None;
    }

    pub(in crate::platform::macos) fn consume_quiet_deadline(&mut self, now: Instant) {
        if self.quiet_until.is_some_and(|deadline| now >= deadline) {
            self.quiet_until = None;
            if let Some(burst) = self.movement {
                self.read(InspectionKind::Positions(PositionReadOrigin::Settle(burst)));
            }
        }
    }

    pub(in crate::platform::macos) fn take_next_eligible(
        &mut self,
        now: Instant,
    ) -> Option<ApplicationOperation> {
        let index =
            self.pending
                .operations
                .iter()
                .position(|operation| match operation {
                    ApplicationOperation::Read(kind) => {
                        !self.hidden
                            && !self.suspended
                            && (!geometry_read(*kind)
                                || self
                                    .movement
                                    .is_none_or(|burst| now >= burst.last + QUIET_INTERVAL))
                    }
                    ApplicationOperation::ResolveTitle(_)
                    | ApplicationOperation::RegisterObserver => !self.hidden && !self.suspended,
                    _ => true,
                })?;
        self.pending.operations.remove(index)
    }

    pub(in crate::platform::macos) fn permits_reads(&self) -> bool {
        !self.hidden && !self.suspended
    }

    pub(in crate::platform::macos) fn next_deadline(&self) -> Option<Instant> {
        self.quiet_until
    }

    pub(in crate::platform::macos) fn disconnected(&mut self) {
        if self.hidden || self.suspended {
            self.pending.operations.retain(|operation| {
                !matches!(
                    operation,
                    ApplicationOperation::Read(_)
                        | ApplicationOperation::ResolveTitle(_)
                        | ApplicationOperation::RegisterObserver
                )
            });
            self.quiet_until = None;
        }
    }
}

impl PendingOperations {
    fn push(&mut self, mut operation: ApplicationOperation) {
        use ApplicationOperation::*;
        if matches!(operation, Close(_) | Unminimize(_)) {
            if self
                .operations
                .back()
                .is_some_and(|previous| match (previous, &operation) {
                    (Close(a), Close(b)) | (Unminimize(a), Unminimize(b)) => a.cg_id() == b.cg_id(),
                    _ => false,
                })
            {
                return;
            }
        } else if !matches!(
            operation,
            RegisterObserver | RemoveObserver(_) | Terminate | Restore(_)
        ) {
            for index in (0..self.operations.len()).rev() {
                let previous = &self.operations[index];
                if matches!(
                    previous,
                    Close(_)
                        | Unminimize(_)
                        | RegisterObserver
                        | RemoveObserver(_)
                        | Terminate
                        | Restore(_)
                ) {
                    break;
                }
                if setter_window(previous)
                    .zip(setter_window(&operation))
                    .is_some_and(|(a, b)| a == b)
                    || matches!((previous, &operation), (Focus(_), Focus(_)))
                {
                    self.operations[index] = operation;
                    return;
                }
                if let (Read(old), Read(new)) = (previous, &mut operation)
                    && same_read(*old, *new)
                {
                    if let (
                        InspectionKind::Positions(PositionReadOrigin::Settle(burst)),
                        InspectionKind::Positions(PositionReadOrigin::Periodic),
                    ) = (*old, *new)
                    {
                        *new = InspectionKind::Positions(PositionReadOrigin::Settle(burst));
                    }
                    self.operations.remove(index);
                    break;
                }
            }
        }
        self.operations.push_back(operation);
    }
}

fn setter_window(operation: &ApplicationOperation) -> Option<u32> {
    use ApplicationOperation::*;
    match operation {
        SetFrame { window, .. }
        | MoveOffscreen { window, .. }
        | Minimize(window)
        | Recover { window, .. } => Some(window.cg_id()),
        _ => None,
    }
}

fn same_read(a: InspectionKind, b: InspectionKind) -> bool {
    match (a, b) {
        (InspectionKind::Positions(_), InspectionKind::Positions(_)) => true,
        _ => a == b,
    }
}
