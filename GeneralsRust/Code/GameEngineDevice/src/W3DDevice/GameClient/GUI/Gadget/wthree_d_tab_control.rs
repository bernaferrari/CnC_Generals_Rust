//! W3D tab control draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DTabControl.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D tab control draw (non-image variant).
pub fn w3d_gadget_tab_control_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D tab control draw (image variant).
pub fn w3d_gadget_tab_control_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
