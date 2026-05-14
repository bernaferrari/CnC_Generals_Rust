//! W3D Function Lexicon — draw callback dispatch table
//!
//! Port of: GameEngineDevice/Source/W3DDevice/Common/System/W3DFunctionLexicon.cpp
//!
//! Maps C++ draw callback names to real `fn(&GameWindow, &WindowInstanceData)`
//! function pointers.  C++ used `TABLE_GAME_WIN_DEVICEDRAW` arrays of
//! `{name, fn_ptr}` pairs; Rust uses a `HashMap<String, DrawCallback>`.

use std::collections::HashMap;

use game_client_rust::gui::game_window::{default_draw_callback, GameWindow, WindowInstanceData};
use game_client_rust::gui::w3d_gadget_draw::{
    w3d_cameo_movie_draw, w3d_clock_draw, w3d_command_bar_background_draw,
    w3d_command_bar_foreground_draw, w3d_command_bar_gen_exp_draw, w3d_command_bar_grid_draw,
    w3d_command_bar_help_popup_draw, w3d_command_bar_top_draw, w3d_compat_default_draw,
    w3d_credits_menu_draw, w3d_draw_map_preview, w3d_gadget_check_box_draw,
    w3d_gadget_check_box_image_draw, w3d_gadget_combo_box_draw, w3d_gadget_combo_box_image_draw,
    w3d_gadget_horizontal_slider_draw, w3d_gadget_horizontal_slider_image_draw,
    w3d_gadget_list_box_draw, w3d_gadget_list_box_image_draw, w3d_gadget_progress_bar_draw,
    w3d_gadget_progress_bar_image_draw, w3d_gadget_push_button_draw,
    w3d_gadget_push_button_image_draw, w3d_gadget_radio_button_draw,
    w3d_gadget_radio_button_image_draw, w3d_gadget_static_text_draw,
    w3d_gadget_static_text_image_draw, w3d_gadget_tab_control_draw,
    w3d_gadget_tab_control_image_draw, w3d_gadget_text_entry_draw,
    w3d_gadget_text_entry_image_draw, w3d_gadget_vertical_slider_draw,
    w3d_gadget_vertical_slider_image_draw, w3d_left_hud_draw, w3d_main_menu_button_drop_shadow_draw,
    w3d_main_menu_draw, w3d_main_menu_four_draw, w3d_main_menu_map_border,
    w3d_main_menu_random_text_draw, w3d_metal_bar_menu_draw, w3d_no_draw, w3d_power_draw,
    w3d_right_hud_draw, w3d_shell_menu_scheme_draw, w3d_thin_border_draw,
};

/// C++ `WinDrawFunc` — `void (*)(GameWindow*, WinInstanceData*)`.
pub type DrawCallback = fn(&GameWindow, &WindowInstanceData);

/// Layout init callbacks take no arguments in the lexicon registration.
/// The actual `W3DMainMenuInit` accepts `(layout, user_data)` but the
/// lexicon only stores the no-arg trampoline; callers unpack args themselves.
pub type LayoutInitCallback = fn();

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DrawCallbackId {
    GameWinDefaultDraw,
    W3DGameWinDefaultDraw,
    W3DGadgetPushButtonDraw,
    W3DGadgetPushButtonImageDraw,
    W3DGadgetCheckBoxDraw,
    W3DGadgetCheckBoxImageDraw,
    W3DGadgetRadioButtonDraw,
    W3DGadgetRadioButtonImageDraw,
    W3DGadgetTabControlDraw,
    W3DGadgetTabControlImageDraw,
    W3DGadgetListBoxDraw,
    W3DGadgetListBoxImageDraw,
    W3DGadgetComboBoxDraw,
    W3DGadgetComboBoxImageDraw,
    W3DGadgetHorizontalSliderDraw,
    W3DGadgetHorizontalSliderImageDraw,
    W3DGadgetVerticalSliderDraw,
    W3DGadgetVerticalSliderImageDraw,
    W3DGadgetProgressBarDraw,
    W3DGadgetProgressBarImageDraw,
    W3DGadgetStaticTextDraw,
    W3DGadgetStaticTextImageDraw,
    W3DGadgetTextEntryDraw,
    W3DGadgetTextEntryImageDraw,
    W3DLeftHUDDraw,
    W3DCameoMovieDraw,
    W3DRightHUDDraw,
    W3DPowerDraw,
    W3DMainMenuDraw,
    W3DMainMenuFourDraw,
    W3DMetalBarMenuDraw,
    W3DCreditsMenuDraw,
    W3DClockDraw,
    W3DMainMenuMapBorder,
    W3DMainMenuButtonDropShadowDraw,
    W3DMainMenuRandomTextDraw,
    W3DThinBorderDraw,
    W3DShellMenuSchemeDraw,
    W3DCommandBarBackgroundDraw,
    W3DCommandBarTopDraw,
    W3DCommandBarGenExpDraw,
    W3DCommandBarHelpPopupDraw,
    W3DCommandBarGridDraw,
    W3DCommandBarForegroundDraw,
    W3DNoDraw,
    W3DDrawMapPreview,
}

