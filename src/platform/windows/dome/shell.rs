use std::any::Any;

use dome_auxiliary_window::{AppShell, AppShellHandler, MenuEntry};

use crate::action::{Actions, WorkspaceInfo};
use crate::platform::shell_menu::{ShellMessage, build_menu, focused_tooltip, id_to_action};
use crate::platform::windows::dome::tray::load_tray_icon;
use crate::platform::windows::{HubEvent, HubSender};

pub(in crate::platform::windows) trait ShellApi {
    fn update(&self, workspaces: &[WorkspaceInfo]);
}

struct ShellHandler {
    hub_sender: HubSender,
    workspaces: Vec<WorkspaceInfo>,
}

impl AppShellHandler for ShellHandler {
    fn on_display_changed(&mut self) {
        self.hub_sender.send(HubEvent::DisplayChanged);
    }

    fn on_work_area_changed(&mut self) {
        self.hub_sender.send(HubEvent::WorkAreaChanged);
    }

    fn menu(&mut self) -> Vec<MenuEntry> {
        build_menu(&self.workspaces, false)
    }

    fn on_menu_selected(&mut self, id: u32) {
        let action = id_to_action(id, &self.workspaces);
        if let Some(action) = action {
            self.hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])));
        }
    }

    fn on_message(&mut self, message: Box<dyn Any>) {
        let msg = message
            .downcast::<ShellMessage>()
            .expect("app shell received a non-ShellMessage payload");
        match *msg {
            ShellMessage::Workspaces(workspaces) => self.workspaces = workspaces,
        }
    }
}

pub(in crate::platform::windows) struct ShellHandle {
    // app_shell owns the hidden window and its tray icon. Its Drop removes the icon, then
    // destroys the window, so no manual ordering is needed here.
    app_shell: AppShell,
}

impl ShellHandle {
    pub(in crate::platform::windows) fn new(hub_sender: HubSender) -> anyhow::Result<Box<Self>> {
        let handler = ShellHandler {
            hub_sender,
            workspaces: Vec::new(),
        };
        let icon = load_tray_icon()?;
        let app_shell = AppShell::new(icon, Box::new(handler))?;
        Ok(Box::new(ShellHandle { app_shell }))
    }
}

impl ShellApi for ShellHandle {
    fn update(&self, workspaces: &[WorkspaceInfo]) {
        self.app_shell
            .deliver(Box::new(ShellMessage::Workspaces(workspaces.to_vec())));
        self.app_shell.set_tooltip(&focused_tooltip(workspaces));
    }
}
