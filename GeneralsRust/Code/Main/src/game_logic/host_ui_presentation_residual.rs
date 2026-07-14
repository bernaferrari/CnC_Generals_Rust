//! Wave 91 residual peels: Tooltip / HelpBox / Message / EVA / Video names /
//! mission briefing residual.
//!
//! Orthogonal UI presentation residual packs for host honesty. Freezes retail
//! ZH INI + C++ residual constants used by mouse tooltips, ControlBar help box
//! (ControlBarPopupDescription), in-game messages, EVA speech enum, Video.ini
//! name tables (not codec), and Campaign.ini mission briefing residual.
//!
//! Sources (retail ZH INI + C++):
//! - Mouse.ini tooltip residual + Mouse.cpp word-wrap residual
//! - ControlBarPopupDescription.cpp / ControlBarPopupDescription.wnd HelpBox residual
//! - InGameUI.ini message / floating text / popup residual + InGameUI.h MAX_UI_MESSAGES
//! - Eva.h / Eva.cpp TheEvaMessageNames + EvaCheckInfo defaults
//! - Video.ini internal name + filename residual (names only; not Bink codec)
//! - CampaignManager.h MAX_OBJECTIVE_LINES / MAX_DISPLAYED_UNITS + Campaign.ini
//!   TRAINING / USA / GLA / China mission briefing residual
//!
//! Fail-closed:
//! - Not full Mouse tooltip GPU draw / ControlBar help-box animate residual
//! - Not full UIMessage DisplayString / FloatingText GPU residual
//! - Not full Eva speech Miles playback residual
//! - Not full Bink video codec / stream decode residual
//! - Not full LoadScreen objective GPU / briefing voice playback residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared logic-frame residual
// ---------------------------------------------------------------------------

/// C++ `LOGICFRAMES_PER_SECOND` residual (30 FPS logic).
pub const LOGIC_FRAMES_PER_SECOND_RESIDUAL: i32 = 30;

/// Convert duration ms → logic frames residual (`ceil(ms * 30 / 1000)`).
pub fn duration_ms_to_frames_ceil_residual(ms: u32) -> u32 {
    let frames = (ms as f32) * (LOGIC_FRAMES_PER_SECOND_RESIDUAL as f32) / 1000.0;
    frames.ceil() as u32
}

/// Convert velocity dist/sec → dist/frame residual (`dist * (1/30)`).
pub fn velocity_secs_to_frames_residual(dist_per_sec: f32) -> f32 {
    dist_per_sec / (LOGIC_FRAMES_PER_SECOND_RESIDUAL as f32)
}

// ---------------------------------------------------------------------------
// 1. Tooltip residual peels (Mouse.ini + Mouse.cpp)
// ---------------------------------------------------------------------------

/// Retail Mouse.ini `TooltipFontName` residual.
pub const TOOLTIP_FONT_NAME_RESIDUAL: &str = "Arial";
/// Retail Mouse.ini `TooltipFontSize` residual.
pub const TOOLTIP_FONT_SIZE_RESIDUAL: i32 = 8;
/// Retail Mouse.ini `TooltipFontIsBold` residual.
pub const TOOLTIP_FONT_IS_BOLD_RESIDUAL: bool = false;
/// Retail Mouse.ini `TooltipAnimateBackground` residual.
pub const TOOLTIP_ANIMATE_BACKGROUND_RESIDUAL: bool = false;
/// Retail Mouse.ini `TooltipFillTime` residual (ms).
pub const TOOLTIP_FILL_TIME_MS_RESIDUAL: i32 = 250;
/// Retail Mouse.ini `TooltipDelayTime` residual (ms).
pub const TOOLTIP_DELAY_TIME_MS_RESIDUAL: i32 = 800;
/// Retail Mouse.ini `TooltipWidth` residual (percent screen width, raw INI).
pub const TOOLTIP_WIDTH_PERCENT_RESIDUAL: f32 = 20.0;
/// parsePercentToReal residual for TooltipWidth 20 → 0.20.
pub const TOOLTIP_WIDTH_FRACTION_RESIDUAL: f32 = 0.20;
/// Mouse.cpp DisplayString word-wrap residual pixels.
pub const TOOLTIP_WORD_WRAP_PIXELS_RESIDUAL: i32 = 120;
/// Retail Mouse.ini TooltipTextColor residual RGBA.
pub const TOOLTIP_TEXT_COLOR_RESIDUAL: (u8, u8, u8, u8) = (220, 220, 220, 255);
/// Retail Mouse.ini TooltipHighlightColor residual RGBA.
pub const TOOLTIP_HIGHLIGHT_COLOR_RESIDUAL: (u8, u8, u8, u8) = (255, 255, 0, 255);
/// Retail Mouse.ini TooltipShadowColor residual RGBA.
pub const TOOLTIP_SHADOW_COLOR_RESIDUAL: (u8, u8, u8, u8) = (0, 0, 0, 255);
/// Retail Mouse.ini TooltipBorderColor residual RGBA.
pub const TOOLTIP_BORDER_COLOR_RESIDUAL: (u8, u8, u8, u8) = (60, 60, 155, 255);
/// Retail Mouse.ini TooltipBackgroundColor residual RGBA.
pub const TOOLTIP_BACKGROUND_COLOR_RESIDUAL: (u8, u8, u8, u8) = (20, 20, 20, 255);
/// Retail Mouse.ini UseTooltipAltTextColor residual.
pub const TOOLTIP_USE_ALT_TEXT_COLOR_RESIDUAL: bool = false;
/// Retail Mouse.ini UseTooltipAltBackColor residual.
pub const TOOLTIP_USE_ALT_BACK_COLOR_RESIDUAL: bool = true;
/// Retail Mouse.ini AdjustTooltipAltColor residual.
pub const TOOLTIP_ADJUST_ALT_COLOR_RESIDUAL: bool = true;

/// Tooltip delay override residual: C++ `m_tooltipDelay >= 0` replaces Mouse.ini delay.
pub fn tooltip_effective_delay_ms_residual(ini_delay: i32, override_delay: i32) -> i32 {
    if override_delay >= 0 {
        override_delay
    } else {
        ini_delay
    }
}

