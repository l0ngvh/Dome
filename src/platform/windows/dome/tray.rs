use std::cell::RefCell;
use std::mem::size_of;

use anyhow::{Result, anyhow};
use windows::Win32::Foundation::{HINSTANCE, HWND, LPARAM, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HICON, IMAGE_ICON, LR_DEFAULTSIZE,
    LR_SHARED, LoadImageW, MENU_ITEM_FLAGS, MF_CHECKED, MF_SEPARATOR, MF_STRING, PostMessageW,
    SetForegroundWindow, TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_APP,
    WM_NULL,
};
use windows::core::PCWSTR;

use crate::action::{Action, Actions, FocusTarget};
use crate::core::WorkspaceInfo;
use crate::platform::windows::HubSender;
use crate::platform::windows::dome::HubEvent;

pub(in crate::platform::windows) const TRAY_CALLBACK_MSG: u32 = WM_APP + 5;

const TRAY_CMD_EXIT: u32 = 1;
const TRAY_CMD_WORKSPACE_BASE: u32 = 100;
const STATUS_TOOLTIP_MAX_CHARS: usize = 20;
const TRAY_ICON_RESOURCE_ID: u16 = 1;
const TRAY_UID: u32 = 1;

/// The callback HWND lives on `AppWindow`. This struct only owns the shell
/// notification data and the workspace list backing the popup menu.
pub(in crate::platform::windows) struct TrayIndicator {
    data: RefCell<NOTIFYICONDATAW>,
    hub_sender: HubSender,
    workspaces: RefCell<Vec<WorkspaceInfo>>,
}

impl TrayIndicator {
    pub(super) fn new(hub_sender: HubSender, callback_hwnd: HWND) -> Result<Box<Self>> {
        let hmodule = unsafe { GetModuleHandleW(None) }?;
        let instance = HINSTANCE(hmodule.0);

        // LR_DEFAULTSIZE picks the correct system-tray icon size for the
        // current DPI. LR_SHARED lets Windows cache the icon handle instead of
        // demanding a matching DestroyIcon on drop, which suits an app-lifetime
        // resource.
        let icon_handle = unsafe {
            LoadImageW(
                Some(instance),
                PCWSTR(TRAY_ICON_RESOURCE_ID as usize as *const u16),
                IMAGE_ICON,
                0,
                0,
                LR_DEFAULTSIZE | LR_SHARED,
            )
        }?;
        let hicon = HICON(icon_handle.0);

        let data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: callback_hwnd,
            uID: TRAY_UID,
            uFlags: NIF_MESSAGE | NIF_ICON | NIF_TIP,
            uCallbackMessage: TRAY_CALLBACK_MSG,
            hIcon: hicon,
            szTip: [0u16; 128],
            ..Default::default()
        };

        let this = Box::new(Self {
            data: RefCell::new(data),
            hub_sender,
            workspaces: RefCell::new(Vec::new()),
        });

        this.add_icon()?;
        Ok(this)
    }

    pub(super) fn add_icon(&self) -> Result<()> {
        let ok = unsafe { Shell_NotifyIconW(NIM_ADD, &*self.data.borrow()) };
        if !ok.as_bool() {
            return Err(anyhow!("Shell_NotifyIconW(NIM_ADD) failed"));
        }
        tracing::info!("tray icon added");
        Ok(())
    }

    pub(in crate::platform::windows) fn update(&self, workspaces: &[WorkspaceInfo]) {
        *self.workspaces.borrow_mut() = workspaces.to_vec();
        let tip = focused_tooltip(workspaces);
        let mut data = self.data.borrow_mut();
        write_wide_into(&mut data.szTip, &tip);
        let ok = unsafe { Shell_NotifyIconW(NIM_MODIFY, &*data) };
        if !ok.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_MODIFY) failed");
        }
    }

    pub(super) fn show_menu(&self, hwnd: HWND) {
        // TrackPopupMenu docs require the owner window to be foreground first,
        // otherwise the menu can fail to dismiss on click-outside.
        if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
            tracing::warn!("SetForegroundWindow failed for tray menu owner");
        }

        let menu = match unsafe { CreatePopupMenu() } {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!(?e, "CreatePopupMenu failed");
                return;
            }
        };

        let workspaces = self.workspaces.borrow();
        for (i, ws) in workspaces.iter().enumerate() {
            let flags: MENU_ITEM_FLAGS = if ws.is_focused {
                MF_STRING | MF_CHECKED
            } else {
                MF_STRING
            };
            let name_wide = to_wide_null(&ws.name);
            if let Err(e) = unsafe {
                AppendMenuW(
                    menu,
                    flags,
                    (TRAY_CMD_WORKSPACE_BASE + i as u32) as usize,
                    PCWSTR(name_wide.as_ptr()),
                )
            } {
                tracing::warn!(?e, "AppendMenuW workspace failed");
            }
        }
        if let Err(e) = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) } {
            tracing::warn!(?e, "AppendMenuW separator failed");
        }
        let exit_wide = to_wide_null("Exit Dome");
        if let Err(e) = unsafe {
            AppendMenuW(
                menu,
                MF_STRING,
                TRAY_CMD_EXIT as usize,
                PCWSTR(exit_wide.as_ptr()),
            )
        } {
            tracing::warn!(?e, "AppendMenuW exit failed");
        }

        let mut pt = POINT::default();
        if let Err(e) = unsafe { GetCursorPos(&mut pt) } {
            tracing::warn!(?e, "GetCursorPos failed");
            if let Err(e2) = unsafe { DestroyMenu(menu) } {
                tracing::warn!(?e2, "DestroyMenu failed after GetCursorPos error");
            }
            return;
        }

        // TPM_RETURNCMD makes TrackPopupMenu return the selected id directly
        // instead of posting WM_COMMAND. TPM_NONOTIFY suppresses WM_MENUCOMMAND
        // for the same reason. 0 return = click-outside.
        let cmd = unsafe {
            TrackPopupMenu(
                menu,
                TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
                pt.x,
                pt.y,
                None,
                hwnd,
                None,
            )
        }
        .0 as u32;

        // TrackPopupMenu docs recommend posting a dummy message so the menu
        // dismisses cleanly if the user right-clicks the tray twice in a row.
        if let Err(e) = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) } {
            tracing::warn!(?e, "PostMessageW(WM_NULL) failed");
        }
        if let Err(e) = unsafe { DestroyMenu(menu) } {
            tracing::warn!(?e, "DestroyMenu failed");
        }

        if let Some(action) = command_to_action(cmd, &workspaces) {
            self.hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])));
        }
    }
}

