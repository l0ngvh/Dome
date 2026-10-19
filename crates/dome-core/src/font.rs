// Smaller values produce unreadable glyphs.
pub const MIN_FONT_SIZE: f32 = 4.0;
pub const MAX_FONT_SIZE: f32 = 128.0;

#[derive(Debug, Clone, PartialEq)]
pub struct FontConfig {
    pub size: f32,
    pub family: Option<String>,
}