/// Wave 91 honesty: tooltip residual peels pack.
pub fn honesty_tooltip_residual_pack_wave91() -> bool {
    TOOLTIP_FONT_NAME_RESIDUAL == "Arial"
        && TOOLTIP_FONT_SIZE_RESIDUAL == 8
        && !TOOLTIP_FONT_IS_BOLD_RESIDUAL
        && !TOOLTIP_ANIMATE_BACKGROUND_RESIDUAL
        && TOOLTIP_FILL_TIME_MS_RESIDUAL == 250
        && TOOLTIP_DELAY_TIME_MS_RESIDUAL == 800
        && (TOOLTIP_WIDTH_PERCENT_RESIDUAL - 20.0).abs() < 1e-5
        && (TOOLTIP_WIDTH_FRACTION_RESIDUAL - 0.20).abs() < 1e-5
        && TOOLTIP_WORD_WRAP_PIXELS_RESIDUAL == 120
        && TOOLTIP_TEXT_COLOR_RESIDUAL == (220, 220, 220, 255)
        && TOOLTIP_HIGHLIGHT_COLOR_RESIDUAL == (255, 255, 0, 255)
        && TOOLTIP_SHADOW_COLOR_RESIDUAL == (0, 0, 0, 255)
        && TOOLTIP_BORDER_COLOR_RESIDUAL == (60, 60, 155, 255)
        && TOOLTIP_BACKGROUND_COLOR_RESIDUAL == (20, 20, 20, 255)
        && !TOOLTIP_USE_ALT_TEXT_COLOR_RESIDUAL
        && TOOLTIP_USE_ALT_BACK_COLOR_RESIDUAL
        && TOOLTIP_ADJUST_ALT_COLOR_RESIDUAL
        && tooltip_effective_delay_ms_residual(TOOLTIP_DELAY_TIME_MS_RESIDUAL, -1) == 800
        && tooltip_effective_delay_ms_residual(TOOLTIP_DELAY_TIME_MS_RESIDUAL, 0) == 0
        && tooltip_effective_delay_ms_residual(TOOLTIP_DELAY_TIME_MS_RESIDUAL, 500) == 500
}

// ---------------------------------------------------------------------------
// 2. HelpBox residual peels (ControlBarPopupDescription / build tooltip layout)
// ---------------------------------------------------------------------------

/// ControlBar build help-box layout residual.
pub const HELP_BOX_LAYOUT_WND_RESIDUAL: &str = "ControlBarPopupDescription.wnd";
/// Help-box StaticTextName window id residual.
pub const HELP_BOX_STATIC_TEXT_NAME_ID_RESIDUAL: &str =
    "ControlBarPopupDescription.wnd:StaticTextName";
/// Help-box StaticTextCost window id residual.
pub const HELP_BOX_STATIC_TEXT_COST_ID_RESIDUAL: &str =
    "ControlBarPopupDescription.wnd:StaticTextCost";
/// Help-box StaticTextDescription window id residual.
pub const HELP_BOX_STATIC_TEXT_DESCRIPTION_ID_RESIDUAL: &str =
    "ControlBarPopupDescription.wnd:StaticTextDescription";
/// Money display tooltip subject residual (ControlBar.wnd:MoneyDisplay).
pub const HELP_BOX_MONEY_DISPLAY_ID_RESIDUAL: &str = "ControlBar.wnd:MoneyDisplay";
/// Power window tooltip subject residual.
pub const HELP_BOX_POWER_WINDOW_ID_RESIDUAL: &str = "ControlBar.wnd:PowerWindow";
/// Generals experience tooltip subject residual.
pub const HELP_BOX_GENERALS_EXP_ID_RESIDUAL: &str = "ControlBar.wnd:GeneralsExp";

/// Help-box subject residual discriminants (matches ControlBar populate paths).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HelpBoxSubjectResidual {
    CommandButton = 0,
    MoneyDisplay = 1,
    PowerWindow = 2,
    GeneralsExp = 3,
}

/// CanMakeStatus residual status message strings (ControlBarPopupDescription).
pub const HELP_BOX_STATUS_NO_MONEY_RESIDUAL: &str = "Not enough money to build";
pub const HELP_BOX_STATUS_QUEUE_FULL_RESIDUAL: &str = "Cannot purchase because build queue is full";
pub const HELP_BOX_STATUS_PARKING_FULL_RESIDUAL: &str = "Cannot build unit because parking is full";
pub const HELP_BOX_STATUS_MAXED_UNIT_RESIDUAL: &str =
    "Cannot build unit because maximum number reached";
pub const HELP_BOX_STATUS_MAXED_STRUCTURE_RESIDUAL: &str =
    "Cannot build building because maximum number reached";

/// CanMakeStatus residual enum order (matches ControlBarPopupDescription port).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum HelpBoxCanMakeStatusResidual {
    Ok = 0,
    NoPrereq = 1,
    NoMoney = 2,
    FactoryDisabled = 3,
    QueueFull = 4,
    ParkingPlacesFull = 5,
    MaxedOutForPlayer = 6,
}

/// Resolve CanMakeStatus residual status message.
pub fn help_box_can_make_status_message_residual(
    status: HelpBoxCanMakeStatusResidual,
    is_structure: bool,
) -> Option<&'static str> {
    match status {
        HelpBoxCanMakeStatusResidual::Ok
        | HelpBoxCanMakeStatusResidual::NoPrereq
        | HelpBoxCanMakeStatusResidual::FactoryDisabled => None,
        HelpBoxCanMakeStatusResidual::NoMoney => Some(HELP_BOX_STATUS_NO_MONEY_RESIDUAL),
        HelpBoxCanMakeStatusResidual::QueueFull => Some(HELP_BOX_STATUS_QUEUE_FULL_RESIDUAL),
        HelpBoxCanMakeStatusResidual::ParkingPlacesFull => {
            Some(HELP_BOX_STATUS_PARKING_FULL_RESIDUAL)
        }
        HelpBoxCanMakeStatusResidual::MaxedOutForPlayer => {
            if is_structure {
                Some(HELP_BOX_STATUS_MAXED_STRUCTURE_RESIDUAL)
            } else {
                Some(HELP_BOX_STATUS_MAXED_UNIT_RESIDUAL)
            }
        }
    }
}

