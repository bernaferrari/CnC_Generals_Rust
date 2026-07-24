//! Wave 113 residual peels: Gadget / WinInstance / Video / Audio residual
//! (host-testable client UI+media path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 106 GameWindow WIN_STATUS/GWM/WindowLayout residual,
//! Wave 107 FX/OCL/particle/audio residual, Wave 111 Drawable/Display residual,
//! Wave 112 Mouse/Keyboard/View residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - GameWindow.h WIN_MAX_WINDOWS / TOOLTIP / CURSOR_MOVE_TOL_SQ / GWM_USER
//! - GameWindowManager.h MAX_LAYOUT_FUNC_LEN
//! - GameWindowManagerScript.cpp WindowStyleNames / WIN_STACK_DEPTH
//! - Gadget.h GadgetGameMessage band + NUM_BORDER_PIECES
//! - WinInstanceData.h MAX_WINDOW_NAME_LEN / MAX_DRAW_DATA / MAX_TEXT_LABEL
//! - VideoPlayer.h VideoBuffer::Type
//! - AudioEventRTS.h OwnerType / PortionToPlay
//! - AudioEventInfo.h AudioPriority / SoundType bits / AudioControl bits
//!
//! Fail-closed:
//! - Not full .wnd script parse / gadget factory residual
//! - Not full Bink/video decode residual
//! - Not full Miles audio mixer residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// GameWindow manager residual (not covered by Wave 106)
// ---------------------------------------------------------------------------

/// Retail `WIN_MAX_WINDOWS` residual.
pub const WIN_MAX_WINDOWS: u32 = 576;
/// Retail `CURSOR_MOVE_TOL_SQ` residual.
pub const CURSOR_MOVE_TOL_SQ: u32 = 4;
/// Retail `TOOLTIP_DELAY` residual (frames).
pub const TOOLTIP_DELAY: u32 = 10;
/// Retail `WIN_TOOLTIP_LEN` residual.
pub const WIN_TOOLTIP_LEN: u32 = 64;
/// Retail `MAX_LAYOUT_FUNC_LEN` residual.
pub const MAX_LAYOUT_FUNC_LEN: u32 = 256;
/// Retail `WIN_STACK_DEPTH` residual (GameWindowManagerScript.cpp).
pub const WIN_STACK_DEPTH: u32 = 10;
/// Retail `GWM_USER` residual base (cross-link Wave 106).
pub const GWM_USER: u32 = 32768;

/// Retail `WinInputReturnCode` residual names.
pub const WIN_INPUT_RETURN_CODE_NAMES: &[&str] = &["WIN_INPUT_NOT_USED", "WIN_INPUT_USED"];
pub const WIN_INPUT_NOT_USED: u32 = 0;
pub const WIN_INPUT_USED: u32 = 1;

// ---------------------------------------------------------------------------
// WinInstanceData residual
// ---------------------------------------------------------------------------

/// Retail `MAX_WINDOW_NAME_LEN` residual.
pub const MAX_WINDOW_NAME_LEN: u32 = 64;
/// Retail `MAX_DRAW_DATA` residual.
pub const MAX_DRAW_DATA: u32 = 9;
/// Retail `MAX_TEXT_LABEL` residual.
pub const MAX_TEXT_LABEL: u32 = 128;

// ---------------------------------------------------------------------------
// Window style residual (WindowStyleNames[])
// ---------------------------------------------------------------------------

/// Retail `WindowStyleNames[]` residual (NULL-terminated table without NULL).
pub const WINDOW_STYLE_NAMES: &[&str] = &[
    "PUSHBUTTON",
    "RADIOBUTTON",
    "CHECKBOX",
    "VERTSLIDER",
    "HORZSLIDER",
    "SCROLLLISTBOX",
    "ENTRYFIELD",
    "STATICTEXT",
    "PROGRESSBAR",
    "USER",
    "MOUSETRACK",
    "ANIMATED",
    "TABSTOP",
    "TABCONTROL",
    "TABPANE",
    "COMBOBOX",
];
/// Retail window style name count residual.
pub const WINDOW_STYLE_COUNT: usize = 16;

// ---------------------------------------------------------------------------
// Gadget residual
// ---------------------------------------------------------------------------

/// Retail `GGM_LEFT_DRAG` residual base for gadget messages.
pub const GGM_LEFT_DRAG: u32 = 16384;

