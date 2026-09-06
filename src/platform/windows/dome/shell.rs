use std::cell::RefCell;
use std::rc::Rc;

use dome_auxiliary_window::{AppShell, AppShellHandler, MenuEntry};

use crate::action::{Actions, WorkspaceInfo};
use crate::platform::shell_menu::{build_menu, focused_tooltip, id_to_action};
use crate::platform::windows::dome::tray::load_tray_icon;
use crate::platform::windows::{HubEvent, HubSender};

pub(in crate::platform::windows) trait ShellApi {
    fn update(&self, workspaces: &[WorkspaceInfo]);
}

struct ShellHandler {
    hub_sender: HubSender,
    workspaces: Rc<RefCell<Vec<WorkspaceInfo>>>,
}

impl AppShellHandler for ShellHandler {
    fn on_display_changed(&mut self) {
        self.hub_sender.send(HubEvent::DisplayChanged);
    }

    fn on_work_area_changed(&mut self) {
        self.hub_sender.send(HubEvent::WorkAreaChanged);
    }

    fn menu(&mut self) -> Vec<MenuEntry> {
        build_menu(&self.workspaces.borrow(), false)
    }

    fn on_menu_selected(&mut self, id: u32) {
        let action = id_to_action(id, &self.workspaces.borrow());
        if let Some(action) = action {
            self.hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])));
        }
    }
}

pub(in crate::platform::windows) struct ShellHandle {
    // Shared with the handler so the menu, its selection handling, and update all
    // read the same workspace list.
    workspaces: Rc<RefCell<Vec<WorkspaceInfo>>>,
    // app_shell owns the hidden window and its tray icon. Its Drop removes the icon, then
    // destroys the window, so no manual ordering is needed here.
    app_shell: AppShell,
}

impl ShellHandle {
    pub(in crate::platform::windows) fn new(hub_sender: HubSender) -> anyhow::Result<Box<Self>> {
        let workspaces: Rc<RefCell<Vec<WorkspaceInfo>>> = Rc::new(RefCell::new(Vec::new()));
        let handler = ShellHandler {
            hub_sender,
            workspaces: Rc::clone(&workspaces),
        };
        let icon = load_tray_icon()?;
        let app_shell = AppShell::new(icon, Box::new(handler))?;
        Ok(Box::new(ShellHandle {
            workspaces,
            app_shell,
        }))
    }
}

impl ShellApi for ShellHandle {
    fn update(&self, workspaces: &[WorkspaceInfo]) {
        *self.workspaces.borrow_mut() = workspaces.to_vec();
        self.app_shell.set_tooltip(&focused_tooltip(workspaces));
    }
}
