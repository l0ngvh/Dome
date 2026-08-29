use std::cell::RefCell;
use std::rc::Rc;

use dome_auxiliary_window::{
    AuxiliaryWindow, AuxiliaryWindowExtWindows, AuxiliaryWindowHandler, MenuEntry,
    PhysicalPosition, PhysicalSize, WindowAttributes,
};

use crate::action::{Actions, WorkspaceInfo};
use crate::platform::windows::dome::tray::{
    build_menu, command_to_action, focused_tooltip, load_tray_icon,
};
use crate::platform::windows::{HubEvent, HubSender};

pub(in crate::platform::windows) trait AppWindowApi {
    fn update_tray(&self, workspaces: &[WorkspaceInfo]);
}

struct AppWindowHandler {
    hub_sender: HubSender,
    workspaces: Rc<RefCell<Vec<WorkspaceInfo>>>,
}

impl AuxiliaryWindowHandler for AppWindowHandler {
    fn on_display_changed(&mut self) {
        self.hub_sender.send(HubEvent::DisplayChanged);
    }

    fn on_work_area_changed(&mut self) {
        self.hub_sender.send(HubEvent::WorkAreaChanged);
    }

    fn tray_menu(&mut self) -> Vec<MenuEntry> {
        build_menu(&self.workspaces.borrow())
    }

    fn on_tray_menu_selected(&mut self, id: u32) {
        let action = command_to_action(id, &self.workspaces.borrow());
        if let Some(action) = action {
            self.hub_sender
                .send(HubEvent::Action(Actions::new(vec![action])));
        }
    }
}

pub(in crate::platform::windows) struct AppWindow {
    // Shared with the handler so the menu, its selection handling, and update_tray all
    // read the same workspace list.
    workspaces: Rc<RefCell<Vec<WorkspaceInfo>>>,
    // aux owns the window and its tray icon. Its Drop removes the icon, then destroys the
    // window, so no manual ordering is needed here.
    aux: AuxiliaryWindow,
}

impl AppWindow {
    pub(in crate::platform::windows) fn new(hub_sender: HubSender) -> anyhow::Result<Box<Self>> {
        let workspaces: Rc<RefCell<Vec<WorkspaceInfo>>> = Rc::new(RefCell::new(Vec::new()));
        let handler = AppWindowHandler {
            hub_sender,
            workspaces: Rc::clone(&workspaces),
        };
        let attributes = WindowAttributes {
            position: PhysicalPosition { x: 0, y: 0 },
            size: PhysicalSize {
                width: 0,
                height: 0,
            },
            click_through: false,
            focusable: false,
        };
        let aux = AuxiliaryWindow::new(&attributes, Box::new(handler))?;

        let icon = load_tray_icon()?;
        aux.install_tray_icon(icon, "")?;

        Ok(Box::new(AppWindow { workspaces, aux }))
    }
}

impl AppWindowApi for AppWindow {
    fn update_tray(&self, workspaces: &[WorkspaceInfo]) {
        *self.workspaces.borrow_mut() = workspaces.to_vec();
        self.aux.set_tray_tooltip(&focused_tooltip(workspaces));
    }
}
