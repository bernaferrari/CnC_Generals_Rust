//! Wave 110 residual peels: MessageStream / MetaEvent / InGameUI input residual
//! (host-testable client message path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 79 input residual, Wave 88 radius-cursor names, Wave 89
//! hotkey CommandMap, Wave 91 message residual, and Wave 106 MainMenu/GameWindow.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - MessageStream.h GameMessage::Type markers (META / NETWORK ranges)
//! - MessageStream.h GameMessageArgumentDataType
//! - MetaEvent.h MappableKeyCategories / CATEGORY_NUM_CATEGORIES
//! - InGameUI.h RadiusCursorType / MAX_BUILD_PROGRESS
//! - Player.h NUM_HOTKEY_SQUADS = 10
//!
//! Fail-closed:
//! - Not full MessageStream translator priority / append-process residual
//! - Not full MetaEvent CommandMap INI load residual
//! - Not full InGameUI drawable hint / build progress UI residual
//! - Not full network GameMessage serialization residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// MessageStream GameMessage::Type residual markers
// ---------------------------------------------------------------------------

/// Retail `MSG_INVALID` residual (never posted).
pub const MSG_INVALID: i32 = 0;
/// Retail `MSG_FRAME_TICK` residual ordinal after INVALID.
pub const MSG_FRAME_TICK: i32 = 1;

/// Retail meta-message band marker residual (`MSG_BEGIN_META_MESSAGES`).
/// Ordinal is after RAW mouse/key + processed mouse + CLEAR/NEW_GAME.
/// Host residual peels relative band markers, not absolute fragile ordinals for
/// every demo/debug cheat message.
pub const MSG_BEGIN_NETWORK_MESSAGES: i32 = 1000;
/// Retail debug-network band start residual.
pub const MSG_BEGIN_DEBUG_NETWORK_MESSAGES: i32 = 1900;
/// Retail network band end residual.
pub const MSG_END_NETWORK_MESSAGES: i32 = 1999;

/// Retail hotkey squad count (`Player::NUM_HOTKEY_SQUADS`).
pub const NUM_HOTKEY_SQUADS: u32 = 10;
/// Retail `NO_HOTKEY_SQUAD` residual.
pub const NO_HOTKEY_SQUAD: i32 = -1;

/// Whether network message type residual is inside the network band.
pub fn msg_is_network_band_residual(msg_type: i32) -> bool {
    msg_type >= MSG_BEGIN_NETWORK_MESSAGES && msg_type <= MSG_END_NETWORK_MESSAGES
}

/// Whether debug-network residual is inside the debug network band.
pub fn msg_is_debug_network_band_residual(msg_type: i32) -> bool {
    msg_type >= MSG_BEGIN_DEBUG_NETWORK_MESSAGES && msg_type <= MSG_END_NETWORK_MESSAGES
}

/// Hotkey squad index residual is legal (0..NUM_HOTKEY_SQUADS-1).
pub fn hotkey_squad_index_legal_residual(idx: i32) -> bool {
    idx >= 0 && (idx as u32) < NUM_HOTKEY_SQUADS
}

/// Hotkey squad create/select/add network message family residual peels.
/// Retail: MSG_CREATE_TEAM0..9 / MSG_SELECT_TEAM0..9 / MSG_ADD_TEAM0..9 are contiguous.
pub fn hotkey_team_message_offset_residual(team_index: u32) -> Option<u32> {
    if team_index < NUM_HOTKEY_SQUADS {
        Some(team_index)
    } else {
        None
    }
}

