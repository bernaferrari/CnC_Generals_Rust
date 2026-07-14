//! Wave 90 residual peels: GameSpeed / frame-rate deepen / debug tables /
//! language deepen / credits residual.
//!
//! Orthogonal to Waves 86 (GameData FPS/camera), 74/65 (GameText multi-locale CSF),
//! 89 (options residual including FPSLimit Yes).
//! Host-testable packs for engine timing + shell UI residual honesty.
//!
//! Sources (retail ZH INI + C++):
//! - GameCommon.h LOGICFRAMES_PER_SECOND / ConvertDurationFromMsecsToFrames
//! - GameEngine.h DEFAULT_MAX_FPS / GameEngine set/getFramesPerSecondLimit
//! - W3DDisplay.cpp updateAverageFPS FPS_HISTORY_SIZE / limit sleep residual
//! - DebugDisplay.h Color enum + W3DDisplay.h DisplayString debug slots
//! - LanguageFilter.h XOR key / langdata.dat / unHaxor residual
//! - English Language.ini GlobalLanguage residual fonts / adjustFontSize
//! - Credits.h/.cpp + Credits.ini scroll/style/color residual
//!
//! Fail-closed:
//! - Not full GameEngine main-loop sleep / live FPS lock residual
//! - Not full W3DDisplay drawDebugStats GPU residual
//! - Not full LanguageFilter langdata.dat live load / network chat filter residual
//! - Not full CreditsMenu.wnd GPU / scroll DisplayString residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// 1. GameSpeed residual (logic frame clock)
// ---------------------------------------------------------------------------

/// C++ `LOGICFRAMES_PER_SECOND` residual (GameCommon.h).
pub const LOGICFRAMES_PER_SECOND_RESIDUAL: i32 = 30;
/// C++ `MSEC_PER_SECOND` residual.
pub const MSEC_PER_SECOND_RESIDUAL: i32 = 1000;
/// C++ `MSEC_PER_LOGICFRAME_REAL` residual ≈ 33.333… ms/frame.
pub const MSEC_PER_LOGICFRAME_REAL_RESIDUAL: f32 =
    (MSEC_PER_SECOND_RESIDUAL as f32) / (LOGICFRAMES_PER_SECOND_RESIDUAL as f32);
/// C++ `LOGICFRAMES_PER_MSEC_REAL` residual = 0.03 frames/ms.
pub const LOGICFRAMES_PER_MSEC_REAL_RESIDUAL: f32 =
    (LOGICFRAMES_PER_SECOND_RESIDUAL as f32) / (MSEC_PER_SECOND_RESIDUAL as f32);
/// C++ `SECONDS_PER_LOGICFRAME_REAL` residual = 1/30.
pub const SECONDS_PER_LOGICFRAME_REAL_RESIDUAL: f32 =
    1.0 / (LOGICFRAMES_PER_SECOND_RESIDUAL as f32);

/// C++ `GameEngine.h` `DEFAULT_MAX_FPS` residual (engine constructor default).
pub const DEFAULT_MAX_FPS_RESIDUAL: i32 = 45;
/// Retail GameData.ini FramesPerSecondLimit residual (logic target after postProcessLoad).
pub const GAME_DATA_FRAMES_PER_SECOND_LIMIT_RESIDUAL: i32 = 30;
/// GlobalData constructor default for m_framesPerSecondLimit residual (before INI).
pub const GLOBAL_DATA_FRAMES_PER_SECOND_LIMIT_CTOR_RESIDUAL: i32 = 0;
/// GlobalData constructor default for m_useFpsLimit residual (before INI → FALSE).
pub const GLOBAL_DATA_USE_FPS_LIMIT_CTOR_RESIDUAL: bool = false;
/// Retail GameData.ini UseFPSLimit residual after INI load.
pub const GAME_DATA_USE_FPS_LIMIT_RESIDUAL: bool = true;