/// Wave 91 honesty: HelpBox residual peels pack.
pub fn honesty_help_box_residual_pack_wave91() -> bool {
    HELP_BOX_LAYOUT_WND_RESIDUAL == "ControlBarPopupDescription.wnd"
        && HELP_BOX_STATIC_TEXT_NAME_ID_RESIDUAL == "ControlBarPopupDescription.wnd:StaticTextName"
        && HELP_BOX_STATIC_TEXT_COST_ID_RESIDUAL == "ControlBarPopupDescription.wnd:StaticTextCost"
        && HELP_BOX_STATIC_TEXT_DESCRIPTION_ID_RESIDUAL
            == "ControlBarPopupDescription.wnd:StaticTextDescription"
        && HELP_BOX_MONEY_DISPLAY_ID_RESIDUAL == "ControlBar.wnd:MoneyDisplay"
        && HELP_BOX_POWER_WINDOW_ID_RESIDUAL == "ControlBar.wnd:PowerWindow"
        && HELP_BOX_GENERALS_EXP_ID_RESIDUAL == "ControlBar.wnd:GeneralsExp"
        && HelpBoxSubjectResidual::CommandButton as u8 == 0
        && HelpBoxSubjectResidual::GeneralsExp as u8 == 3
        && help_box_can_make_status_message_residual(HelpBoxCanMakeStatusResidual::Ok, false)
            .is_none()
        && help_box_can_make_status_message_residual(HelpBoxCanMakeStatusResidual::NoMoney, false)
            == Some(HELP_BOX_STATUS_NO_MONEY_RESIDUAL)
        && help_box_can_make_status_message_residual(HelpBoxCanMakeStatusResidual::QueueFull, false)
            == Some(HELP_BOX_STATUS_QUEUE_FULL_RESIDUAL)
        && help_box_can_make_status_message_residual(
            HelpBoxCanMakeStatusResidual::ParkingPlacesFull,
            false,
        ) == Some(HELP_BOX_STATUS_PARKING_FULL_RESIDUAL)
        && help_box_can_make_status_message_residual(
            HelpBoxCanMakeStatusResidual::MaxedOutForPlayer,
            false,
        ) == Some(HELP_BOX_STATUS_MAXED_UNIT_RESIDUAL)
        && help_box_can_make_status_message_residual(
            HelpBoxCanMakeStatusResidual::MaxedOutForPlayer,
            true,
        ) == Some(HELP_BOX_STATUS_MAXED_STRUCTURE_RESIDUAL)
        && help_box_can_make_status_message_residual(HelpBoxCanMakeStatusResidual::NoPrereq, false)
            .is_none()
}

// ---------------------------------------------------------------------------
// 3. Message residual peels (InGameUI.ini + InGameUI.h/cpp)
// ---------------------------------------------------------------------------

/// C++ `MAX_UI_MESSAGES` residual.
pub const MAX_UI_MESSAGES_RESIDUAL: usize = 6;
/// Retail InGameUI.ini MessageColor1 residual RGB (alpha full).
pub const MESSAGE_COLOR1_RESIDUAL: (u8, u8, u8, u8) = (255, 255, 255, 255);
/// Retail InGameUI.ini MessageColor2 residual RGB.
pub const MESSAGE_COLOR2_RESIDUAL: (u8, u8, u8, u8) = (180, 180, 255, 255);
/// Retail InGameUI.ini MessagePosition residual.
pub const MESSAGE_POSITION_RESIDUAL: (i32, i32) = (10, 10);
/// Retail InGameUI.ini MessageFont residual.
pub const MESSAGE_FONT_RESIDUAL: &str = "Arial";
/// Retail InGameUI.ini MessagePointSize residual.
pub const MESSAGE_POINT_SIZE_RESIDUAL: i32 = 10;
/// Retail InGameUI.ini MessageBold residual.
pub const MESSAGE_BOLD_RESIDUAL: bool = true;
/// Retail InGameUI.ini MessageDelayMS residual.
pub const MESSAGE_DELAY_MS_RESIDUAL: i32 = 75_000;
/// C++ constructor MessageDelayMS default residual (pre-INI).
pub const MESSAGE_DELAY_MS_CTOR_DEFAULT_RESIDUAL: i32 = 5_000;
/// C++ integer timeout residual: `delayMS / LOGICFRAMES_PER_SECOND / 1000`.
pub fn message_timeout_frames_cpp_residual(delay_ms: i32) -> i32 {
    delay_ms / LOGIC_FRAMES_PER_SECOND_RESIDUAL / 1000
}
/// Popup message layout residual.
pub const POPUP_MESSAGE_LAYOUT_WND_RESIDUAL: &str = "InGamePopupMessage.wnd";
/// Retail InGameUI.ini PopupMessageColor residual.
pub const POPUP_MESSAGE_COLOR_RESIDUAL: (u8, u8, u8, u8) = (255, 255, 255, 255);
/// Retail FloatingTextTimeOut ms residual (parseDurationUnsignedInt → frames).
pub const FLOATING_TEXT_TIMEOUT_MS_RESIDUAL: u32 = 333;
/// Floating text timeout frames residual: ceil(333 * 30 / 1000) = 10.
pub const FLOATING_TEXT_TIMEOUT_FRAMES_RESIDUAL: u32 = 10;
/// C++ DEFAULT_FLOATING_TEXT_TIMEOUT residual (= LOGICFRAMES_PER_SECOND/3).
pub const DEFAULT_FLOATING_TEXT_TIMEOUT_CTOR_RESIDUAL: u32 = 10;
/// Retail FloatingTextMoveUpSpeed residual (dist/sec → dist/frame = 1.0).
pub const FLOATING_TEXT_MOVE_UP_SPEED_SEC_RESIDUAL: f32 = 30.0;
pub const FLOATING_TEXT_MOVE_UP_SPEED_FRAME_RESIDUAL: f32 = 1.0;
/// Retail FloatingTextVanishRate residual (dist/sec → 0.1 / frame).
pub const FLOATING_TEXT_VANISH_RATE_SEC_RESIDUAL: f32 = 3.0;
pub const FLOATING_TEXT_VANISH_RATE_FRAME_RESIDUAL: f32 = 0.1;
/// Message fade amount residual scale (`age * 0.01`).
pub const MESSAGE_FADE_AMOUNT_SCALE_RESIDUAL: f32 = 0.01;

