//! W3D horizontal slider draw callbacks.
//!
//! C++ reference: W3DDevice/GameClient/GUI/Gadget/W3DHorizontalSlider.cpp

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};

/// W3D horizontal slider draw (non-image variant).
pub fn w3d_gadget_horizontal_slider_draw(window: &GameWindow, inst_data: &WindowInstanceData) {
    default_draw_callback(window, inst_data);
}

/// W3D horizontal slider draw (image variant).
pub fn w3d_gadget_horizontal_slider_image_draw(
    window: &GameWindow,
    inst_data: &WindowInstanceData,
) {
    default_draw_callback(window, inst_data);
}