/// C++ `ConvertDurationFromMsecsToFrames` residual (returns Real; callers ceil).
pub fn convert_duration_from_msecs_to_frames_residual(msec: f32) -> f32 {
    msec * LOGICFRAMES_PER_MSEC_REAL_RESIDUAL
}

/// Host residual: msec → logic frames with ceil (C++ common call pattern).
pub fn msec_to_logic_frames_ceil_residual(msec: f32) -> i32 {
    convert_duration_from_msecs_to_frames_residual(msec).ceil() as i32
}

/// Host residual: whole seconds → logic frames.
pub fn seconds_to_logic_frames_residual(seconds: i32) -> i32 {
    seconds * LOGICFRAMES_PER_SECOND_RESIDUAL
}

/// Wave 90 honesty: GameSpeed residual pack.
pub fn honesty_gamespeed_residual_pack_wave90() -> bool {
    LOGICFRAMES_PER_SECOND_RESIDUAL == 30
        && MSEC_PER_SECOND_RESIDUAL == 1000
        && (MSEC_PER_LOGICFRAME_REAL_RESIDUAL - (1000.0 / 30.0)).abs() < 1e-5
        && (LOGICFRAMES_PER_MSEC_REAL_RESIDUAL - 0.03).abs() < 1e-5
        && (SECONDS_PER_LOGICFRAME_REAL_RESIDUAL - (1.0 / 30.0)).abs() < 1e-5
        && DEFAULT_MAX_FPS_RESIDUAL == 45
        && GAME_DATA_FRAMES_PER_SECOND_LIMIT_RESIDUAL == 30
        && GLOBAL_DATA_FRAMES_PER_SECOND_LIMIT_CTOR_RESIDUAL == 0
        && !GLOBAL_DATA_USE_FPS_LIMIT_CTOR_RESIDUAL
        && GAME_DATA_USE_FPS_LIMIT_RESIDUAL
        && DEFAULT_MAX_FPS_RESIDUAL > GAME_DATA_FRAMES_PER_SECOND_LIMIT_RESIDUAL
        && msec_to_logic_frames_ceil_residual(1000.0) == 30
        && msec_to_logic_frames_ceil_residual(500.0) == 15
        && msec_to_logic_frames_ceil_residual(33.0) == 1 // 0.99 → ceil 1
        && msec_to_logic_frames_ceil_residual(34.0) == 2 // 1.02 → ceil 2
        && seconds_to_logic_frames_residual(3) == 90
        && seconds_to_logic_frames_residual(0) == 0
        && convert_duration_from_msecs_to_frames_residual(0.0) == 0.0
}

// ---------------------------------------------------------------------------
// 2. Frame rate residual deepen (beyond Wave 86 FPSLimit constants)
// ---------------------------------------------------------------------------

/// C++ W3DDisplay::updateAverageFPS `FPS_HISTORY_SIZE` residual.
pub const FPS_HISTORY_SIZE_RESIDUAL: i32 = 30;
/// C++ GameEngine execute loop residual: `DWORD limit = (1000.0f/m_maxFPS)-1`.
pub fn fps_limit_sleep_ms_residual(max_fps: i32) -> i32 {
    if max_fps <= 0 {
        return 0;
    }
    ((1000.0f32 / (max_fps as f32)) - 1.0).floor() as i32
}

/// Host residual: average of last N FPS samples (mirrors updateAverageFPS mean).
pub fn average_fps_history_residual(samples: &[f64]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let n = samples.len().min(FPS_HISTORY_SIZE_RESIDUAL as usize);
    let slice = &samples[samples.len() - n..];
    slice.iter().sum::<f64>() / (n as f64)
}

/// Host residual FPS lock state after GameData post-load residual.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FpsLockStateResidual {
    pub use_fps_limit: bool,
    pub frames_per_second_limit: i32,
    pub engine_max_fps: i32,
}

