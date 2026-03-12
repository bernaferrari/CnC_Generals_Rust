//! W3D static text draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DStaticText.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D static text draw (non-image variant).
pub fn w3d_gadget_static_text_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D static text draw (image variant).
pub fn w3d_gadget_static_text_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
