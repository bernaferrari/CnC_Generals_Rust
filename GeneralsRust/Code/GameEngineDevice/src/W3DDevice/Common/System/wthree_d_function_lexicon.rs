//! W3D Function Lexicon — draw callback dispatch table
//!
//! Port of: GameEngineDevice/Source/W3DDevice/Common/System/W3DFunctionLexicon.cpp
//!
//! Registers W3D-specific GUI draw callbacks and layout init callbacks.
//! C++ uses function pointer tables (TABLE_GAME_WIN_DEVICEDRAW, TABLE_WIN_LAYOUT_DEVICEINIT).
//! Rust uses a HashMap<String, DrawCallback> for equivalent dispatch.

use std::collections::HashMap;

pub type DrawCallback = fn();

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

fn stub_draw_callback() {
    // Placeholder — individual draw implementations live in W3DGadget* modules
}

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
        for name in W3D_DRAW_CALLBACK_NAMES {
            self.draw_table
                .insert((*name).to_string(), stub_draw_callback);
        }
    }

    fn load_layout_init_table(&mut self) {
        self.layout_init_table
            .insert("W3DMainMenuInit".to_string(), || {});
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
}