/// Residual after GameEngine postProcessLoad applies GameData FramesPerSecondLimit.
pub fn fps_lock_state_after_gamedata_load_residual() -> FpsLockStateResidual {
    FpsLockStateResidual {
        use_fps_limit: GAME_DATA_USE_FPS_LIMIT_RESIDUAL,
        frames_per_second_limit: GAME_DATA_FRAMES_PER_SECOND_LIMIT_RESIDUAL,
        engine_max_fps: GAME_DATA_FRAMES_PER_SECOND_LIMIT_RESIDUAL,
    }
}

/// Residual constructor defaults before INI (GlobalData + GameEngine).
pub fn fps_lock_state_ctor_residual() -> FpsLockStateResidual {
    FpsLockStateResidual {
        use_fps_limit: GLOBAL_DATA_USE_FPS_LIMIT_CTOR_RESIDUAL,
        frames_per_second_limit: GLOBAL_DATA_FRAMES_PER_SECOND_LIMIT_CTOR_RESIDUAL,
        engine_max_fps: DEFAULT_MAX_FPS_RESIDUAL,
    }
}

/// Wave 90 honesty: frame rate residual deepen pack.
pub fn honesty_frame_rate_residual_deepen_pack_wave90() -> bool {
    FPS_HISTORY_SIZE_RESIDUAL == 30
        && FPS_HISTORY_SIZE_RESIDUAL == LOGICFRAMES_PER_SECOND_RESIDUAL
        && fps_limit_sleep_ms_residual(30) == 32 // (1000/30)-1 = 32.333… → floor 32
        && fps_limit_sleep_ms_residual(45) == 21 // (1000/45)-1 = 21.222… → floor 21
        && fps_limit_sleep_ms_residual(60) == 15 // (1000/60)-1 = 15.666… → floor 15
        && fps_limit_sleep_ms_residual(0) == 0
        && {
            let hist = [30.0, 30.0, 30.0];
            (average_fps_history_residual(&hist) - 30.0).abs() < 1e-9
        }
        && {
            let hist = [20.0, 40.0];
            (average_fps_history_residual(&hist) - 30.0).abs() < 1e-9
        }
        && average_fps_history_residual(&[]) == 0.0
        && {
            let after = fps_lock_state_after_gamedata_load_residual();
            after.use_fps_limit
                && after.frames_per_second_limit == 30
                && after.engine_max_fps == 30
        }
        && {
            let ctor = fps_lock_state_ctor_residual();
            !ctor.use_fps_limit
                && ctor.frames_per_second_limit == 0
                && ctor.engine_max_fps == 45
        }
}

// ---------------------------------------------------------------------------
// 3. Debug residual tables (host-only)
// ---------------------------------------------------------------------------

/// C++ `DebugDisplayInterface::Color` residual order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DebugDisplayColorResidual {
    White = 0,
    Black = 1,
    Yellow = 2,
    Red = 3,
    Green = 4,
    Blue = 5,
}

/// C++ `DebugDisplayInterface::Color::NUM_COLORS` residual.
pub const DEBUG_DISPLAY_NUM_COLORS_RESIDUAL: u32 = 6;

/// Ordered DebugDisplay color name residual table.
pub const DEBUG_DISPLAY_COLOR_NAMES: &[&str] =
    &["WHITE", "BLACK", "YELLOW", "RED", "GREEN", "BLUE"];

/// C++ W3DDisplay debug `DisplayString` slot residual order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum DebugDisplayStringSlotResidual {
    Fps = 0,
    Frame = 1,
    Polygons = 2,
    Vertices = 3,
    VideoRam = 4,
    DebugInfo = 5,
    KeyMouseStates = 6,
    MousePosition = 7,
    Particles = 8,
    Objects = 9,
    NetIncoming = 10,
    NetOutgoing = 11,
    NetStats = 12,
    NetFpsAverages = 13,
    SelectedInfo = 14,
    TerrainStats = 15,
}

/// C++ W3DDisplay `DisplayStringCount` residual.
pub const DEBUG_DISPLAY_STRING_COUNT_RESIDUAL: u32 = 16;

