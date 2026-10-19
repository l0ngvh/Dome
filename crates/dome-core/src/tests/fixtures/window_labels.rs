use crate::core::WindowId;
use std::collections::BTreeMap;

#[derive(Default)]
pub struct WindowLabels(BTreeMap<WindowId, String>);

impl WindowLabels {
    pub fn insert(&mut self, label: &str) -> WindowId {
        assert!(
            !self.0.values().any(|existing| existing == label),
            "test windows need distinct labels, use window(title).label(label) for duplicate titles"
        );
        let id = WindowId::new(self.0.len());
        self.0.insert(id, label.to_owned());
        id
    }

    pub fn entries(&self) -> &BTreeMap<WindowId, String> {
        &self.0
    }
}
