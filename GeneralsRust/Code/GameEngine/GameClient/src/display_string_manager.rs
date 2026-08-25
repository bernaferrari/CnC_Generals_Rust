use crate::draw_group_info::get_draw_group_info;
use crate::game_text::GameText;
use crate::gui::display_string::{DisplayStringHandle, get_display_string_manager};
pub use crate::gui::display_string::{DisplayStringManager, DisplayStringManagerAccess};
use crate::gui::font::{FontDesc, get_font_library};

pub fn new_display_string() -> DisplayStringHandle {
    get_display_string_manager().new_display_string()
}

pub fn free_display_string(handle: DisplayStringHandle) {
    get_display_string_manager().free_display_string(handle);
}

/// C++ `W3DDisplayStringManager::getGroupNumeralString` after `postProcessLoad`.
pub fn get_group_numeral_string(numeral: i32) -> Option<DisplayStringHandle> {
    let handle = get_display_string_manager().get_group_numeral_string(numeral)?;
    apply_w3d_group_numeral(&handle, numeral);
    Some(handle)
}

/// C++ `W3DDisplayStringManager::m_formationLetterDisplayString`.
pub fn get_formation_letter_string() -> Option<DisplayStringHandle> {
    let handle = get_display_string_manager().get_formation_letter_string()?;
    apply_w3d_formation_letter(&handle);
    Some(handle)
}

fn apply_w3d_group_numeral(handle: &DisplayStringHandle, numeral: i32) {
    let idx = numeral.clamp(0, 9);
    let text = GameText::fetch(&format!("NUMBER:{idx}"));
    let mut display = handle.borrow_mut();
    display.set_text(text);
    if let Some(font) = draw_group_font() {
        display.set_font(font);
    }
}

fn apply_w3d_formation_letter(handle: &DisplayStringHandle) {
    let text = GameText::fetch("LABEL:FORMATION");
    let mut display = handle.borrow_mut();
    display.set_text(text);
    if let Some(font) = draw_group_font() {
        display.set_font(font);
    }
}

fn draw_group_font() -> Option<std::sync::Arc<crate::gui::font::GameFont>> {
    let info = get_draw_group_info()
        .read()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let desc = FontDesc::new(&info.font_name, info.font_size, info.font_is_bold);
    get_font_library().get_font(&desc).ok()
}

pub fn init_display_string_manager() -> Result<(), Box<dyn std::error::Error>> {
    get_display_string_manager().init()
}

pub fn reset_display_string_manager() -> Result<(), Box<dyn std::error::Error>> {
    get_display_string_manager().reset()
}

pub fn update_display_string_manager() -> Result<(), Box<dyn std::error::Error>> {
    get_display_string_manager().update()
}

// PARITY_NOTE: the original C++ manager maintained an intrusive linked list of live strings.
// The Rust port keeps ownership in the canonical GUI display-string module and exposes the same
// creation/destruction entry points here as the compatibility facade.