/// Ordered W3DDisplay debug DisplayString slot name residual.
pub const DEBUG_DISPLAY_STRING_SLOT_NAMES: &[&str] = &[
    "FPS",
    "Frame",
    "Polygons",
    "Vertices",
    "VideoRam",
    "DebugInfo",
    "KEY_MOUSE_STATES",
    "MousePosition",
    "Particles",
    "Objects",
    "NetIncoming",
    "NetOutgoing",
    "NetStats",
    "NetFPSAverages",
    "SelectedInfo",
    "TerrainStats",
];

/// Wave 90 honesty: debug residual tables pack (host-only).
pub fn honesty_debug_residual_tables_pack_wave90() -> bool {
    DEBUG_DISPLAY_NUM_COLORS_RESIDUAL == 6
        && DEBUG_DISPLAY_COLOR_NAMES.len() as u32 == DEBUG_DISPLAY_NUM_COLORS_RESIDUAL
        && DEBUG_DISPLAY_COLOR_NAMES[0] == "WHITE"
        && DEBUG_DISPLAY_COLOR_NAMES[5] == "BLUE"
        && DebugDisplayColorResidual::White as u8 == 0
        && DebugDisplayColorResidual::Blue as u8 == 5
        && DEBUG_DISPLAY_STRING_COUNT_RESIDUAL == 16
        && DEBUG_DISPLAY_STRING_SLOT_NAMES.len() as u32 == DEBUG_DISPLAY_STRING_COUNT_RESIDUAL
        && DEBUG_DISPLAY_STRING_SLOT_NAMES[0] == "FPS"
        && DEBUG_DISPLAY_STRING_SLOT_NAMES[8] == "Particles"
        && DEBUG_DISPLAY_STRING_SLOT_NAMES[9] == "Objects"
        && DEBUG_DISPLAY_STRING_SLOT_NAMES[13] == "NetFPSAverages"
        && DEBUG_DISPLAY_STRING_SLOT_NAMES[15] == "TerrainStats"
        && DebugDisplayStringSlotResidual::Fps as u8 == 0
        && DebugDisplayStringSlotResidual::Particles as u8 == 8
        && DebugDisplayStringSlotResidual::TerrainStats as u8 == 15
}

// ---------------------------------------------------------------------------
// 4. Language residual deepen (beyond CSF multi-locale path tables)
// ---------------------------------------------------------------------------

/// C++ `LanguageFilter.h` `LANGUAGE_XOR_KEY` residual.
pub const LANGUAGE_XOR_KEY_RESIDUAL: u16 = 0x5555;
/// C++ `LanguageFilter.h` `BadWordFileName` residual.
pub const LANGUAGE_BAD_WORD_FILE_RESIDUAL: &str = "langdata.dat";
/// LanguageFilter `unHaxor` ignored chars residual (`-_*'"`).
pub const LANGUAGE_UNHAXOR_IGNORED_CHARS_RESIDUAL: &str = "-_*'\"";
/// LanguageFilter filterLine token separators residual.
pub const LANGUAGE_FILTER_TOKEN_SEPARATORS_RESIDUAL: &str = " ;,.!?:=\\/><`~()&^%#\n\t";

/// Language.ini residual defaults (English Language.ini / FontDesc ctor).
pub const LANGUAGE_UNICODE_FONT_NAME_RESIDUAL: &str = "Arial Unicode MS";
/// MilitaryCaptionSpeed residual (English Language.ini).
pub const LANGUAGE_MILITARY_CAPTION_SPEED_RESIDUAL: i32 = 1;
/// MilitaryCaptionDelayMS residual (English Language.ini + GlobalLanguage ctor 750).
pub const LANGUAGE_MILITARY_CAPTION_DELAY_MS_RESIDUAL: i32 = 750;
/// ResolutionFontAdjustment residual (English Language.ini / GlobalLanguage ctor 0.7).
pub const LANGUAGE_RESOLUTION_FONT_ADJUSTMENT_RESIDUAL: f32 = 0.7;
/// CreditsTitleFont size residual (English Language.ini Arial 22).
pub const LANGUAGE_CREDITS_TITLE_FONT_SIZE_RESIDUAL: i32 = 22;
/// CreditsMinorTitleFont size residual (Arial 16 Yes).
pub const LANGUAGE_CREDITS_MINOR_TITLE_FONT_SIZE_RESIDUAL: i32 = 16;
/// CreditsNormalFont size residual (Arial 14 No).
pub const LANGUAGE_CREDITS_NORMAL_FONT_SIZE_RESIDUAL: i32 = 14;
/// NativeDebugDisplay font residual (FixedSys 8).
pub const LANGUAGE_NATIVE_DEBUG_DISPLAY_FONT_SIZE_RESIDUAL: i32 = 8;
/// FontDesc constructor default size residual.
pub const LANGUAGE_FONT_DESC_DEFAULT_SIZE_RESIDUAL: i32 = 12;

