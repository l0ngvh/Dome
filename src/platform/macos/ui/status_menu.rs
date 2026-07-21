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

use crate::action::{Action, Actions, FocusTarget};
use crate::core::WorkspaceInfo;
use crate::platform::macos::dome::HubEvent;

const STATUS_TOOLTIP_MAX_CHARS: usize = 20;

pub(super) struct StatusMenu {
    status_item: Retained<NSStatusItem>,
    button: Retained<NSStatusBarButton>,
    menu: Retained<NSMenu>,
    target: Retained<StatusMenuTarget>,
    last_workspaces: RefCell<Vec<(String, bool)>>,
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
            let names: Vec<String> = workspaces.iter().map(|w| w.name.clone()).collect();
            self.target.set_workspace_names(names);

            for (i, ws) in workspaces.iter().enumerate() {
                let title = NSString::from_str(&ws.name);
                let empty = NSString::from_str("");
                let alloc = NSMenuItem::alloc(mtm);
                let item: Retained<NSMenuItem> = unsafe {
                    NSMenuItem::initWithTitle_action_keyEquivalent(
                        alloc,
                        &title,
                        Some(sel!(workspaceClicked:)),
                        &empty,
                    )
                };
                item.setTag(i as NSInteger);
                unsafe {
                    item.setTarget(Some(&self.target));
                }
                self.menu.addItem(&item);
            }

            self.menu.addItem(&NSMenuItem::separatorItem(mtm));

            let exit_title = NSString::from_str("Exit Dome");
            let empty = NSString::from_str("");
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
                .map(|w| (w.name.clone(), w.is_visible))
                .collect();
        }

        for (i, ws) in workspaces.iter().enumerate() {
            if let Some(item) = self.menu.itemAtIndex(i as NSInteger) {
                let state = if ws.is_focused {
                    NSControlStateValueOn
                } else {
                    NSControlStateValueOff
                };
                item.setState(state);
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
    workspace_names: RefCell<Vec<String>>,
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
            let names = self.ivars().workspace_names.borrow();
            if let Some(name) = names.get(tag) {
                let action = Action::Focus(FocusTarget::Workspace { name: name.clone() });
                self.ivars()
                    .hub_sender
                    .send(HubEvent::Action(Actions::new(vec![action])))
                    .ok();
            }
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
            workspace_names: RefCell::new(Vec::new()),
        });
        unsafe { msg_send![super(this), init] }
    }

    fn set_workspace_names(&self, names: Vec<String>) {
        *self.ivars().workspace_names.borrow_mut() = names;
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
// NSMenuItem allocs per switch, acceptable.
fn workspaces_layout_changed(old: &[(String, bool)], new: &[WorkspaceInfo]) -> bool {
    if old.len() != new.len() {
        return true;
    }
    old.iter()
        .zip(new.iter())
        .any(|((n, v), w)| n != &w.name || *v != w.is_visible)
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
        let old = vec![("1".into(), true), ("2".into(), false)];
        let new = vec![ws("1", true, true), ws("2", false, false)];
        assert!(!workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_different_len_changed() {
        let old = vec![("1".into(), true)];
        let new = vec![ws("1", true, true), ws("2", false, false)];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_different_name_changed() {
        let old = vec![("1".into(), true)];
        let new = vec![ws("2", true, true)];
        assert!(workspaces_layout_changed(&old, &new));
    }

    #[test]
    fn layout_different_visible_changed() {
        let old = vec![("1".into(), true)];
        let new = vec![ws("1", true, false)];
        assert!(workspaces_layout_changed(&old, &new));
    }
}