/// Retail `GadgetGameMessage` residual names (ordered from GGM_LEFT_DRAG).
pub const GADGET_GAME_MESSAGE_NAMES: &[&str] = &[
    "GGM_LEFT_DRAG",
    "GGM_SET_LABEL",
    "GGM_GET_LABEL",
    "GGM_FOCUS_CHANGE",
    "GGM_RESIZED",
    "GGM_CLOSE",
    "GBM_MOUSE_ENTERING",
    "GBM_MOUSE_LEAVING",
    "GBM_SELECTED",
    "GBM_SELECTED_RIGHT",
    "GBM_SET_SELECTION",
    "GSM_SLIDER_TRACK",
    "GSM_SET_SLIDER",
    "GSM_SET_MIN_MAX",
    "GSM_SLIDER_DONE",
    "GLM_ADD_ENTRY",
    "GLM_DEL_ENTRY",
    "GLM_DEL_ALL",
    "GLM_SELECTED",
    "GLM_DOUBLE_CLICKED",
    "GLM_RIGHT_CLICKED",
    "GLM_SET_SELECTION",
    "GLM_GET_SELECTION",
    "GLM_TOGGLE_MULTI_SELECTION",
    "GLM_GET_TEXT",
    "GLM_SET_UP_BUTTON",
    "GLM_SET_DOWN_BUTTON",
    "GLM_SET_SLIDER",
    "GLM_SCROLL_BUFFER",
    "GLM_UPDATE_DISPLAY",
    "GLM_GET_ITEM_DATA",
    "GLM_SET_ITEM_DATA",
    "GCM_ADD_ENTRY",
    "GCM_DEL_ENTRY",
    "GCM_DEL_ALL",
    "GCM_SELECTED",
    "GCM_GET_TEXT",
    "GCM_SET_TEXT",
    "GCM_EDIT_DONE",
    "GCM_GET_ITEM_DATA",
    "GCM_SET_ITEM_DATA",
    "GCM_GET_SELECTION",
    "GCM_SET_SELECTION",
    "GCM_UPDATE_TEXT",
    "GEM_GET_TEXT",
    "GEM_SET_TEXT",
    "GEM_EDIT_DONE",
    "GEM_UPDATE_TEXT",
    "GPM_SET_PROGRESS",
];

/// Retail gadget message count residual.
pub const GADGET_GAME_MESSAGE_COUNT: usize = 49;

/// Map residual gadget message name → absolute message id (GGM_LEFT_DRAG + index).
pub fn gadget_message_id_residual(name: &str) -> Option<u32> {
    residual_name_index(GADGET_GAME_MESSAGE_NAMES, name).map(|i| GGM_LEFT_DRAG + i as u32)
}

/// Retail border piece residual names (`NUM_BORDER_PIECES`).
pub const BORDER_PIECE_NAMES: &[&str] = &[
    "BORDER_CORNER_UL",
    "BORDER_CORNER_UR",
    "BORDER_CORNER_LL",
    "BORDER_CORNER_LR",
    "BORDER_VERTICAL_LEFT",
    "BORDER_VERTICAL_LEFT_SHORT",
    "BORDER_VERTICAL_RIGHT",
    "BORDER_VERTICAL_RIGHT_SHORT",
    "BORDER_HORIZONTAL_TOP",
    "BORDER_HORIZONTAL_TOP_SHORT",
    "BORDER_HORIZONTAL_BOTTOM",
    "BORDER_HORIZONTAL_BOTTOM_SHORT",
];
/// Retail `NUM_BORDER_PIECES` residual.
pub const NUM_BORDER_PIECES: usize = 12;

// ---------------------------------------------------------------------------
// Video residual
// ---------------------------------------------------------------------------

/// Retail `VideoBuffer::Type` residual names.
pub const VIDEO_BUFFER_TYPE_NAMES: &[&str] = &[
    "TYPE_UNKNOWN",
    "TYPE_R8G8B8",
    "TYPE_X8R8G8B8",
    "TYPE_R5G6B5",
    "TYPE_X1R5G5B5",
];
/// Retail `VideoBuffer::NUM_TYPES` residual.
pub const VIDEO_BUFFER_NUM_TYPES: usize = 5;

// ---------------------------------------------------------------------------
// Audio residual
// ---------------------------------------------------------------------------

/// Retail `OwnerType` residual names (AudioEventRTS.h).
pub const AUDIO_OWNER_TYPE_NAMES: &[&str] = &[
    "OT_Positional",
    "OT_Drawable",
    "OT_Object",
    "OT_Dead",
    "OT_INVALID",
];
pub const AUDIO_OWNER_TYPE_COUNT: usize = 5;

/// Retail `PortionToPlay` residual names.
pub const AUDIO_PORTION_TO_PLAY_NAMES: &[&str] = &["PP_Attack", "PP_Sound", "PP_Decay", "PP_Done"];
pub const AUDIO_PORTION_TO_PLAY_COUNT: usize = 4;

/// Retail `AudioPriority` residual names.
pub const AUDIO_PRIORITY_NAMES: &[&str] =
    &["AP_LOWEST", "AP_LOW", "AP_NORMAL", "AP_HIGH", "AP_CRITICAL"];
pub const AUDIO_PRIORITY_COUNT: usize = 5;

