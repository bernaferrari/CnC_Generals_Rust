//! W3D text entry draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DTextEntry.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D text entry draw (non-image variant).
pub fn w3d_gadget_text_entry_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D text entry draw (image variant).
pub fn w3d_gadget_text_entry_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