/// C++ `LanguageFilter::unHaxor` leet residual mapping for a single char.
pub fn language_unhaxor_char_residual(c: char) -> Option<char> {
    match c {
        '1' => Some('l'),
        '3' => Some('e'),
        '4' | '@' => Some('a'),
        '5' | '$' => Some('s'),
        '6' => Some('b'),
        '7' | '+' => Some('t'),
        '0' => Some('o'),
        _ => None,
    }
}

/// Host residual: LanguageFilter unHaxor word transform (leet + ph→f + strip ignored).
pub fn language_unhaxor_word_residual(word: &str) -> String {
    let chars: Vec<char> = word.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        if (c == 'p' || c == 'P')
            && i + 1 < chars.len()
            && (chars[i + 1] == 'h' || chars[i + 1] == 'H')
        {
            out.push('f');
            i += 2;
            continue;
        }
        if let Some(mapped) = language_unhaxor_char_residual(c) {
            out.push(mapped);
            i += 1;
            continue;
        }
        if LANGUAGE_UNHAXOR_IGNORED_CHARS_RESIDUAL.contains(c) {
            i += 1;
            continue;
        }
        out.push(c);
        i += 1;
    }
    out
}

/// C++ `GlobalLanguage::adjustFontSize` residual (base res 800, clamp 1.0..2.0).
pub fn language_adjust_font_size_residual(
    the_font_size: i32,
    x_resolution: f32,
    resolution_font_size_adjustment: f32,
) -> i32 {
    let mut adjust_factor = x_resolution / 800.0;
    adjust_factor = 1.0 + (adjust_factor - 1.0) * resolution_font_size_adjustment;
    if adjust_factor < 1.0 {
        adjust_factor = 1.0;
    }
    if adjust_factor > 2.0 {
        adjust_factor = 2.0;
    }
    // REAL_TO_INT_FLOOR
    ((the_font_size as f32) * adjust_factor).floor() as i32
}

/// XOR residual for one UTF-16 code unit from langdata.dat.
pub fn language_xor_code_unit_residual(unit: u16) -> u16 {
    unit ^ LANGUAGE_XOR_KEY_RESIDUAL
}