/// Honesty: MessageStream network/meta marker residual pack.
pub fn honesty_message_stream_marker_residual_wave110() -> bool {
    MSG_INVALID == 0
        && MSG_FRAME_TICK == 1
        && MSG_BEGIN_NETWORK_MESSAGES == 1000
        && MSG_BEGIN_DEBUG_NETWORK_MESSAGES == 1900
        && MSG_END_NETWORK_MESSAGES == 1999
        && msg_is_network_band_residual(1000)
        && msg_is_network_band_residual(1500)
        && msg_is_network_band_residual(1999)
        && !msg_is_network_band_residual(999)
        && !msg_is_network_band_residual(2000)
        && msg_is_debug_network_band_residual(1900)
        && msg_is_debug_network_band_residual(1999)
        && !msg_is_debug_network_band_residual(1899)
        && NUM_HOTKEY_SQUADS == 10
        && NO_HOTKEY_SQUAD == -1
        && hotkey_squad_index_legal_residual(0)
        && hotkey_squad_index_legal_residual(9)
        && !hotkey_squad_index_legal_residual(-1)
        && !hotkey_squad_index_legal_residual(10)
        && hotkey_team_message_offset_residual(0) == Some(0)
        && hotkey_team_message_offset_residual(9) == Some(9)
        && hotkey_team_message_offset_residual(10).is_none()
}

// ---------------------------------------------------------------------------
// GameMessageArgumentDataType residual
// ---------------------------------------------------------------------------

/// Retail `GameMessageArgumentDataType` residual names (order matches C++ enum).
pub const GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES: &[&str] = &[
    "INTEGER",
    "REAL",
    "BOOLEAN",
    "OBJECTID",
    "DRAWABLEID",
    "TEAMID",
    "LOCATION",
    "PIXEL",
    "PIXELREGION",
    "TIMESTAMP",
    "WIDECHAR",
    "UNKNOWN",
];

/// Retail argument data type count residual (including UNKNOWN).
pub const GAME_MESSAGE_ARGUMENT_DATA_TYPE_COUNT: usize = 12;

/// Honesty: GameMessageArgumentDataType residual pack.
pub fn honesty_game_message_argument_type_residual_wave110() -> bool {
    GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES.len() == GAME_MESSAGE_ARGUMENT_DATA_TYPE_COUNT
        && residual_name_index(GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES, "INTEGER") == Some(0)
        && residual_name_index(GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES, "OBJECTID") == Some(3)
        && residual_name_index(GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES, "LOCATION") == Some(6)
        && residual_name_index(GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES, "UNKNOWN") == Some(11)
        && residual_name_index(GAME_MESSAGE_ARGUMENT_DATA_TYPE_NAMES, "NOPE").is_none()
}

// ---------------------------------------------------------------------------
// MetaEvent MappableKeyCategories residual
// ---------------------------------------------------------------------------

/// Retail `MappableKeyCategories` residual names.
pub const MAPPABLE_KEY_CATEGORY_NAMES: &[&str] = &[
    "CONTROL",
    "INFORMATION",
    "INTERFACE",
    "SELECTION",
    "TAUNT",
    "TEAM",
    "MISC",
    "DEBUG",
];

/// Retail `CATEGORY_NUM_CATEGORIES` residual.
pub const CATEGORY_NUM_CATEGORIES: usize = 8;

/// Honesty: MetaEvent category residual pack.
pub fn honesty_meta_event_category_residual_wave110() -> bool {
    MAPPABLE_KEY_CATEGORY_NAMES.len() == CATEGORY_NUM_CATEGORIES
        && residual_name_index(MAPPABLE_KEY_CATEGORY_NAMES, "CONTROL") == Some(0)
        && residual_name_index(MAPPABLE_KEY_CATEGORY_NAMES, "TEAM") == Some(5)
        && residual_name_index(MAPPABLE_KEY_CATEGORY_NAMES, "DEBUG") == Some(7)
        && residual_name_index(MAPPABLE_KEY_CATEGORY_NAMES, "NETWORK").is_none()
}

// ---------------------------------------------------------------------------
// InGameUI residual peels
// ---------------------------------------------------------------------------