/// Retail `SoundType` residual bit flags.
pub const SOUND_TYPE_UI: u32 = 0x0001;
pub const SOUND_TYPE_WORLD: u32 = 0x0002;
pub const SOUND_TYPE_SHROUDED: u32 = 0x0004;
pub const SOUND_TYPE_GLOBAL: u32 = 0x0008;
pub const SOUND_TYPE_VOICE: u32 = 0x0010;
pub const SOUND_TYPE_PLAYER: u32 = 0x0020;
pub const SOUND_TYPE_ALLIES: u32 = 0x0040;
pub const SOUND_TYPE_ENEMIES: u32 = 0x0080;
pub const SOUND_TYPE_EVERYONE: u32 = 0x0100;

/// Retail sound-type residual name/bit pairs (ordered low→high bit).
pub const SOUND_TYPE_TABLE: &[(&str, u32)] = &[
    ("ST_UI", SOUND_TYPE_UI),
    ("ST_WORLD", SOUND_TYPE_WORLD),
    ("ST_SHROUDED", SOUND_TYPE_SHROUDED),
    ("ST_GLOBAL", SOUND_TYPE_GLOBAL),
    ("ST_VOICE", SOUND_TYPE_VOICE),
    ("ST_PLAYER", SOUND_TYPE_PLAYER),
    ("ST_ALLIES", SOUND_TYPE_ALLIES),
    ("ST_ENEMIES", SOUND_TYPE_ENEMIES),
    ("ST_EVERYONE", SOUND_TYPE_EVERYONE),
];

/// Retail `AudioControl` residual bit flags.
pub const AUDIO_CONTROL_LOOP: u32 = 0x0001;
pub const AUDIO_CONTROL_RANDOM: u32 = 0x0002;
pub const AUDIO_CONTROL_ALL: u32 = 0x0004;
pub const AUDIO_CONTROL_POSTDELAY: u32 = 0x0008;
pub const AUDIO_CONTROL_INTERRUPT: u32 = 0x0010;

