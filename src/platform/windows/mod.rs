mod dome;
mod event_listener;
pub(super) mod external;
mod handle;
mod keyboard;
mod taskbar;
mod throttle;

#[cfg(test)]
mod tests;

use std::collections::HashMap;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use crate::logging::Logger;
use anyhow::Result;

use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, HDC, HMONITOR, MONITORINFOEXW,
};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow,
    GetMessageW, KillTimer, MONITORINFOF_PRIMARY, MSG, PostQuitMessage, PostThreadMessageW,
    RegisterClassW, SetTimer, TranslateMessage, WM_APP, WM_DISPLAYCHANGE, WM_PAINT, WM_QUIT,
    WM_TIMER, WNDCLASSW, WS_EX_TOOLWINDOW, WS_POPUP,
};
use windows::core::{BOOL, PCWSTR};

use crate::action::{Action, Actions};
use crate::config::{Config, start_config_watcher};
use crate::core::Dimension;
use crate::ipc;
use dome::overlay::{
    CONTAINER_OVERLAY_CLASS, WINDOW_OVERLAY_CLASS, container_wnd_proc, raw_window_handle,
};
use dome::{Dome, HubEvent};
use event_listener::install_event_hooks;
use external::{HwndId, ManageExternalHwnd};
use glutin::display::{Display as GlDisplay, DisplayApiPreference};
use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use raw_window_handle::{RawDisplayHandle, WindowsDisplayHandle};
use taskbar::Taskbar;
use throttle::{Throttle, ThrottleResult};

#[derive(Clone)]
pub(super) struct ScreenInfo {
    pub handle: isize,
    pub name: String,
    pub dimension: Dimension,
    pub is_primary: bool,
}

pub(super) const WM_APP_HUBEVENT: u32 = WM_APP;
pub(super) const WM_APP_DISPLAY_CHANGE: u32 = WM_APP + 1;

#[derive(Clone)]
struct HubSender {
    thread_id: u32,
}

impl HubSender {
    fn send(&self, event: HubEvent) {
        let ptr = Box::into_raw(Box::new(event)) as usize;
        unsafe {
            PostThreadMessageW(self.thread_id, WM_APP_HUBEVENT, WPARAM(ptr), LPARAM(0)).ok();
        }
    }
}