/// Wave 91 honesty: message residual peels pack.
pub fn honesty_message_residual_pack_wave91() -> bool {
    MAX_UI_MESSAGES_RESIDUAL == 6
        && MESSAGE_COLOR1_RESIDUAL == (255, 255, 255, 255)
        && MESSAGE_COLOR2_RESIDUAL == (180, 180, 255, 255)
        && MESSAGE_POSITION_RESIDUAL == (10, 10)
        && MESSAGE_FONT_RESIDUAL == "Arial"
        && MESSAGE_POINT_SIZE_RESIDUAL == 10
        && MESSAGE_BOLD_RESIDUAL
        && MESSAGE_DELAY_MS_RESIDUAL == 75_000
        && MESSAGE_DELAY_MS_CTOR_DEFAULT_RESIDUAL == 5_000
        && message_timeout_frames_cpp_residual(MESSAGE_DELAY_MS_RESIDUAL) == 2
        && message_timeout_frames_cpp_residual(MESSAGE_DELAY_MS_CTOR_DEFAULT_RESIDUAL) == 0
        && POPUP_MESSAGE_LAYOUT_WND_RESIDUAL == "InGamePopupMessage.wnd"
        && POPUP_MESSAGE_COLOR_RESIDUAL == (255, 255, 255, 255)
        && FLOATING_TEXT_TIMEOUT_MS_RESIDUAL == 333
        && duration_ms_to_frames_ceil_residual(FLOATING_TEXT_TIMEOUT_MS_RESIDUAL)
            == FLOATING_TEXT_TIMEOUT_FRAMES_RESIDUAL
        && FLOATING_TEXT_TIMEOUT_FRAMES_RESIDUAL == 10
        && DEFAULT_FLOATING_TEXT_TIMEOUT_CTOR_RESIDUAL
            == (LOGIC_FRAMES_PER_SECOND_RESIDUAL as u32) / 3
        && (velocity_secs_to_frames_residual(FLOATING_TEXT_MOVE_UP_SPEED_SEC_RESIDUAL)
            - FLOATING_TEXT_MOVE_UP_SPEED_FRAME_RESIDUAL)
            .abs()
            < 1e-5
        && (velocity_secs_to_frames_residual(FLOATING_TEXT_VANISH_RATE_SEC_RESIDUAL)
            - FLOATING_TEXT_VANISH_RATE_FRAME_RESIDUAL)
            .abs()
            < 1e-5
        && (MESSAGE_FADE_AMOUNT_SCALE_RESIDUAL - 0.01).abs() < 1e-6
}

// ---------------------------------------------------------------------------
// 4. EVA residual peels (Eva.h / Eva.cpp TheEvaMessageNames)
// ---------------------------------------------------------------------------

/// C++ `EVA_COUNT` residual (message slots 0..52; EVA_Invalid = -1).
pub const EVA_COUNT_RESIDUAL: usize = 53;
/// EvaCheckInfo default priority residual.
pub const EVA_PRIORITY_DEFAULT_RESIDUAL: u32 = 1;
/// EvaCheckInfo default framesBetweenChecks residual (900 = 30s @ 30fps).
pub const EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL: u32 = 900;
/// EvaCheckInfo default framesToExpire residual (150 = 5s @ 30fps).
pub const EVA_FRAMES_TO_EXPIRE_DEFAULT_RESIDUAL: u32 = 150;
/// Eva enabled default residual.
pub const EVA_ENABLED_DEFAULT_RESIDUAL: bool = true;
/// EvaCheck TRIGGEREDON_NOT residual.
pub const EVA_TRIGGERED_ON_NOT_RESIDUAL: u32 = u32::MAX;
/// EvaCheck NEXT_CHECK_NOW residual.
pub const EVA_NEXT_CHECK_NOW_RESIDUAL: u32 = 0;

/// Ordered C++ `TheEvaMessageNames` residual (EVA_FIRST..EVA_COUNT-1).
pub const EVA_MESSAGE_NAME_LIST: &[&str] = &[
    "LOWPOWER",                                 // 0
    "INSUFFICIENTFUNDS",                        // 1
    "SUPERWEAPONDETECTED_OWN_PARTICLECANNON",   // 2
    "SUPERWEAPONDETECTED_OWN_NUKE",             // 3
    "SUPERWEAPONDETECTED_OWN_SCUDSTORM",        // 4
    "SUPERWEAPONDETECTED_ALLY_PARTICLECANNON",  // 5
    "SUPERWEAPONDETECTED_ALLY_NUKE",            // 6
    "SUPERWEAPONDETECTED_ALLY_SCUDSTORM",       // 7
    "SUPERWEAPONDETECTED_ENEMY_PARTICLECANNON", // 8
    "SUPERWEAPONDETECTED_ENEMY_NUKE",           // 9
    "SUPERWEAPONDETECTED_ENEMY_SCUDSTORM",      // 10
    "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON",   // 11
    "SUPERWEAPONLAUNCHED_OWN_NUKE",             // 12
    "SUPERWEAPONLAUNCHED_OWN_SCUDSTORM",        // 13
    "SUPERWEAPONLAUNCHED_ALLY_PARTICLECANNON",  // 14
    "SUPERWEAPONLAUNCHED_ALLY_NUKE",            // 15
    "SUPERWEAPONLAUNCHED_ALLY_SCUDSTORM",       // 16
    "SUPERWEAPONLAUNCHED_ENEMY_PARTICLECANNON", // 17
    "SUPERWEAPONLAUNCHED_ENEMY_NUKE",           // 18
    "SUPERWEAPONLAUNCHED_ENEMY_SCUDSTORM",      // 19
    "SUPERWEAPONREADY_OWN_PARTICLECANNON",      // 20
    "SUPERWEAPONREADY_OWN_NUKE",                // 21
    "SUPERWEAPONREADY_OWN_SCUDSTORM",           // 22
    "SUPERWEAPONREADY_ALLY_PARTICLECANNON",     // 23
    "SUPERWEAPONREADY_ALLY_NUKE",               // 24
    "SUPERWEAPONREADY_ALLY_SCUDSTORM",          // 25
    "SUPERWEAPONREADY_ENEMY_PARTICLECANNON",    // 26
    "SUPERWEAPONREADY_ENEMY_NUKE",              // 27
    "SUPERWEAPONREADY_ENEMY_SCUDSTORM",         // 28
    "BUILDINGLOST",                             // 29
    "BASEUNDERATTACK",                          // 30
    "ALLYUNDERATTACK",                          // 31
    "BEACONDETECTED",                           // 32
    "ENEMYBLACKLOTUSDETECTED",                  // 33
    "ENEMYJARMENKELLDETECTED",                  // 34
    "ENEMYCOLONELBURTONDETECTED",               // 35
    "OWNBLACKLOTUSDETECTED",                    // 36
    "OWNJARMENKELLDETECTED",                    // 37
    "OWNCOLONELBURTONDETECTED",                 // 38
    "UNITLOST",                                 // 39
    "GENERALLEVELUP",                           // 40
    "VEHICLESTOLEN",                            // 41
    "BUILDINGSTOLEN",                           // 42
    "CASHSTOLEN",                               // 43
    "UPGRADECOMPLETE",                          // 44
    "BUILDINGBEINGSTOLEN",                      // 45
    "BUILDINGSABOTAGED",                        // 46
    "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER",    // 47
    "SUPERWEAPONLAUNCHED_ALLY_GPS_SCRAMBLER",   // 48
    "SUPERWEAPONLAUNCHED_ENEMY_GPS_SCRAMBLER",  // 49
    "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK",     // 50
    "SUPERWEAPONLAUNCHED_ALLY_SNEAK_ATTACK",    // 51
    "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK",   // 52
];

