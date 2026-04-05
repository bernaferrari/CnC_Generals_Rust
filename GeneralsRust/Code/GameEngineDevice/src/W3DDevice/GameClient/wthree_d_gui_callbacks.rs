//! C++-compatible GUI callback exports for W3D.
//!
//! This module mirrors `W3DGUICallbacks.h` by exposing the device-specific
//! callback entry points as thin wrappers around the already ported GameClient
//! rendering and shell code.

pub use crate::w3d_device::gui::gui_callbacks::wthree_d_main_menu::W3DMainMenuInit;
pub use game_client_rust::gui::w3d_gadget_draw::{
    w3d_cameo_movie_draw as W3DCameoMovieDraw, w3d_clock_draw as W3DClockDraw,
    w3d_credits_menu_draw as W3DCreditsMenuDraw, w3d_draw_map_preview as W3DDrawMapPreview,
    w3d_left_hud_draw as W3DLeftHUDDraw,
    w3d_main_menu_button_drop_shadow_draw as W3DMainMenuButtonDropShadowDraw,
    w3d_main_menu_draw as W3DMainMenuDraw, w3d_main_menu_four_draw as W3DMainMenuFourDraw,
    w3d_main_menu_map_border as W3DMainMenuMapBorder,
    w3d_main_menu_random_text_draw as W3DMainMenuRandomTextDraw,
    w3d_metal_bar_menu_draw as W3DMetalBarMenuDraw, w3d_no_draw as W3DNoDraw,
    w3d_power_draw as W3DPowerDraw, w3d_right_hud_draw as W3DRightHUDDraw,
    w3d_shell_menu_scheme_draw as W3DShellMenuSchemeDraw,
    w3d_thin_border_draw as W3DThinBorderDraw,
};
