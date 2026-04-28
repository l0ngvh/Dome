use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, PostQuitMessage, PostThreadMessageW, SetTimer, WM_QUIT,
};

use crate::action::{Action, Actions};
use crate::platform::windows::WM_APP_DISPATCH_RESULT;
use crate::platform::windows::dome::{Dome, HubEvent, ObservedPosition};
use crate::platform::windows::external::{HwndId, InspectExternalHwnd, ManageExternalHwnd};
use crate::platform::windows::handle::ExternalHwnd;
use crate::platform::windows::throttle::{Throttle, ThrottleResult};

const FOCUS_THROTTLE_INTERVAL: Duration = Duration::from_millis(500);
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_TIMEOUT: Duration = Duration::from_secs(60);

enum TimerKind {
    FocusThrottle,
    PlacementDebounce(HwndId),
    DragSafety(HwndId),
}

pub(super) struct Runner {
    dome: Dome,
    dispatcher: ReadDispatcher,
    focus_throttle: Throttle<HwndId>,
    focus_timer_id: Option<usize>,
    window_timers: HashMap<HwndId, usize>,
    main_thread_id: u32,
}

impl Runner {
    pub(super) fn new(dome: Dome, thread_id: u32, main_thread_id: u32) -> Self {
        Self {
            dome,
            dispatcher: ReadDispatcher::new(thread_id),
            focus_throttle: Throttle::new(FOCUS_THROTTLE_INTERVAL),
            focus_timer_id: None,
            window_timers: HashMap::new(),
            main_thread_id,
        }
    }

    fn schedule_timer(&mut self, kind: TimerKind, delay: Duration) -> usize {
        // With hWnd=NULL, SetTimer ignores nIDEvent when it doesn't match an
        // existing timer and returns a new system-generated ID. Pass the
        // previous ID to replace an existing timer, or 0 to create a new one.
        let hint = match &kind {
            TimerKind::FocusThrottle => self.focus_timer_id.unwrap_or(0),
            _ => 0,
        };
        let id = unsafe { SetTimer(None, hint, delay.as_millis() as u32, None) };
        match &kind {
            TimerKind::FocusThrottle => self.focus_timer_id = Some(id),
            TimerKind::PlacementDebounce(hwnd) | TimerKind::DragSafety(hwnd) => {
                self.window_timers.insert(*hwnd, id);
            }
        }
        id
    }

    fn cancel_timer(&mut self, hwnd: &HwndId) {
        if let Some(id) = self.window_timers.remove(hwnd) {
            unsafe { KillTimer(None, id).ok() };
        }
    }