/// Sentinel after EVA_COUNT residual.
pub const EVA_INVALID_SENTINEL_NAME_RESIDUAL: &str = "EVA_INVALID";

/// Lookup EvaMessage residual name index (case-insensitive).
pub fn eva_message_name_index(name: &str) -> Option<usize> {
    EVA_MESSAGE_NAME_LIST
        .iter()
        .position(|&n| n.eq_ignore_ascii_case(name))
}

/// Wave 91 honesty: EVA residual peels pack.
pub fn honesty_eva_residual_pack_wave91() -> bool {
    EVA_COUNT_RESIDUAL == 53
        && EVA_MESSAGE_NAME_LIST.len() == EVA_COUNT_RESIDUAL
        && EVA_PRIORITY_DEFAULT_RESIDUAL == 1
        && EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL == 900
        && EVA_FRAMES_TO_EXPIRE_DEFAULT_RESIDUAL == 150
        && EVA_ENABLED_DEFAULT_RESIDUAL
        && EVA_TRIGGERED_ON_NOT_RESIDUAL == u32::MAX
        && EVA_NEXT_CHECK_NOW_RESIDUAL == 0
        && EVA_MESSAGE_NAME_LIST[0] == "LOWPOWER"
        && EVA_MESSAGE_NAME_LIST[1] == "INSUFFICIENTFUNDS"
        && EVA_MESSAGE_NAME_LIST[2] == "SUPERWEAPONDETECTED_OWN_PARTICLECANNON"
        && EVA_MESSAGE_NAME_LIST[11] == "SUPERWEAPONLAUNCHED_OWN_PARTICLECANNON"
        && EVA_MESSAGE_NAME_LIST[20] == "SUPERWEAPONREADY_OWN_PARTICLECANNON"
        && EVA_MESSAGE_NAME_LIST[29] == "BUILDINGLOST"
        && EVA_MESSAGE_NAME_LIST[39] == "UNITLOST"
        && EVA_MESSAGE_NAME_LIST[40] == "GENERALLEVELUP"
        && EVA_MESSAGE_NAME_LIST[46] == "BUILDINGSABOTAGED"
        && EVA_MESSAGE_NAME_LIST[47] == "SUPERWEAPONLAUNCHED_OWN_GPS_SCRAMBLER"
        && EVA_MESSAGE_NAME_LIST[50] == "SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK"
        && EVA_MESSAGE_NAME_LIST[52] == "SUPERWEAPONLAUNCHED_ENEMY_SNEAK_ATTACK"
        && eva_message_name_index("LOWPOWER") == Some(0)
        && eva_message_name_index("unitlost") == Some(39)
        && eva_message_name_index("SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK") == Some(50)
        && eva_message_name_index("not_an_eva").is_none()
        && EVA_INVALID_SENTINEL_NAME_RESIDUAL == "EVA_INVALID"
        && {
            let mut names: Vec<&str> = EVA_MESSAGE_NAME_LIST.to_vec();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
        // 900 frames = 30s; 150 frames = 5s at 30 FPS residual.
        && EVA_FRAMES_BETWEEN_CHECKS_DEFAULT_RESIDUAL
            == (30 * LOGIC_FRAMES_PER_SECOND_RESIDUAL) as u32
        && EVA_FRAMES_TO_EXPIRE_DEFAULT_RESIDUAL
            == (5 * LOGIC_FRAMES_PER_SECOND_RESIDUAL) as u32
}

// ---------------------------------------------------------------------------
// 5. Video residual peels (Video.ini names only — not full codec)
// ---------------------------------------------------------------------------

/// C++ `VideoBuffer::Type` residual count (`NUM_TYPES`).
pub const VIDEO_BUFFER_NUM_TYPES_RESIDUAL: u32 = 5;
/// VideoBuffer type residual ordered names (TYPE_UNKNOWN..TYPE_X1R5G5B5).
pub const VIDEO_BUFFER_TYPE_NAME_LIST: &[&str] = &[
    "TYPE_UNKNOWN",  // 0
    "TYPE_R8G8B8",   // 1
    "TYPE_X8R8G8B8", // 2
    "TYPE_R5G6B5",   // 3
    "TYPE_X1R5G5B5", // 4
];

/// Retail Video.ini residual entry (internal name + filename on disk).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoNameResidual {
    pub internal_name: &'static str,
    pub filename: &'static str,
}

