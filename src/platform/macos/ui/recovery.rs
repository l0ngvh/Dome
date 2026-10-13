use crate::core::PixelRect;
use crate::platform::macos::window_server::RecoveryEntry;
use objc2_core_graphics::CGWindowID;
use std::collections::HashMap;

#[derive(Default)]
pub(super) struct Recovery {
    entries: HashMap<CGWindowID, RecoveryEntry>,
}

impl Recovery {
    pub(super) fn track(
        &mut self,
        key: CGWindowID,
        original: Option<PixelRect>,
        primary: Option<PixelRect>,
    ) {
        let (Some(original), Some(primary)) = (original, primary) else {
            return;
        };
        self.entries.entry(key).or_insert_with(|| RecoveryEntry {
            window: key,
            original_rect: PixelRect::from_pixels(
                primary.x() + (primary.width() - original.width()) / 2,
                primary.y() + (primary.height() - original.height()) / 2,
                original.width(),
                original.height(),
            ),
        });
    }

    pub(super) fn untrack(&mut self, key: CGWindowID) {
        self.entries.remove(&key);
    }

    pub(super) fn take(&mut self) -> Vec<RecoveryEntry> {
        self.entries.drain().map(|(_, entry)| entry).collect()
    }
}
