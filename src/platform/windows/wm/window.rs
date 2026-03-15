use std::collections::HashMap;

use glutin::display::Display;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Gdi::SetWindowRgn;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, SendInput, VK_MENU,
};
use windows::Win32::UI::Shell::{QUNS_RUNNING_D3D_FULL_SCREEN, SHQueryUserNotificationState};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumThreadWindows, GA_ROOTOWNER, GetAncestor, GetForegroundWindow, GetWindowRect,
    GetWindowThreadProcessId, HWND_TOPMOST, IsIconic, SW_MINIMIZE, SW_RESTORE, SWP_ASYNCWINDOWPOS,
    SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SetForegroundWindow, SetWindowPos,
    ShowWindowAsync, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};
use windows::core::{BOOL, PCWSTR};

use super::super::OFFSCREEN_POS;
use super::super::dome::WindowShow;
use super::super::handle::{ManagedHwnd, WindowMode, get_dimension, get_invisible_border};
use super::overlay::{OverlayRenderer, OwnedHwnd, build_window_border_region};
use crate::config::Config;
use crate::core::{Child, Dimension, WindowId, WindowPlacement};
use crate::overlay;

pub(super) struct ManagedWindow {
    handle: WindowHandle,
    overlay: Option<WindowOverlay>,
}

impl ManagedWindow {
    pub(super) fn new(
        display: &Display,
        hwnd: HWND,
        title: Option<String>,
        process: String,
        mode: WindowMode,
    ) -> Self {
        let handle = WindowHandle::new_from_create(hwnd, title, process, mode);
        let overlay = match WindowOverlay::new(display) {
            Ok(o) => Some(o),
            Err(e) => {
                tracing::debug!("Failed to create window overlay: {e:#}");
                None
            }
        };
        Self { handle, overlay }
    }

    pub(super) fn hwnd(&self) -> HWND {
        self.handle.hwnd()
    }

    pub(super) fn managed_hwnd(&self) -> ManagedHwnd {
        ManagedHwnd::new(self.handle.hwnd())
    }

    pub(super) fn mode(&self) -> WindowMode {
        self.handle.mode()
    }

    pub(super) fn set_mode(&mut self, mode: WindowMode) {
        self.handle.set_mode(mode);
    }

    pub(super) fn title(&self) -> Option<&str> {
        self.handle.title()
    }

    /// Position this window and its border overlay at the given placement.
    ///
    /// `z` is precomputed by the app-level z-order logic:
    /// - `Some(HWND_TOPMOST)`: window first, overlay second — both get TOPMOST,
    ///   call order ensures overlay ends up above window.
    /// - All other values (`Some(HWND_NOTOPMOST)`, `Some(specific_hwnd)`, `None`):
    ///   overlay first (`z`), window second (`overlay.hwnd()`). Placing a window
    ///   below a non-topmost overlay implicitly strips WS_EX_TOPMOST, so
    ///   HWND_NOTOPMOST needs no special path.
    /// - `None` means overlay gets SWP_NOZORDER, but the window still anchors
    ///   below the overlay via `overlay.hwnd()`.
    #[tracing::instrument(skip_all)]
    pub(super) fn show(
        &mut self,
        ws: &WindowShow,
        is_focused: bool,
        border: f32,
        config: &Config,
        z: Option<HWND>,
    ) {
        tracing::debug!("show {self} frame={:?} float={}", ws.frame, ws.is_float);

        if z == Some(HWND_TOPMOST) {
            self.handle.show(&ws.frame, border, ws.is_float, z);
            if let Some(overlay) = &mut self.overlay {
                overlay.update(ws, is_focused, config, z);
            }
        } else {
            if let Some(overlay) = &mut self.overlay {
                overlay.update(ws, is_focused, config, z);
            }
            let window_z = self.overlay.as_ref().map(|o| o.hwnd());
            self.handle.show(&ws.frame, border, ws.is_float, window_z);
        }
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn hide(&mut self) {
        tracing::debug!("hide {self}");
        self.handle.hide();
        self.hide_overlay();
    }

    #[tracing::instrument(skip_all)]
    pub(super) fn set_fullscreen(&mut self, dim: &Dimension) {
        tracing::debug!("set_fullscreen {self} dim={dim:?}");
        self.handle.set_fullscreen(dim);
        self.hide_overlay();
    }

    pub(super) fn focus(&self) {
        self.handle.focus();
    }

    pub(super) fn set_title(&mut self, title: Option<String>) {
        self.handle.set_title(title);
    }

    fn hide_overlay(&mut self) {
        if let Some(overlay) = &mut self.overlay {
            overlay.hide();
        }
    }
}

impl std::fmt::Display for ManagedWindow {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.handle.fmt(f)
    }
}

