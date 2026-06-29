use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{PostQuitMessage, PostThreadMessageW, WM_QUIT};

use crate::action::{Action, Actions};
use crate::keymap::KeymapState;
use crate::platform::windows::WM_APP_DISPATCH_RESULT;
use crate::platform::windows::dome::rejection_log_filter::{RejectionLogFilter, RejectionReason};
use crate::platform::windows::dome::{Dome, HubEvent, NewWindow, WindowsMetadata};
use crate::platform::windows::external::{HwndId, InspectExternalWindow, ManageExternalWindow};
use crate::platform::windows::handle::ExternalHwnd;
use crate::platform::windows::throttle::{Throttle, ThrottleResult};
use crate::platform::windows::timer_registry::{TimerKind, TimerRegistry, Win32Timer};

const FOCUS_THROTTLE_INTERVAL: Duration = Duration::from_millis(500);
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_TIMEOUT: Duration = Duration::from_secs(60);
const PRUNE_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub(super) struct Runner {
    dome: Dome,
    dispatcher: ReadDispatcher,
    focus_throttle: Throttle<HwndId>,
    // Drop order: timers is dropped after dome/dispatcher because the message loop
    // has already exited by Runner drop, so no field dropped earlier can re-enter
    // the registry. KillTimer is idempotent on already-fired one-shots.
    timers: TimerRegistry,
    main_thread_id: u32,
    keymap_state: Arc<RwLock<KeymapState>>,
    rejection_log_filter: Arc<RejectionLogFilter>,
}

impl Runner {
    pub(super) fn new(
        dome: Dome,
        thread_id: u32,
        main_thread_id: u32,
        keymap_state: Arc<RwLock<KeymapState>>,
    ) -> Self {
        let mut timers = TimerRegistry::new(Box::new(Win32Timer));
        timers.schedule_prune(PRUNE_INTERVAL);
        Self {
            dome,
            dispatcher: ReadDispatcher::new(thread_id),
            focus_throttle: Throttle::new(FOCUS_THROTTLE_INTERVAL),
            timers,
            main_thread_id,
            keymap_state,
            rejection_log_filter: Arc::new(RejectionLogFilter::new()),
        }
    }

    pub(super) fn handle_timer(&mut self, timer_id: usize) {
        let Some(kind) = self.timers.dispatch(timer_id) else {
            return;
        };
        match kind {
            TimerKind::Focus => {
                if let Some(id) = self.focus_throttle.flush() {
                    self.dome.handle_focus(id);
                    self.dome.apply_layout();
                }
            }
            TimerKind::MoveSettle { hwnd, observed_at } => {
                self.dome.clear_move_state(hwnd);
                self.dispatch_placement_read(hwnd, observed_at);
            }
            TimerKind::Prune => {
                self.rejection_log_filter.prune(Instant::now());
            }
        }
    }

