use crate::platform::keymap::{KeymapPublisher, KeymapView};
mod dome;
mod event_listener;
mod external;
mod font;
mod foreground;
mod handle;
mod keyboard;
mod login_item;
mod process;
mod runner;
mod spawn;
mod taskbar;
mod ui;
mod window_server;

#[cfg(test)]
mod native_ui;
#[cfg(test)]
mod startup_tests;
#[cfg(test)]
mod tests;

use std::cell::OnceCell;
use std::collections::HashMap;
use std::ops::ControlFlow;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::time::Instant;

use crate::logging::Logger;
use anyhow::Result;

use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Com::{COINIT_APARTMENTTHREADED, CoInitializeEx};
use windows::Win32::System::Console::{
    CTRL_BREAK_EVENT, CTRL_C_EVENT, CTRL_CLOSE_EVENT, SetConsoleCtrlHandler,
};
use windows::Win32::System::Threading::{GetCurrentProcess, GetCurrentThreadId};
use windows::Win32::UI::HiDpi::{
    AreDpiAwarenessContextsEqual, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
    GetDpiAwarenessContextForProcess, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, MSG, PM_NOREMOVE, PeekMessageW, PostThreadMessageW,
    TranslateMessage, WM_APP, WM_QUIT,
};
use windows::core::BOOL;

use crate::action::Actions;
use crate::config::watch::start_file_watcher;
use crate::config::{
    Appearance, LuaRuntime, PreferredLayouts, bootstrap::resolve_config_path, paths,
};
use crate::core::TilingConfig;
use crate::ipc;
use crate::platform::render::WgpuContext;
use crate::platform::shell_menu::{build_menu, focused_tooltip, id_to_action};
use dome::events::{HubMessage, SceneSender};
use dome::{Dome, HubEvent};
use dome_auxiliary_window::{App, AppHandler, Icon, LoopWaker, MenuEntry, Shell};
use event_listener::install_event_hooks;
use external::HwndId;
use ui::overlay::WgpuOverlayFactory;
use ui::{AuxiliaryWindows, UiState};
use window_server::{NativeEvent, NativeEventSender, WindowServer, WindowsWindowServer};

use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use taskbar::Taskbar;

pub(super) const WM_APP_HUBEVENT: u32 = WM_APP;

/// The tray icon compiled into the executable's resources. Matches the id in the resource
/// script the build embeds.
const TRAY_ICON_RESOURCE_ID: u16 = 1;

static MAIN_THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Wake {
    Ui,
    WindowServer,
    FocusRequest,
}

#[derive(Clone)]
struct HubSender {
    thread_id: u32,
    events: Sender<HubEvent>,
}
impl HubSender {
    fn send(&self, event: HubEvent) {
        if self.events.send(event).is_ok() {
            unsafe {
                PostThreadMessageW(self.thread_id, WM_APP_HUBEVENT, WPARAM(0), LPARAM(0)).ok();
            }
        }
    }
}

#[derive(Clone)]
struct UiSender {
    scenes: Sender<HubMessage>,
    waker: LoopWaker<Wake>,
}
impl SceneSender for UiSender {
    fn send(&mut self, message: HubMessage) {
        if self.scenes.send(message).is_ok() {
            self.waker.wake(Wake::Ui);
        }
    }
}
struct UiThreadReady {
    ui: UiSender,
    native: NativeEventSender,
}
struct StartupReady {
    hub: HubSender,
    native: NativeEventSender,
    acknowledged: Sender<bool>,
}