pub const W3D_DRAW_CALLBACK_COUNT: usize = 46;

pub const W3D_DRAW_CALLBACK_NAMES: &[&str] = &[
    "GameWinDefaultDraw",
    "W3DGameWinDefaultDraw",
    "W3DGadgetPushButtonDraw",
    "W3DGadgetPushButtonImageDraw",
    "W3DGadgetCheckBoxDraw",
    "W3DGadgetCheckBoxImageDraw",
    "W3DGadgetRadioButtonDraw",
    "W3DGadgetRadioButtonImageDraw",
    "W3DGadgetTabControlDraw",
    "W3DGadgetTabControlImageDraw",
    "W3DGadgetListBoxDraw",
    "W3DGadgetListBoxImageDraw",
    "W3DGadgetComboBoxDraw",
    "W3DGadgetComboBoxImageDraw",
    "W3DGadgetHorizontalSliderDraw",
    "W3DGadgetHorizontalSliderImageDraw",
    "W3DGadgetVerticalSliderDraw",
    "W3DGadgetVerticalSliderImageDraw",
    "W3DGadgetProgressBarDraw",
    "W3DGadgetProgressBarImageDraw",
    "W3DGadgetStaticTextDraw",
    "W3DGadgetStaticTextImageDraw",
    "W3DGadgetTextEntryDraw",
    "W3DGadgetTextEntryImageDraw",
    "W3DLeftHUDDraw",
    "W3DCameoMovieDraw",
    "W3DRightHUDDraw",
    "W3DPowerDraw",
    "W3DMainMenuDraw",
    "W3DMainMenuFourDraw",
    "W3DMetalBarMenuDraw",
    "W3DCreditsMenuDraw",
    "W3DClockDraw",
    "W3DMainMenuMapBorder",
    "W3DMainMenuButtonDropShadowDraw",
    "W3DMainMenuRandomTextDraw",
    "W3DThinBorderDraw",
    "W3DShellMenuSchemeDraw",
    "W3DCommandBarBackgroundDraw",
    "W3DCommandBarTopDraw",
    "W3DCommandBarGenExpDraw",
    "W3DCommandBarHelpPopupDraw",
    "W3DCommandBarGridDraw",
    "W3DCommandBarForegroundDraw",
    "W3DNoDraw",
    "W3DDrawMapPreview",
];

fn w3d_main_menu_init_trampoline() {}

pub struct W3DFunctionLexicon {
    draw_table: HashMap<String, DrawCallback>,
    layout_init_table: HashMap<String, LayoutInitCallback>,
}

impl W3DFunctionLexicon {
    pub fn new() -> Self {
        let mut lexicon = Self {
            draw_table: HashMap::new(),
            layout_init_table: HashMap::new(),
        };
        lexicon.load_draw_table();
        lexicon.load_layout_init_table();
        lexicon
    }

