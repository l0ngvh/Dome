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
mod throttle;
mod timer_registry;
mod ui;

#[cfg(test)]
mod tests;

use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::thread;

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
    DispatchMessageW, GetMessageW, MSG, PostThreadMessageW, TranslateMessage, WM_APP, WM_QUIT,
    WM_TIMER,
};
use windows::core::BOOL;

use crate::config::{
    Config, LayoutConfig, LayoutWorkspaceConfig, layout_default_path, load_or_default,
    start_config_watcher,
};
use crate::ipc;
use crate::keymap::KeymapState;
use crate::platform::render::WgpuContext;
use dome::app_window::AppWindow;
use dome::events::{HubMessage, SceneSender};
use dome::{Dome, HubEvent};
use dome_auxiliary_window::{AuxiliaryLoopHandler, EventLoop, LoopWaker};
use event_listener::install_event_hooks;
use external::HwndId;
use ui::WindowThread;
use ui::overlay::WgpuOverlayFactory;

use keyboard::{install_keyboard_hook, uninstall_keyboard_hook};
use taskbar::Taskbar;

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

pub(super) const WM_APP_HUBEVENT: u32 = WM_APP;
pub(super) const WM_APP_DISPATCH_RESULT: u32 = WM_APP + 1;

static MAIN_THREAD_ID: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

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

/// The window thread's `EventLoop` owns the receiver, so a failed send means it is
/// gone during shutdown and there is nothing to wake.
struct SceneThreadSender {
    scenes: Sender<HubMessage>,
    waker: LoopWaker,
}

impl SceneSender for SceneThreadSender {
    fn send(&mut self, msg: HubMessage) {
        if self.scenes.send(msg).is_ok() {
            self.waker.wake();
        }
    }
}

/// Handed from the window thread to the domain thread once the window loop is built.
struct WindowThreadReady {
    scenes: Sender<HubMessage>,
    waker: LoopWaker,
    thread_id: u32,
}

/// Drives `WindowThread` from the auxiliary window crate's loop.
struct WindowLoopHandler {
    window_thread: WindowThread,
    scenes: Receiver<HubMessage>,
}

impl AuxiliaryLoopHandler for WindowLoopHandler {
    fn on_wake(&mut self) {
        while let Ok(scene) = self.scenes.try_recv() {
            self.window_thread.send(scene);
        }
    }
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

pub fn run_app(config_path: Option<String>, layout_path: Option<String>) -> Result<()> {
    ensure_per_monitor_v2_awareness()?;

    // COM needed for shell APIs on main thread
    unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()? };

    let logger = Logger::init();

    let config_path = config_path.unwrap_or_else(Config::default_path);
    let config = load_or_default(&config_path, Config::load);
    logger.set_level(config.log_level);
    tracing::info!(%config_path, "Loaded config");

    let layout_path = layout_path.unwrap_or_else(|| {
        layout_default_path(std::path::Path::new(&config_path))
            .to_string_lossy()
            .into_owned()
    });
    let layout = load_or_default(&layout_path, LayoutConfig::load);
    tracing::info!(path = %layout_path, "Loaded layout");

    login_item::sync_login_item(config.start_at_login);

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

    let dome_thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let keymap_state = Arc::new(RwLock::new(KeymapState::new(config.keymaps.clone())));

    let config_clone = config.clone();
    let layout_clone = layout.workspace.clone();
    let tid = Arc::clone(&dome_thread_id);
    let bar = Arc::clone(&barrier);
    let keymap_clone = Arc::clone(&keymap_state);
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
            run_dome(config_clone, layout_clone, main_thread_id, keymap_clone);
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

    let keyboard_hook = install_keyboard_hook(hub_sender.clone(), Arc::clone(&keymap_state))?;
    let _event_hooks = install_event_hooks(hub_sender.clone())?;

    ipc::start_server(layout_path.clone(), {
        let sender = hub_sender.clone();
        move |ev| {
            match ev {
                ipc::IpcEvent::Action(actions) => sender.send(HubEvent::Action(actions)),
                ipc::IpcEvent::Query { query, reply } => sender.send(HubEvent::Query {
                    query,
                    sender: reply,
                }),
                ipc::IpcEvent::ExportLayout(path) => sender.send(HubEvent::ExportLayout(path)),
            }
            Ok(())
        }
    })?;

