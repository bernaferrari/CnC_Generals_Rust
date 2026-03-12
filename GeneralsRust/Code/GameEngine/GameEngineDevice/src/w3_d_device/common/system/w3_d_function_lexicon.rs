// FILE: w3_d_function_lexicon.rs
// Ported from C++ W3DFunctionLexicon.cpp

use crate::w3_d_device::common::w3_d_function_lexicon::{
    FunctionLexicon, FunctionPtr, TableEntry, TableIndex,
};
use game_client::gui::callbacks::skirmish_map_select_menu::draw_map_preview;
use game_client::gui::game_window::{default_draw_callback, WindowInstanceData};
use game_client::gui::w3d_gadget_draw::{
    w3d_cameo_movie_draw, w3d_clock_draw, w3d_command_bar_background_draw,
    w3d_command_bar_foreground_draw, w3d_command_bar_gen_exp_draw, w3d_command_bar_grid_draw,
    w3d_command_bar_help_popup_draw, w3d_command_bar_top_draw, w3d_credits_menu_draw,
    w3d_gadget_check_box_draw, w3d_gadget_check_box_image_draw, w3d_gadget_combo_box_draw,
    w3d_gadget_combo_box_image_draw, w3d_gadget_horizontal_slider_draw,
    w3d_gadget_horizontal_slider_image_draw, w3d_gadget_list_box_draw,
    w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_push_button_draw,
    w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw, w3d_left_hud_draw,
    w3d_main_menu_button_drop_shadow_draw, w3d_main_menu_draw, w3d_main_menu_four_draw,
    w3d_main_menu_map_border, w3d_main_menu_random_text_draw, w3d_metal_bar_menu_draw, w3d_no_draw,
    w3d_power_draw, w3d_right_hud_draw, w3d_shell_menu_scheme_draw, w3d_thin_border_draw,
};
use game_client::gui::GameWindow;

type GameWinDrawFunc = fn(&GameWindow, &WindowInstanceData);

/// Load the W3D-specific function tables.
pub fn load_w3d_tables(lexicon: &mut FunctionLexicon) {
    lexicon.load_table(game_win_draw_table(), TableIndex::GameWinDeviceDraw);
    lexicon.load_table(layout_init_table(), TableIndex::WinLayoutDeviceInit);
}

fn draw_func_ptr(func: GameWinDrawFunc) -> FunctionPtr {
    FunctionPtr(func as *const ())
}

