//! Generic menu vocabulary shared across platforms. Names no Dome domain type.

/// One selectable row in a context menu. `id` is the consumer's own value, returned
/// verbatim from the handler's selection callback.
#[derive(Clone, Debug)]
pub struct MenuItem {
    pub label: String,
    pub id: u32,
    pub checked: bool,
}

/// One entry in a context menu. A submenu holds one level of items.
#[derive(Clone, Debug)]
pub enum MenuEntry {
    Item(MenuItem),
    Separator,
    /// A submenu shows grayed when `enabled` is false.
    Submenu {
        label: String,
        items: Vec<MenuItem>,
        enabled: bool,
    },
}
