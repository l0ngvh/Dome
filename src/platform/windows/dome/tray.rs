use anyhow::Result;
use dome_auxiliary_window::{MenuEntry, MenuItem};
use windows::Win32::Foundation::HINSTANCE;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    HICON, IMAGE_ICON, LR_DEFAULTSIZE, LR_SHARED, LoadImageW,
};
use windows::core::PCWSTR;

use crate::action::{Action, FocusTarget, WorkspaceInfo, WorkspaceState};

const TRAY_CMD_EXIT: u32 = 1;
const TRAY_CMD_WORKSPACE_BASE: u32 = 100;
const STATUS_TOOLTIP_MAX_CHARS: usize = 20;
const TRAY_ICON_RESOURCE_ID: u16 = 1;

/// Each workspace row's id is its index into `workspaces`, offset by
/// `TRAY_CMD_WORKSPACE_BASE`, so `command_to_action` can recover it.
pub(super) fn build_menu(workspaces: &[WorkspaceInfo]) -> Vec<MenuEntry> {
    let mut entries = Vec::new();
    for group in group_by_monitor(workspaces) {
        let items = group
            .rows
            .iter()
            .map(|(idx, ws)| MenuItem {
                label: ws.name.clone(),
                id: TRAY_CMD_WORKSPACE_BASE + *idx as u32,
                checked: ws.is_focused,
            })
            .collect();
        let mut label = group.monitor.to_string();
        if group.detached {
            // The origin monitor is gone. Clicking an entry surfaces the parked workspace
            // on the primary. The submenu stays enabled, because a grayed popup will not
            // expand on Win32, which would strand the parked rows out of reach.
            label.push_str(" (detached)");
        }
        entries.push(MenuEntry::Submenu { label, items });
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Item(MenuItem {
        label: "Exit Dome".to_string(),
        id: TRAY_CMD_EXIT,
        checked: false,
    }));
    entries
}

/// `LR_DEFAULTSIZE` picks the system-tray size for the current DPI. `LR_SHARED` lets
/// Windows cache the handle, so no `DestroyIcon` is owed, which suits an app-lifetime
/// resource.
pub(super) fn load_tray_icon() -> Result<HICON> {
    let hmodule = unsafe { GetModuleHandleW(None) }?;
    let instance = HINSTANCE(hmodule.0);
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
    Ok(HICON(icon_handle.0))
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
                monitor: Some(ws.monitor.clone()),
            }));
        }
    }
    None
}

struct MonitorGroup<'a> {
    monitor: &'a str,
    detached: bool,
    rows: Vec<(usize, &'a WorkspaceInfo)>,
}