/// Retail Video.ini residual name table (41 entries; names only).
pub const VIDEO_NAME_TABLE_RESIDUAL: &[VideoNameResidual] = &[
    VideoNameResidual {
        internal_name: "Sizzle",
        filename: "sizzle_review",
    },
    VideoNameResidual {
        internal_name: "Sizzle640",
        filename: "sizzle_review640",
    },
    VideoNameResidual {
        internal_name: "EALogoMovie",
        filename: "EA_LOGO",
    },
    VideoNameResidual {
        internal_name: "EALogoMovie640",
        filename: "EA_LOGO640",
    },
    VideoNameResidual {
        internal_name: "GeneralsChallengeBackground",
        filename: "GC_Background",
    },
    VideoNameResidual {
        internal_name: "VSSmall",
        filename: "VS_small",
    },
    VideoNameResidual {
        internal_name: "PortraitDrThraxLeft",
        filename: "Comp_ThraxGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitDrThraxRight",
        filename: "Comp_ThraxGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitAirGenLeft",
        filename: "Comp_AirGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitAirGenRight",
        filename: "Comp_AirGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitBossGenLeft",
        filename: "Comp_BossGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitBossGenRight",
        filename: "Comp_BossGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitDemolitionGenLeft",
        filename: "Comp_DemolGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitDemolitionGenRight",
        filename: "Comp_DemolGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitInfantryGenLeft",
        filename: "Comp_InfantryGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitInfantryGenRight",
        filename: "Comp_InfantryGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitLaserGenLeft",
        filename: "Comp_LaserGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitLaserGenRight",
        filename: "Comp_LaserGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitNukeGenLeft",
        filename: "Comp_NukeGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitNukeGenRight",
        filename: "Comp_NukeGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitTankGenLeft",
        filename: "Comp_TankGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitTankGenRight",
        filename: "Comp_TankGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitSuperGenLeft",
        filename: "Comp_SuperGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitSuperGenRight",
        filename: "Comp_SuperGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "PortraitStealthGenLeft",
        filename: "Comp_StealthGen_000",
    },
    VideoNameResidual {
        internal_name: "PortraitStealthGenRight",
        filename: "Comp_StealthGen_inv_000",
    },
    VideoNameResidual {
        internal_name: "MD_China01",
        filename: "MD_China01_0",
    },
    VideoNameResidual {
        internal_name: "MD_China02",
        filename: "MD_China02_0",
    },
    VideoNameResidual {
        internal_name: "MD_China03",
        filename: "MD_China03_0",
    },
    VideoNameResidual {
        internal_name: "MD_China04",
        filename: "MD_China04_0",
    },
    VideoNameResidual {
        internal_name: "MD_China05",
        filename: "MD_China05_0",
    },
    VideoNameResidual {
        internal_name: "MD_GLA01",
        filename: "MD_GLA01_0",
    },
    VideoNameResidual {
        internal_name: "MD_GLA02",
        filename: "MD_GLA02_0",
    },
    VideoNameResidual {
        internal_name: "MD_GLA03",
        filename: "MD_GLA03_0",
    },
    VideoNameResidual {
        internal_name: "MD_GLA04",
        filename: "MD_GLA04_0",
    },
    VideoNameResidual {
        internal_name: "MD_GLA05",
        filename: "MD_GLA05_0",
    },
    VideoNameResidual {
        internal_name: "MD_USA01",
        filename: "MD_USA01_0",
    },
    VideoNameResidual {
        internal_name: "MD_USA02",
        filename: "MD_USA02_0",
    },
    VideoNameResidual {
        internal_name: "MD_USA03",
        filename: "MD_USA03_0",
    },
    VideoNameResidual {
        internal_name: "MD_USA04",
        filename: "MD_USA04_0",
    },
    VideoNameResidual {
        internal_name: "MD_USA05",
        filename: "MD_USA05_0",
    },
];

/// Retail Video.ini residual entry count.
pub const VIDEO_NAME_TABLE_COUNT_RESIDUAL: usize = 41;

/// Lookup Video.ini residual by internal name (case-insensitive).
pub fn video_name_residual_by_internal(name: &str) -> Option<&'static VideoNameResidual> {
    VIDEO_NAME_TABLE_RESIDUAL
        .iter()
        .find(|v| v.internal_name.eq_ignore_ascii_case(name))
}

/// Wave 91 honesty: video residual name table pack (names only).
pub fn honesty_video_residual_name_table_wave91() -> bool {
    VIDEO_BUFFER_NUM_TYPES_RESIDUAL == 5
        && VIDEO_BUFFER_TYPE_NAME_LIST.len() == 5
        && VIDEO_BUFFER_TYPE_NAME_LIST[0] == "TYPE_UNKNOWN"
        && VIDEO_BUFFER_TYPE_NAME_LIST[2] == "TYPE_X8R8G8B8"
        && VIDEO_BUFFER_TYPE_NAME_LIST[4] == "TYPE_X1R5G5B5"
        && VIDEO_NAME_TABLE_RESIDUAL.len() == VIDEO_NAME_TABLE_COUNT_RESIDUAL
        && VIDEO_NAME_TABLE_COUNT_RESIDUAL == 41
        && VIDEO_NAME_TABLE_RESIDUAL[0].internal_name == "Sizzle"
        && VIDEO_NAME_TABLE_RESIDUAL[0].filename == "sizzle_review"
        && VIDEO_NAME_TABLE_RESIDUAL[2].internal_name == "EALogoMovie"
        && VIDEO_NAME_TABLE_RESIDUAL[4].internal_name == "GeneralsChallengeBackground"
        && VIDEO_NAME_TABLE_RESIDUAL[4].filename == "GC_Background"
        && VIDEO_NAME_TABLE_RESIDUAL[26].internal_name == "MD_China01"
        && VIDEO_NAME_TABLE_RESIDUAL[31].internal_name == "MD_GLA01"
        && VIDEO_NAME_TABLE_RESIDUAL[36].internal_name == "MD_USA01"
        && VIDEO_NAME_TABLE_RESIDUAL[40].internal_name == "MD_USA05"
        && VIDEO_NAME_TABLE_RESIDUAL[40].filename == "MD_USA05_0"
        && video_name_residual_by_internal("MD_USA01")
            .map(|v| v.filename == "MD_USA01_0")
            .unwrap_or(false)
        && video_name_residual_by_internal("md_china05")
            .map(|v| v.filename == "MD_China05_0")
            .unwrap_or(false)
        && video_name_residual_by_internal("GeneralsChallengeBackground")
            .map(|v| v.filename == "GC_Background")
            .unwrap_or(false)
        && video_name_residual_by_internal("not_a_video").is_none()
        && {
            let mut names: Vec<&str> = VIDEO_NAME_TABLE_RESIDUAL
                .iter()
                .map(|v| v.internal_name)
                .collect();
            names.sort_unstable();
            names.windows(2).all(|w| w[0] != w[1])
        }
        // Campaign transition movie residual cluster contiguous residual.
        && VIDEO_NAME_TABLE_RESIDUAL
            .iter()
            .filter(|v| v.internal_name.starts_with("MD_"))
            .count()
            == 15
}