/// Wave 90 honesty: language residual deepen pack.
pub fn honesty_language_residual_deepen_pack_wave90() -> bool {
    LANGUAGE_XOR_KEY_RESIDUAL == 0x5555
        && LANGUAGE_BAD_WORD_FILE_RESIDUAL == "langdata.dat"
        && LANGUAGE_UNHAXOR_IGNORED_CHARS_RESIDUAL.contains('-')
        && LANGUAGE_UNHAXOR_IGNORED_CHARS_RESIDUAL.contains('_')
        && LANGUAGE_UNHAXOR_IGNORED_CHARS_RESIDUAL.contains('*')
        && LANGUAGE_FILTER_TOKEN_SEPARATORS_RESIDUAL.contains(' ')
        && LANGUAGE_FILTER_TOKEN_SEPARATORS_RESIDUAL.contains(';')
        && LANGUAGE_UNICODE_FONT_NAME_RESIDUAL == "Arial Unicode MS"
        && LANGUAGE_MILITARY_CAPTION_SPEED_RESIDUAL == 1
        && LANGUAGE_MILITARY_CAPTION_DELAY_MS_RESIDUAL == 750
        && (LANGUAGE_RESOLUTION_FONT_ADJUSTMENT_RESIDUAL - 0.7).abs() < 1e-5
        && LANGUAGE_CREDITS_TITLE_FONT_SIZE_RESIDUAL == 22
        && LANGUAGE_CREDITS_MINOR_TITLE_FONT_SIZE_RESIDUAL == 16
        && LANGUAGE_CREDITS_NORMAL_FONT_SIZE_RESIDUAL == 14
        && LANGUAGE_NATIVE_DEBUG_DISPLAY_FONT_SIZE_RESIDUAL == 8
        && LANGUAGE_FONT_DESC_DEFAULT_SIZE_RESIDUAL == 12
        && language_unhaxor_word_residual("ph00") == "foo"
        && language_unhaxor_word_residual("t3st") == "test"
        && language_unhaxor_word_residual("a_b") == "ab"
        && language_unhaxor_word_residual("4pple") == "apple"
        && language_xor_code_unit_residual(0x5555) == 0
        && language_xor_code_unit_residual(0x0000) == 0x5555
        && language_xor_code_unit_residual(language_xor_code_unit_residual(0xABCD)) == 0xABCD
        // 800 res → factor 1.0 → same size
        && language_adjust_font_size_residual(12, 800.0, 0.7) == 12
        // 1600 res → factor 1 + 1*0.7 = 1.7 → floor(12*1.7)=20
        && language_adjust_font_size_residual(12, 1600.0, 0.7) == 20
        // 640 res → raw 0.8 → clamp to 1.0 → 12
        && language_adjust_font_size_residual(12, 640.0, 0.7) == 12
        && LANGUAGE_CREDITS_TITLE_FONT_SIZE_RESIDUAL
            > LANGUAGE_CREDITS_MINOR_TITLE_FONT_SIZE_RESIDUAL
        && LANGUAGE_CREDITS_MINOR_TITLE_FONT_SIZE_RESIDUAL
            > LANGUAGE_CREDITS_NORMAL_FONT_SIZE_RESIDUAL
}

// ---------------------------------------------------------------------------
// 5. Credits residual
// ---------------------------------------------------------------------------

/// C++ `CREDIT_STYLE_*` residual order (Credits.h).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum CreditStyleResidual {
    Title = 0,
    Position = 1, // MINORTITLE
    Normal = 2,
    Column = 3,
    Blank = 4,
}

/// C++ `MAX_CREDIT_STYLES` residual.
pub const MAX_CREDIT_STYLES_RESIDUAL: u32 = 5;
/// C++ `CREDIT_SPACE_OFFSET` residual.
pub const CREDIT_SPACE_OFFSET_RESIDUAL: i32 = 2;

/// Ordered CreditStyleNames residual (INI lookup; excludes BLANK/MAX).
pub const CREDIT_STYLE_NAMES: &[&str] = &["TITLE", "MINORTITLE", "NORMAL", "COLUMN"];

/// CreditsManager constructor ScrollRate residual (pixels).
pub const CREDITS_SCROLL_RATE_CTOR_RESIDUAL: i32 = 1;
/// CreditsManager constructor ScrollRateEveryFrames residual.
pub const CREDITS_SCROLL_RATE_PER_FRAMES_CTOR_RESIDUAL: i32 = 1;
/// CreditsManager constructor ScrollDown residual (TRUE).
pub const CREDITS_SCROLL_DOWN_CTOR_RESIDUAL: bool = true;
/// CreditsManager constructor normal font height residual.
pub const CREDITS_NORMAL_FONT_HEIGHT_CTOR_RESIDUAL: i32 = 10;
/// Credits.ini retail ScrollRate residual.
pub const CREDITS_SCROLL_RATE_INI_RESIDUAL: i32 = 2;
/// Credits.ini retail ScrollRateEveryFrames residual.
pub const CREDITS_SCROLL_RATE_PER_FRAMES_INI_RESIDUAL: i32 = 1;
/// Credits.ini retail ScrollDown residual (NO → false; scroll bottom-up).
pub const CREDITS_SCROLL_DOWN_INI_RESIDUAL: bool = false;

