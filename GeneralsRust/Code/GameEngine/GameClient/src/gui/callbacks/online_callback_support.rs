//! Shared adapters so WOL/GameSpy callback modules match the registry traits.
//!
//! C++ `GUICallbacks.h` and `FunctionLexicon.cpp` register layout callbacks as
//! `void (*)(WindowLayout*, void*)` and window callbacks as
//! `WindowMsgHandledType (*)(GameWindow*, UnsignedInt, WindowMsgData, WindowMsgData)`.
//! The Rust registry uses `&WindowLayout` + `Option<&dyn Any>` and
//! `&GameWindow` + `WindowMsgData` (`usize`). These helpers keep that shape
//! without widening the offline stubs.

use crate::display::image::get_mapped_image_collection;
use crate::gui::game_window::Image as WindowImage;
use crate::gui::Color;
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::gamespy::peer_defs::{get_gamespy_info, GameSpyInfo};

/// C++ `TheNameKeyGenerator->nameToKey` stored as `WindowId` (`i32`).
pub fn name_to_window_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

/// Unpack a GameSpy packed RGBA `u32` into the GUI `Color` struct.
pub fn packed_ui_color(packed: u32) -> Color {
    Color::new(
        ((packed >> 16) & 0xFF) as u8,
        ((packed >> 8) & 0xFF) as u8,
        (packed & 0xFF) as u8,
        ((packed >> 24) & 0xFF) as u8,
    )
}

pub fn with_gamespy_info<R>(f: impl FnOnce(&GameSpyInfo) -> R) -> Option<R> {
    let slot = get_gamespy_info()?;
    let Ok(guard) = slot.lock() else {
        return None;
    };
    Some(f(&guard))
}

pub fn lookup_window_image(name: &str) -> Option<WindowImage> {
    let collection = get_mapped_image_collection();
    let guard = collection.try_read()?;
    let found = guard.find_image_by_name(name)?;
    let size = found.get_image_size();
    Some(WindowImage {
        name: name.to_string(),
        width: size.x,
        height: size.y,
    })
}

pub fn with_gamespy_info_mut<R>(f: impl FnOnce(&mut GameSpyInfo) -> R) -> Option<R> {
    let slot = get_gamespy_info()?;
    let Ok(mut guard) = slot.lock() else {
        return None;
    };
    Some(f(&mut guard))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packed_ui_color_unpacks_gamespy_rgba_when_make_color_layout() {
        let packed = (0xAAu32 << 24) | (0x11 << 16) | (0x22 << 8) | 0x33;
        let color = packed_ui_color(packed);
        assert_eq!(color.r, 0x11);
        assert_eq!(color.g, 0x22);
        assert_eq!(color.b, 0x33);
        assert_eq!(color.a, 0xAA);
    }

    #[test]
    fn name_to_window_id_is_stable_i32_when_same_name() {
        let first = name_to_window_id("WOLWelcomeMenu.wnd:ButtonBack");
        let second = name_to_window_id("WOLWelcomeMenu.wnd:ButtonBack");
        assert_eq!(first, second);
        assert_ne!(first, 0);
    }
}
