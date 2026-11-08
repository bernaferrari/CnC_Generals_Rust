use crate::gui::display_string::{DisplayStringHandle, get_display_string_manager};
pub use crate::gui::display_string::{DisplayStringManager, DisplayStringManagerAccess};

pub fn new_display_string() -> DisplayStringHandle {
    get_display_string_manager().new_display_string()
}

pub fn free_display_string(handle: DisplayStringHandle) {
    get_display_string_manager().free_display_string(handle);
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