/// Credits.ini TitleColor residual RGBA.
pub const CREDITS_TITLE_COLOR_RGBA_RESIDUAL: (u8, u8, u8, u8) = (161, 179, 255, 255);
/// Credits.ini MinorTitleColor residual RGBA.
pub const CREDITS_MINOR_TITLE_COLOR_RGBA_RESIDUAL: (u8, u8, u8, u8) = (161, 179, 255, 255);
/// Credits.ini NormalColor residual RGBA.
pub const CREDITS_NORMAL_COLOR_RGBA_RESIDUAL: (u8, u8, u8, u8) = (209, 218, 255, 255);
/// CreditsManager constructor default color residual (white opaque).
pub const CREDITS_DEFAULT_COLOR_RGBA_RESIDUAL: (u8, u8, u8, u8) = (255, 255, 255, 255);

/// Credits.ini load path residual.
pub const CREDITS_INI_PATH_RESIDUAL: &str = "Data\\INI\\Credits.ini";
/// CreditsMenu layout residual label.
pub const CREDITS_MENU_LAYOUT_RESIDUAL: &str = "CreditsMenu.wnd";
/// MainMenu ButtonCredits control residual.
pub const CREDITS_BUTTON_CONTROL_RESIDUAL: &str = "MainMenu.wnd:ButtonCredits";
/// Sample Credits.ini string-label residual (looked up via GameText).
pub const CREDITS_SAMPLE_LABEL_RESIDUAL: &str = "CREDITS:ExecutiveProducer";
/// Sample Credits.ini quoted-name residual (not translated).
pub const CREDITS_SAMPLE_NAME_RESIDUAL: &str = "Mark Skaggs";
/// Blank text token residual.
pub const CREDITS_BLANK_TEXT_TOKEN_RESIDUAL: &str = "<BLANK>";

/// C++ GameMakeColor residual: (A<<24)|(R<<16)|(G<<8)|B.
pub fn game_make_color_residual(r: u8, g: u8, b: u8, a: u8) -> u32 {
    ((a as u32) << 24) | ((r as u32) << 16) | ((g as u32) << 8) | (b as u32)
}

/// Credits.ini TitleColor packed residual.
pub fn credits_title_color_packed_residual() -> u32 {
    let (r, g, b, a) = CREDITS_TITLE_COLOR_RGBA_RESIDUAL;
    game_make_color_residual(r, g, b, a)
}

/// Host residual: style name → CreditStyle residual (case-insensitive).
pub fn credit_style_from_name_residual(name: &str) -> Option<CreditStyleResidual> {
    match name.to_ascii_uppercase().as_str() {
        "TITLE" => Some(CreditStyleResidual::Title),
        "MINORTITLE" => Some(CreditStyleResidual::Position),
        "NORMAL" => Some(CreditStyleResidual::Normal),
        "COLUMN" => Some(CreditStyleResidual::Column),
        _ => None,
    }
}

/// Host residual: detect Credits.ini string-label vs quoted name.
/// C++ uses presence of ':' to select GameText lookup.
pub fn credits_text_is_string_label_residual(text: &str) -> bool {
    text.contains(':')
}

