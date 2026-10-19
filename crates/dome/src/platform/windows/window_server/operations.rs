use super::{
    NativeWindowServer, PlacementObservation, WindowMetadata, WindowNotification,
    WindowObservation, WindowOrder, WindowServer,
};
use crate::platform::windows::dispatcher::DispatcherMarker;
use crate::platform::windows::event_loop::RunLoop;
use crate::platform::windows::external::{HwndId, NativeWindowOrder, ShowCmd};
use crate::platform::windows::handle::{ExternalHwnd, enum_windows};
use dome_core::{FractionalLength, LimitObservation, Logical, Physical, PixelRect, WindowId};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow;

impl NativeWindowServer {
    pub(super) fn discover(&self, native: HwndId) -> Option<WindowId> {
        if let Some(id) = self.registry.borrow().window_id(native) {
            return Some(id);
        }
        use windows::Win32::UI::WindowsAndMessaging::{GA_ROOT, GetAncestor};
        let handle = HWND::from(native);
        (unsafe { GetAncestor(handle, GA_ROOT) } == handle && !handle.is_invalid())
            .then(|| self.register(native))
    }

    pub(in crate::platform::windows) fn register_thumbnail(
        &self,
        destination: &crate::platform::windows::ui::window::Window,
        source: WindowId,
    ) -> anyhow::Result<crate::platform::windows::thumbnail::NativeThumbnail> {
        let native = self
            .registry
            .borrow()
            .native_id(source)
            .ok_or_else(|| anyhow::anyhow!("Unknown thumbnail source {source}"))?;
        crate::platform::windows::thumbnail::register_native_thumbnail(destination, native)
    }

    pub(in crate::platform::windows) fn native_order(
        &self,
        order: WindowOrder,
    ) -> NativeWindowOrder {
        match order {
            WindowOrder::Topmost => NativeWindowOrder::Topmost,
            WindowOrder::NotTopmost => NativeWindowOrder::NotTopmost,
            WindowOrder::Unchanged => NativeWindowOrder::Unchanged,
            WindowOrder::After(id) => self
                .registry
                .borrow()
                .native_id(id)
                .map_or(NativeWindowOrder::Unchanged, NativeWindowOrder::After),
        }
    }

    fn register(&self, native: HwndId) -> WindowId {
        self.registry.borrow_mut().register(native)
    }

    fn window(&self, id: WindowId) -> anyhow::Result<ExternalHwnd> {
        let native = self
            .registry
            .borrow()
            .native_id(id)
            .ok_or_else(|| anyhow::anyhow!("Unknown window {id}"))?;
        Ok(ExternalHwnd::new(native.into()))
    }
}

impl NativeWindowServer {
    pub(super) async fn read_window(
        &self,
        window_id: WindowId,
    ) -> anyhow::Result<(WindowObservation, PixelRect<Physical>, isize)> {
        let inspect = self.window(window_id)?;
        let result = RunLoop::dispatch(move |marker| {
            let metadata = read_metadata(&inspect, marker);
            (
                WindowObservation {
                    id: window_id,
                    pid: inspect.pid(),
                    metadata,
                    constraints: inspect.get_size_constraints(marker),
                    state: inspect.read_native_state(marker),
                },
                inspect.get_visible_rect(),
                inspect.get_monitor(),
            )
        });
        Ok(result.await?)
    }

    pub(super) async fn read_placement(
        &self,
        window_id: WindowId,
    ) -> anyhow::Result<(PlacementObservation, LimitObservation)> {
        let inspect = self.window(window_id)?;
        let result = RunLoop::dispatch(move |marker| {
            let placement = if inspect.is_minimized() {
                PlacementObservation::Minimized
            } else {
                PlacementObservation::Visible {
                    rect: inspect.get_visible_rect(),
                    monitor: inspect.get_monitor(),
                }
            };
            (placement, inspect.get_size_constraints(marker))
        });
        Ok(result.await?)
    }

