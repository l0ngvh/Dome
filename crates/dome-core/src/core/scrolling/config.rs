use crate::core::SizeConstraint;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ScrollingConfig {
    /// Width of a new column. A config reload also applies it to every column of a workspace
    /// that `layout.lua` does not name.
    pub(crate) column_width: SizeConstraint,
}
