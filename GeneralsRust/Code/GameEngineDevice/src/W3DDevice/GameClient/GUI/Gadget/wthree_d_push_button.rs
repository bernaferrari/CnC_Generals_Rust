//! W3D push button draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DPushButton.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D push button draw (non-image variant).
pub fn w3d_gadget_push_button_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D push button draw (image variant).
pub fn w3d_gadget_push_button_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
