use super::dome::{Dome, HubEvent};
use super::login_item;
use crate::logging::Logger;
use windows::Win32::UI::WindowsAndMessaging::PostQuitMessage;

pub(super) struct DomeRunner {
    dome: Dome,
    main_thread_id: u32,
    logger: Logger,
}

impl DomeRunner {
    pub(super) fn new(mut dome: Dome, main_thread_id: u32, logger: Logger) -> Self {
        dome.apply_layout();
        Self {
            dome,
            main_thread_id,
            logger,
        }
    }

    pub(super) fn handle_event(&mut self, event: HubEvent) {
        let Err(event) = self.dome.handle_report(event) else {
            return;
        };
        match event {
            HubEvent::WindowsDiscovered(_)
            | HubEvent::WindowGone(_)
            | HubEvent::WindowMinimized(_)
            | HubEvent::WindowRestored(_)
            | HubEvent::FocusReported(_)
            | HubEvent::TitleChanged { .. }
            | HubEvent::SizeConstraint { .. }
            | HubEvent::FloatMoved { .. }
            | HubEvent::FullscreenChanged { .. } => unreachable!("semantic report already handled"),
            HubEvent::Shutdown => {
                unsafe { PostQuitMessage(0) };
                return;
            }
            HubEvent::DpiChanged | HubEvent::WorkAreaChanged | HubEvent::DisplayChanged => {
                self.dome.handle_display_change()
            }
            HubEvent::RunBinding { keymap, keystroke } => {
                self.dome
                    .run_binding(&keymap, &keystroke, self.main_thread_id);
                return;
            }
            HubEvent::Action(actions) => {
                self.dome.handle_actions(&actions, self.main_thread_id);
                return;
            }
            HubEvent::ReloadConfig => {
                if let Some(config) = self.dome.reload() {
                    self.logger.set_level(config.log_level);
                    login_item::sync_login_item(config.start_at_login);
                }
                return;
            }
            HubEvent::ApplyLayout(path) => {
                self.dome.apply_layout_file(&path);
                return;
            }
            HubEvent::SaveLayout(path) => {
                self.dome.save_layout_file(std::path::Path::new(&path));
                return;
            }
            HubEvent::TabClicked(id, index) => {
                self.dome.tab_clicked(id, index);
                return;
            }
            HubEvent::Query { query, sender } => {
                let json = match query {
                    crate::action::Query::Workspaces => self.dome.query_workspaces_json(),
                    crate::action::Query::MinimizedWindows => {
                        self.dome.query_minimized_windows_json()
                    }
                    crate::action::Query::Monitors => self.dome.query_monitors_json(),
                };
                let _ = sender.send(json);
                return;
            }
        }
        self.dome.apply_layout();
    }
}
