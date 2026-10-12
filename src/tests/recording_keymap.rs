use crate::config::ModalKeymaps;
use crate::config::lua::KeymapEffects;

#[derive(Default)]
pub(crate) struct RecordingKeymap {
    pub switched: Vec<String>,
    /// The last published bindings, sorted as `mode keystroke`.
    pub published: Vec<String>,
}

impl KeymapEffects for RecordingKeymap {
    fn switch_mode(&mut self, name: &str) {
        self.switched.push(name.to_string());
    }
    fn update_keymaps(&mut self, keymaps: &ModalKeymaps) {
        let mut published: Vec<String> = keymaps
            .modes
            .iter()
            .flat_map(|(mode, bindings)| {
                bindings
                    .keys()
                    .map(move |keystroke| format!("{mode} {keystroke}"))
            })
            .collect();
        published.sort();
        self.published = published;
    }
}