pub fn run_app(config_path: Option<String>) -> Result<()> {
    unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2).ok() };

    // COM needed for shell APIs on main thread
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = Config::load(&config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load config from {config_path}: {e}, using defaults");
        Config::default()
    });

    let logger = Logger::init(&config);
    tracing::info!(%config_path, "Loaded config");

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        tracing::error!("Application panicked: {panic_info}. Backtrace: {backtrace:?}");
    }));

    let main_thread_id = unsafe { GetCurrentThreadId() };
    let dome_thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(2));

    let config_clone = config.clone();
    let tid = Arc::clone(&dome_thread_id);
    let bar = Arc::clone(&barrier);
    let dome_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                .ok()
                .expect("CoInitializeEx failed");
            tid.store(
                unsafe { GetCurrentThreadId() },
                std::sync::atomic::Ordering::Release,
            );
            bar.wait();
            run_dome(config_clone, main_thread_id);
        }));
        if result.is_err() {
            tracing::error!("Dome thread panicked");
        }
        unsafe { PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    });

    barrier.wait();
    let hub_sender = HubSender {
        thread_id: dome_thread_id.load(std::sync::atomic::Ordering::Acquire),
    };

    let keyboard_hook = install_keyboard_hook(hub_sender.clone(), config)?;
    let _event_hooks = install_event_hooks(hub_sender.clone())?;

    ipc::start_server({
        let sender = hub_sender.clone();
        move |actions| {
            sender.send(HubEvent::Action(actions));
            Ok(())
        }
    })?;

    let _config_watcher = start_config_watcher(&config_path, {
        let sender = hub_sender.clone();
        move |cfg| {
            logger.set_level(cfg.log_level);
            keyboard::update_config(cfg.clone());
            sender.send(HubEvent::ConfigChanged(cfg));
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    // Main thread: bare message pump for hooks only
    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }

    hub_sender.send(HubEvent::Shutdown);
    dome_thread.join().ok();
    uninstall_keyboard_hook(keyboard_hook);

    Ok(())
}

fn get_all_screens() -> Result<Vec<ScreenInfo>> {
    let mut monitors = Vec::new();

    unsafe extern "system" fn enum_proc(
        hmonitor: HMONITOR,
        _hdc: HDC,
        _rect: *mut RECT,
        lparam: LPARAM,
    ) -> BOOL {
        let monitors = unsafe { &mut *(lparam.0 as *mut Vec<ScreenInfo>) };
        let mut info = MONITORINFOEXW {
            monitorInfo: windows::Win32::Graphics::Gdi::MONITORINFO {
                cbSize: size_of::<MONITORINFOEXW>() as u32,
                ..Default::default()
            },
            ..Default::default()
        };

        if unsafe { GetMonitorInfoW(hmonitor, &mut info.monitorInfo) }.as_bool() {
            let rc = info.monitorInfo.rcWork;
            let name = String::from_utf16_lossy(
                &info
                    .szDevice
                    .iter()
                    .take_while(|&&c| c != 0)
                    .copied()
                    .collect::<Vec<_>>(),
            );

            monitors.push(ScreenInfo {
                handle: hmonitor.0 as isize,
                name,
                dimension: Dimension {
                    x: rc.left as f32,
                    y: rc.top as f32,
                    width: (rc.right - rc.left) as f32,
                    height: (rc.bottom - rc.top) as f32,
                },
                is_primary: info.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
            });
        }
        BOOL(1)
    }

    let success = unsafe {
        EnumDisplayMonitors(
            None,
            None,
            Some(enum_proc),
            LPARAM(&mut monitors as *mut _ as isize),
        )
    };
    anyhow::ensure!(success.as_bool(), "EnumDisplayMonitors failed");
    Ok(monitors)
}

const FOCUS_THROTTLE_INTERVAL: Duration = Duration::from_millis(500);
const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(100);
const DRAG_SAFETY_TIMEOUT: Duration = Duration::from_secs(60);

const TIMER_FOCUS: usize = 1;
const TIMER_WINDOW_BASE: usize = 0x1000;

enum TimerKind {
    FocusThrottle,
    PlacementDebounce(HwndId),
    DragSafety(HwndId),
}

struct Runner {
    dome: Dome,
    focus_throttle: Throttle<HwndId>,
    window_timers: HashMap<HwndId, usize>,
    next_timer_id: usize,
    main_thread_id: u32,
}

impl Runner {
    fn schedule_timer(&mut self, kind: TimerKind, delay: Duration) -> usize {
        let id = match &kind {
            TimerKind::FocusThrottle => TIMER_FOCUS,
            _ => {
                let id = self.next_timer_id;
                self.next_timer_id += 1;
                id
            }
        };
        if let TimerKind::PlacementDebounce(hwnd) | TimerKind::DragSafety(hwnd) = &kind {
            self.window_timers.insert(*hwnd, id);
        }
        unsafe {
            SetTimer(None, id, delay.as_millis() as u32, None);
        }
        id
    }

    fn cancel_timer(&mut self, hwnd: &HwndId) {
        if let Some(id) = self.window_timers.remove(hwnd) {
            unsafe { KillTimer(None, id).ok() };
        }
    }

    fn handle_timer(&mut self, timer_id: usize) {
        unsafe { KillTimer(None, timer_id).ok() };
        if timer_id == TIMER_FOCUS {
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
            self.dome.apply_layout();
        }
    }

    fn handle_event(&mut self, event: HubEvent) {
        match event {
            HubEvent::Shutdown => {
                tracing::info!("Shutdown requested");
                unsafe { PostQuitMessage(0) };
                return;
            }
            HubEvent::ConfigChanged(c) => {
                self.dome.config_changed(c);
            }
            HubEvent::WindowCreated(ext) => {
                if let Some(actions) = self.dome.window_created(ext) {
                    self.handle_actions(&actions);
                }
            }
            HubEvent::WindowDestroyed(ext) => {
                self.dome.window_destroyed(ext);
            }
            HubEvent::WindowMinimized(ext) => {
                self.dome.window_minimized(ext);
            }
            HubEvent::WindowFocused(ext) => {
                let id = ext.id();
                match self.focus_throttle.submit(id) {
                    ThrottleResult::Send(id) => {
                        self.dome.handle_focus(id);
                    }
                    ThrottleResult::Pending => return,
                    ThrottleResult::ScheduleFlush(delay) => {
                        self.focus_throttle.mark_timer_scheduled();
                        self.schedule_timer(TimerKind::FocusThrottle, delay);
                        return;
                    }
                }
            }
            HubEvent::MoveSizeStart(ext) => {
                let id = ext.id();
                self.cancel_timer(&id);
                self.dome.move_size_started(ext);
                self.schedule_timer(TimerKind::DragSafety(id), DRAG_SAFETY_TIMEOUT);
                return;
            }
            HubEvent::MoveSizeEnd(ext) => {
                let id = ext.id();
                self.cancel_timer(&id);
                self.dome.move_size_ended(ext);
            }
            HubEvent::LocationChanged(ext) => {
                let id = ext.id();
                if self.dome.location_changed(ext) {
                    self.cancel_timer(&id);
                    self.schedule_timer(TimerKind::PlacementDebounce(id), DEBOUNCE_INTERVAL);
                }
                return;
            }
            HubEvent::WindowTitleChanged(ext) => {
                if let Some(actions) = self.dome.title_changed(ext) {
                    self.handle_actions(&actions);
                }
            }
            HubEvent::Action(a) => {
                self.handle_actions(&a);
            }
            HubEvent::TabClicked(id, idx) => {
                self.dome.tab_clicked(id, idx);
            }
        }
        self.dome.apply_layout();
    }

    #[tracing::instrument(skip(self))]
    fn handle_actions(&mut self, actions: &Actions) {
        for action in actions {
            match action {
                Action::Hub(hub) => self.dome.execute_hub_action(hub),
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
            }
        }
    }
}

struct GlOverlayFactory {
    display: glutin::display::Display,
    hub_sender: HubSender,
}

impl dome::CreateOverlay for GlOverlayFactory {
    fn create_window_overlay(&self) -> anyhow::Result<Box<dyn dome::overlay::WindowOverlayApi>> {
        dome::overlay::create_window_overlay(&self.display)
    }
    fn create_container_overlay(
        &self,
        config: Config,
    ) -> anyhow::Result<Box<dyn dome::overlay::ContainerOverlayApi>> {
        Ok(dome::overlay::ContainerOverlay::new(
            &self.display,
            config,
            self.hub_sender.clone(),
        )?)
    }
}

unsafe extern "system" fn app_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_DISPLAYCHANGE => {
            unsafe {
                PostThreadMessageW(
                    GetCurrentThreadId(),
                    WM_APP_DISPLAY_CHANGE,
                    WPARAM(0),
                    LPARAM(0),
                )
                .ok()
            };
            LRESULT(0)
        }
        WM_PAINT => LRESULT(0),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

unsafe extern "system" fn window_overlay_wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn is_d3d_exclusive_fullscreen_active() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

struct Win32Display;

impl dome::QueryDisplay for Win32Display {
    fn get_all_screens(&self) -> anyhow::Result<Vec<ScreenInfo>> {
        get_all_screens()
    }

    fn get_exclusive_fullscreen_hwnd(&self) -> Option<HwndId> {
        if is_d3d_exclusive_fullscreen_active() {
            Some(HwndId::from(unsafe { GetForegroundWindow() }))
        } else {
            None
        }
    }
}

fn run_dome(config: Config, main_thread_id: u32) {
    let hinstance = unsafe { GetModuleHandleW(None) }.expect("GetModuleHandleW failed");

    const APP_CLASS: PCWSTR = windows::core::w!("DomeApp");

    let wc = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(app_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: APP_CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc) };

    let wc_window = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(window_overlay_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: WINDOW_OVERLAY_CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_window) };

    let wc_container = WNDCLASSW {
        style: CS_HREDRAW | CS_VREDRAW,
        lpfnWndProc: Some(container_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: CONTAINER_OVERLAY_CLASS,
        ..Default::default()
    };
    unsafe { RegisterClassW(&wc_container) };

    let app_hwnd = unsafe {
        CreateWindowExW(
            WS_EX_TOOLWINDOW,
            APP_CLASS,
            windows::core::w!(""),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
    }
    .expect("Failed to create app window");

    let raw_display = RawDisplayHandle::Windows(WindowsDisplayHandle::new());
    let raw_window = raw_window_handle(app_hwnd);
    let display =
        unsafe { GlDisplay::new(raw_display, DisplayApiPreference::Wgl(Some(raw_window))) }
            .expect("Failed to create GL display");

    let taskbar = Taskbar::new().expect("Failed to create Taskbar");

    let hub_sender = HubSender {
        thread_id: unsafe { GetCurrentThreadId() },
    };
    let overlays = GlOverlayFactory {
        display,
        hub_sender,
    };

    let dome = Dome::new(
        config.clone(),
        Arc::new(taskbar),
        Box::new(overlays),
        Box::new(Win32Display),
    )
    .expect("Failed to initialize Dome");

    let mut initial_windows: Vec<Arc<dyn ManageExternalHwnd>> = Vec::new();
    if let Err(e) = handle::enum_windows(|hwnd| {
        initial_windows.push(Arc::new(handle::ExternalHwnd::new(hwnd)));
    }) {
        tracing::warn!("Failed to enumerate windows: {e}");
    }

    let mut runner = Runner {
        dome,
        focus_throttle: Throttle::new(FOCUS_THROTTLE_INTERVAL),
        window_timers: HashMap::new(),
        next_timer_id: TIMER_WINDOW_BASE,
        main_thread_id,
    };

    let on_open = runner.dome.app_initialized(initial_windows);
    for actions in on_open {
        runner.handle_actions(&actions);
    }
    runner.dome.apply_layout();

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            match msg.message {
                WM_APP_HUBEVENT => {
                    let event = *Box::from_raw(msg.wParam.0 as *mut HubEvent);
                    runner.handle_event(event);
                }
                WM_APP_DISPLAY_CHANGE => {
                    runner.dome.handle_display_change();
                    runner.dome.apply_layout();
                }
                WM_TIMER => {
                    runner.handle_timer(msg.wParam.0);
                }
                _ => {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
        }
    }
}