struct WindowLoopCallbacks {
    wake: Box<dyn Fn(Wake)>,
    wake_at: Box<dyn Fn(Wake, Option<Instant>)>,
    stop_wakes: Box<dyn Fn()>,
    terminate: Box<dyn Fn()>,
}
struct WindowLoopHandler {
    ui: UiState,
    windows: Box<dyn WindowServer>,
    callbacks: Rc<OnceCell<WindowLoopCallbacks>>,
    hub_sender: HubSender,
    observe_foreground: Box<dyn Fn()>,
    stopping: bool,
}
impl WindowLoopHandler {
    fn callbacks(&self) -> &WindowLoopCallbacks {
        self.callbacks
            .get()
            .expect("loop callbacks bound before delivery")
    }
    fn process_window_server_at(&mut self, now: Instant) {
        if self.stopping {
            return;
        }
        self.windows.ingest_native_events(now);
        self.windows.ingest_read_results(now);
        self.windows.read_settled_windows(now);
        (self.callbacks().wake_at)(Wake::WindowServer, self.windows.next_deadline());
        if self.windows.has_observations() {
            (self.callbacks().wake)(Wake::Ui);
        }
    }
    fn process_ui_at(&mut self, now: Instant) -> ControlFlow<()> {
        if self.stopping {
            return ControlFlow::Break(());
        }
        for observation in self.windows.take_observations() {
            self.ui.observe(self.windows.as_mut(), observation, now);
        }
        let ControlFlow::Continue((scene, commands)) = self.ui.drain_intake(self.windows.as_mut())
        else {
            self.stop();
            (self.callbacks().terminate)();
            return ControlFlow::Break(());
        };
        if let Some(scene) = &scene {
            self.ui.apply_placements(self.windows.as_mut(), scene, now);
        }
        for command in commands {
            self.ui.apply_command(self.windows.as_mut(), command);
        }
        if let Some(scene) = &scene {
            let keys: Vec<_> = scene.windows.keys().copied().collect();
            for observation in self.windows.read_corner_radii(&keys) {
                self.ui.observe(self.windows.as_mut(), observation, now);
            }
            self.ui.show_internal_windows(scene);
        }
        self.ui.apply_overlay_focus(self.windows.as_mut());
        if let Some(scene) = &scene {
            self.ui.restack(self.windows.as_mut(), scene);
        }
        (self.callbacks().wake_at)(Wake::FocusRequest, self.ui.focus_deadline(now));
        ControlFlow::Continue(())
    }
    fn process_focus_request_at(&mut self, now: Instant) {
        if self.stopping {
            return;
        }
        self.ui.submit_pending_focus(self.windows.as_mut(), now);
        (self.callbacks().wake_at)(Wake::FocusRequest, self.ui.focus_deadline(now));
    }
    fn process_wake_at(&mut self, wake: Wake, now: Instant) -> ControlFlow<()> {
        match wake {
            Wake::WindowServer => self.process_window_server_at(now),
            Wake::Ui => return self.process_ui_at(now),
            Wake::FocusRequest => self.process_focus_request_at(now),
        }
        ControlFlow::Continue(())
    }

    fn stop(&mut self) {
        if self.stopping {
            return;
        }
        self.stopping = true;
        (self.callbacks().stop_wakes)();
        self.ui.recover(self.windows.as_mut());
    }
}
impl AppHandler for WindowLoopHandler {
    type Wake = Wake;
    fn on_wake(&mut self, shell: &Shell, wake: Wake) {
        let now = Instant::now();
        if self.process_wake_at(wake, now).is_continue() && wake == Wake::Ui {
            shell.set_tooltip(&focused_tooltip(&self.ui.workspaces));
        }
    }
    fn on_stopping(&mut self) {
        self.stop();
    }
    fn menu(&mut self) -> Vec<MenuEntry> {
        build_menu(&self.ui.workspaces, false)
    }
    fn on_menu_selected(&mut self, id: u32) {
        if let Some(action) = id_to_action(id, &self.ui.workspaces) {
            self.hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])));
        }
    }
    fn on_display_changed(&mut self) {
        (self.observe_foreground)();
        self.hub_sender.send(HubEvent::DisplayChanged);
    }
    fn on_work_area_changed(&mut self) {
        self.hub_sender.send(HubEvent::WorkAreaChanged);
    }
}

