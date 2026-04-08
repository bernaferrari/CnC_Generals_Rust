use crate::gui::display_string::{get_display_string_manager, DisplayStringHandle};
pub use crate::gui::display_string::{DisplayStringManager, DisplayStringManagerAccess};

pub fn new_display_string() -> DisplayStringHandle {
    get_display_string_manager().new_display_string()
}

pub fn free_display_string(handle: DisplayStringHandle) {
    get_display_string_manager().free_display_string(handle);
}

pub fn get_group_numeral_string(numeral: i32) -> Option<DisplayStringHandle> {
    get_display_string_manager().get_group_numeral_string(numeral)
}

pub fn get_formation_letter_string() -> Option<DisplayStringHandle> {
    get_display_string_manager().get_formation_letter_string()
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