// ---------------------------------------------------------------------------
// 6. Mission briefing residual peels (CampaignManager + Campaign.ini)
// ---------------------------------------------------------------------------

/// C++ `MAX_OBJECTIVE_LINES` residual.
pub const MAX_OBJECTIVE_LINES_RESIDUAL: usize = 5;
/// C++ `MAX_DISPLAYED_UNITS` residual.
pub const MAX_DISPLAYED_UNITS_RESIDUAL: usize = 3;
/// C++ `MAX_SUBTITLE_LINES` residual (military caption / briefing subtitle).
pub const MAX_SUBTITLE_LINES_RESIDUAL: usize = 4;
/// C++ CampaignManager::INVALID_MISSION_NUMBER residual.
pub const INVALID_MISSION_NUMBER_RESIDUAL: i32 = -1;
/// Retail SinglePlayerLoadScreen layout residual.
pub const SINGLE_PLAYER_LOAD_SCREEN_WND_RESIDUAL: &str = "Menus/SinglePlayerLoadScreen.wnd";
/// Retail military caption audio residual.
pub const MILITARY_SUBTITLES_TYPING_AUDIO_RESIDUAL: &str = "MilitarySubtitlesTyping";
/// Retail InGameUI.ini MilitaryCaptionPosition residual.
pub const MILITARY_CAPTION_POSITION_RESIDUAL: (i32, i32) = (10, 340);
/// Retail InGameUI.ini MilitaryCaptionTitleFont residual.
pub const MILITARY_CAPTION_TITLE_FONT_RESIDUAL: &str = "Courier New";
/// Retail InGameUI.ini MilitaryCaptionTitlePointSize residual.
pub const MILITARY_CAPTION_TITLE_POINT_SIZE_RESIDUAL: i32 = 12;
/// Retail InGameUI.ini MilitaryCaptionTitleBold residual.
pub const MILITARY_CAPTION_TITLE_BOLD_RESIDUAL: bool = true;
/// Retail InGameUI.ini MilitaryCaptionFont residual.
pub const MILITARY_CAPTION_FONT_RESIDUAL: &str = "Courier New";
/// Retail InGameUI.ini MilitaryCaptionPointSize residual.
pub const MILITARY_CAPTION_POINT_SIZE_RESIDUAL: i32 = 12;
/// Retail InGameUI.ini MilitaryCaptionBold residual.
pub const MILITARY_CAPTION_BOLD_RESIDUAL: bool = false;
/// Retail InGameUI.ini MilitaryCaptionRandomizeTyping residual.
pub const MILITARY_CAPTION_RANDOMIZE_TYPING_RESIDUAL: bool = true;
/// Retail InGameUI.ini MilitaryCaptionColor residual.
pub const MILITARY_CAPTION_COLOR_RESIDUAL: (u8, u8, u8, u8) = (255, 255, 255, 255);
/// Display coordinate residual baseline for military subtitle scale (800x600).
pub const MILITARY_CAPTION_BASE_DISPLAY_W_RESIDUAL: f32 = 800.0;
pub const MILITARY_CAPTION_BASE_DISPLAY_H_RESIDUAL: f32 = 600.0;

/// Mission briefing residual row (Campaign.ini mission part anchors).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MissionBriefingResidual {
    pub campaign: &'static str,
    pub mission: &'static str,
    pub map: &'static str,
    pub intro_movie: &'static str,
    pub briefing_voice: Option<&'static str>,
    pub objective_line0: Option<&'static str>,
    pub unit_names0: Option<&'static str>,
    pub voice_length: Option<i32>,
}