    let _config_watcher = start_config_watcher(&config_path, Config::load, {
        let sender = hub_sender.clone();
        let keymap_state = Arc::clone(&keymap_state);
        move |cfg| {
            logger.set_level(cfg.log_level);
            keymap_state
                .write()
                .unwrap()
                .update_keymaps(cfg.keymaps.clone());
            let start_at_login = cfg.start_at_login;
            sender.send(HubEvent::ConfigChanged(Box::new(cfg)));
            login_item::sync_login_item(start_at_login);
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup config watcher: {e:#}"))
    .ok();

    let _layout_watcher = start_config_watcher(&layout_path, LayoutConfig::load, {
        let sender = hub_sender.clone();
        move |new_layout| {
            sender.send(HubEvent::LayoutConfigChanged(Box::new(new_layout)));
        }
    })
    .inspect_err(|e| tracing::warn!("Failed to setup layout watcher: {e:#}"))
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

fn run_dome(
    config: Config,
    workspace_overrides: Vec<LayoutWorkspaceConfig>,
    main_thread_id: u32,
    keymap_state: Arc<RwLock<KeymapState>>,
) {
    let domain_thread_id = unsafe { GetCurrentThreadId() };

    let (handshake_tx, handshake_rx) = std::sync::mpsc::channel::<WindowThreadReady>();
    let wt_config = config.clone();
    let window_thread = thread::spawn(move || {
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run_window_thread(domain_thread_id, handshake_tx, wt_config);
        }));
        if result.is_err() {
            tracing::error!("Window thread panicked");
        }
        // Bring the domain thread down so recovery runs and the process exits.
        unsafe { PostThreadMessageW(domain_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    });

    // Blocks until the window thread has built its windows and loop. The window exists by
    // then, so the thread's message queue exists and the first waker post is not dropped.
    let ready = match handshake_rx.recv() {
        Ok(ready) => ready,
        Err(_) => {
            // The window thread exited before signaling ready, e.g. wgpu init panicked. It
            // already posted WM_QUIT to bring us down, so there is nothing left to run.
            window_thread.join().ok();
            return;
        }
    };
    let window_thread_id = ready.thread_id;

    let taskbar = Taskbar::new().expect("Failed to create Taskbar");

    let dome = Dome::new(
        config.clone(),
        workspace_overrides,
        Rc::new(taskbar),
        Box::new(dome::Win32Display),
        Box::new(SceneThreadSender {
            scenes: ready.scenes,
            waker: ready.waker,
        }),
    )
    .expect("Failed to initialize Dome");

    let mut initial_hwnds = Vec::new();
    if let Err(e) = handle::enum_windows(|hwnd| {
        initial_hwnds.push(HwndId::from(hwnd));
    }) {
        tracing::warn!("Failed to enumerate windows: {e}");
    }

    let mut runner = runner::Runner::new(dome, domain_thread_id, main_thread_id, keymap_state);

    for hwnd_id in initial_hwnds {
        runner.dispatch_window_created(hwnd_id);
    }

    let mut msg = MSG::default();
    unsafe {
        while GetMessageW(&mut msg, None, 0, 0).into() {
            match msg.message {
                WM_APP_HUBEVENT => {
                    let event = *Box::from_raw(msg.wParam.0 as *mut HubEvent);
                    runner.handle_event(event);
                }
                WM_APP_DISPATCH_RESULT => {
                    let apply = *Box::from_raw(msg.wParam.0 as *mut runner::ApplyFn);
                    apply(&mut runner);
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

    // Domain pump exited on shutdown. Quit the window thread and wait for it to
    // destroy its overlays on its own thread before recovery runs at scope exit.
    unsafe { PostThreadMessageW(window_thread_id, WM_QUIT, WPARAM(0), LPARAM(0)).ok() };
    window_thread.join().ok();
}

fn run_window_thread(domain_thread_id: u32, handshake: Sender<WindowThreadReady>, config: Config) {
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

    let hub_sender = HubSender {
        thread_id: domain_thread_id,
    };

    let overlay_factory = WgpuOverlayFactory::new(
        WgpuContext::new(instance, adapter, device, queue),
        hub_sender.clone(),
    )
    .expect("DirectComposition device init");

    let app_window = AppWindow::new(hub_sender.clone()).expect("Failed to create app window");

    let window_thread = WindowThread::new(
        config,
        Box::new(overlay_factory),
        app_window,
        Box::new(handle::Win32ZOrder),
    );

    let (scene_tx, scene_rx) = std::sync::mpsc::channel::<HubMessage>();
    let event_loop = EventLoop::new(Box::new(WindowLoopHandler {
        window_thread,
        scenes: scene_rx,
    }));
    // The domain can post scenes the moment it holds these. The window already exists, so
    // the queue exists and a waker post cannot be dropped.
    handshake
        .send(WindowThreadReady {
            scenes: scene_tx,
            waker: event_loop.waker(),
            thread_id: unsafe { GetCurrentThreadId() },
        })
        .ok();

    event_loop.run();
}
