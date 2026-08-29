use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU, MENU_ITEM_FLAGS, MF_CHECKED,
    MF_POPUP, MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_CONTEXTMENU, WM_NULL, WM_RBUTTONUP,
};
use windows::core::PCWSTR;

/// One selectable row in a tray context menu. `id` is the consumer's own value, returned
/// verbatim from `on_tray_menu_selected`.
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub id: u32,
    pub checked: bool,
}

/// One entry in a tray context menu. A submenu holds one level of items.
#[derive(Clone, Debug)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu { label: String, items: Vec<MenuItem> },
}

/// Whether the tray callback's payload is a context-menu request (right-click or menu
/// key). The legacy notify-icon protocol packs the triggering mouse message into the low
/// word of `lparam`.
pub(super) fn is_tray_context_menu(lparam: LPARAM) -> bool {
    matches!((lparam.0 & 0xFFFF) as u32, WM_RBUTTONUP | WM_CONTEXTMENU)
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn append_item(menu: HMENU, item: &MenuItem) {
    let flags: MENU_ITEM_FLAGS = if item.checked {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let label = to_wide_null(&item.label);
    if let Err(e) = unsafe { AppendMenuW(menu, flags, item.id as usize, PCWSTR(label.as_ptr())) } {
        tracing::warn!(?e, "AppendMenuW item failed");
    }
}

fn append_entry(menu: HMENU, entry: &MenuEntry) {
    match entry {
        MenuEntry::Separator => {
            if let Err(e) = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) } {
                tracing::warn!(?e, "AppendMenuW separator failed");
            }
        }
        MenuEntry::Item(item) => append_item(menu, item),
        MenuEntry::Submenu { label, items } => {
            let submenu = match unsafe { CreatePopupMenu() } {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(?e, "CreatePopupMenu (submenu) failed");
                    return;
                }
            };
            for item in items {
                append_item(submenu, item);
            }
            // The MF_POPUP id is the child HMENU handle. A child attached this way is
            // freed by the root DestroyMenu, so only the failed-attach branch frees it.
            let label = to_wide_null(label);
            if let Err(e) =
                unsafe { AppendMenuW(menu, MF_POPUP, submenu.0 as usize, PCWSTR(label.as_ptr())) }
            {
                tracing::warn!(?e, "AppendMenuW submenu failed");
                if let Err(e2) = unsafe { DestroyMenu(submenu) } {
                    tracing::warn!(?e2, "DestroyMenu (orphaned submenu) failed");
                }
            }
        }
    }
}

/// Shows `entries` as a popup menu owned by `hwnd` and returns the chosen `MenuItem::id`,
/// or `None` when dismissed. Runs a modal message loop (`TrackPopupMenu`), so the caller
/// must hold no `WindowState` borrow across this call.
pub(super) fn show_context_menu(hwnd: HWND, entries: &[MenuEntry]) -> Option<u32> {
    // TrackPopupMenu docs require the owner window to be foreground first, otherwise the
    // menu can fail to dismiss on click-outside.
    if !unsafe { SetForegroundWindow(hwnd) }.as_bool() {
        tracing::warn!("SetForegroundWindow failed for tray menu owner");
    }

    let menu = match unsafe { CreatePopupMenu() } {
        Ok(m) => m,
        Err(e) => {
            tracing::warn!(?e, "CreatePopupMenu failed");
            return None;
        }
    };
    for entry in entries {
        append_entry(menu, entry);
    }

    let mut pt = POINT::default();
    if let Err(e) = unsafe { GetCursorPos(&mut pt) } {
        tracing::warn!(?e, "GetCursorPos failed");
        if let Err(e2) = unsafe { DestroyMenu(menu) } {
            tracing::warn!(?e2, "DestroyMenu failed after GetCursorPos error");
        }
        return None;
    }

    // TPM_RETURNCMD returns the selected id directly instead of posting WM_COMMAND, and
    // TPM_NONOTIFY suppresses WM_MENUCOMMAND for the same reason. 0 means click-outside.
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

    // TrackPopupMenu docs recommend posting a dummy message so the menu dismisses cleanly
    // if the user right-clicks the tray twice in a row.
    if let Err(e) = unsafe { PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0)) } {
        tracing::warn!(?e, "PostMessageW(WM_NULL) failed");
    }
    if let Err(e) = unsafe { DestroyMenu(menu) } {
        tracing::warn!(?e, "DestroyMenu failed");
    }

    (cmd != 0).then_some(cmd)
}
