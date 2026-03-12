//! W3D list box draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DListBox.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D list box draw (non-image variant).
pub fn w3d_gadget_list_box_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D list box draw (image variant).
pub fn w3d_gadget_list_box_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