struct DomainExitGuard {
    ui: UiSender,
    thread: Option<thread::JoinHandle<()>>,
}
impl Drop for DomainExitGuard {
    fn drop(&mut self) {
        self.ui.send(HubMessage::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn finish_domain_thread(main_thread_id: u32, panicked: bool) {
    if panicked {
        tracing::error!("Domain thread panicked");
    }
    unsafe { PostThreadMessageW(main_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
}

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> Result<()> {
    ensure_per_monitor_v2_awareness()?;

    // COM needed for shell APIs on main thread
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    let logger = Logger::init();

    let config_path = resolve_config_path(config_path);

    let layout_path = layout_path.unwrap_or_else(|| {
        paths::layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });
    let layout = PreferredLayouts::load_or_default(&layout_path);
    tracing::info!(path = %layout_path, "Loaded layout");

    std::panic::set_hook(Box::new(|panic_info| {
        let backtrace = backtrace::Backtrace::new();
        let thread = std::thread::current();
        let thread_name = thread.name().unwrap_or("<unnamed>");
        tracing::error!(
            thread = %thread_name,
            "Application panicked: {panic_info}. Backtrace: {backtrace:?}"
        );
    }));

    let main_thread_id = unsafe { GetCurrentThreadId() };

    MAIN_THREAD_ID.store(main_thread_id, std::sync::atomic::Ordering::Release);
    if unsafe { SetConsoleCtrlHandler(Some(console_ctrl_handler), true) }.is_err() {
        tracing::warn!("Failed to install console control handler");
    }

    let mut message = MSG::default();
    unsafe {
        let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
    }
    let (keymap_tx, keymap_rx) = mpsc::channel::<KeymapView>();

    // Only the domain thread may touch the VM.
    let (init_tx, init_rx) = mpsc::channel::<StartupReady>();
    let domain_thread = thread::spawn({
        let logger = logger.clone();
        let config_path = config_path.clone();
        let layout = layout.clone();
        move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }
                    .ok()
                    .expect("CoInitializeEx failed");
                let mut keymap = KeymapPublisher::new(KeymapView::new(), keymap_tx);
                let (runtime, config) = match LuaRuntime::load(config_path.clone(), &mut keymap) {
                    Ok(loaded) => loaded,
                    Err(e) => {
                        tracing::error!(error = %e, "Failed to build the Lua VM, aborting startup");
                        return;
                    }
                };
                // Log before set_level so the line survives a configured level
                // above info.
                tracing::info!(%config_path, "Loaded config");
                logger.set_level(config.log_level);
                login_item::sync_login_item(config.start_at_login);
                let tid = unsafe { GetCurrentThreadId() };
                let mut message = MSG::default();
                unsafe {
                    let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                }
                let (events, intake) = mpsc::channel();
                let hub_sender = HubSender {
                    thread_id: tid,
                    events,
                };
                run_dome(
                    DomeConfig {
                        appearance: config.appearance,
                        tiling: config.tiling,
                        env: config.env,
                    },
                    layout,
                    main_thread_id,
                    runtime,
                    keymap,
                    logger,
                    hub_sender,
                    intake,
                    init_tx,
                );
            }));
            finish_domain_thread(main_thread_id, result.is_err());
        }
    });

    let ready = match init_rx.recv() {
        Ok(ready) => ready,
        Err(_) => {
            let _ = domain_thread.join();
            anyhow::bail!("domain or UI thread exited before readiness");
        }
    };
    let hub_sender = ready.hub;
    let keyboard_hook = match install_keyboard_hook(keymap_rx, hub_sender.clone()) {
        Ok(hook) => hook,
        Err(error) => {
            let _ = ready.acknowledged.send(false);
            let _ = domain_thread.join();
            return Err(error);
        }
    };
    let _event_hooks = match install_event_hooks(ready.native) {
        Ok(hooks) => hooks,
        Err(error) => {
            let _ = ready.acknowledged.send(false);
            let _ = domain_thread.join();
            uninstall_keyboard_hook(keyboard_hook);
            return Err(error);
        }
    };
    let _ = ready.acknowledged.send(true);

    if let Err(error) = ipc::start_server(layout_path.clone(), {
        let sender = hub_sender.clone();
        move |ev| {
            match ev {
                ipc::IpcEvent::Action(actions) => sender.send(HubEvent::Action(actions)),
                ipc::IpcEvent::Query { query, reply } => sender.send(HubEvent::Query {
                    query,
                    sender: reply,
                }),
                ipc::IpcEvent::SaveLayout(path) => sender.send(HubEvent::SaveLayout(path)),
                ipc::IpcEvent::ApplyLayout(path) => sender.send(HubEvent::ApplyLayout(path)),
            }
            Ok(())
        }
    }) {
        hub_sender.send(HubEvent::Shutdown);
        let _ = domain_thread.join();
        uninstall_keyboard_hook(keyboard_hook);
        return Err(error);
    }