impl Drop for TrayIndicator {
    // NIM_DELETE must run while the AppWindow-owned callback HWND is still
    // alive. Shell_NotifyIconW keys the icon by (hWnd, uID), so a stale HWND
    // here would leave a dangling tray entry until the next explorer restart.
    // AppWindow declares `tray` before `hwnd`, so this drop runs first.
    fn drop(&mut self) {
        let ok = unsafe { Shell_NotifyIconW(NIM_DELETE, &*self.data.borrow()) };
        if !ok.as_bool() {
            tracing::warn!("Shell_NotifyIconW(NIM_DELETE) failed");
        }
        tracing::info!("tray icon removed");
    }
}

pub(super) fn truncate_tooltip(name: &str) -> String {
    if name.chars().count() <= STATUS_TOOLTIP_MAX_CHARS {
        return name.to_string();
    }
    let cutoff: String = name.chars().take(STATUS_TOOLTIP_MAX_CHARS - 1).collect();
    format!("{cutoff}\u{2026}")
}

pub(super) fn focused_tooltip(workspaces: &[WorkspaceInfo]) -> String {
    workspaces
        .iter()
        .find(|w| w.is_focused)
        .map(|w| truncate_tooltip(&w.name))
        .unwrap_or_default()
}

pub(super) fn write_wide_into(dst: &mut [u16], s: &str) {
    let wide: Vec<u16> = s.encode_utf16().collect();
    let n = wide.len().min(dst.len().saturating_sub(1));
    if n > 0 {
        dst[..n].copy_from_slice(&wide[..n]);
    }
    for slot in dst.iter_mut().skip(n) {
        *slot = 0;
    }
}

pub(super) fn command_to_action(cmd: u32, workspaces: &[WorkspaceInfo]) -> Option<Action> {
    if cmd == 0 {
        return None;
    }
    if cmd == TRAY_CMD_EXIT {
        return Some(Action::Exit);
    }
    if cmd >= TRAY_CMD_WORKSPACE_BASE {
        let idx = (cmd - TRAY_CMD_WORKSPACE_BASE) as usize;
        if let Some(ws) = workspaces.get(idx) {
            return Some(Action::Focus(FocusTarget::Workspace {
                name: ws.name.clone(),
            }));
        }
    }
    None
}

fn to_wide_null(s: &str) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_utf16().collect();
    v.push(0);
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(name: &str, focused: bool, visible: bool) -> WorkspaceInfo {
        WorkspaceInfo {
            name: name.into(),
            is_focused: focused,
            is_visible: visible,
            window_count: 0,
        }
    }

    #[test]
    fn truncate_short() {
        assert_eq!(truncate_tooltip("Main"), "Main");
    }

    #[test]
    fn truncate_exact() {
        let s: String = "a".repeat(STATUS_TOOLTIP_MAX_CHARS);
        assert_eq!(truncate_tooltip(&s), s);
    }

    #[test]
    fn truncate_long() {
        let s: String = "a".repeat(STATUS_TOOLTIP_MAX_CHARS + 5);
        let out = truncate_tooltip(&s);
        assert_eq!(out.chars().count(), STATUS_TOOLTIP_MAX_CHARS);
        assert!(out.ends_with('\u{2026}'));
    }

    #[test]
    fn focused_tooltip_none() {
        let list = vec![ws("1", false, true)];
        assert_eq!(focused_tooltip(&list), "");
    }

    #[test]
    fn focused_tooltip_picks() {
        let list = vec![ws("1", false, true), ws("2", true, true)];
        assert_eq!(focused_tooltip(&list), "2");
    }

    #[test]
    fn write_wide_truncates() {
        let mut buf = [1u16; 8];
        write_wide_into(&mut buf, "hello world");
        assert_eq!(buf[7], 0);
        assert_ne!(buf[6], 0);
    }

    #[test]
    fn write_wide_short() {
        let mut buf = [1u16; 8];
        write_wide_into(&mut buf, "hi");
        assert_eq!(buf[0], 'h' as u16);
        assert_eq!(buf[1], 'i' as u16);
        assert_eq!(buf[2], 0);
    }

    #[test]
    fn cmd_zero_none() {
        assert!(command_to_action(0, &[]).is_none());
    }

    #[test]
    fn cmd_exit() {
        assert!(matches!(
            command_to_action(TRAY_CMD_EXIT, &[]),
            Some(Action::Exit)
        ));
    }

    #[test]
    fn cmd_workspace() {
        let list = vec![ws("Alpha", false, true), ws("Beta", true, true)];
        let action = command_to_action(TRAY_CMD_WORKSPACE_BASE + 1, &list).unwrap();
        match action {
            Action::Focus(FocusTarget::Workspace { name }) => assert_eq!(name, "Beta"),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn cmd_workspace_out_of_range() {
        assert!(command_to_action(TRAY_CMD_WORKSPACE_BASE + 5, &[]).is_none());
    }
}
