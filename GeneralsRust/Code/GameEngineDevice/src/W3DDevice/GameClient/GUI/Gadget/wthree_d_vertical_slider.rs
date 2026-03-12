//! W3D vertical slider draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DVerticalSlider.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D vertical slider draw (non-image variant).
pub fn w3d_gadget_vertical_slider_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D vertical slider draw (image variant).
pub fn w3d_gadget_vertical_slider_image_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}