fn game_win_draw_table() -> Vec<TableEntry> {
    vec![
        TableEntry::new(
            "GameWinDefaultDraw",
            Some(draw_func_ptr(default_draw_callback)),
        ),
        TableEntry::new(
            "W3DGameWinDefaultDraw",
            Some(draw_func_ptr(default_draw_callback)),
        ),
        TableEntry::new(
            "W3DGadgetPushButtonDraw",
            Some(draw_func_ptr(w3d_gadget_push_button_draw)),
        ),
        TableEntry::new(
            "W3DGadgetPushButtonImageDraw",
            Some(draw_func_ptr(w3d_gadget_push_button_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetCheckBoxDraw",
            Some(draw_func_ptr(w3d_gadget_check_box_draw)),
        ),
        TableEntry::new(
            "W3DGadgetCheckBoxImageDraw",
            Some(draw_func_ptr(w3d_gadget_check_box_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetRadioButtonDraw",
            Some(draw_func_ptr(w3d_gadget_radio_button_draw)),
        ),
        TableEntry::new(
            "W3DGadgetRadioButtonImageDraw",
            Some(draw_func_ptr(w3d_gadget_radio_button_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetTabControlDraw",
            Some(draw_func_ptr(w3d_gadget_tab_control_draw)),
        ),
        TableEntry::new(
            "W3DGadgetTabControlImageDraw",
            Some(draw_func_ptr(w3d_gadget_tab_control_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetListBoxDraw",
            Some(draw_func_ptr(w3d_gadget_list_box_draw)),
        ),
        TableEntry::new(
            "W3DGadgetListBoxImageDraw",
            Some(draw_func_ptr(w3d_gadget_list_box_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetComboBoxDraw",
            Some(draw_func_ptr(w3d_gadget_combo_box_draw)),
        ),
        TableEntry::new(
            "W3DGadgetComboBoxImageDraw",
            Some(draw_func_ptr(w3d_gadget_combo_box_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetHorizontalSliderDraw",
            Some(draw_func_ptr(w3d_gadget_horizontal_slider_draw)),
        ),
        TableEntry::new(
            "W3DGadgetHorizontalSliderImageDraw",
            Some(draw_func_ptr(w3d_gadget_horizontal_slider_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetVerticalSliderDraw",
            Some(draw_func_ptr(w3d_gadget_vertical_slider_draw)),
        ),
        TableEntry::new(
            "W3DGadgetVerticalSliderImageDraw",
            Some(draw_func_ptr(w3d_gadget_vertical_slider_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetProgressBarDraw",
            Some(draw_func_ptr(w3d_gadget_progress_bar_draw)),
        ),
        TableEntry::new(
            "W3DGadgetProgressBarImageDraw",
            Some(draw_func_ptr(w3d_gadget_progress_bar_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetStaticTextDraw",
            Some(draw_func_ptr(w3d_gadget_static_text_draw)),
        ),
        TableEntry::new(
            "W3DGadgetStaticTextImageDraw",
            Some(draw_func_ptr(w3d_gadget_static_text_image_draw)),
        ),
        TableEntry::new(
            "W3DGadgetTextEntryDraw",
            Some(draw_func_ptr(w3d_gadget_text_entry_draw)),
        ),
        TableEntry::new(
            "W3DGadgetTextEntryImageDraw",
            Some(draw_func_ptr(w3d_gadget_text_entry_image_draw)),
        ),
        TableEntry::new("W3DLeftHUDDraw", Some(draw_func_ptr(w3d_left_hud_draw))),
        TableEntry::new(
            "W3DCameoMovieDraw",
            Some(draw_func_ptr(w3d_cameo_movie_draw)),
        ),
        TableEntry::new("W3DRightHUDDraw", Some(draw_func_ptr(w3d_right_hud_draw))),
        TableEntry::new("W3DPowerDraw", Some(draw_func_ptr(w3d_power_draw))),
        TableEntry::new("W3DMainMenuDraw", Some(draw_func_ptr(w3d_main_menu_draw))),
        TableEntry::new(
            "W3DMainMenuFourDraw",
            Some(draw_func_ptr(w3d_main_menu_four_draw)),
        ),
        TableEntry::new(
            "W3DMetalBarMenuDraw",
            Some(draw_func_ptr(w3d_metal_bar_menu_draw)),
        ),
        TableEntry::new(
            "W3DCreditsMenuDraw",
            Some(draw_func_ptr(w3d_credits_menu_draw)),
        ),
        TableEntry::new("W3DClockDraw", Some(draw_func_ptr(w3d_clock_draw))),
        TableEntry::new(
            "W3DMainMenuMapBorder",
            Some(draw_func_ptr(w3d_main_menu_map_border)),
        ),
        TableEntry::new(
            "W3DMainMenuButtonDropShadowDraw",
            Some(draw_func_ptr(w3d_main_menu_button_drop_shadow_draw)),
        ),
        TableEntry::new(
            "W3DMainMenuRandomTextDraw",
            Some(draw_func_ptr(w3d_main_menu_random_text_draw)),
        ),
        TableEntry::new(
            "W3DThinBorderDraw",
            Some(draw_func_ptr(w3d_thin_border_draw)),
        ),
        TableEntry::new(
            "W3DShellMenuSchemeDraw",
            Some(draw_func_ptr(w3d_shell_menu_scheme_draw)),
        ),
        TableEntry::new(
            "W3DCommandBarBackgroundDraw",
            Some(draw_func_ptr(w3d_command_bar_background_draw)),
        ),
        TableEntry::new(
            "W3DCommandBarTopDraw",
            Some(draw_func_ptr(w3d_command_bar_top_draw)),
        ),
        TableEntry::new(
            "W3DCommandBarGenExpDraw",
            Some(draw_func_ptr(w3d_command_bar_gen_exp_draw)),
        ),
        TableEntry::new(
            "W3DCommandBarHelpPopupDraw",
            Some(draw_func_ptr(w3d_command_bar_help_popup_draw)),
        ),
        TableEntry::new(
            "W3DCommandBarGridDraw",
            Some(draw_func_ptr(w3d_command_bar_grid_draw)),
        ),
        TableEntry::new(
            "W3DCommandBarForegroundDraw",
            Some(draw_func_ptr(w3d_command_bar_foreground_draw)),
        ),
        TableEntry::new("W3DNoDraw", Some(draw_func_ptr(w3d_no_draw))),
        TableEntry::new("W3DDrawMapPreview", Some(draw_func_ptr(draw_map_preview))),
    ]
}

fn layout_init_table() -> Vec<TableEntry> {
    vec![TableEntry::new("W3DMainMenuInit", None)]
}