pub(super) struct Registry {
    windows: HashMap<ManagedHwnd, WindowId>,
    reverse: HashMap<WindowId, ManagedWindow>,
}

impl Registry {
    pub(super) fn new() -> Self {
        Self {
            windows: HashMap::new(),
            reverse: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, mw: ManagedWindow, id: WindowId) {
        tracing::info!(%mw, %id, "Window inserted");
        self.windows.insert(mw.managed_hwnd(), id);
        self.reverse.insert(id, mw);
    }

    pub(super) fn remove(&mut self, id: WindowId) {
        if let Some(mw) = self.reverse.remove(&id) {
            tracing::info!(%mw, %id, "Window removed");
            self.windows.remove(&mw.managed_hwnd());
        }
    }

    pub(super) fn get(&self, id: WindowId) -> Option<&ManagedWindow> {
        self.reverse.get(&id)
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> Option<&mut ManagedWindow> {
        self.reverse.get_mut(&id)
    }

    pub(super) fn get_by_hwnd(&self, hwnd: ManagedHwnd) -> Option<&ManagedWindow> {
        self.windows.get(&hwnd).and_then(|id| self.reverse.get(id))
    }

    pub(super) fn get_id(&self, hwnd: ManagedHwnd) -> Option<WindowId> {
        self.windows.get(&hwnd).copied()
    }

    pub(super) fn get_by_hwnd_mut(&mut self, hwnd: ManagedHwnd) -> Option<&mut ManagedWindow> {
        self.windows
            .get(&hwnd)
            .and_then(|id| self.reverse.get_mut(id))
    }

    pub(super) fn set_title(&mut self, hwnd: ManagedHwnd, title: Option<String>) {
        if let Some(&id) = self.windows.get(&hwnd)
            && let Some(mw) = self.reverse.get_mut(&id)
        {
            mw.set_title(title);
        }
    }

    pub(super) fn resolve_tab_titles(&self, children: &[Child]) -> Vec<String> {
        children
            .iter()
            .map(|c| match c {
                Child::Window(wid) => self
                    .get(*wid)
                    .and_then(|mw| mw.title())
                    .unwrap_or("<no title>")
                    .to_owned(),
                Child::Container(_) => "Container".to_owned(),
            })
            .collect()
    }
}

pub(super) fn is_d3d_exclusive_fullscreen_active() -> bool {
    unsafe { SHQueryUserNotificationState() }
        .is_ok_and(|state| state == QUNS_RUNNING_D3D_FULL_SCREEN)
}

pub(super) const WINDOW_OVERLAY_CLASS: PCWSTR = windows::core::w!("DomeWindowOverlay");

// HWND is safe to send across threads, but doesn't implement Send
// https://users.rust-lang.org/t/moving-window-hwnd-or-handle-from-one-thread-to-a-new-one/126341/2
#[derive(Clone)]
struct WindowHandle {
    hwnd: HWND,
    title: Option<String>,
    process: String,
    mode: WindowMode,
}

unsafe impl Send for WindowHandle {}

impl WindowHandle {
    /// Construct from pre-queried data — no Win32 calls. Used by the UI thread
    /// when it receives a WindowCreate from the dome thread.
    fn new_from_create(
        hwnd: HWND,
        title: Option<String>,
        process: String,
        mode: WindowMode,
    ) -> Self {
        Self {
            hwnd,
            title,
            process,
            mode,
        }
    }

    fn hwnd(&self) -> HWND {
        self.hwnd
    }

    fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    fn set_title(&mut self, title: Option<String>) {
        self.title = title;
    }

    fn mode(&self) -> WindowMode {
        self.mode
    }

    fn set_mode(&mut self, mode: WindowMode) {
        self.mode = mode;
    }

    fn set_fullscreen(&mut self, dim: &Dimension) {
        match self.mode {
            WindowMode::FullscreenBorderless | WindowMode::FullscreenExclusive => return,
            _ => {}
        }
        self.mode = WindowMode::ManagedFullscreen;
        set_position(self.hwnd, dim, None);
    }

    fn show(&mut self, dim: &Dimension, border: f32, is_float: bool, z_after: Option<HWND>) {
        if self.mode == WindowMode::FullscreenExclusive {
            return;
        }
        let content = apply_inset(*dim, border);
        set_position(self.hwnd, &content, z_after);
        if is_float && self.mode != WindowMode::Float {
            set_children_topmost(self.hwnd);
        }
        self.mode = if is_float {
            WindowMode::Float
        } else {
            WindowMode::Tiling
        };
    }

    fn hide(&self) {
        match self.mode {
            WindowMode::FullscreenExclusive => {}
            WindowMode::FullscreenBorderless => {
                unsafe { ShowWindowAsync(self.hwnd, SW_MINIMIZE) };
            }
            _ => {
                unsafe {
                    SetWindowPos(
                        self.hwnd,
                        None,
                        OFFSCREEN_POS as i32,
                        OFFSCREEN_POS as i32,
                        0,
                        0,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
                    )
                    .ok()
                };
            }
        }
    }

    fn focus(&self) {
        if self.mode == WindowMode::FullscreenExclusive {
            return;
        }
        if unsafe { GetForegroundWindow() } == self.hwnd {
            return;
        }
        // Simulate ALT key press to bypass SetForegroundWindow restrictions
        // https://gist.github.com/Aetopia/1581b40f00cc0cadc93a0e8ccb65dc8c
        let inputs = [
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        ..Default::default()
                    },
                },
            },
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_MENU,
                        dwFlags: KEYEVENTF_KEYUP,
                        ..Default::default()
                    },
                },
            },
        ];
        unsafe { SendInput(&inputs, size_of::<INPUT>() as i32) };
        if !unsafe { SetForegroundWindow(self.hwnd) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed, another app may have focus lock");
        }
    }
}