/// Retail audio-control residual name/bit pairs.
pub const AUDIO_CONTROL_TABLE: &[(&str, u32)] = &[
    ("AC_LOOP", AUDIO_CONTROL_LOOP),
    ("AC_RANDOM", AUDIO_CONTROL_RANDOM),
    ("AC_ALL", AUDIO_CONTROL_ALL),
    ("AC_POSTDELAY", AUDIO_CONTROL_POSTDELAY),
    ("AC_INTERRUPT", AUDIO_CONTROL_INTERRUPT),
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: GameWindow manager size residual pack.
pub fn honesty_game_window_manager_residual_wave113() -> bool {
    WIN_MAX_WINDOWS == 576
        && CURSOR_MOVE_TOL_SQ == 4
        && TOOLTIP_DELAY == 10
        && WIN_TOOLTIP_LEN == 64
        && MAX_LAYOUT_FUNC_LEN == 256
        && WIN_STACK_DEPTH == 10
        && GWM_USER == 32768
        && WIN_INPUT_RETURN_CODE_NAMES.len() == 2
        && WIN_INPUT_NOT_USED == 0
        && WIN_INPUT_USED == 1
        && residual_name_index(WIN_INPUT_RETURN_CODE_NAMES, "WIN_INPUT_NOT_USED") == Some(0)
        && residual_name_index(WIN_INPUT_RETURN_CODE_NAMES, "WIN_INPUT_USED") == Some(1)
        && MAX_WINDOW_NAME_LEN == 64
        && MAX_DRAW_DATA == 9
        && MAX_TEXT_LABEL == 128
}

/// Honesty: Window style residual pack.
pub fn honesty_window_style_residual_wave113() -> bool {
    WINDOW_STYLE_NAMES.len() == WINDOW_STYLE_COUNT
        && residual_name_index(WINDOW_STYLE_NAMES, "PUSHBUTTON") == Some(0)
        && residual_name_index(WINDOW_STYLE_NAMES, "COMBOBOX") == Some(15)
        && residual_name_index(WINDOW_STYLE_NAMES, "SCROLLLISTBOX") == Some(5)
        && residual_name_index(WINDOW_STYLE_NAMES, "NoSuch").is_none()
}

/// Honesty: Gadget message + border residual pack.
pub fn honesty_gadget_residual_wave113() -> bool {
    GGM_LEFT_DRAG == 16384
        && GADGET_GAME_MESSAGE_NAMES.len() == GADGET_GAME_MESSAGE_COUNT
        && residual_name_index(GADGET_GAME_MESSAGE_NAMES, "GGM_LEFT_DRAG") == Some(0)
        && residual_name_index(GADGET_GAME_MESSAGE_NAMES, "GBM_SELECTED") == Some(8)
        && residual_name_index(GADGET_GAME_MESSAGE_NAMES, "GPM_SET_PROGRESS")
            == Some(GADGET_GAME_MESSAGE_COUNT - 1)
        && gadget_message_id_residual("GGM_LEFT_DRAG") == Some(16384)
        && gadget_message_id_residual("GBM_SELECTED") == Some(16384 + 8)
        && gadget_message_id_residual("GPM_SET_PROGRESS")
            == Some(16384 + (GADGET_GAME_MESSAGE_COUNT as u32 - 1))
        && gadget_message_id_residual("NoSuch").is_none()
        // Gadget band sits below GWM_USER.
        && GGM_LEFT_DRAG < GWM_USER
        && (GGM_LEFT_DRAG + GADGET_GAME_MESSAGE_COUNT as u32) <= GWM_USER
        && BORDER_PIECE_NAMES.len() == NUM_BORDER_PIECES
        && residual_name_index(BORDER_PIECE_NAMES, "BORDER_CORNER_UL") == Some(0)
        && residual_name_index(BORDER_PIECE_NAMES, "BORDER_HORIZONTAL_BOTTOM_SHORT") == Some(11)
}

/// Honesty: Video buffer type residual pack.
pub fn honesty_video_buffer_residual_wave113() -> bool {
    VIDEO_BUFFER_TYPE_NAMES.len() == VIDEO_BUFFER_NUM_TYPES
        && residual_name_index(VIDEO_BUFFER_TYPE_NAMES, "TYPE_UNKNOWN") == Some(0)
        && residual_name_index(VIDEO_BUFFER_TYPE_NAMES, "TYPE_X1R5G5B5") == Some(4)
        && residual_name_index(VIDEO_BUFFER_TYPE_NAMES, "TYPE_R5G6B5") == Some(3)
}

/// Honesty: AudioEventRTS / AudioEventInfo residual pack.
pub fn honesty_audio_event_residual_wave113() -> bool {
    AUDIO_OWNER_TYPE_NAMES.len() == AUDIO_OWNER_TYPE_COUNT
        && residual_name_index(AUDIO_OWNER_TYPE_NAMES, "OT_Positional") == Some(0)
        && residual_name_index(AUDIO_OWNER_TYPE_NAMES, "OT_INVALID") == Some(4)
        && AUDIO_PORTION_TO_PLAY_NAMES.len() == AUDIO_PORTION_TO_PLAY_COUNT
        && residual_name_index(AUDIO_PORTION_TO_PLAY_NAMES, "PP_Attack") == Some(0)
        && residual_name_index(AUDIO_PORTION_TO_PLAY_NAMES, "PP_Done") == Some(3)
        && AUDIO_PRIORITY_NAMES.len() == AUDIO_PRIORITY_COUNT
        && residual_name_index(AUDIO_PRIORITY_NAMES, "AP_LOWEST") == Some(0)
        && residual_name_index(AUDIO_PRIORITY_NAMES, "AP_CRITICAL") == Some(4)
        && SOUND_TYPE_TABLE.len() == 9
        && SOUND_TYPE_TABLE[0] == ("ST_UI", 0x0001)
        && SOUND_TYPE_TABLE[8] == ("ST_EVERYONE", 0x0100)
        && SOUND_TYPE_TABLE
            .iter()
            .enumerate()
            .all(|(i, (_, bit))| *bit == (1u32 << i))
        && AUDIO_CONTROL_TABLE.len() == 5
        && AUDIO_CONTROL_TABLE[0] == ("AC_LOOP", 0x0001)
        && AUDIO_CONTROL_TABLE[4] == ("AC_INTERRUPT", 0x0010)
        && AUDIO_CONTROL_TABLE
            .iter()
            .enumerate()
            .all(|(i, (_, bit))| *bit == (1u32 << i))
}

/// Wave 113 composite residual honesty pack.
pub fn honesty_gadget_video_audio_residual_pack_wave113() -> bool {
    honesty_game_window_manager_residual_wave113()
        && honesty_window_style_residual_wave113()
        && honesty_gadget_residual_wave113()
        && honesty_video_buffer_residual_wave113()
        && honesty_audio_event_residual_wave113()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn game_window_manager_residual() {
        assert!(honesty_game_window_manager_residual_wave113());
    }

    #[test]
    fn window_style_residual() {
        assert!(honesty_window_style_residual_wave113());
    }

    #[test]
    fn gadget_residual() {
        assert!(honesty_gadget_residual_wave113());
    }

    #[test]
    fn video_buffer_residual() {
        assert!(honesty_video_buffer_residual_wave113());
    }

    #[test]
    fn audio_event_residual() {
        assert!(honesty_audio_event_residual_wave113());
    }

    #[test]
    fn wave113_composite_pack() {
        assert!(honesty_gadget_video_audio_residual_pack_wave113());
    }
}
