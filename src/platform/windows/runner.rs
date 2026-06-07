use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    KillTimer, PostQuitMessage, PostThreadMessageW, SetTimer, WM_QUIT,
};

use crate::action::{Action, Actions};
use crate::keymap::KeymapState;
use crate::platform::windows::WM_APP_DISPATCH_RESULT;
use crate::platform::windows::dome::{Dome, HubEvent, NewWindow};
use crate::platform::windows::external::{HwndId, InspectExternalWindow, ManageExternalWindow};
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
    keymap_state: Arc<RwLock<KeymapState>>,
}

impl Runner {
    pub(super) fn new(
        dome: Dome,
        thread_id: u32,
        main_thread_id: u32,
        keymap_state: Arc<RwLock<KeymapState>>,
    ) -> Self {
        Self {
            dome,
            dispatcher: ReadDispatcher::new(thread_id),
            focus_throttle: Throttle::new(FOCUS_THROTTLE_INTERVAL),
            focus_timer_id: None,
            window_timers: HashMap::new(),
            main_thread_id,
            keymap_state,
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
            HubEvent::WindowRestored(hwnd_id) => {
                // On window restored, we need its actual placement in order to do correct things
                self.dispatch_placement_read(hwnd_id);
            }
            HubEvent::WindowFocused(hwnd_id) => match self.focus_throttle.submit(hwnd_id) {
                ThrottleResult::Send(id) => {
                    self.dome.handle_focus(id);
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
                    let inspect: Arc<dyn InspectExternalWindow> =
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
        self.dispatcher.dispatch(
            move || {
                if !inspect.is_manageable() {
                    return None;
                }
                Some((
                    NewWindow {
                        ext: manage,
                        title: inspect.get_window_title(),
                        process: inspect.get_process_name().unwrap_or_default(),
                        constraints: inspect.get_size_constraints(),
                        app_name: inspect.get_app_display_name(),
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

    fn dispatch_placement_read(&mut self, hwnd_id: HwndId) {
        let Some(id) = self.dome.registry_get_id(hwnd_id) else {
            return;
        };
        let inspect: Arc<dyn InspectExternalWindow> = Arc::new(ExternalHwnd::new(hwnd_id.into()));
        self.dispatcher.dispatch(
            move || {
                // This means this is a stale read dispatch. Minimized event is emitted and handled
                // properly by window_minimized, so there is no need to handle them here
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
                if runner.dome.registry_get_id(hwnd_id) != Some(id) {
                    return;
                }
                runner.dome.window_moved(hwnd_id, rect, monitor);
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
        let Some(id) = self.dome.registry_get_id(hwnd_id) else {
            return;
        };
        let inspect: Arc<dyn InspectExternalWindow> = Arc::new(ExternalHwnd::new(hwnd_id.into()));
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