// Buckets rows by their disambiguated `monitor` string, preserving
// first-appearance order so the rendered submenus are deterministic (a
// `HashMap` would leak iteration-order nondeterminism into the menu). Each row
// carries its original index into `workspaces` so the command id stays
// `TRAY_CMD_WORKSPACE_BASE + original_index` regardless of grouping. A group is
// detached when it holds no Attached rows: a present monitor contributes
// Attached rows, while a gone origin contributes only Parked rows.
fn group_by_monitor(workspaces: &[WorkspaceInfo]) -> Vec<MonitorGroup<'_>> {
    let mut groups: Vec<MonitorGroup> = Vec::new();
    for (i, ws) in workspaces.iter().enumerate() {
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
        slot.rows.push((i, ws));
        if ws.state == WorkspaceState::Attached {
            slot.detached = false;
        }
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(
        name: &str,
        monitor: &str,
        state: WorkspaceState,
        focused: bool,
        visible: bool,
    ) -> WorkspaceInfo {
        WorkspaceInfo {
            name: name.into(),
            monitor: monitor.into(),
            state,
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
        let list = vec![ws("1", "1", WorkspaceState::Attached, false, true)];
        assert_eq!(focused_tooltip(&list), "");
    }

    #[test]
    fn focused_tooltip_picks() {
        let list = vec![
            ws("1", "1", WorkspaceState::Attached, false, true),
            ws("2", "2", WorkspaceState::Attached, true, true),
        ];
        assert_eq!(focused_tooltip(&list), "2");
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
        let list = vec![
            ws("Alpha", "Alpha", WorkspaceState::Attached, false, true),
            ws("Beta", "Beta", WorkspaceState::Attached, true, true),
        ];
        let action = command_to_action(TRAY_CMD_WORKSPACE_BASE + 1, &list).unwrap();
        match action {
            Action::Focus(FocusTarget::Workspace { name, .. }) => assert_eq!(name, "Beta"),
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn cmd_attached_focus_carries_monitor() {
        let list = vec![
            ws("Alpha", "Mon1", WorkspaceState::Attached, false, true),
            ws("Beta", "Mon2", WorkspaceState::Attached, true, true),
        ];
        let action = command_to_action(TRAY_CMD_WORKSPACE_BASE + 1, &list).unwrap();
        match action {
            Action::Focus(FocusTarget::Workspace { name, monitor }) => {
                assert_eq!(name, "Beta");
                assert_eq!(monitor, Some("Mon2".to_string()));
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn cmd_parked_emits_unified_monitor() {
        let list = vec![
            ws("Alpha", "Mon1", WorkspaceState::Attached, true, true),
            ws("Ghost", "GoneOrigin", WorkspaceState::Parked, false, false),
        ];
        let action = command_to_action(TRAY_CMD_WORKSPACE_BASE + 1, &list).unwrap();
        match action {
            Action::Focus(FocusTarget::Workspace { name, monitor }) => {
                assert_eq!(name, "Ghost");
                assert_eq!(monitor, Some("GoneOrigin".to_string()));
            }
            other => panic!("wrong variant: {other:?}"),
        }
    }

    #[test]
    fn cmd_workspace_out_of_range() {
        assert!(command_to_action(TRAY_CMD_WORKSPACE_BASE + 5, &[]).is_none());
    }

    #[test]
    fn build_menu_groups_and_appends_exit() {
        let list = vec![
            ws("Main", "Mon1", WorkspaceState::Attached, true, true),
            ws("Side", "Mon1", WorkspaceState::Attached, false, true),
            ws("Ghost", "Gone", WorkspaceState::Parked, false, false),
        ];
        let entries = build_menu(&list);
        assert_eq!(entries.len(), 4);
        match &entries[0] {
            MenuEntry::Submenu { label, items } => {
                assert_eq!(label, "Mon1");
                assert_eq!(items.len(), 2);
                assert_eq!(items[0].id, TRAY_CMD_WORKSPACE_BASE);
                assert!(items[0].checked);
                assert!(!items[1].checked);
            }
            other => panic!("expected submenu, got {other:?}"),
        }
        match &entries[1] {
            MenuEntry::Submenu { label, items } => {
                assert_eq!(label, "Gone (detached)");
                assert_eq!(items[0].id, TRAY_CMD_WORKSPACE_BASE + 2);
            }
            other => panic!("expected detached submenu, got {other:?}"),
        }
        assert!(matches!(entries[2], MenuEntry::Separator));
        match &entries[3] {
            MenuEntry::Item(item) => assert_eq!(item.id, TRAY_CMD_EXIT),
            other => panic!("expected exit item, got {other:?}"),
        }
    }

    #[test]
    fn group_by_monitor_splits_by_monitor() {
        let list = vec![
            ws("1", "A", WorkspaceState::Attached, false, true),
            ws("1", "B", WorkspaceState::Attached, false, true),
            ws("2", "A", WorkspaceState::Attached, false, true),
        ];
        let groups = group_by_monitor(&list);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].monitor, "A");
        assert_eq!(
            groups[0].rows.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![0, 2]
        );
        assert_eq!(groups[1].monitor, "B");
        assert_eq!(
            groups[1].rows.iter().map(|(i, _)| *i).collect::<Vec<_>>(),
            vec![1]
        );
    }

    #[test]
    fn group_by_monitor_preserves_original_index() {
        let list = vec![
            ws("a", "A", WorkspaceState::Attached, false, true),
            ws("b", "B", WorkspaceState::Attached, false, true),
            ws("c", "A", WorkspaceState::Attached, false, true),
            ws("d", "C", WorkspaceState::Attached, false, true),
        ];
        let groups = group_by_monitor(&list);
        for group in &groups {
            for (idx, row) in &group.rows {
                assert_eq!(list[*idx].name, row.name);
            }
        }
    }

    #[test]
    fn group_by_monitor_detached_when_no_attached() {
        let list = vec![
            ws("live", "Present", WorkspaceState::Attached, true, true),
            ws("ghost1", "Gone", WorkspaceState::Parked, false, false),
            ws("ghost2", "Gone", WorkspaceState::Parked, false, false),
        ];
        let groups = group_by_monitor(&list);
        let present = groups.iter().find(|g| g.monitor == "Present").unwrap();
        let gone = groups.iter().find(|g| g.monitor == "Gone").unwrap();
        assert!(!present.detached);
        assert!(gone.detached);
    }
}
