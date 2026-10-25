//! Shim for InGamePopupMessage.cpp callbacks.

pub use crate::gui::callbacks::in_game_popup_message::{
    in_game_popup_message_init, in_game_popup_message_input,
    in_game_popup_message_system_with_sink as in_game_popup_message_system,
};