    pub(super) fn handle_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Shutdown => {
                tracing::info!("Shutdown requested");
                unsafe { PostQuitMessage(0) };
            }
            HubEvent::ConfigChanged(c) => {
                self.dome.config_changed(*c);
            }
            HubEvent::LayoutConfigChanged(c) => {
                self.dome.layout_changed(*c);
            }
            HubEvent::WindowCreated(hwnd_id) => {
                self.dispatch_window_created(hwnd_id);
            }
            HubEvent::WindowDestroyed(hwnd_id) => {
                self.dome.window_destroyed(hwnd_id);
            }
            HubEvent::WindowMinimized(hwnd_id) => {
                self.dome.window_minimized(hwnd_id);
            }
            HubEvent::WindowRestored {
                hwnd_id,
                observed_at,
            } => {
                self.dispatch_placement_read(hwnd_id, observed_at);
            }
            HubEvent::WindowFocused(hwnd_id) => match self.focus_throttle.submit(hwnd_id) {
                ThrottleResult::Send(id) => {
                    self.dome.handle_focus(id);
                }
                ThrottleResult::Pending => {}
                ThrottleResult::ScheduleFlush(delay) => {
                    self.focus_throttle.mark_timer_scheduled();
                    self.timers.schedule_focus(delay);
                }
            },
            HubEvent::MoveSizeStart(hwnd_id) => {
                self.dome.move_size_started(hwnd_id);
                // schedule_move_settle cancels any existing entry internally. The move-size-started
                // notification is independent of the cancel and may run first.
                self.timers
                    .schedule_move_settle(hwnd_id, Instant::now(), DRAG_SAFETY_TIMEOUT);
            }
            HubEvent::MoveSizeEnd {
                hwnd_id,
                observed_at,
            } => {
                self.timers.cancel_move_settle(hwnd_id);
                self.dome.clear_move_state(hwnd_id);
                self.dispatch_placement_read(hwnd_id, observed_at);
            }
            HubEvent::LocationChanged {
                hwnd_id,
                observed_at,
            } => {
                if self.dome.location_changed(hwnd_id) {
                    self.timers
                        .schedule_move_settle(hwnd_id, observed_at, DEBOUNCE_INTERVAL);
                }
            }
            HubEvent::WindowTitleChanged(hwnd_id) => {
                let inspect: Arc<dyn InspectExternalWindow> =
                    Arc::new(ExternalHwnd::new(hwnd_id.into()));
                self.dispatcher.dispatch(
                    move || inspect.get_window_title(),
                    move |title, runner| {
                        runner.dome.update_titles(vec![(hwnd_id, title)]);
                    },
                );
            }
            HubEvent::Action(a) => {
                self.handle_actions(&a);
            }
            HubEvent::Query { query, sender } => {
                let json = match query {
                    crate::action::Query::Workspaces => self.dome.query_workspaces_json(),
                };
                if sender.send(json).is_err() {
                    tracing::debug!("Query response dropped -- receiver gone");
                }
            }
            HubEvent::TabClicked(id, idx) => {
                self.dome.tab_clicked(id, idx);
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_actions(&mut self, actions: &Actions) {
        for action in actions {
            match action {
                Action::Focus(t) => {
                    self.dome.apply_focus(t);
                }
                Action::Move(t) => {
                    self.dome.apply_move(t);
                }
                Action::Toggle(t) => {
                    self.dome.apply_toggle(t);
                }
                Action::Master(t) => {
                    self.dome.apply_master(t);
                }
                Action::ToggleMinimized => {
                    self.dome.toggle_picker();
                    if self.dome.picker_visible() {
                        self.dispatch_picker_icons();
                    }
                }
                Action::Exec { command } => {
                    if let Err(e) = crate::platform::windows::spawn::spawn(command) {
                        tracing::warn!(%command, "Failed to exec: {e:#}");
                    }
                }
                Action::Exit => {
                    unsafe {
                        PostThreadMessageW(self.main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok()
                    };
                    unsafe { PostQuitMessage(0) };
                }
                Action::UnminimizeWindow(id) => {
                    self.dome.picker_unminimize_window(*id);
                }
                Action::Mode { name } => {
                    self.keymap_state.write().unwrap().switch_mode(name);
                    tracing::debug!(mode = %name, "Switching to mode");
                }
            }
        }
        self.dome.apply_layout();
    }

    pub(super) fn dispatch_window_created(&mut self, hwnd_id: HwndId) {
        let ext = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        let inspect: Arc<dyn InspectExternalWindow> = ext.clone();
        let manage: Arc<dyn ManageExternalWindow> = ext;
        let log_filter = Arc::clone(&self.rejection_log_filter);
        let ignore_rules = self.dome.ignore_rules().to_vec();
        self.dispatcher.dispatch(
            move || {
                if let Some(reason) = inspect.check_unmanageable() {
                    let title = inspect.get_window_title();
                    let pid = manage.pid();
                    if pid == 0 {
                        // Zombie HWND: GetWindowThreadProcessId returned 0. No
                        // stable key for dedup, log unconditionally.
                        tracing::trace!(?hwnd_id, ?title, ?reason, "not manageable");
                    } else if log_filter.record_and_should_log(hwnd_id, pid, reason, Instant::now())
                    {
                        tracing::trace!(?hwnd_id, ?title, ?reason, "not manageable");
                    }
                    return None;
                }
                let class = inspect.get_class_name();
                let aumid = inspect.get_aumid();
                let process = inspect.get_process_name().unwrap_or_default();
                let title = inspect.get_window_title();
                let matched = ignore_rules.iter().find(|r| {
                    r.matches(
                        &process,
                        title.as_deref(),
                        class.as_deref(),
                        aumid.as_deref(),
                    )
                });
                if let Some(_rule) = matched {
                    let pid = manage.pid();
                    let reason = RejectionReason::IgnoredByRule;
                    if pid == 0 {
                        tracing::trace!(?hwnd_id, ?reason, "not manageable");
                    } else if log_filter.record_and_should_log(hwnd_id, pid, reason, Instant::now())
                    {
                        tracing::trace!(?hwnd_id, ?reason, "not manageable");
                    }
                    return None;
                }
                Some((
                    NewWindow {
                        ext: manage,
                        metadata: WindowsMetadata {
                            title,
                            process,
                            class,
                            aumid,
                            app_name: inspect.get_app_display_name(),
                        },
                        constraints: inspect.get_size_constraints(),
                    },
                    inspect.get_visible_rect(),
                    inspect.get_monitor(),
                ))
            },
            move |result, runner| {
                let Some((new, rect, monitor)) = result else {
                    return;
                };
                runner.dome.add_window(new, rect, monitor);
            },
        );
    }

    fn dispatch_placement_read(&mut self, hwnd_id: HwndId, observed_at: Instant) {
        let inspect: Arc<dyn InspectExternalWindow> = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        self.dispatcher.dispatch(
            move || {
                if inspect.is_minimized() {
                    return None;
                }
                let rect = inspect.get_visible_rect();
                let monitor = inspect.get_monitor();
                Some((rect, monitor))
            },
            move |observation, runner| {
                let Some((rect, monitor)) = observation else {
                    return;
                };
                runner
                    .dome
                    .handle_window_moved(hwnd_id, rect, monitor, observed_at);
            },
        );
    }

    pub(super) fn handle_display_change(&mut self) {
        let to_refresh = self.dome.handle_display_change();
        for hwnd_id in to_refresh {
            self.dispatch_constraint_read(hwnd_id);
        }
        self.dome.apply_layout();
    }

    pub(super) fn handle_dpi_change(&mut self, handle: isize, dpi: u32) {
        self.dome.monitor_dpi_changed(handle, dpi);
        // apply_layout is idempotent: runs even when monitor_dpi_changed
        // early-returns on same-scale, because stored targets are physical
        // and Hub state is unchanged so positions match.
        self.dome.apply_layout();
    }

    fn dispatch_constraint_read(&mut self, hwnd_id: HwndId) {
        let inspect: Arc<dyn InspectExternalWindow> = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        self.dispatcher.dispatch(
            move || inspect.get_size_constraints(),
            move |constraints, runner| {
                runner.dome.set_constraints_for(hwnd_id, constraints);
                runner.dome.apply_layout();
            },
        );
    }

    fn dispatch_picker_icons(&mut self) {
        let to_load = self.dome.picker_icons_to_load();
        let scale = self.dome.picker_scale().unwrap_or_else(|| {
            panic!("dispatch_picker_icons: picker_visible() was true but picker_scale() returned None -- picker state desynced")
        });
        for (app_id, hwnd_id) in to_load {
            self.dispatcher.dispatch(
                move || {
                    let hwnd = windows::Win32::Foundation::HWND::from(hwnd_id);
                    crate::platform::windows::dome::icon::load_app_icon(hwnd, scale)
                        .map(|image| (app_id, image))
                },
                move |result, runner| {
                    if let Some((app_id, image)) = result {
                        runner.dome.picker_receive_icon(app_id, image);
                        runner.dome.picker_rerender();
                    }
                },
            );
        }
    }
}

pub(super) type ApplyFn = Box<dyn FnOnce(&mut Runner)>;

struct ReadDispatcher {
    pool: rayon::ThreadPool,
    thread_id: u32,
}

impl ReadDispatcher {
    fn new(thread_id: u32) -> Self {
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(50)
            .thread_name(|i| format!("dome-read-{i}"))
            .start_handler(|_| {
                // AUMID lookup via SHGetPropertyStoreForWindow requires COM.
                // MTA is correct here: read-pool workers do not pump messages.
                use windows::Win32::System::Com::{COINIT_MULTITHREADED, CoInitializeEx};
                let hr = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
                debug_assert!(
                    hr.is_ok(),
                    "CoInitializeEx failed on read pool worker: {hr:?}"
                );
            })
            .build()
            .expect("Failed to create read dispatcher thread pool");
        Self { pool, thread_id }
    }

    fn dispatch<W, R, A>(&self, work: W, apply: A)
    where
        W: FnOnce() -> R + Send + 'static,
        R: Send + 'static,
        A: FnOnce(R, &mut Runner) + Send + 'static,
    {
        let thread_id = self.thread_id;
        self.pool.spawn(move || {
            let result = work();
            let boxed: ApplyFn = Box::new(move |runner| apply(result, runner));
            let ptr = Box::into_raw(Box::new(boxed)) as usize;
            unsafe {
                if PostThreadMessageW(thread_id, WM_APP_DISPATCH_RESULT, WPARAM(ptr), LPARAM(0))
                    .is_err()
                {
                    drop(Box::from_raw(ptr as *mut ApplyFn));
                }
            }
        });
    }
}