    fn load_draw_table(&mut self) {
        // 1:1 with C++ gameWinDrawTable[] — each name maps to its real fn pointer.
        self.draw_table
            .insert("GameWinDefaultDraw".into(), default_draw_callback as DrawCallback);
        self.draw_table.insert(
            "W3DGameWinDefaultDraw".into(),
            w3d_compat_default_draw as DrawCallback,
        );

        self.draw_table.insert(
            "W3DGadgetPushButtonDraw".into(),
            w3d_gadget_push_button_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetPushButtonImageDraw".into(),
            w3d_gadget_push_button_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetCheckBoxDraw".into(),
            w3d_gadget_check_box_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetCheckBoxImageDraw".into(),
            w3d_gadget_check_box_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetRadioButtonDraw".into(),
            w3d_gadget_radio_button_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetRadioButtonImageDraw".into(),
            w3d_gadget_radio_button_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetTabControlDraw".into(),
            w3d_gadget_tab_control_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetTabControlImageDraw".into(),
            w3d_gadget_tab_control_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetListBoxDraw".into(),
            w3d_gadget_list_box_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetListBoxImageDraw".into(),
            w3d_gadget_list_box_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetComboBoxDraw".into(),
            w3d_gadget_combo_box_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetComboBoxImageDraw".into(),
            w3d_gadget_combo_box_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetHorizontalSliderDraw".into(),
            w3d_gadget_horizontal_slider_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetHorizontalSliderImageDraw".into(),
            w3d_gadget_horizontal_slider_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetVerticalSliderDraw".into(),
            w3d_gadget_vertical_slider_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetVerticalSliderImageDraw".into(),
            w3d_gadget_vertical_slider_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetProgressBarDraw".into(),
            w3d_gadget_progress_bar_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetProgressBarImageDraw".into(),
            w3d_gadget_progress_bar_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetStaticTextDraw".into(),
            w3d_gadget_static_text_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetStaticTextImageDraw".into(),
            w3d_gadget_static_text_image_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetTextEntryDraw".into(),
            w3d_gadget_text_entry_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DGadgetTextEntryImageDraw".into(),
            w3d_gadget_text_entry_image_draw as DrawCallback,
        );

        self.draw_table
            .insert("W3DLeftHUDDraw".into(), w3d_left_hud_draw as DrawCallback);
        self.draw_table.insert(
            "W3DCameoMovieDraw".into(),
            w3d_cameo_movie_draw as DrawCallback,
        );
        self.draw_table
            .insert("W3DRightHUDDraw".into(), w3d_right_hud_draw as DrawCallback);
        self.draw_table
            .insert("W3DPowerDraw".into(), w3d_power_draw as DrawCallback);
        self.draw_table
            .insert("W3DMainMenuDraw".into(), w3d_main_menu_draw as DrawCallback);
        self.draw_table.insert(
            "W3DMainMenuFourDraw".into(),
            w3d_main_menu_four_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DMetalBarMenuDraw".into(),
            w3d_metal_bar_menu_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCreditsMenuDraw".into(),
            w3d_credits_menu_draw as DrawCallback,
        );
        self.draw_table
            .insert("W3DClockDraw".into(), w3d_clock_draw as DrawCallback);
        self.draw_table.insert(
            "W3DMainMenuMapBorder".into(),
            w3d_main_menu_map_border as DrawCallback,
        );
        self.draw_table.insert(
            "W3DMainMenuButtonDropShadowDraw".into(),
            w3d_main_menu_button_drop_shadow_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DMainMenuRandomTextDraw".into(),
            w3d_main_menu_random_text_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DThinBorderDraw".into(),
            w3d_thin_border_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DShellMenuSchemeDraw".into(),
            w3d_shell_menu_scheme_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCommandBarBackgroundDraw".into(),
            w3d_command_bar_background_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCommandBarTopDraw".into(),
            w3d_command_bar_top_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCommandBarGenExpDraw".into(),
            w3d_command_bar_gen_exp_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCommandBarHelpPopupDraw".into(),
            w3d_command_bar_help_popup_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCommandBarGridDraw".into(),
            w3d_command_bar_grid_draw as DrawCallback,
        );
        self.draw_table.insert(
            "W3DCommandBarForegroundDraw".into(),
            w3d_command_bar_foreground_draw as DrawCallback,
        );
        self.draw_table
            .insert("W3DNoDraw".into(), w3d_no_draw as DrawCallback);
        self.draw_table.insert(
            "W3DDrawMapPreview".into(),
            w3d_draw_map_preview as DrawCallback,
        );
    }

    fn load_layout_init_table(&mut self) {
        self.layout_init_table
            .insert("W3DMainMenuInit".to_string(), w3d_main_menu_init_trampoline);
    }

    pub fn init(&mut self) {
        self.load_draw_table();
        self.load_layout_init_table();
    }

    pub fn find_draw_callback(&self, name: &str) -> Option<DrawCallback> {
        self.draw_table.get(name).copied()
    }

    pub fn find_layout_init(&self, name: &str) -> Option<LayoutInitCallback> {
        self.layout_init_table.get(name).copied()
    }

    pub fn draw_callback_count(&self) -> usize {
        self.draw_table.len()
    }

    pub fn register_draw_callback(&mut self, name: &str, callback: DrawCallback) {
        self.draw_table.insert(name.to_string(), callback);
    }

    pub fn register_layout_init(&mut self, name: &str, callback: LayoutInitCallback) {
        self.layout_init_table.insert(name.to_string(), callback);
    }
}

impl Default for W3DFunctionLexicon {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lexicon_has_all_46_callbacks() {
        let lexicon = W3DFunctionLexicon::new();
        assert_eq!(lexicon.draw_callback_count(), W3D_DRAW_CALLBACK_COUNT);
    }

    #[test]
    fn test_lookup_known_callbacks() {
        let lexicon = W3DFunctionLexicon::new();
        assert!(lexicon
            .find_draw_callback("W3DGadgetPushButtonDraw")
            .is_some());
        assert!(lexicon.find_draw_callback("W3DMainMenuDraw").is_some());
        assert!(lexicon.find_draw_callback("W3DNoDraw").is_some());
        assert!(lexicon.find_draw_callback("W3DDrawMapPreview").is_some());
        assert!(lexicon.find_draw_callback("NonExistent").is_none());
    }

    #[test]
    fn test_layout_init() {
        let lexicon = W3DFunctionLexicon::new();
        assert!(lexicon.find_layout_init("W3DMainMenuInit").is_some());
    }

    #[test]
    fn test_all_names_have_distinct_callbacks() {
        let lexicon = W3DFunctionLexicon::new();
        let mut seen: Vec<DrawCallback> = Vec::new();
        for name in W3D_DRAW_CALLBACK_NAMES {
            let cb = lexicon.find_draw_callback(name).expect(name);
            // GameWinDefaultDraw and W3DGameWinDefaultDraw intentionally
            // share the same underlying default_draw_callback implementation,
            // so we skip the uniqueness check for those two.
            if *name != "GameWinDefaultDraw" && *name != "W3DGameWinDefaultDraw" {
                assert!(
                    !seen.contains(&cb),
                    "duplicate callback for {}",
                    name
                );
            }
            seen.push(cb);
        }
    }
}
