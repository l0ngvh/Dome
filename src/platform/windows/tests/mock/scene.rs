use crate::platform::windows::dome::events::{HubMessage, Scene, SceneSender};
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc;

pub(crate) struct MockSceneSender {
    messages: Vec<HubMessage>,
    pub latest: Option<Scene>,
    pub hold_scenes: bool,
}
impl MockSceneSender {
    pub(crate) fn new() -> Self {
        Self {
            messages: Vec::new(),
            latest: None,
            hold_scenes: false,
        }
    }
    pub(crate) fn deliver(&mut self, intake: &mpsc::Sender<HubMessage>) -> usize {
        let mut delivered = 0;
        let mut retained = Vec::new();
        for message in self.messages.drain(..) {
            if self.hold_scenes && matches!(message, HubMessage::Scene(_)) {
                retained.push(message);
            } else {
                intake.send(message).unwrap();
                delivered += 1;
            }
        }
        self.messages = retained;
        delivered
    }
}
impl SceneSender for Rc<RefCell<MockSceneSender>> {
    fn send(&mut self, message: HubMessage) {
        let mut sender = self.borrow_mut();
        if let HubMessage::Scene(scene) = &message {
            sender.latest = Some(scene.clone());
        }
        sender.messages.push(message);
    }
}