    pub(super) async fn read_title(&self, window_id: WindowId) -> anyhow::Result<Option<String>> {
        let inspect = self.window(window_id)?;
        let result = RunLoop::dispatch(move |marker| inspect.get_window_title(marker));
        Ok(result.await?)
    }

    pub(super) fn enumerate_windows(&self) -> Vec<WindowId> {
        let mut windows = Vec::new();
        if let Err(error) = enum_windows(|window| windows.push(self.register(HwndId::from(window))))
        {
            tracing::warn!(%error, "Failed to enumerate windows");
        }
        windows
    }

    async fn read_constraints(
        &self,
        window_id: WindowId,
    ) -> anyhow::Result<dome_core::LimitObservation> {
        let inspect = self.window(window_id)?;
        let result = RunLoop::dispatch(move |marker| inspect.get_size_constraints(marker));
        Ok(result.await?)
    }
}

fn is_d3d_exclusive_fullscreen_active(_marker: &DispatcherMarker) -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

#[async_trait::async_trait(?Send)]
impl WindowServer for NativeWindowServer {
    fn request_constraints(&self, windows: Vec<WindowId>) {
        for window_id in windows {
            let owner = self.owner.clone();
            RunLoop::current().spawn(async move {
                let Some(server) = owner.upgrade() else {
                    return;
                };
                if let Ok(constraints) = server.read_constraints(window_id).await
                    && server.registry.borrow().eligible(window_id)
                {
                    server.emit(WindowNotification::ConstraintsChanged {
                        window_id,
                        constraints,
                    });
                }
            });
        }
    }

    async fn get_exclusive_fullscreen_window(&self) -> anyhow::Result<Option<WindowId>> {
        let result = RunLoop::dispatch(|marker| {
            is_d3d_exclusive_fullscreen_active(marker)
                .then(|| HwndId::from(unsafe { GetForegroundWindow() }))
        });
        let native = result.await?;
        Ok(native.and_then(|native| self.registry.borrow().window_id(native)))
    }

    fn add_tab(&self, id: WindowId) {
        if let Some(native) = self.registry.borrow().native_id(id) {
            self.taskbar.add_tab(native);
        }
    }
    fn delete_tab(&self, id: WindowId) {
        if let Some(native) = self.registry.borrow().native_id(id) {
            self.taskbar.delete_tab(native);
        }
    }

    fn set_position(&self, id: WindowId, z: WindowOrder, rect: PixelRect<Physical>) {
        if let Ok(window) = self.window(id) {
            window.set_position(self.native_order(z), rect);
        }
    }
    fn set_z_order(&self, id: WindowId, z: WindowOrder) {
        if let Ok(window) = self.window(id) {
            window.set_z_order(self.native_order(z));
        }
    }
    fn move_offscreen(&self, id: WindowId) {
        if let Ok(window) = self.window(id) {
            window.move_offscreen();
        }
    }
    fn show_cmd(&self, id: WindowId, command: ShowCmd) -> anyhow::Result<()> {
        self.window(id)?.show_cmd(command)
    }
    fn close(&self, id: WindowId) -> anyhow::Result<()> {
        self.window(id)?.close()
    }
    fn set_foreground_window(&self, id: WindowId) {
        if let Ok(window) = self.window(id) {
            window.set_foreground_window();
        }
    }
    fn corner_radius(&self, id: WindowId) -> FractionalLength<Logical> {
        self.window(id)
            .map_or(FractionalLength::ZERO, |window| window.corner_radius())
    }
}

fn read_metadata(inspect: &ExternalHwnd, marker: &DispatcherMarker) -> WindowMetadata {
    WindowMetadata {
        title: inspect.get_window_title(marker),
        process: inspect.get_process_name(marker).unwrap_or_default(),
        process_path: inspect.get_process_path(marker).ok(),
        class: inspect.get_class_name(),
        aumid: inspect.get_aumid(marker),
        app_name: inspect.get_app_display_name(marker),
    }
}