impl std::fmt::Display for WindowHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let title = self.title().unwrap_or("<no title>");
        write!(
            f,
            "'{title}' from '{}' hwnd={:?} mode={}",
            self.process, self.hwnd, self.mode
        )
    }
}

/// `renderer` is declared before `window` so it drops first —
/// GL cleanup runs while the window's HDC is still alive.
struct WindowOverlay {
    renderer: OverlayRenderer,
    width: u32,
    height: u32,
    window: OwnedHwnd,
}

impl WindowOverlay {
    fn new(display: &Display) -> anyhow::Result<Self> {
        let window = OwnedHwnd::new(WINDOW_OVERLAY_CLASS, WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE)?;
        let renderer = OverlayRenderer::new(display, window.hwnd(), 1, 1)?;
        Ok(Self {
            renderer,
            width: 1,
            height: 1,
            window,
        })
    }

    fn hwnd(&self) -> HWND {
        self.window.hwnd()
    }

    /// Update border content and position. `z_after` is caller-decided:
    /// `Some(hwnd)` places this overlay after that HWND, `None` uses `SWP_NOZORDER`.
    fn update(
        &mut self,
        ws: &WindowShow,
        is_focused: bool,
        config: &Config,
        z_after: Option<HWND>,
    ) {
        let vf = ws.visible_frame;
        let w = vf.width.max(1.0) as u32;
        let h = vf.height.max(1.0) as u32;

        if self.width != w || self.height != h {
            self.renderer.resize(w, h);
            self.width = w;
            self.height = h;
        }

        let placement = WindowPlacement {
            id: ws.id,
            frame: ws.frame,
            visible_frame: ws.visible_frame,
            is_float: ws.is_float,
            is_focused,
            spawn_mode: ws.spawn_mode,
        };

        self.renderer.render(w, h, 1.0, vec![], |ui| {
            overlay::paint_window_border(ui.painter(), &placement, config);
        });

        let region = build_window_border_region(&placement, config);
        unsafe { SetWindowRgn(self.window.hwnd(), Some(region), true) };

        let mut flags = SWP_NOACTIVATE;
        if z_after.is_none() {
            flags |= SWP_NOZORDER;
        }
        unsafe {
            SetWindowPos(
                self.window.hwnd(),
                z_after,
                vf.x as i32,
                vf.y as i32,
                w as i32,
                h as i32,
                flags,
            )
            .ok();
        }

        self.window.show();
    }

