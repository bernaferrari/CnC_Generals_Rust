//! W3D progress bar draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DProgressBar.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D progress bar draw (non-image variant).
pub fn w3d_gadget_progress_bar_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D progress bar draw (image variant).
pub fn w3d_gadget_progress_bar_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
