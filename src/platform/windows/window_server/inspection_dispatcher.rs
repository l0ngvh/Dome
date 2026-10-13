use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::{Arc, mpsc};
use std::time::Instant;

use super::schedule::SharedSchedule;
use super::{
    DiscoveredWindow, HwndId, InspectionKind, InspectionOutcome, InspectionResult, Observation,
    SettleRead,
};
use crate::platform::windows::dome::WindowsMetadata;
use crate::platform::windows::external::InspectExternalWindow;
use crate::platform::windows::handle::{self, ExternalHwnd};

thread_local! {
    static COMPONENT_OBJECT_MODEL_INITIALIZED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

pub(super) struct InspectionDispatcher {
    pool: rayon::ThreadPool,
    sender: mpsc::Sender<InspectionResult>,
    schedule: SharedSchedule,
    wake: Arc<dyn Fn() + Send + Sync>,
    #[cfg(test)]
    primitives: Option<Arc<dyn Fn(HwndId, InspectionKind) -> InspectionOutcome + Send + Sync>>,
    #[cfg(test)]
    native_reads: Option<handle::native::fixtures::Reads>,
    #[cfg(test)]
    before_gate: Option<Arc<dyn Fn() + Send + Sync>>,
    #[cfg(test)]
    after_job: Option<Arc<dyn Fn(bool, bool) + Send + Sync>>,
}

impl InspectionDispatcher {
    pub fn new(
        sender: mpsc::Sender<InspectionResult>,
        schedule: SharedSchedule,
        wake: Arc<dyn Fn() + Send + Sync>,
    ) -> anyhow::Result<Self> {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(50)
            .thread_name(|index| format!("dome-inspection-{index}"))
            .start_handler(|_| {
                use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
                let result = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                COMPONENT_OBJECT_MODEL_INITIALIZED.set(result.is_ok());
                if result.is_err() {
                    tracing::warn!(?result, "Inspection COM initialization failed");
                }
            })
            .exit_handler(|_| {
                if COMPONENT_OBJECT_MODEL_INITIALIZED.replace(false) {
                    unsafe { windows::Win32::System::Com::CoUninitialize() };
                }
            })
            .build()?;
        Ok(Self {
            pool,
            sender,
            schedule,
            wake,
            #[cfg(test)]
            primitives: None,
            #[cfg(test)]
            native_reads: None,
            #[cfg(test)]
            before_gate: None,
            #[cfg(test)]
            after_job: None,
        })
    }

    pub fn submit(&self, window: HwndId, kind: InspectionKind) {
        match kind {
            InspectionKind::Discovery => self.discover(window),
            InspectionKind::Title => self.title(window),
            InspectionKind::SizeLimits => self.size_limits(window),
            InspectionKind::Placement => self.placement(window),
            InspectionKind::Settle { observed_at } => self.settle(window, observed_at),
        }
    }

    pub fn discover(&self, window: HwndId) {
        self.spawn(window, InspectionKind::Discovery);
    }
    pub fn title(&self, window: HwndId) {
        self.spawn(window, InspectionKind::Title);
    }
    pub fn size_limits(&self, window: HwndId) {
        self.spawn(window, InspectionKind::SizeLimits);
    }
    pub fn placement(&self, window: HwndId) {
        self.spawn(window, InspectionKind::Placement);
    }
    fn settle(&self, window: HwndId, observed_at: Instant) {
        self.spawn(window, InspectionKind::Settle { observed_at });
    }

    fn spawn(&self, window: HwndId, kind: InspectionKind) {
        let sender = self.sender.clone();
        let schedule = self.schedule.clone();
        let wake = self.wake.clone();
        #[cfg(test)]
        let primitives = self.primitives.clone();
        #[cfg(test)]
        let native_reads = self.native_reads.clone();
        #[cfg(test)]
        let before_gate = self.before_gate.clone();
        #[cfg(test)]
        let after_job = self.after_job.clone();
        self.pool.spawn(move || {
            let outcome = catch_unwind(AssertUnwindSafe(|| {
                #[cfg(test)]
                if let Some(before_gate) = before_gate {
                    before_gate();
                }
                match schedule.may_read(window, Instant::now()) {
                    None => return InspectionOutcome::Cancelled,
                    Some(false) if kind != InspectionKind::Title => {
                        return InspectionOutcome::Deferred;
                    }
                    _ => {}
                }
                #[cfg(test)]
                if let Some(primitives) = primitives {
                    return primitives(window, kind);
                }
                #[cfg(test)]
                if let Some(reads) = native_reads {
                    return handle::native::fixtures::with(reads, || {
                        read_concrete_job(window, kind, &schedule)
                    });
                }
                read_concrete_job(window, kind, &schedule)
            }))
            .unwrap_or(InspectionOutcome::Failed);
            #[cfg(test)]
            let read_completed = matches!(outcome, InspectionOutcome::Completed { .. });
            let delivered = sender
                .send(InspectionResult {
                    window,
                    kind,
                    outcome,
                })
                .is_ok();
            if delivered {
                wake();
            }
            #[cfg(test)]
            if let Some(after_job) = after_job {
                after_job(delivered, read_completed);
            }
        });
    }

    #[cfg(test)]
    pub(super) fn set_after_job(&mut self, after_job: Arc<dyn Fn(bool, bool) + Send + Sync>) {
        self.after_job = Some(after_job);
    }

    #[cfg(test)]
    pub(super) fn set_native_reads(&mut self, reads: handle::native::fixtures::Reads) {
        self.native_reads = Some(reads);
    }

    #[cfg(test)]
    pub(super) fn set_primitives(
        &mut self,
        primitives: Arc<dyn Fn(HwndId, InspectionKind) -> InspectionOutcome + Send + Sync>,
    ) {
        self.primitives = Some(primitives);
    }

    #[cfg(test)]
    pub(super) fn set_before_gate(&mut self, before_gate: Arc<dyn Fn() + Send + Sync>) {
        self.before_gate = Some(before_gate);
    }
}

fn read_concrete_job(
    window: HwndId,
    kind: InspectionKind,
    schedule: &SharedSchedule,
) -> InspectionOutcome {
    let inspect = ExternalHwnd::new(window.into());
    let mut placement = None;
    let mut owner = None;
    let observation = match kind {
        InspectionKind::Discovery => {
            owner = handle::read_root_owner(window);
            if inspect.check_unmanageable() {
                return InspectionOutcome::Completed {
                    observation: None,
                    placement: None,
                    owner,
                };
            }
            let Some(frame) = handle::read_visible_frame(window) else {
                return InspectionOutcome::Failed;
            };
            placement = handle::read_placement(window, |child| {
                schedule.may_read(child, Instant::now()) != Some(false)
            });
            let Some(_) = placement else {
                return InspectionOutcome::Failed;
            };
            Some(Observation::Discovered(DiscoveredWindow {
                key: window,
                process_id: inspect.process_id(),
                is_maximized: inspect.is_maximized(),
                metadata: WindowsMetadata {
                    title: inspect.get_window_title(),
                    process: inspect.get_process_name().unwrap_or_default(),
                    process_path: inspect.get_process_path().ok(),
                    class: inspect.get_class_name(),
                    aumid: inspect.get_aumid(),
                    app_name: inspect.get_app_display_name(),
                },
                frame,
                monitor: inspect.get_monitor(),
                limits: inspect.get_size_constraints(),
                fullscreen: None,
            }))
        }
        InspectionKind::Title => Some(Observation::Title {
            window,
            title: inspect.get_window_title(),
        }),
        InspectionKind::SizeLimits => {
            let Some(limits) = inspect.get_size_constraints() else {
                return InspectionOutcome::Failed;
            };
            Some(Observation::SizeLimits { window, limits })
        }
        InspectionKind::Placement => {
            placement = handle::read_placement(window, |child| {
                schedule.may_read(child, Instant::now()) != Some(false)
            });
            if placement.is_none() {
                return InspectionOutcome::Failed;
            }
            None
        }
        InspectionKind::Settle { observed_at } => {
            let minimized = inspect.is_minimized();
            let is_maximized = inspect.is_maximized();
            let geometry = if minimized {
                None
            } else {
                let Some(frame) = handle::read_visible_frame(window) else {
                    return InspectionOutcome::Failed;
                };
                placement = handle::read_placement(window, |child| {
                    schedule.may_read(child, Instant::now()) != Some(false)
                });
                if placement.is_none() {
                    return InspectionOutcome::Failed;
                }
                Some((frame, inspect.get_monitor()))
            };
            Some(Observation::Settled {
                window,
                read: SettleRead {
                    observed_at,
                    minimized,
                    is_maximized,
                    placement: geometry,
                },
            })
        }
    };
    InspectionOutcome::Completed {
        observation: observation.map(Box::new),
        placement,
        owner,
    }
}
