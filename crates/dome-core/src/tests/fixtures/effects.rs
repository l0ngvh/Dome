use crate::config::lua::PlatformEffects;
use crate::core::WindowId;

#[derive(Default)]
pub struct RecordingEffects {
    pub closed: Vec<WindowId>,
    pub executed: Vec<String>,
    pub exited: bool,
    pub unminimized: Vec<WindowId>,
}

impl PlatformEffects for RecordingEffects {
    fn close(&mut self, id: WindowId) {
        self.closed.push(id);
    }
    fn execute(&mut self, command: &str) {
        self.executed.push(command.to_string());
    }
    fn exit(&mut self) {
        self.exited = true;
    }
    fn unminimize(&mut self, id: WindowId) {
        self.unminimized.push(id);
    }
}