    let _config_watcher = start_file_watcher(&config_path, {
        let hub_sender = hub_sender.clone();
        move || {
            hub_sender.send(HubEvent::ReloadConfig);
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
    domain_thread.join().ok();
    uninstall_keyboard_hook(keyboard_hook);

    Ok(())
}

/// Verifies the process runs at Per-Monitor V2 DPI awareness, aborting otherwise because
/// every downstream geometry and rendering assumption requires PMv2. See BRD risk #6.
fn ensure_per_monitor_v2_awareness() -> anyhow::Result<()> {
    let result =
        unsafe { SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
    if result.is_ok() {
        return Ok(());
    }
    let err = result.unwrap_err();

    // GetDpiAwarenessContextForProcess + AreDpiAwarenessContextsEqual require Windows 10
    // 1803+. This path is only reachable there anyway, because PMv2 needs 1703+ and a
    // failed Set means awareness was pinned, which only a manifest or shim does on 1803+.
    let current_ctx = unsafe { GetDpiAwarenessContextForProcess(GetCurrentProcess()) };
    let is_pmv2 = unsafe {
        AreDpiAwarenessContextsEqual(current_ctx, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
    };
    if is_pmv2.as_bool() {
        tracing::info!(
            err = %err,
            "DPI awareness already PMv2 (likely manifest or compat shim); continuing"
        );
        return Ok(());
    }

    tracing::error!(
        err = %err,
        "Failed to set PMv2 DPI awareness; refusing to start because geometry would be wrong"
    );
    anyhow::bail!(
        "Process DPI awareness is not Per-Monitor V2. \
         Dome requires PMv2 for correct geometry. \
         Check compatibility settings or application manifest. Original error: {err}"
    );
}

/// Handles Ctrl+C, Ctrl+Break, and console close by posting WM_QUIT to the main
/// thread, triggering the existing graceful shutdown path (Dome drop -> recovery).
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> BOOL {
    match ctrl_type {
        CTRL_C_EVENT | CTRL_BREAK_EVENT | CTRL_CLOSE_EVENT => {
            tracing::info!(ctrl_type, "Received console control event");
            let thread_id = MAIN_THREAD_ID.load(std::sync::atomic::Ordering::Relaxed);
            if thread_id != 0 {
                // Result ignored: the handler can't meaningfully recover from a failure,
                // and returning TRUE still prevents the default handler from killing the process.
                unsafe { PostThreadMessageW(thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
            }
            // Windows terminates the process shortly after the handler returns for
            // CTRL_CLOSE_EVENT. Sleep to give the main thread time to shut down gracefully.
            if ctrl_type == CTRL_CLOSE_EVENT {
                std::thread::sleep(std::time::Duration::from_secs(2));
            }
            BOOL(1)
        }
        _ => BOOL(0),
    }
}

struct DomeConfig {
    appearance: Appearance,
    tiling: TilingConfig,
    env: HashMap<String, String>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "domain startup owns configuration and channel endpoints"
)]
fn run_dome(
    config: DomeConfig,
    workspace_overrides: PreferredLayouts,
    main_thread_id: u32,
    runtime: LuaRuntime,
    keymap_publisher: KeymapPublisher,
    logger: Logger,
    hub_sender: HubSender,
    intake: Receiver<HubEvent>,
    startup: Sender<StartupReady>,
) {
    let domain_thread_id = hub_sender.thread_id;
    let (handshake_tx, handshake_rx) = mpsc::channel::<UiThreadReady>();
    let appearance = config.appearance;
    let ui_hub = hub_sender.clone();
    let ui_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_ui_thread(ui_hub, handshake_tx, appearance);
        }));
        if result.is_err() {
            tracing::error!("UI thread panicked");
        }
        unsafe {
            PostThreadMessageW(domain_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok();
        }
    });
    coordinate_ui_startup(ui_thread, handshake_rx, startup, hub_sender, move |ready| {
        let taskbar = Taskbar::new().expect("Failed to create Taskbar");
        let dome = Dome::new(
            config.tiling,
            workspace_overrides,
            Rc::new(taskbar),
            Box::new(dome::Win32Display),
            Box::new(ready.ui),
            runtime,
            keymap_publisher,
            config.env,
        )
        .expect("Failed to initialize Dome");
        let mut runner = runner::DomeRunner::new(dome, main_thread_id, logger);
        ready.native.observe_foreground(None);
        if let Err(error) = handle::enum_windows(|hwnd| {
            ready.native.send(NativeEvent::Discover(HwndId::from(hwnd)))
        }) {
            tracing::warn!(%error, "Failed to enumerate windows");
        }
        let mut message = MSG::default();
        unsafe {
            while GetMessageW(&mut message, None, 0, 0).into() {
                if message.message == WM_APP_HUBEVENT {
                    while let Ok(event) = intake.try_recv() {
                        runner.handle_event(event);
                    }
                } else {
                    let _ = TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
            }
        }
    });
}

fn coordinate_ui_startup(
    ui_thread: thread::JoinHandle<()>,
    readiness: Receiver<UiThreadReady>,
    startup: Sender<StartupReady>,
    hub: HubSender,
    start: impl FnOnce(UiThreadReady),
) {
    let ready = match readiness.recv() {
        Ok(ready) => ready,
        Err(_) => {
            let _ = ui_thread.join();
            return;
        }
    };
    let _exit = DomainExitGuard {
        ui: ready.ui.clone(),
        thread: Some(ui_thread),
    };
    let (acknowledged, acknowledgment) = mpsc::channel();
    if startup
        .send(StartupReady {
            hub,
            native: ready.native.clone(),
            acknowledged,
        })
        .is_err()
    {
        return;
    }
    if acknowledgment.recv() == Ok(true) {
        start(ready);
    }
}

fn run_ui_thread(hub_sender: HubSender, handshake: Sender<UiThreadReady>, appearance: Appearance) {
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED)
            .ok()
            .expect("CoInitializeEx failed");
    }

    // wgpu 29 dropped Default on InstanceDescriptor for explicit constructors.
    // new_without_display_handle suits a headless overlay that never presents to a
    // winit display.
    let mut instance_descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    instance_descriptor.backends = wgpu::Backends::DX12;
    let instance = wgpu::Instance::new(instance_descriptor);
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("No DX12 adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("Failed to create wgpu device");
    let device = Arc::new(device);
    let queue = Arc::new(queue);

    let overlay_factory = WgpuOverlayFactory::new(
        WgpuContext::new(instance, adapter, device, queue),
        hub_sender.clone(),
    )
    .expect("DirectComposition device init");

    let internal = AuxiliaryWindows::new(appearance, overlay_factory);
    let (scene_tx, scene_rx) = mpsc::channel::<HubMessage>();
    let ui = UiState::new(scene_rx, hub_sender.clone(), Box::new(internal));
    let wake_binding: Arc<OnceLock<LoopWaker<Wake>>> = Arc::new(OnceLock::new());
    let binding = wake_binding.clone();
    let windows = WindowsWindowServer::new(Arc::new(move || {
        if let Some(waker) = binding.get() {
            waker.wake(Wake::WindowServer);
        }
    }))
    .expect("window server init");
    let native = windows.native_sender();
    let callbacks = Rc::new(OnceCell::new());
    let app = App::new(
        Icon::from_resource_id(TRAY_ICON_RESOURCE_ID).expect("tray icon loads"),
        Box::new(WindowLoopHandler {
            ui,
            windows: Box::new(windows),
            callbacks: callbacks.clone(),
            hub_sender,
            observe_foreground: Box::new({
                let native = native.clone();
                move || native.observe_foreground(None)
            }),
            stopping: false,
        }),
    )
    .expect("app init");
    let _ = wake_binding.set(app.waker());
    let waker = app.waker();
    let deadline = app.handle();
    let stop = app.handle();
    let terminate = app.handle();
    let _ = callbacks.set(WindowLoopCallbacks {
        wake: Box::new(move |wake| waker.wake(wake)),
        wake_at: Box::new(move |wake, at| deadline.wake_at(wake, at)),
        stop_wakes: Box::new(move || stop.stop_wakes()),
        terminate: Box::new(move || terminate.terminate()),
    });
    if handshake
        .send(UiThreadReady {
            ui: UiSender {
                scenes: scene_tx,
                waker: app.waker(),
            },
            native,
        })
        .is_err()
    {
        app.handle().terminate();
    }
    app.run();
}
