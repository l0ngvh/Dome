use std::collections::HashMap;
use std::sync::Arc;

use crate::core::{Child, WindowId};
use crate::platform::windows::external::{HwndId, ManageExternalHwnd};

use super::overlay::WindowOverlayApi;
use super::window::WindowState;

pub(super) struct WindowEntry {
    pub(super) ext: Arc<dyn ManageExternalHwnd>,
    pub(super) state: WindowState,
    pub(super) title: Option<String>,
    pub(super) process: String,
    pub(super) overlay: Box<dyn WindowOverlayApi>,
}

impl std::fmt::Display for WindowEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}|{}]", self.ext.id(), self.process)?;
        if let Some(title) = &self.title {
            write!(f, " {title}")?;
        }
        Ok(())
    }
}

pub(super) struct WindowRegistry {
    by_hwnd: HashMap<HwndId, WindowId>,
    by_id: HashMap<WindowId, WindowEntry>,
}

impl WindowRegistry {
    pub(super) fn new() -> Self {
        Self {
            by_hwnd: HashMap::new(),
            by_id: HashMap::new(),
        }
    }

    pub(super) fn insert(&mut self, id: HwndId, window_id: WindowId, entry: WindowEntry) {
        self.by_hwnd.insert(id, window_id);
        self.by_id.insert(window_id, entry);
    }

    pub(super) fn remove_by_hwnd(&mut self, id: HwndId) -> Option<WindowId> {
        let window_id = self.by_hwnd.remove(&id)?;
        self.by_id.remove(&window_id);
        Some(window_id)
    }

    pub(super) fn get(&self, id: WindowId) -> &WindowEntry {
        &self.by_id[&id]
    }

    pub(super) fn get_mut(&mut self, id: WindowId) -> &mut WindowEntry {
        self.by_id.get_mut(&id).unwrap()
    }

    pub(super) fn get_id(&self, id: HwndId) -> Option<WindowId> {
        self.by_hwnd.get(&id).copied()
    }

    pub(super) fn contains_hwnd(&self, id: HwndId) -> bool {
        self.by_hwnd.contains_key(&id)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (HwndId, WindowId)> + '_ {
        self.by_hwnd.iter().map(|(&h, &id)| (h, id))
    }

    pub(super) fn set_title(&mut self, hwnd: HwndId, title: Option<String>) {
        if let Some(&id) = self.by_hwnd.get(&hwnd)
            && let Some(entry) = self.by_id.get_mut(&id)
        {
            entry.title = title;
        }
    }

    pub(super) fn resolve_tab_titles(&self, children: &[Child]) -> Vec<String> {
        children
            .iter()
            .map(|c| match c {
                Child::Window(wid) => self
                    .get(*wid)
                    .title
                    .as_deref()
                    .unwrap_or("<no title>")
                    .to_owned(),
                Child::Container(_) => "Container".to_owned(),
            })
            .collect()
    }
}
