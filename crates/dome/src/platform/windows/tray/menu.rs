use crate::action::{Action, FocusTarget, WorkspaceInfo, WorkspaceState};
use windows::Win32::Foundation::{HWND, LPARAM, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreatePopupMenu, DestroyMenu, GetCursorPos, HMENU, MENU_ITEM_FLAGS, MF_CHECKED,
    MF_POPUP, MF_SEPARATOR, MF_STRING, PostMessageW, SetForegroundWindow, TPM_NONOTIFY,
    TPM_RETURNCMD, TPM_RIGHTBUTTON, TrackPopupMenu, WM_CONTEXTMENU, WM_NULL, WM_RBUTTONUP,
};
use windows::core::PCWSTR;

use std::rc::Rc;

pub(super) fn is_tray_context_menu(lparam: LPARAM) -> bool {
    matches!((lparam.0 & 0xFFFF) as u32, WM_RBUTTONUP | WM_CONTEXTMENU)
}

/// Shows `entries` as a popup menu owned by `hwnd` and returns the chosen row,
/// or `None` when dismissed. The caller must release shared handler and menu-state
/// borrows before entering this modal message loop.
pub(super) fn show_context_menu(hwnd: HWND, entries: &[MenuEntry]) -> Option<&MenuItem> {
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
    let mut next_id = 1;
    for entry in entries {
        append_entry(menu, entry, &mut next_id);
    }

    let mut pt = POINT::default();
    if let Err(e) = unsafe { GetCursorPos(&mut pt) } {
        tracing::warn!(?e, "GetCursorPos failed");
        if let Err(e2) = unsafe { DestroyMenu(menu) } {
            tracing::warn!(?e2, "DestroyMenu failed after GetCursorPos error");
        }
        return None;
    }

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

    let index = cmd.checked_sub(1)? as usize;
    entries
        .iter()
        .flat_map(|entry| match entry {
            MenuEntry::Item(item) => std::slice::from_ref(item),
            MenuEntry::Submenu { items, .. } => items.as_slice(),
            MenuEntry::Separator => &[],
        })
        .nth(index)
}

#[derive(Clone)]
pub(super) struct MenuItem {
    pub label: String,
    pub action: Rc<dyn Fn()>,
    pub checked: bool,
}

#[derive(Clone, Debug)]
pub(super) enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu { label: String, items: Vec<MenuItem> },
}

pub(super) fn focused_tooltip(workspaces: &[WorkspaceInfo]) -> String {
    workspaces
        .iter()
        .find(|w| w.is_focused)
        .map(|w| truncate_tooltip(&w.name))
        .unwrap_or_default()
}

pub(super) fn build_menu(workspaces: &[WorkspaceInfo], send: Rc<dyn Fn(Action)>) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    for group in group_by_monitor(workspaces) {
        let items = group
            .rows
            .iter()
            .map(|ws| MenuItem {
                label: ws.name.clone(),
                action: {
                    let action = Action::Focus {
                        target: FocusTarget::Workspace {
                            name: ws.name.clone(),
                            monitor: Some(ws.monitor.clone()),
                        },
                    };
                    let send = send.clone();
                    Rc::new(move || send(action.clone()))
                },
                checked: ws.is_focused,
            })
            .collect();
        let mut label = group.monitor.to_string();
        if group.detached {
            label.push_str(" (detached)");
        }
        entries.push(MenuEntry::Submenu { label, items });
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Item(MenuItem {
        label: "Exit Dome".to_string(),
        action: Rc::new(move || send(Action::Exit)),
        checked: false,
    }));
    entries
}