    fn hide(&mut self) {
        self.window.hide();
    }
}

fn apply_inset(dim: Dimension, border: f32) -> Dimension {
    Dimension {
        x: dim.x + border,
        y: dim.y + border,
        width: (dim.width - 2.0 * border).max(0.0),
        height: (dim.height - 2.0 * border).max(0.0),
    }
}

fn for_each_owned<F: FnMut(HWND)>(hwnd: HWND, callback: F) {
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, None) };
    if thread_id == 0 {
        return;
    }

    unsafe extern "system" fn enum_proc<F: FnMut(HWND)>(child: HWND, lparam: LPARAM) -> BOOL {
        let (owner, callback) = unsafe { &mut *(lparam.0 as *mut (HWND, F)) };
        let root_owner = unsafe { GetAncestor(child, GA_ROOTOWNER) };
        if root_owner == *owner && child != *owner {
            callback(child);
        }
        BOOL(1)
    }

    let mut data = (hwnd, callback);
    // BOOL is FALSE when the callback returns FALSE or no windows are found,
    // neither of which is an error condition.
    unsafe {
        EnumThreadWindows(
            thread_id,
            Some(enum_proc::<F>),
            LPARAM(&mut data as *mut _ as isize),
        )
        .ok()
        .ok();
    }
}

/// Position a window via `SetWindowPos`. `z_after` is the `hWndInsertAfter`
/// parameter — the positioned window is placed below the supplied HWND.
/// `None` adds `SWP_NOZORDER` (no z-order change).
fn set_position(hwnd: HWND, dim: &Dimension, z_after: Option<HWND>) {
    if unsafe { IsIconic(hwnd) }.as_bool() {
        let _was_visible = unsafe { ShowWindowAsync(hwnd, SW_RESTORE) };
    }
    let old = get_dimension(hwnd);
    let (left, top, right, bottom) = get_invisible_border(hwnd);
    let mut flags = SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS;
    if z_after.is_none() {
        flags |= SWP_NOZORDER;
    }
    unsafe {
        SetWindowPos(
            hwnd,
            z_after,
            dim.x as i32 - left,
            dim.y as i32 - top,
            dim.width as i32 + left + right,
            dim.height as i32 + top + bottom,
            flags,
        )
        .ok()
    };
    let dx = (dim.x as i32 - left) - old.x as i32;
    let dy = (dim.y as i32 - top) - old.y as i32;
    if dx != 0 || dy != 0 {
        for_each_owned(hwnd, |child| {
            let mut rect = RECT::default();
            if unsafe { GetWindowRect(child, &mut rect).is_ok() } {
                unsafe {
                    SetWindowPos(
                        child,
                        None,
                        rect.left + dx,
                        rect.top + dy,
                        0,
                        0,
                        SWP_NOZORDER | SWP_NOACTIVATE | SWP_NOSIZE | SWP_ASYNCWINDOWPOS,
                    )
                    .ok()
                };
            }
        });
    }
}

fn set_children_topmost(hwnd: HWND) {
    for_each_owned(hwnd, |child| {
        unsafe {
            SetWindowPos(
                child,
                Some(windows::Win32::UI::WindowsAndMessaging::HWND_TOPMOST),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            )
            .ok()
        };
    });
}
