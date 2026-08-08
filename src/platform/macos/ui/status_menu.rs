use std::cell::RefCell;

use calloop::channel::Sender;
use objc2::AnyThread;
use objc2::rc::Retained;
use objc2::{DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, sel};
use objc2_app_kit::{
    NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuItem,
    NSSquareStatusItemLength, NSStatusBar, NSStatusBarButton, NSStatusItem,
};
use objc2_foundation::{NSData, NSInteger, NSObject, NSObjectProtocol, NSSize, NSString};

use crate::action::{Action, Actions, FocusTarget, WorkspaceInfo, WorkspaceState};
use crate::platform::macos::dome::HubEvent;

const STATUS_TOOLTIP_MAX_CHARS: usize = 20;

pub(super) struct StatusMenu {
    status_item: Retained<NSStatusItem>,
    button: Retained<NSStatusBarButton>,
    menu: Retained<NSMenu>,
    target: Retained<StatusMenuTarget>,
    last_workspaces: RefCell<Vec<(String, String, bool, WorkspaceState)>>,
}

impl StatusMenu {
    pub(super) fn new(mtm: MainThreadMarker, hub_sender: Sender<HubEvent>) -> Self {
        let status_bar = NSStatusBar::systemStatusBar();
        let status_item = status_bar.statusItemWithLength(NSSquareStatusItemLength);
        let button = status_item
            .button(mtm)
            .expect("NSStatusItem should always have a button");

        // Template PNG embedded at compile time. macOS auto-tints alpha-defined shapes
        // to match dark or light mode when setTemplate is true. Embedding avoids the
        // bundle-path search that NSImage::imageNamed uses, so cargo run and
        // cargo make bundle both work with no fork.
        const STATUS_BAR_ICON_PNG: &[u8] = include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/resources/macos/status_bar_icon.png"
        ));

        let data = NSData::with_bytes(STATUS_BAR_ICON_PNG);
        let image = NSImage::initWithData(NSImage::alloc(), &data)
            .expect("status_bar_icon.png must decode");
        image.setSize(NSSize::new(22.0, 22.0));
        image.setTemplate(true);
        button.setImage(Some(&image));

        let menu = NSMenu::new(mtm);
        status_item.setMenu(Some(&menu));

        let target = StatusMenuTarget::new(mtm, hub_sender);

        Self {
            status_item,
            button,
            menu,
            target,
            last_workspaces: RefCell::new(Vec::new()),
        }
    }

    pub(super) fn update(&self, mtm: MainThreadMarker, workspaces: &[WorkspaceInfo]) {
        let focused = workspaces
            .iter()
            .find(|w| w.is_focused)
            .map(|w| w.name.as_str())
            .unwrap_or("");
        let tip = truncate_tooltip(focused);
        let ns_tip = NSString::from_str(&tip);
        self.button.setToolTip(Some(&ns_tip));

        let changed = {
            let last = self.last_workspaces.borrow();
            workspaces_layout_changed(&last, workspaces)
        };

        if changed {
            self.menu.removeAllItems();
            // Manual enabled-state control so a disabled (grayed) detached submenu
            // title still expands. Without this, AppKit auto-disables items whose
            // action is unhandled.
            self.menu.setAutoenablesItems(false);

            let groups = group_workspaces(workspaces);
            self.target.set_click_targets(click_targets(workspaces));

            let empty = NSString::from_str("");
            for group in &groups {
                let mut title = group.monitor.clone();
                if group.detached {
                    title.push_str(" (detached)");
                }
                let ns_title = NSString::from_str(&title);

                let parent_alloc = NSMenuItem::alloc(mtm);
                let parent: Retained<NSMenuItem> = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        parent_alloc,
                        &ns_title,
                        None,
                        &empty,
                    )
                };

                let submenu = NSMenu::new(mtm);
                submenu.setAutoenablesItems(false);
                for entry in &group.entries {
                    let item_title = NSString::from_str(&entry.name);
                    let alloc = NSMenuItem::alloc(mtm);
                    let item: Retained<NSMenuItem> = unsafe {
                        NSMenuItem::initWithTitle_action_keyEquivalent(
                            alloc,
                            &item_title,
                            Some(sel!(workspaceClicked:)),
                            &empty,
                        )
                    };
                    item.setTag(entry.tag as NSInteger);
                    unsafe {
                        item.setTarget(Some(&self.target));
                    }
                    submenu.addItem(&item);
                }
                parent.setSubmenu(Some(&submenu));
                if group.detached {
                    // Grayed-but-expandable: the title dims but the submenu still
                    // opens because setAutoenablesItems(false) is set above. The
                    // " (detached)" suffix is the guaranteed marker, the gray-out
                    // is the secondary cue.
                    parent.setEnabled(false);
                }
                self.menu.addItem(&parent);
            }

            self.menu.addItem(&NSMenuItem::separatorItem(mtm));

            let exit_title = NSString::from_str("Exit Dome");
            let exit_alloc = NSMenuItem::alloc(mtm);
            let exit_item: Retained<NSMenuItem> = unsafe {
                NSMenuItem::initWithTitle_action_keyEquivalent(
                    exit_alloc,
                    &exit_title,
                    Some(sel!(exitClicked:)),
                    &empty,
                )
            };
            unsafe {
                exit_item.setTarget(Some(&self.target));
            }
            self.menu.addItem(&exit_item);

            *self.last_workspaces.borrow_mut() = workspaces
                .iter()
                .map(|w| {
                    (
                        w.name.clone(),
                        w.monitor.clone(),
                        w.is_visible,
                        w.state.clone(),
                    )
                })
                .collect();
        }

        let focused_index = workspaces.iter().position(|w| w.is_focused);
        for top in self.menu.itemArray().iter() {
            if let Some(sub) = top.submenu() {
                for item in sub.itemArray().iter() {
                    let on = Some(item.tag() as usize) == focused_index;
                    item.setState(if on {
                        NSControlStateValueOn
                    } else {
                        NSControlStateValueOff
                    });
                }
            }
        }
    }
}

