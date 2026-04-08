pub use crate::gui::font::FontDesc;

pub fn new_font_desc(name: &str, size: i32, bold: bool) -> FontDesc {
    FontDesc::new(name, size, bold)
}

pub fn default_font_desc() -> FontDesc {
    FontDesc::default()
}