fn to_wide_null(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn append_item(menu: HMENU, item: &MenuItem, next_id: &mut u32) {
    let id = *next_id;
    *next_id += 1;
    let flags: MENU_ITEM_FLAGS = if item.checked {
        MF_STRING | MF_CHECKED
    } else {
        MF_STRING
    };
    let label = to_wide_null(&item.label);
    if let Err(e) = unsafe { AppendMenuW(menu, flags, id as usize, PCWSTR(label.as_ptr())) } {
        tracing::warn!(?e, "AppendMenuW item failed");
    }
}

fn append_entry(menu: HMENU, entry: &MenuEntry, next_id: &mut u32) {
    match entry {
        MenuEntry::Separator => {
            if let Err(e) = unsafe { AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null()) } {
                tracing::warn!(?e, "AppendMenuW separator failed");
            }
        }
        MenuEntry::Item(item) => append_item(menu, item, next_id),
        MenuEntry::Submenu { label, items } => {
            let submenu = match unsafe { CreatePopupMenu() } {
                Ok(m) => m,
                Err(e) => {
                    tracing::warn!(?e, "CreatePopupMenu (submenu) failed");
                    *next_id += items.len() as u32;
                    return;
                }
            };
            for item in items {
                append_item(submenu, item, next_id);
            }
            let label = to_wide_null(label);
            // Successful attachment transfers submenu destruction to the root menu.
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

const STATUS_TOOLTIP_MAX_CHARS: usize = 20;

fn truncate_tooltip(name: &str) -> String {
    if name.chars().count() <= STATUS_TOOLTIP_MAX_CHARS {
        return name.to_string();
    }
    let cutoff: String = name.chars().take(STATUS_TOOLTIP_MAX_CHARS - 1).collect();
    format!("{cutoff}\u{2026}")
}

struct MonitorGroup<'a> {
    monitor: &'a str,
    detached: bool,
    rows: Vec<&'a WorkspaceInfo>,
}

fn group_by_monitor(workspaces: &[WorkspaceInfo]) -> Vec<MonitorGroup<'_>> {
    let mut groups: Vec<MonitorGroup> = Vec::new();
    for ws in workspaces {
        let slot = match groups.iter_mut().find(|g| g.monitor == ws.monitor) {
            Some(g) => g,
            None => {
                groups.push(MonitorGroup {
                    monitor: &ws.monitor,
                    detached: true,
                    rows: Vec::new(),
                });
                groups.last_mut().unwrap()
            }
        };
        slot.rows.push(ws);
        if ws.state == WorkspaceState::Attached {
            slot.detached = false;
        }
    }
    groups
}

impl std::fmt::Debug for MenuItem {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MenuItem")
            .field("label", &self.label)
            .field("checked", &self.checked)
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn workspace(name: &str, monitor: &str, state: WorkspaceState, focused: bool) -> WorkspaceInfo {
        WorkspaceInfo {
            name: name.into(),
            monitor: monitor.into(),
            state,
            is_focused: focused,
            is_visible: focused,
        }
    }

    #[test]
    fn build_menu_groups_workspaces_and_preserves_row_actions() {
        let mut workspaces = vec![
            workspace("Parked", "Mixed", WorkspaceState::Parked, false),
            workspace("Other", "Second", WorkspaceState::Attached, false),
            workspace("Editing", "Mixed", WorkspaceState::Attached, true),
            workspace("Away", "Missing", WorkspaceState::Parked, false),
        ];
        let actions = Rc::new(RefCell::new(Vec::new()));
        let recorded = actions.clone();
        let entries = build_menu(
            &workspaces,
            Rc::new(move |action| recorded.borrow_mut().push(action)),
        );
        let [
            MenuEntry::Submenu { label, items, .. },
            MenuEntry::Submenu {
                label: second,
                items: second_items,
                ..
            },
            MenuEntry::Submenu {
                label: detached,
                items: detached_items,
                ..
            },
            MenuEntry::Separator,
            MenuEntry::Item(exit),
        ] = entries.as_slice()
        else {
            panic!("unexpected menu {entries:?}");
        };
        assert_eq!(label, "Mixed");
        assert_eq!(
            items
                .iter()
                .map(|item| (item.label.as_str(), item.checked))
                .collect::<Vec<_>>(),
            [("Parked", false), ("Editing", true)]
        );
        assert_eq!(second, "Second");
        assert_eq!(second_items.len(), 1);
        assert_eq!(second_items[0].label, "Other");
        assert!(!second_items[0].checked);
        assert_eq!(detached, "Missing (detached)");
        assert_eq!(detached_items.len(), 1);
        assert_eq!(detached_items[0].label, "Away");
        assert!(!detached_items[0].checked);

        assert_eq!(exit.label, "Exit Dome");
        assert!(!exit.checked);

        workspaces[2].name = "Renamed".into();
        workspaces[2].monitor = "Moved".into();
        (items[1].action)();
        (detached_items[0].action)();

        (exit.action)();
        let actions = actions.borrow();
        assert!(matches!(actions.as_slice(), [
            Action::Focus { target: FocusTarget::Workspace { name, monitor: Some(monitor) } },
            Action::Focus { target: FocusTarget::Workspace { name: away, monitor: Some(missing) } },
            Action::Exit,
        ] if name == "Editing" && monitor == "Mixed" && away == "Away" && missing == "Missing" ));
    }

    #[test]
    fn select_focused_tooltip_and_truncate_at_unicode_character_boundaries() {
        let mut workspaces = vec![
            workspace("Not focused", "First", WorkspaceState::Attached, false),
            workspace(&"界".repeat(20), "Second", WorkspaceState::Attached, true),
        ];
        assert_eq!(focused_tooltip(&workspaces), "界".repeat(20));
        workspaces[1].name.push('🦀');
        assert_eq!(
            focused_tooltip(&workspaces),
            format!("{}…", "界".repeat(19))
        );
        workspaces[1].name = "Editing 🦀".into();
        assert_eq!(focused_tooltip(&workspaces), "Editing 🦀");
        workspaces[1].is_focused = false;
        assert_eq!(focused_tooltip(&workspaces), "");
        assert_eq!(focused_tooltip(&[]), "");
    }
}