impl Drop for StatusMenu {
    fn drop(&mut self) {
        NSStatusBar::systemStatusBar().removeStatusItem(&self.status_item);
    }
}

struct StatusMenuTargetIvars {
    hub_sender: Sender<HubEvent>,
    click_targets: RefCell<Vec<ClickTarget>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "DomeStatusMenuTarget"]
    #[ivars = StatusMenuTargetIvars]
    struct StatusMenuTarget;

    impl StatusMenuTarget {
        #[unsafe(method(workspaceClicked:))]
        fn workspace_clicked(&self, sender: &NSMenuItem) {
            let tag = sender.tag() as usize;
            let targets = self.ivars().click_targets.borrow();
            let Some(target) = targets.get(tag) else {
                return;
            };
            let action = Action::Focus(FocusTarget::Workspace {
                name: target.name.clone(),
                monitor: Some(target.monitor.clone()),
            });
            self.ivars()
                .hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])))
                .ok();
        }

        #[unsafe(method(exitClicked:))]
        fn exit_clicked(&self, _sender: &NSMenuItem) {
            self.ivars()
                .hub_sender
                .send(HubEvent::Action(Actions::new(vec![Action::Exit])))
                .ok();
        }
    }
);

unsafe impl NSObjectProtocol for StatusMenuTarget {}