    pub(super) fn handle_timer(&mut self, timer_id: usize) {
        unsafe { KillTimer(None, timer_id).ok() };
        if self.focus_timer_id == Some(timer_id) {
            if let Some(id) = self.focus_throttle.flush() {
                self.dome.handle_focus(id);
                self.dome.apply_layout();
            }
            return;
        }
        let hwnd = self
            .window_timers
            .iter()
            .find(|(_, v)| **v == timer_id)
            .map(|(k, _)| *k);
        if let Some(hwnd) = hwnd {
            self.window_timers.remove(&hwnd);
            self.dome.placement_timeout(hwnd);
            self.dispatch_placement_read(hwnd);
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
                self.dome.apply_layout();
            }
            HubEvent::WindowCreated(hwnd_id) => {
                self.dispatch_window_created(hwnd_id);
            }
            HubEvent::WindowDestroyed(hwnd_id) => {
                self.dome.window_destroyed(hwnd_id);
                self.dome.apply_layout();
            }
            HubEvent::WindowMinimized(hwnd_id) => {
                self.dome.window_minimized(hwnd_id);
                self.dome.apply_layout();
            }
            HubEvent::WindowRestored(hwnd_id) => {
                self.dome.window_restored(hwnd_id);
                self.dome.apply_layout();
            }
            HubEvent::WindowFocused(hwnd_id) => match self.focus_throttle.submit(hwnd_id) {
                ThrottleResult::Send(id) => {
                    self.dome.handle_focus(id);
                    self.dome.apply_layout();
                }
                ThrottleResult::Pending => {}
                ThrottleResult::ScheduleFlush(delay) => {
                    self.focus_throttle.mark_timer_scheduled();
                    self.schedule_timer(TimerKind::FocusThrottle, delay);
                }
            },
            HubEvent::MoveSizeStart(hwnd_id) => {
                self.cancel_timer(&hwnd_id);
                self.dome.move_size_started(hwnd_id);
                self.schedule_timer(TimerKind::DragSafety(hwnd_id), DRAG_SAFETY_TIMEOUT);
            }
            HubEvent::MoveSizeEnd(hwnd_id) => {
                self.cancel_timer(&hwnd_id);
                self.dome.move_size_ended(hwnd_id);
                self.dome.placement_timeout(hwnd_id);
                self.dispatch_placement_read(hwnd_id);
            }
            HubEvent::LocationChanged(hwnd_id) => {
                if self.dome.location_changed(hwnd_id) {
                    self.cancel_timer(&hwnd_id);
                    self.schedule_timer(TimerKind::PlacementDebounce(hwnd_id), DEBOUNCE_INTERVAL);
                }
            }
            HubEvent::WindowTitleChanged(hwnd_id) => {
                if self.dome.registry_contains_hwnd(hwnd_id) {
                    let inspect: Arc<dyn InspectExternalHwnd> =
                        Arc::new(ExternalHwnd::new(hwnd_id.into()));
                    self.dispatcher.dispatch(
                        move || inspect.get_window_title(),
                        move |title, runner| {
                            if runner.dome.registry_contains_hwnd(hwnd_id) {
                                runner.dome.update_titles(vec![(hwnd_id, title)]);
                            }
                        },
                    );
                } else {
                    self.dispatch_window_created(hwnd_id);
                }
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
                self.dome.apply_layout();
            }
        }
    }

    #[tracing::instrument(skip(self))]
    fn handle_actions(&mut self, actions: &Actions) {
        for action in actions {
            match action {
                Action::Hub(hub) => {
                    self.dome.execute_hub_action(hub);
                    self.dome.apply_layout();
                }
                Action::Exec { command } => {
                    if let Err(e) = std::process::Command::new("cmd")
                        .args(["/C", command])
                        .spawn()
                    {
                        tracing::warn!(%command, "Failed to exec: {e}");
                    }
                }
                Action::Exit => {
                    unsafe {
                        PostThreadMessageW(self.main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok()
                    };
                    unsafe { PostQuitMessage(0) };
                }
                Action::ToggleMinimizePicker => {
                    self.dome.toggle_picker();
                    if self.dome.picker_visible() {
                        self.dispatch_picker_icons();
                    }
                }
                Action::UnminimizeWindow(id) => {
                    self.dome.picker_unminimize_window(*id);
                    self.dome.apply_layout();
                }
            }
        }
    }

    pub(super) fn dispatch_window_created(&mut self, hwnd_id: HwndId) {
        let ext = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        let inspect: Arc<dyn InspectExternalHwnd> = ext.clone();
        let manage: Arc<dyn ManageExternalHwnd> = ext;
        self.dispatcher.dispatch(
            move || {
                if !inspect.is_manageable() {
                    return None;
                }
                let observation = if inspect.is_fullscreen() {
                    ObservedPosition::Fullscreen
                } else {
                    let (x, y, w, h) = inspect.get_visible_rect();
                    ObservedPosition::Visible(x, y, w, h)
                };
                Some((
                    inspect.get_window_title(),
                    inspect.get_process_name().unwrap_or_default(),
                    inspect.get_size_constraints(),
                    observation,
                ))
            },
            move |result, runner| {
                let Some((title, process, constraints, observation)) = result else {
                    return;
                };
                if runner.dome.registry_contains_hwnd(manage.id()) {
                    return;
                }
                let actions =
                    runner
                        .dome
                        .try_manage_window(manage, title, process, constraints, observation);
                // Flush unconditionally: try_manage_window may have inserted a
                // window even when returning None (inserted but no on_open rules).
                runner.dome.apply_layout();
                if let Some(actions) = actions {
                    runner.handle_actions(&actions);
                }
            },
        );
    }

    fn dispatch_placement_read(&mut self, hwnd_id: HwndId) {
        let Some(id) = self.dome.registry_get_id(hwnd_id) else {
            return;
        };
        let inspect: Arc<dyn InspectExternalHwnd> = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        self.dispatcher.dispatch(
            move || {
                if inspect.is_fullscreen() {
                    ObservedPosition::Fullscreen
                } else {
                    let (x, y, w, h) = inspect.get_visible_rect();
                    ObservedPosition::Visible(x, y, w, h)
                }
            },
            move |observation, runner| {
                if runner.dome.registry_get_id(hwnd_id) != Some(id) {
                    return;
                }
                runner.dome.window_moved(hwnd_id, observation);
                runner.dome.apply_layout();
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

    fn dispatch_constraint_read(&mut self, hwnd_id: HwndId) {
        let Some(id) = self.dome.registry_get_id(hwnd_id) else {
            return;
        };
        let inspect: Arc<dyn InspectExternalHwnd> = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        self.dispatcher.dispatch(
            move || inspect.get_size_constraints(),
            move |constraints, runner| {
                if runner.dome.registry_get_id(hwnd_id) != Some(id) {
                    return;
                }
                runner.dome.set_constraints(id, constraints);
                runner.dome.apply_layout();
            },
        );
    }

    fn dispatch_picker_icons(&mut self) {
        let to_load = self.dome.picker_icons_to_load();
        for (app_id, hwnd_id) in to_load {
            self.dispatcher.dispatch(
                move || {
                    let hwnd = windows::Win32::Foundation::HWND::from(hwnd_id);
                    crate::platform::windows::dome::icon::load_app_icon(hwnd)
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
