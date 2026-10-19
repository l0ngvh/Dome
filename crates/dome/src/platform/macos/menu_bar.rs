use crate::action::{Action, FocusTarget, WorkspaceInfo, WorkspaceState};
use objc2::rc::Retained;
use objc2::{
    AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel,
};
use objc2_app_kit::{
    NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuItem, NSStatusBar,
    NSStatusBarButton, NSStatusItem,
};
use objc2_foundation::{NSData, NSObject, NSObjectProtocol, NSSize, NSString};
use std::rc::Rc;

pub(super) trait MenuBar {
    fn update_workspaces(&self, workspaces: &[WorkspaceInfo]);
}

pub(super) struct NativeMenuBar {
    button: Retained<NSStatusBarButton>,
    status_item: Retained<NSStatusItem>,
    on_action: Rc<dyn Fn(Action)>,
}

impl NativeMenuBar {
    pub(super) fn new(image: &NSImage, on_action: Rc<dyn Fn(Action)>) -> Self {
        let marker = MainThreadMarker::new().expect("menu bar created on main thread");
        let status_item = NSStatusBar::systemStatusBar()
            .statusItemWithLength(objc2_app_kit::NSSquareStatusItemLength);
        let button = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let button = status_item
                .button(marker)
                .expect("status item has a button");
            button.setImage(Some(image));
            button
        }));
        match button {
            Ok(button) => Self {
                button,
                status_item,
                on_action,
            },
            Err(panic) => {
                NSStatusBar::systemStatusBar().removeStatusItem(&status_item);
                std::panic::resume_unwind(panic);
            }
        }
    }
}

impl MenuBar for NativeMenuBar {
    fn update_workspaces(&self, workspaces: &[WorkspaceInfo]) {
        let marker = MainThreadMarker::new().expect("menu bar used on main thread");
        self.button
            .setToolTip(Some(&NSString::from_str(&focused_tooltip(workspaces))));
        let entries = build_menu(workspaces, self.on_action.clone());
        self.status_item
            .setMenu(Some(&build_native_menu(marker, &entries)));
    }
}

pub(super) fn load_icon(bytes: &[u8]) -> anyhow::Result<Retained<NSImage>> {
    let data = NSData::with_bytes(bytes);
    let image = NSImage::initWithData(NSImage::alloc(), &data)
        .ok_or_else(|| anyhow::anyhow!("status bar icon must decode"))?;
    image.setSize(NSSize::new(22.0, 22.0));
    image.setTemplate(true);
    Ok(image)
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DomeMenuAction"]
    #[ivars = Rc<dyn Fn()>]
    struct MenuAction;

    unsafe impl NSObjectProtocol for MenuAction {}

    impl MenuAction {
        #[unsafe(method(menuClicked:))]
        fn menu_clicked(&self, _sender: &NSMenuItem) {
            (self.ivars())();
        }
    }
);

impl NativeMenuBar {
    pub(super) fn remove(&self) {
        NSStatusBar::systemStatusBar().removeStatusItem(&self.status_item);
    }
}

fn make_item(mtm: MainThreadMarker, item: &MenuItem, empty: &NSString) -> Retained<NSMenuItem> {
    let title = NSString::from_str(&item.label);
    let allocated = MenuAction::alloc(mtm).set_ivars(item.action.clone());
    let action: Retained<MenuAction> = unsafe { msg_send![super(allocated), init] };
    let ns_item = unsafe {
        NSMenuItem::initWithTitle_action_keyEquivalent(
            NSMenuItem::alloc(mtm),
            &title,
            Some(sel!(menuClicked:)),
            empty,
        )
    };
    // NSMenuItem does not retain its target, so the row owns it through representedObject.
    unsafe {
        ns_item.setRepresentedObject(Some(&action));
        ns_item.setTarget(Some(&*action));
    }
    ns_item.setState(if item.checked {
        NSControlStateValueOn
    } else {
        NSControlStateValueOff
    });
    ns_item
}

fn build_native_menu(mtm: MainThreadMarker, entries: &[MenuEntry]) -> Retained<NSMenu> {
    let menu = NSMenu::new(mtm);
    menu.setAutoenablesItems(false);
    let empty = NSString::from_str("");
    for entry in entries {
        match entry {
            MenuEntry::Separator => menu.addItem(&NSMenuItem::separatorItem(mtm)),
            MenuEntry::Item(item) => menu.addItem(&make_item(mtm, item, &empty)),
            MenuEntry::Submenu {
                label,
                items,
                enabled,
            } => {
                let ns_title = NSString::from_str(label);
                let parent: Retained<NSMenuItem> = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        NSMenuItem::alloc(mtm),
                        &ns_title,
                        None,
                        &empty,
                    )
                };
                let submenu = NSMenu::new(mtm);
                submenu.setAutoenablesItems(false);
                for item in items {
                    submenu.addItem(&make_item(mtm, item, &empty));
                }
                parent.setSubmenu(Some(&submenu));
                parent.setEnabled(*enabled);
                menu.addItem(&parent);
            }
        }
    }
    menu
}

const STATUS_TOOLTIP_MAX_CHARS: usize = 20;

fn build_menu(workspaces: &[WorkspaceInfo], send: Rc<dyn Fn(Action)>) -> Vec<MenuEntry> {
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
        let enabled = !group.detached;
        entries.push(MenuEntry::Submenu {
            label,
            items,
            enabled,
        });
    }
    entries.push(MenuEntry::Separator);
    entries.push(MenuEntry::Item(MenuItem {
        label: "Exit Dome".to_string(),
        action: Rc::new(move || send(Action::Exit)),
        checked: false,
    }));
    entries
}

fn truncate_tooltip(name: &str) -> String {
    if name.chars().count() <= STATUS_TOOLTIP_MAX_CHARS {
        return name.to_string();
    }
    let cutoff: String = name.chars().take(STATUS_TOOLTIP_MAX_CHARS - 1).collect();
    format!("{cutoff}\u{2026}")
}

fn focused_tooltip(workspaces: &[WorkspaceInfo]) -> String {
    workspaces
        .iter()
        .find(|w| w.is_focused)
        .map(|w| truncate_tooltip(&w.name))
        .unwrap_or_default()
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

#[derive(Clone)]
struct MenuItem {
    label: String,
    action: Rc<dyn Fn()>,
    checked: bool,
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

#[derive(Clone, Debug)]
enum MenuEntry {
    Item(MenuItem),
    Separator,
    Submenu {
        label: String,
        items: Vec<MenuItem>,
        enabled: bool,
    },
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
            MenuEntry::Submenu {
                label,
                items,
                enabled,
            },
            MenuEntry::Submenu {
                label: second,
                items: second_items,
                ..
            },
            MenuEntry::Submenu {
                label: detached,
                items: detached_items,
                enabled: detached_enabled,
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
        assert!(*enabled);
        assert!(!detached_enabled);
        assert_eq!(exit.label, "Exit Dome");
        assert!(!exit.checked);

        workspaces[2].name = "Renamed".into();
        workspaces[2].monitor = "Moved".into();
        (items[1].action)();

        (exit.action)();
        let actions = actions.borrow();
        assert!(matches!(actions.as_slice(), [
            Action::Focus { target: FocusTarget::Workspace { name, monitor: Some(monitor) } },
            Action::Exit,
        ] if name == "Editing" && monitor == "Mixed" ));
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
