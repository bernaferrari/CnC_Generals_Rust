pub use crate::gui::game_window_transitions::GameWindowTransitionsHandler;

pub const TRANSITION_FLASH: i32 = 0;
pub const BUTTON_TRANSITION_FLASH: i32 = 1;
pub const WIN_FADE_TRANSITION: i32 = 2;
pub const WIN_SCALE_UP_TRANSITION: i32 = 3;
pub const MAINMENU_SCALE_UP_TRANSITION: i32 = 4;
pub const TEXT_TYPE_TRANSITION: i32 = 5;
pub const SCREEN_FADE_TRANSITION: i32 = 6;
pub const COUNT_UP_TRANSITION: i32 = 7;
pub const FULL_FADE_TRANSITION: i32 = 8;
pub const TEXT_ON_FRAME_TRANSITION: i32 = 9;
pub const MAINMENU_MEDIUM_SCALE_UP_TRANSITION: i32 = 10;
pub const MAINMENU_SMALL_SCALE_DOWN_TRANSITION: i32 = 11;
pub const CONTROL_BAR_ARROW_TRANSITION: i32 = 12;
pub const SCORE_SCALE_UP_TRANSITION: i32 = 13;
pub const REVERSE_SOUND_TRANSITION: i32 = 14;

pub fn transition_style_from_name(token: &str) -> Option<i32> {
    match token.trim().to_ascii_uppercase().as_str() {
        "FLASH" => Some(TRANSITION_FLASH),
        "BUTTONFLASH" => Some(BUTTON_TRANSITION_FLASH),
        "WINFADE" => Some(WIN_FADE_TRANSITION),
        "WINSCALEUP" => Some(WIN_SCALE_UP_TRANSITION),
        "MAINMENUSCALEUP" => Some(MAINMENU_SCALE_UP_TRANSITION),
        "TYPETEXT" => Some(TEXT_TYPE_TRANSITION),
        "SCREENFADE" => Some(SCREEN_FADE_TRANSITION),
        "COUNTUP" => Some(COUNT_UP_TRANSITION),
        "FULLFADE" => Some(FULL_FADE_TRANSITION),
        "TEXTONFRAME" => Some(TEXT_ON_FRAME_TRANSITION),
        "MAINMENUMEDIUMSCALEUP" => Some(MAINMENU_MEDIUM_SCALE_UP_TRANSITION),
        "MAINMENUSMALLSCALEDOWN" => Some(MAINMENU_SMALL_SCALE_DOWN_TRANSITION),
        "CONTROLBARARROW" => Some(CONTROL_BAR_ARROW_TRANSITION),
        "SCORESCALEUP" => Some(SCORE_SCALE_UP_TRANSITION),
        "REVERSESOUND" => Some(REVERSE_SOUND_TRANSITION),
        _ => None,
    }
}

pub fn parse_bool_token(value: &str) -> bool {
    matches!(
        value.trim().to_ascii_lowercase().as_str(),
        "yes" | "true" | "1"
    )
}

// PARITY_NOTE: concrete transition classes are implemented in the canonical
// `gui::game_window_transitions` module; this file restores the named-style compatibility layer.
