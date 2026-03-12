//! C++-style GameFont accessors layered on the Rust font library.

use std::sync::Arc;

use super::font::{get_font_library, FontDesc, GameFont};

/// Return a font matching name/size/bold.
pub fn get_font(name: &str, point_size: i32, bold: bool) -> Option<Arc<GameFont>> {
    get_font_library()
        .get_font_by_name(name, point_size, bold)
        .ok()
}

/// Return the first loaded font.
pub fn first_font() -> Option<Arc<GameFont>> {
    get_font_library().first_font()
}

/// Return the next loaded font after the provided description.
pub fn next_font(desc: &FontDesc) -> Option<Arc<GameFont>> {
    get_font_library().next_font(desc)
}

/// Return the number of loaded fonts.
pub fn get_count() -> usize {
    get_font_library().get_count()
}
