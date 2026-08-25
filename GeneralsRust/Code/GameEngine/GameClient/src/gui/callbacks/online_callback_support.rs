//! Shared adapters so WOL/GameSpy callback modules match the registry traits.
//!
//! C++ `GUICallbacks.h` and `FunctionLexicon.cpp` register layout callbacks as
//! `void (*)(WindowLayout*, void*)` and window callbacks as
//! `WindowMsgHandledType (*)(GameWindow*, UnsignedInt, WindowMsgData, WindowMsgData)`.
//! The Rust registry uses `&WindowLayout` + `Option<&dyn Any>` and
//! `&GameWindow` + `WindowMsgData` (`usize`). These helpers keep that shape
//! without widening the offline stubs.
//!
//! C++ GUI callbacks are single-threaded and re-enter file-static window
//! pointers freely (`WOLStatusMenu.cpp:25-31`, `WOLStatusMenuInput` →
//! `winSendSystemMsg` at lines 112-113). Rust `RefCell` is not re-entrant:
//! clone `Rc` handles and drop the thread-local borrow before dispatching.

use std::cell::RefCell;
use std::rc::Rc;

use crate::color::game_get_color_components;
use crate::display::image::get_mapped_image_collection;
use crate::gui::game_window::Image as WindowImage;
use crate::gui::{Color, GameWindow, WindowMessage, WindowMsgData};
use game_engine::common::name_key_generator::NameKeyGenerator;
use game_network::gamespy::peer_defs::{GameSpyInfo, get_gamespy_info};

/// C++ `TheNameKeyGenerator->nameToKey` stored as `WindowId` (`i32`).
pub fn name_to_window_id(name: &str) -> i32 {
    NameKeyGenerator::name_to_key(name) as i32
}

/// Unpack a GameSpy / `GameMakeColor` packed word into the GUI `Color` struct.
///
/// C++ `GameMakeColor` (`Color.h:37-40`) packs
/// `(alpha << 24) | (red << 16) | (green << 8) | blue`.
/// `GameGetColorComponents` (`Color.cpp:67-70`) unpacks the same bit order.
/// There is no WW3D alpha-invert on this path.
pub fn packed_ui_color(packed: u32) -> Color {
    let (r, g, b, a) = game_get_color_components(packed);
    Color::new(r, g, b, a)
}

pub fn with_gamespy_info<R>(f: impl FnOnce(&GameSpyInfo) -> R) -> Option<R> {
    let slot = get_gamespy_info()?;
    // `std::sync::Mutex` is not re-entrant. C++ `TheGameSpyInfo` is a raw
    // pointer. Nested `with_gamespy_info` on the same thread deadlocks; callers
    // must copy fields out and drop the guard first.
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

/// C++ `TheMappedImageCollection->findImageByName` size, or 10x10 when missing.
pub fn mapped_image_size(name: &str) -> (u32, u32) {
    lookup_window_image(name)
        .map(|img| (img.width.max(1) as u32, img.height.max(1) as u32))
        .unwrap_or((10, 10))
}

/// Combo item `data` is `Option<usize>`; C++ stores signed gadget user-data.
pub fn combo_item_data_eq(item_data: Option<usize>, value: i32) -> bool {
    item_data == Some(value as usize)
}

/// `TheChallengeGenerals` is a `Mutex`; C++ is a raw pointer.
pub fn challenge_general_starts_enabled(template_name: &str) -> bool {
    let Some(generals) = crate::gui::challenge_generals::get_challenge_generals() else {
        return true;
    };
    let Ok(guard) = generals.lock() else {
        return true;
    };
    guard
        .general_by_template_name(template_name)
        .map(|persona| persona.is_starting_enabled())
        .unwrap_or(true)
}

pub fn with_gamespy_info_mut<R>(f: impl FnOnce(&mut GameSpyInfo) -> R) -> Option<R> {
    let slot = get_gamespy_info()?;
    let Ok(mut guard) = slot.lock() else {
        return None;
    };
    Some(f(&mut guard))
}

/// C++ `TheWindowManager->winSendSystemMsg(window, GBM_SELECTED, button, id)`.
///
/// `data1`/`data2` both carry the gadget id because Rust `WindowMsgData` is
/// `usize`, not a `GameWindow*`. System handlers already key off `data1 as i32`.
pub fn send_simulated_gadget_selected(target: &Rc<RefCell<GameWindow>>, control_id: i32) {
    let _ = target.borrow_mut().send_system_message(
        WindowMessage::GadgetSelected,
        control_id as WindowMsgData,
        control_id as WindowMsgData,
    );
}

/// Snapshot-then-dispatch so ESC handlers can drop their `RefCell` borrow
/// before the system callback re-enters the same thread-local state.
pub fn dispatch_esc_gadget_selected(parent: Option<Rc<RefCell<GameWindow>>>, control_id: i32) {
    if let Some(parent) = parent {
        send_simulated_gadget_selected(&parent, control_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::game_make_color;
    use std::cell::RefCell;

    #[test]
    fn packed_ui_color_matches_game_make_color_bit_order() {
        let packed = game_make_color(0x11, 0x22, 0x33, 0xAA);
        let color = packed_ui_color(packed);
        assert_eq!(color.r, 0x11);
        assert_eq!(color.g, 0x22);
        assert_eq!(color.b, 0x33);
        assert_eq!(color.a, 0xAA);
        assert_eq!(packed, (0xAAu32 << 24) | (0x11 << 16) | (0x22 << 8) | 0x33);
    }

    #[test]
    fn name_to_window_id_is_stable_i32_when_same_name() {
        let first = name_to_window_id("WOLWelcomeMenu.wnd:ButtonBack");
        let second = name_to_window_id("WOLWelcomeMenu.wnd:ButtonBack");
        assert_eq!(first, second);
        assert_ne!(first, 0);
    }

    #[test]
    fn refcell_reentry_is_rejected_so_esc_must_drop_borrow_first() {
        // Documents why dispatch_esc_gadget_selected clones the Rc out of
        // thread-local state before send_system_message (C++ file-statics
        // re-enter; RefCell panics).
        let cell = RefCell::new(0u32);
        let mut outer = cell.borrow_mut();
        assert!(
            cell.try_borrow_mut().is_err(),
            "nested borrow_mut must fail so ESC cannot hold state across GBM_SELECTED"
        );
        *outer = 1;
        drop(outer);
        assert_eq!(*cell.borrow(), 1);
    }

    #[test]
    fn dispatch_esc_gadget_selected_is_silent_when_parent_missing() {
        dispatch_esc_gadget_selected(None, 1);
    }

    #[test]
    fn combo_item_data_eq_casts_signed_gadget_data_to_usize() {
        assert!(combo_item_data_eq(Some((-1i32) as usize), -1));
        assert!(combo_item_data_eq(Some(3), 3));
        assert!(!combo_item_data_eq(Some(3), 4));
        assert!(!combo_item_data_eq(None, 0));
    }

    #[test]
    fn mapped_image_size_falls_back_to_ten_when_collection_empty() {
        assert_eq!(mapped_image_size("Password"), (10, 10));
    }

    #[test]
    fn challenge_general_starts_enabled_is_true_when_persona_missing() {
        assert!(challenge_general_starts_enabled("NoSuchTemplate"));
    }
}
