//! W3D main menu compatibility wrappers.
//!
//! This is the Rust equivalent of `W3DMainMenu.cpp`: the C++ file mostly
//! forwards initialization into `MainMenuInit` and exposes the specialized
//! draw callbacks used by the menu layout.

use std::any::Any;

use game_client_rust::gui::shell::main_menu::get_main_menu;

pub use game_client_rust::gui::w3d_gadget_draw::{
    w3d_main_menu_button_drop_shadow_draw as W3DMainMenuButtonDropShadowDraw,
    w3d_main_menu_draw as W3DMainMenuDraw, w3d_main_menu_four_draw as W3DMainMenuFourDraw,
    w3d_main_menu_map_border as W3DMainMenuMapBorder,
    w3d_main_menu_random_text_draw as W3DMainMenuRandomTextDraw,
    w3d_metal_bar_menu_draw as W3DMetalBarMenuDraw,
};

/// C++ `W3DMainMenuInit` wrapper.
pub fn W3DMainMenuInit(layout: &dyn Any, user_data: Option<&dyn Any>) {
    let mut menu = get_main_menu();
    if let Err(err) = menu.init(layout, user_data) {
        log::warn!("W3DMainMenuInit failed: {err}");
    }
}
