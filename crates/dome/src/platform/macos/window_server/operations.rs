use std::collections::HashSet;

use anyhow::Result;
use dome_core::{FractionalCoordinate, Logical, PixelRect, WindowId};
use futures::future::join_all;
use objc2_app_kit::NSWorkspace;

use super::{
    AppObservation, NativeFullscreenObservation, NativeWindowServer, SystemObservation,
    WindowServer,
};
use crate::platform::macos::running_application::RunningApp;

impl NativeWindowServer {
    pub(in crate::platform::macos) async fn create_capture(
        &self,
        id: WindowId,
        queue: &dispatch2::DispatchQueue,
    ) -> Result<Option<crate::platform::macos::capture::CaptureSession>> {
        let native_window_id = self.state.borrow().native_ids.get(&id).copied();
        crate::platform::macos::capture::create_capture(native_window_id, queue).await
    }

    pub(super) async fn read_app_windows(
        &self,
        pid: i32,
        window_ids: &[WindowId],
    ) -> Option<AppObservation> {
        let result = {
            let state = self.state.borrow();
            let requested = state.requested_native_ids(window_ids);
            state.applications.get(&pid)?.read_windows(requested)
        };
        let observation = result.await.ok().flatten()?;
        self.state.borrow_mut().accept_app(observation)
    }

    pub(super) async fn read_all_windows(
        &self,
        window_ids: &[WindowId],
    ) -> Option<SystemObservation> {
        let (results, observed) = {
            let state = self.state.borrow();
            let requested = state.requested_native_ids(window_ids);
            let results = state
                .applications
                .iter()
                .filter(|(pid, _)| state.apps.get(pid) == Some(&true))
                .map(|(_, app)| app.read_windows(requested.clone()))
                .collect::<Vec<_>>();
            let observed = state
                .observed_pids
                .iter()
                .copied()
                .chain(state.window_pids.values().copied())
                .collect::<HashSet<_>>();
            (results, observed)
        };
        let results = join_all(results).await;
        let observation = {
            let mut state = self.state.borrow_mut();
            SystemObservation {
                terminated_pids: observed
                    .into_iter()
                    .filter(|pid| !state.apps.contains_key(pid))
                    .collect(),
                hidden_pids: state
                    .apps
                    .iter()
                    .filter_map(|(&pid, &visible)| (!visible).then_some(pid))
                    .collect(),
                apps: results
                    .into_iter()
                    .filter_map(|result| result.ok().flatten())
                    .filter_map(|app| state.accept_app(app))
                    .collect(),
            }
        };
        self.refresh_system_observers(&observation);
        Some(observation)
    }

    pub(super) async fn read_focus(&self, pid: i32) -> Option<WindowId> {
        if !RunningApp::new(pid)?.is_active() {
            return None;
        }
        let result = self.state.borrow().applications.get(&pid)?.read_focus();
        let focused = result.await.ok().flatten()?;
        if !RunningApp::new(pid)?.is_active() {
            return None;
        }
        self.state.borrow().accept_focus(&focused)
    }

    pub(super) async fn read_native_fullscreen(&self) -> Option<NativeFullscreenObservation> {
        let pid = NSWorkspace::sharedWorkspace()
            .frontmostApplication()?
            .processIdentifier();
        let result = self.state.borrow().applications.get(&pid)?.read_focus();
        let focused = result.await.ok().flatten()?;
        if NSWorkspace::sharedWorkspace()
            .frontmostApplication()?
            .processIdentifier()
            != pid
        {
            return None;
        }
        let id = self.state.borrow().accept_focus(&focused)?;
        Some(NativeFullscreenObservation::new(
            id,
            pid,
            focused.metadata,
            focused.is_native_fullscreen,
            focused.position,
            focused.size,
        ))
    }

    pub(super) async fn read_title(&self, id: WindowId) -> Option<Option<String>> {
        let (native, pid, result) = {
            let state = self.state.borrow();
            let (native, application) = state.application_for_window(id).ok()?;
            (
                native,
                *state.window_pids.get(&native)?,
                application.read_title(native),
            )
        };
        let title = result.await.ok()?;
        let state = self.state.borrow();
        (state.native_ids.get(&id) == Some(&native) && state.eligible(native, pid)).then_some(title)
    }
}

impl WindowServer for NativeWindowServer {
    fn set_frame(&self, id: WindowId, rect: PixelRect) -> Result<()> {
        let state = self.state.borrow();
        let (native, application) = state.application_for_window(id)?;
        application.set_frame(native, rect);
        Ok(())
    }
    fn focus(&self, id: WindowId) -> Result<()> {
        self.state.borrow_mut().request_focus(id)
    }
    fn focus_overlay(&self) {
        self.state.borrow_mut().focus_overlay();
    }
    fn hide_at(
        &self,
        id: WindowId,
        x: FractionalCoordinate<Logical>,
        y: FractionalCoordinate<Logical>,
    ) -> Result<()> {
        let state = self.state.borrow();
        let (native, application) = state.application_for_window(id)?;
        application.hide_at(native, x, y);
        Ok(())
    }
    fn minimize(&self, id: WindowId) -> Result<()> {
        let state = self.state.borrow();
        let (native, application) = state.application_for_window(id)?;
        application.minimize(native);
        Ok(())
    }
    fn unminimize(&self, id: WindowId) -> Result<()> {
        let state = self.state.borrow();
        let (native, application) = state.application_for_window(id)?;
        application.unminimize(native);
        Ok(())
    }
    fn close(&self, id: WindowId) -> Result<()> {
        let state = self.state.borrow();
        let (native, application) = state.application_for_window(id)?;
        application.close(native);
        Ok(())
    }
}