/// Retail `InGameUI::MAX_BUILD_PROGRESS` residual.
pub const MAX_BUILD_PROGRESS: u32 = 64;

/// Retail `RadiusCursorType` residual names (matches `TheRadiusCursorNames` / enum).
/// Wave 88 peels COUNT; Wave 110 deepens name-order + build-progress residual.
pub const RADIUS_CURSOR_TYPE_NAMES: &[&str] = &[
    "NONE",
    "ATTACK_DAMAGE_AREA",
    "ATTACK_SCATTER_AREA",
    "ATTACK_CONTINUE_AREA",
    "GUARD_AREA",
    "EMERGENCY_REPAIR",
    "FRIENDLY_SPECIALPOWER",
    "OFFENSIVE_SPECIALPOWER",
    "SUPERWEAPON_SCATTER_AREA",
    "PARTICLECANNON",
    "A10STRIKE",
    "CARPETBOMB",
    "DAISYCUTTER",
    "PARADROP",
    "SPYSATELLITE",
    "SPECTREGUNSHIP",
    "HELIX_NAPALM_BOMB",
    "NUCLEARMISSILE",
    "EMPPULSE",
    "ARTILLERYBARRAGE",
    "NAPALMSTRIKE",
    "CLUSTERMINES",
    "SCUDSTORM",
    "ANTHRAXBOMB",
    "AMBUSH",
    "RADAR",
    "SPYDRONE",
    "FRENZY",
    "CLEARMINES",
    "AMBULANCE",
];

/// Retail `RADIUSCURSOR_COUNT` residual.
pub const RADIUSCURSOR_COUNT: usize = 30;

/// Build-progress slot residual is legal.
pub fn build_progress_slot_legal_residual(slot: u32) -> bool {
    slot < MAX_BUILD_PROGRESS
}

/// Honesty: InGameUI residual pack (build progress + radius cursor order).
pub fn honesty_ingame_ui_residual_wave110() -> bool {
    MAX_BUILD_PROGRESS == 64
        && build_progress_slot_legal_residual(0)
        && build_progress_slot_legal_residual(63)
        && !build_progress_slot_legal_residual(64)
        && RADIUS_CURSOR_TYPE_NAMES.len() == RADIUSCURSOR_COUNT
        && residual_name_index(RADIUS_CURSOR_TYPE_NAMES, "NONE") == Some(0)
        && residual_name_index(RADIUS_CURSOR_TYPE_NAMES, "PARTICLECANNON") == Some(9)
        && residual_name_index(RADIUS_CURSOR_TYPE_NAMES, "EMPPULSE") == Some(18)
        && residual_name_index(RADIUS_CURSOR_TYPE_NAMES, "AMBULANCE") == Some(29)
        && residual_name_index(RADIUS_CURSOR_TYPE_NAMES, "BOGUS").is_none()
}

// ---------------------------------------------------------------------------
// Composite honesty
// ---------------------------------------------------------------------------

/// Wave 110 composite residual honesty pack.
pub fn honesty_message_stream_meta_ingameui_residual_pack_wave110() -> bool {
    honesty_message_stream_marker_residual_wave110()
        && honesty_game_message_argument_type_residual_wave110()
        && honesty_meta_event_category_residual_wave110()
        && honesty_ingame_ui_residual_wave110()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_stream_markers_and_hotkey_squads() {
        assert!(honesty_message_stream_marker_residual_wave110());
    }

    #[test]
    fn game_message_argument_types() {
        assert!(honesty_game_message_argument_type_residual_wave110());
    }

    #[test]
    fn meta_event_categories() {
        assert!(honesty_meta_event_category_residual_wave110());
    }

    #[test]
    fn ingame_ui_build_progress_and_radius_cursor_order() {
        assert!(honesty_ingame_ui_residual_wave110());
    }

    #[test]
    fn wave110_composite_pack() {
        assert!(honesty_message_stream_meta_ingameui_residual_pack_wave110());
    }
}