impl StatusMenuTarget {
    fn new(mtm: MainThreadMarker, hub_sender: Sender<HubEvent>) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(StatusMenuTargetIvars {
            hub_sender,
            click_targets: RefCell::new(Vec::new()),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn set_click_targets(&self, targets: Vec<ClickTarget>) {
        *self.ivars().click_targets.borrow_mut() = targets;
    }
}

fn truncate_tooltip(name: &str) -> String {
    if name.chars().count() <= STATUS_TOOLTIP_MAX_CHARS {
        return name.to_string();
    }
    let cutoff: String = name.chars().take(STATUS_TOOLTIP_MAX_CHARS - 1).collect();
    format!("{cutoff}\u{2026}")
}

// is_visible is a diff key so a focus switch on a single-monitor host rebuilds
// the menu when the departing workspace becomes invisible. Cost is a dozen
// NSMenuItem allocs per switch, acceptable. monitor and state are diff keys so a
// regrouping or a park/attach transition also rebuilds.
fn workspaces_layout_changed(
    old: &[(String, String, bool, WorkspaceState)],
    new: &[WorkspaceInfo],
) -> bool {
    if old.len() != new.len() {
        return true;
    }
    old.iter().zip(new.iter()).any(|((n, m, v, s), w)| {
        n != &w.name || m != &w.monitor || *v != w.is_visible || *s != w.state
    })
}

#[derive(Debug, PartialEq)]
struct MonitorGroup {
    monitor: String,
    detached: bool,
    entries: Vec<GroupEntry>,
}

#[derive(Debug, PartialEq)]
struct GroupEntry {
    tag: usize,
    name: String,
}

#[derive(Debug, PartialEq)]
struct ClickTarget {
    name: String,
    // Live disambiguated name when Attached, origin name otherwise.
    monitor: String,
}

// Preserves first-appearance order of monitor strings, and workspace order within
// each group, so the menu layout is deterministic across rebuilds. A group is
// detached exactly when no row in it is Attached.
fn group_workspaces(workspaces: &[WorkspaceInfo]) -> Vec<MonitorGroup> {
    let mut groups: Vec<MonitorGroup> = Vec::new();
    for (i, ws) in workspaces.iter().enumerate() {
        let attached = ws.state == WorkspaceState::Attached;
        let entry = GroupEntry {
            tag: i,
            name: ws.name.clone(),
        };
        match groups.iter_mut().find(|g| g.monitor == ws.monitor) {
            Some(g) => {
                g.detached = g.detached && !attached;
                g.entries.push(entry);
            }
            None => groups.push(MonitorGroup {
                monitor: ws.monitor.clone(),
                detached: !attached,
                entries: vec![entry],
            }),
        }
    }
    groups
}

fn click_targets(workspaces: &[WorkspaceInfo]) -> Vec<ClickTarget> {
    workspaces
        .iter()
        .map(|ws| ClickTarget {
            name: ws.name.clone(),
            monitor: ws.monitor.clone(),
        })
        .collect()
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
    fn truncate_short_unchanged() {
        assert_eq!(truncate_tooltip("Main"), "Main");
    }

    #[test]
    fn truncate_exact_boundary() {
        let s: String = "a".repeat(STATUS_TOOLTIP_MAX_CHARS);
        assert_eq!(truncate_tooltip(&s), s);
    }

    #[test]
    fn truncate_long_gets_ellipsis() {
        let s: String = "a".repeat(STATUS_TOOLTIP_MAX_CHARS + 5);
        let out = truncate_tooltip(&s);
        assert_eq!(out.chars().count(), STATUS_TOOLTIP_MAX_CHARS);
        assert!(out.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_multibyte_boundary() {
        let s: String = "あ".repeat(STATUS_TOOLTIP_MAX_CHARS + 5);
        let out = truncate_tooltip(&s);
        assert_eq!(out.chars().count(), STATUS_TOOLTIP_MAX_CHARS);
        assert!(out.ends_with('\u{2026}'));
    }

    #[test]
    fn truncate_empty() {
        assert_eq!(truncate_tooltip(""), "");
    }

    #[test]
    fn layout_same_seq_unchanged() {
        let old = vec![
            ("1".into(), "1".into(), true, WorkspaceState::Attached),
            ("2".into(), "2".into(), false, WorkspaceState::Attached),
        ];
        let new = vec![
            ws("1", "1", WorkspaceState::Attached, true, true),
            ws("2", "2", WorkspaceState::Attached, false, false),
        ];
        assert!(!workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_different_len_changed() {
        let old = vec![("1".into(), "1".into(), true, WorkspaceState::Attached)];
        let new = vec![
            ws("1", "1", WorkspaceState::Attached, true, true),
            ws("2", "2", WorkspaceState::Attached, false, false),
        ];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_different_name_changed() {
        let old = vec![("1".into(), "1".into(), true, WorkspaceState::Attached)];
        let new = vec![ws("2", "1", WorkspaceState::Attached, true, true)];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_different_visible_changed() {
        let old = vec![("1".into(), "1".into(), true, WorkspaceState::Attached)];
        let new = vec![ws("1", "1", WorkspaceState::Attached, true, false)];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_changed_on_monitor_change() {
        let old = vec![("1".into(), "DELL".into(), true, WorkspaceState::Attached)];
        let new = vec![ws("1", "LG", WorkspaceState::Attached, true, true)];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_changed_on_state_change() {
        let old = vec![("1".into(), "DELL".into(), true, WorkspaceState::Attached)];
        let new = vec![ws("1", "DELL", WorkspaceState::Parked, true, true)];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn group_single_monitor_one_group() {
        let workspaces = vec![
            ws("1", "DELL", WorkspaceState::Attached, false, false),
            ws("2", "DELL", WorkspaceState::Attached, false, false),
            ws("3", "DELL", WorkspaceState::Attached, false, false),
        ];
        let groups = group_workspaces(&workspaces);
        assert_eq!(
            groups,
            vec![MonitorGroup {
                monitor: "DELL".into(),
                detached: false,
                entries: vec![
                    GroupEntry {
                        tag: 0,
                        name: "1".into()
                    },
                    GroupEntry {
                        tag: 1,
                        name: "2".into()
                    },
                    GroupEntry {
                        tag: 2,
                        name: "3".into()
                    },
                ],
            }]
        );
    }

    #[test]
    fn group_two_monitors_two_groups() {
        let workspaces = vec![
            ws("1", "DELL", WorkspaceState::Attached, false, false),
            ws("2", "LG", WorkspaceState::Attached, false, false),
            ws("3", "DELL", WorkspaceState::Attached, false, false),
            ws("4", "LG", WorkspaceState::Attached, false, false),
        ];
        let groups = group_workspaces(&workspaces);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].monitor, "DELL");
        assert!(!groups[0].detached);
        assert_eq!(
            groups[0].entries,
            vec![
                GroupEntry {
                    tag: 0,
                    name: "1".into()
                },
                GroupEntry {
                    tag: 2,
                    name: "3".into()
                },
            ]
        );
        assert_eq!(groups[1].monitor, "LG");
        assert!(!groups[1].detached);
        assert_eq!(
            groups[1].entries,
            vec![
                GroupEntry {
                    tag: 1,
                    name: "2".into()
                },
                GroupEntry {
                    tag: 3,
                    name: "4".into()
                },
            ]
        );
    }

    #[test]
    fn group_identical_names_stay_separate() {
        let workspaces = vec![
            ws("1", "DELL #1", WorkspaceState::Attached, false, false),
            ws("2", "DELL #2", WorkspaceState::Attached, false, false),
        ];
        let groups = group_workspaces(&workspaces);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].monitor, "DELL #1");
        assert_eq!(groups[1].monitor, "DELL #2");
    }

    #[test]
    fn group_parked_marked_detached() {
        let all_parked = vec![
            ws("1", "DELL", WorkspaceState::Parked, false, false),
            ws("2", "DELL", WorkspaceState::Parked, false, false),
        ];
        let groups = group_workspaces(&all_parked);
        assert_eq!(groups.len(), 1);
        assert!(groups[0].detached);

        let mixed = vec![
            ws("1", "DELL", WorkspaceState::Parked, false, false),
            ws("2", "DELL", WorkspaceState::Attached, false, false),
        ];
        let groups = group_workspaces(&mixed);
        assert_eq!(groups.len(), 1);
        assert!(!groups[0].detached);
    }

    #[test]
    fn click_target_attached_carries_monitor() {
        let workspaces = vec![ws("1", "DELL", WorkspaceState::Attached, false, false)];
        let targets = click_targets(&workspaces);
        assert_eq!(
            targets,
            vec![ClickTarget {
                name: "1".into(),
                monitor: "DELL".into(),
            }]
        );
    }

    #[test]
    fn click_target_parked_carries_origin_monitor() {
        let workspaces = vec![ws("1", "DELL", WorkspaceState::Parked, false, false)];
        let targets = click_targets(&workspaces);
        assert_eq!(
            targets,
            vec![ClickTarget {
                name: "1".into(),
                monitor: "DELL".into(),
            }]
        );
    }
}