/// Wave 90 honesty: credits residual pack.
pub fn honesty_credits_residual_pack_wave90() -> bool {
    MAX_CREDIT_STYLES_RESIDUAL == 5
        && CREDIT_SPACE_OFFSET_RESIDUAL == 2
        && CREDIT_STYLE_NAMES.len() == 4
        && CREDIT_STYLE_NAMES[0] == "TITLE"
        && CREDIT_STYLE_NAMES[1] == "MINORTITLE"
        && CREDIT_STYLE_NAMES[2] == "NORMAL"
        && CREDIT_STYLE_NAMES[3] == "COLUMN"
        && CreditStyleResidual::Title as u8 == 0
        && CreditStyleResidual::Position as u8 == 1
        && CreditStyleResidual::Blank as u8 == 4
        && credit_style_from_name_residual("TITLE") == Some(CreditStyleResidual::Title)
        && credit_style_from_name_residual("minortitle") == Some(CreditStyleResidual::Position)
        && credit_style_from_name_residual("BLANK").is_none()
        && CREDITS_SCROLL_RATE_CTOR_RESIDUAL == 1
        && CREDITS_SCROLL_RATE_PER_FRAMES_CTOR_RESIDUAL == 1
        && CREDITS_SCROLL_DOWN_CTOR_RESIDUAL
        && CREDITS_NORMAL_FONT_HEIGHT_CTOR_RESIDUAL == 10
        && CREDITS_SCROLL_RATE_INI_RESIDUAL == 2
        && CREDITS_SCROLL_RATE_PER_FRAMES_INI_RESIDUAL == 1
        && !CREDITS_SCROLL_DOWN_INI_RESIDUAL
        && CREDITS_SCROLL_RATE_INI_RESIDUAL > CREDITS_SCROLL_RATE_CTOR_RESIDUAL
        && CREDITS_TITLE_COLOR_RGBA_RESIDUAL == (161, 179, 255, 255)
        && CREDITS_MINOR_TITLE_COLOR_RGBA_RESIDUAL == CREDITS_TITLE_COLOR_RGBA_RESIDUAL
        && CREDITS_NORMAL_COLOR_RGBA_RESIDUAL == (209, 218, 255, 255)
        && CREDITS_DEFAULT_COLOR_RGBA_RESIDUAL == (255, 255, 255, 255)
        && game_make_color_residual(255, 255, 255, 255) == 0xFFFFFFFF
        && credits_title_color_packed_residual()
            == game_make_color_residual(161, 179, 255, 255)
        && CREDITS_INI_PATH_RESIDUAL == "Data\\INI\\Credits.ini"
        && CREDITS_MENU_LAYOUT_RESIDUAL == "CreditsMenu.wnd"
        && CREDITS_BUTTON_CONTROL_RESIDUAL.contains("ButtonCredits")
        && CREDITS_SAMPLE_LABEL_RESIDUAL == "CREDITS:ExecutiveProducer"
        && credits_text_is_string_label_residual(CREDITS_SAMPLE_LABEL_RESIDUAL)
        && !credits_text_is_string_label_residual(CREDITS_SAMPLE_NAME_RESIDUAL)
        && CREDITS_BLANK_TEXT_TOKEN_RESIDUAL == "<BLANK>"
}

// ---------------------------------------------------------------------------
// Combined Wave 90 pack
// ---------------------------------------------------------------------------

/// Combined Wave 90 residual honesty (all peels).
pub fn honesty_timing_shell_residual_pack_wave90() -> bool {
    honesty_gamespeed_residual_pack_wave90()
        && honesty_frame_rate_residual_deepen_pack_wave90()
        && honesty_debug_residual_tables_pack_wave90()
        && honesty_language_residual_deepen_pack_wave90()
        && honesty_credits_residual_pack_wave90()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gamespeed_residual_pack_wave90_honesty() {
        assert!(honesty_gamespeed_residual_pack_wave90());
    }

    #[test]
    fn frame_rate_residual_deepen_pack_wave90_honesty() {
        assert!(honesty_frame_rate_residual_deepen_pack_wave90());
    }

    #[test]
    fn debug_residual_tables_pack_wave90_honesty() {
        assert!(honesty_debug_residual_tables_pack_wave90());
    }

    #[test]
    fn language_residual_deepen_pack_wave90_honesty() {
        assert!(honesty_language_residual_deepen_pack_wave90());
    }

    #[test]
    fn credits_residual_pack_wave90_honesty() {
        assert!(honesty_credits_residual_pack_wave90());
    }

    #[test]
    fn timing_shell_residual_pack_wave90_combined_honesty() {
        assert!(honesty_timing_shell_residual_pack_wave90());
    }
}