/// Retail Campaign.ini mission briefing residual anchors.
pub const MISSION_BRIEFING_RESIDUAL_TABLE: &[MissionBriefingResidual] = &[
    MissionBriefingResidual {
        campaign: "TRAINING",
        mission: "Mission01",
        map: r"Maps\Training01\Training01.map",
        intro_movie: "TrainingCampaign",
        briefing_voice: Some("BriefingUSATraining"),
        objective_line0: Some("GUI:Objectives:"),
        unit_names0: Some("OBJECT:Ranger"),
        voice_length: Some(17),
    },
    MissionBriefingResidual {
        campaign: "USA",
        mission: "Mission01",
        map: r"Maps\MD_USA01\MD_USA01.map",
        intro_movie: "MD_USA01",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
    MissionBriefingResidual {
        campaign: "USA",
        mission: "Mission05",
        map: r"Maps\MD_USA05\MD_USA05.map",
        intro_movie: "MD_USA05",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
    MissionBriefingResidual {
        campaign: "GLA",
        mission: "Mission01",
        map: r"Maps\MD_GLA01\MD_GLA01.map",
        intro_movie: "MD_GLA01",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
    MissionBriefingResidual {
        campaign: "GLA",
        mission: "Mission05",
        map: r"Maps\MD_GLA05\MD_GLA05.map",
        intro_movie: "MD_GLA05",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
    MissionBriefingResidual {
        campaign: "China",
        mission: "Mission01",
        map: r"Maps\MD_CHI01\MD_CHI01.map",
        intro_movie: "MD_China01",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
    MissionBriefingResidual {
        campaign: "China",
        mission: "Mission05",
        map: r"Maps\MD_CHI05\MD_CHI05.map",
        intro_movie: "MD_China05",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
    MissionBriefingResidual {
        campaign: "CHALLENGE_0",
        mission: "Mission01",
        map: r"Maps\GC_ChemGeneral\GC_ChemGeneral.map",
        intro_movie: "GeneralsChallengeBackground",
        briefing_voice: None,
        objective_line0: None,
        unit_names0: None,
        voice_length: None,
    },
];

/// Campaign residual name list (main ZH campaigns + training).
pub const CAMPAIGN_NAME_LIST_RESIDUAL: &[&str] = &["TRAINING", "USA", "GLA", "China"];

/// Missions per main faction campaign residual (ZH MD 5 missions each).
pub const FACTION_CAMPAIGN_MISSION_COUNT_RESIDUAL: usize = 5;

/// Scale military caption position residual from 800×600 design space.
pub fn military_caption_scaled_position_residual(display_w: f32, display_h: f32) -> (f32, f32) {
    let mx = display_w / MILITARY_CAPTION_BASE_DISPLAY_W_RESIDUAL;
    let my = display_h / MILITARY_CAPTION_BASE_DISPLAY_H_RESIDUAL;
    (
        MILITARY_CAPTION_POSITION_RESIDUAL.0 as f32 * mx,
        MILITARY_CAPTION_POSITION_RESIDUAL.1 as f32 * my,
    )
}

/// Lookup mission briefing residual by campaign + mission name.
pub fn mission_briefing_residual(
    campaign: &str,
    mission: &str,
) -> Option<&'static MissionBriefingResidual> {
    MISSION_BRIEFING_RESIDUAL_TABLE.iter().find(|m| {
        m.campaign.eq_ignore_ascii_case(campaign) && m.mission.eq_ignore_ascii_case(mission)
    })
}

/// Wave 91 honesty: mission briefing residual peels pack.
pub fn honesty_mission_briefing_residual_pack_wave91() -> bool {
    MAX_OBJECTIVE_LINES_RESIDUAL == 5
        && MAX_DISPLAYED_UNITS_RESIDUAL == 3
        && MAX_SUBTITLE_LINES_RESIDUAL == 4
        && INVALID_MISSION_NUMBER_RESIDUAL == -1
        && SINGLE_PLAYER_LOAD_SCREEN_WND_RESIDUAL == "Menus/SinglePlayerLoadScreen.wnd"
        && MILITARY_SUBTITLES_TYPING_AUDIO_RESIDUAL == "MilitarySubtitlesTyping"
        && MILITARY_CAPTION_POSITION_RESIDUAL == (10, 340)
        && MILITARY_CAPTION_TITLE_FONT_RESIDUAL == "Courier New"
        && MILITARY_CAPTION_TITLE_POINT_SIZE_RESIDUAL == 12
        && MILITARY_CAPTION_TITLE_BOLD_RESIDUAL
        && MILITARY_CAPTION_FONT_RESIDUAL == "Courier New"
        && MILITARY_CAPTION_POINT_SIZE_RESIDUAL == 12
        && !MILITARY_CAPTION_BOLD_RESIDUAL
        && MILITARY_CAPTION_RANDOMIZE_TYPING_RESIDUAL
        && MILITARY_CAPTION_COLOR_RESIDUAL == (255, 255, 255, 255)
        && FACTION_CAMPAIGN_MISSION_COUNT_RESIDUAL == 5
        && CAMPAIGN_NAME_LIST_RESIDUAL.len() == 4
        && CAMPAIGN_NAME_LIST_RESIDUAL[0] == "TRAINING"
        && CAMPAIGN_NAME_LIST_RESIDUAL[1] == "USA"
        && CAMPAIGN_NAME_LIST_RESIDUAL[3] == "China"
        && {
            let training = mission_briefing_residual("TRAINING", "Mission01").unwrap();
            training.map == r"Maps\Training01\Training01.map"
                && training.intro_movie == "TrainingCampaign"
                && training.briefing_voice == Some("BriefingUSATraining")
                && training.objective_line0 == Some("GUI:Objectives:")
                && training.unit_names0 == Some("OBJECT:Ranger")
                && training.voice_length == Some(17)
        }
        && {
            let usa1 = mission_briefing_residual("USA", "Mission01").unwrap();
            usa1.intro_movie == "MD_USA01"
                && usa1.map == r"Maps\MD_USA01\MD_USA01.map"
                && video_name_residual_by_internal(usa1.intro_movie).is_some()
        }
        && {
            let gla5 = mission_briefing_residual("GLA", "Mission05").unwrap();
            gla5.intro_movie == "MD_GLA05"
        }
        && {
            let chi1 = mission_briefing_residual("China", "Mission01").unwrap();
            chi1.intro_movie == "MD_China01" && chi1.map == r"Maps\MD_CHI01\MD_CHI01.map"
        }
        && {
            let ch0 = mission_briefing_residual("CHALLENGE_0", "Mission01").unwrap();
            ch0.intro_movie == "GeneralsChallengeBackground"
                && video_name_residual_by_internal(ch0.intro_movie)
                    .map(|v| v.filename == "GC_Background")
                    .unwrap_or(false)
        }
        && mission_briefing_residual("USA", "Mission99").is_none()
        && {
            let (x, y) = military_caption_scaled_position_residual(800.0, 600.0);
            (x - 10.0).abs() < 1e-5 && (y - 340.0).abs() < 1e-5
        }
        && {
            let (x, y) = military_caption_scaled_position_residual(1600.0, 1200.0);
            (x - 20.0).abs() < 1e-5 && (y - 680.0).abs() < 1e-5
        }
}

// ---------------------------------------------------------------------------
// Combined Wave 91 pack
// ---------------------------------------------------------------------------

/// Combined Wave 91 honesty pack (all residual peels).
pub fn honesty_ui_presentation_residual_pack_wave91() -> bool {
    honesty_tooltip_residual_pack_wave91()
        && honesty_help_box_residual_pack_wave91()
        && honesty_message_residual_pack_wave91()
        && honesty_eva_residual_pack_wave91()
        && honesty_video_residual_name_table_wave91()
        && honesty_mission_briefing_residual_pack_wave91()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tooltip_residual_pack_wave91_honesty() {
        assert!(honesty_tooltip_residual_pack_wave91());
    }

    #[test]
    fn help_box_residual_pack_wave91_honesty() {
        assert!(honesty_help_box_residual_pack_wave91());
    }

    #[test]
    fn message_residual_pack_wave91_honesty() {
        assert!(honesty_message_residual_pack_wave91());
    }

    #[test]
    fn eva_residual_pack_wave91_honesty() {
        assert!(honesty_eva_residual_pack_wave91());
    }

    #[test]
    fn video_residual_name_table_wave91_honesty() {
        assert!(honesty_video_residual_name_table_wave91());
    }

    #[test]
    fn mission_briefing_residual_pack_wave91_honesty() {
        assert!(honesty_mission_briefing_residual_pack_wave91());
    }

    #[test]
    fn ui_presentation_residual_pack_wave91_combined_honesty() {
        assert!(honesty_ui_presentation_residual_pack_wave91());
    }
}
