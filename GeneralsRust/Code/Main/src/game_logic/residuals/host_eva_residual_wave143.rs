//! Wave 143 residual peels: EVA voice residual
//! (enable/disable/should-play/reset/update; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 142 Beacon residual.
//! Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - Eva.cpp / Eva.ini message table
//! - setShouldPlay / setEnabled / update
//!
//! Fail-closed:
//! - Not full Eva.ini load residual
//! - Not full audio side-sound playback residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// EVA residual tables
// ---------------------------------------------------------------------------

/// Retail EvaMessage name count residual (EVA_MESSAGE_NAMES).
pub const EVA_MESSAGE_COUNT_WAVE143: usize = 53;

/// Core EVA message names residual (first 8 + common combat alerts).
pub const EVA_MESSAGE_NAMES_CORE_WAVE143: &[&str] = &[
    "LOWPOWER",
    "INSUFFICIENTFUNDS",
    "SUPERWEAPONDETECTED_OWN_PARTICLECANNON",
    "SUPERWEAPONDETECTED_OWN_NUKE",
    "SUPERWEAPONDETECTED_OWN_SCUDSTORM",
    "BUILDINGLOST",
    "UNITLOST",
    "GENERALLEVELUP",
];

/// Ordered EVA residual navigation steps.
pub const EVA_NAV_STEPS_WAVE143: &[&str] = &[
    "INITIALIZE_EVA_SYSTEM",
    "SET_EVA_ENABLED",
    "SET_SHOULD_PLAY_LOWPOWER",
    "UPDATE_EVA_SYSTEM",
    "PLAY_SIDE_SOUND_IF_READY",
    "SET_EVA_DISABLED",
    "RESET_EVA_SYSTEM",
];

/// Runtime-host command residual names for EVA peels.
pub const RUNTIME_HOST_EVA_CMD_NAMES_WAVE143: &[&str] = &[
    "click_eva_ok_wnd_enable",
    "click_eva_ok_wnd_disable",
    "click_eva_ok_wnd_should_play",
    "click_eva_ok_wnd_reset",
    "click_eva_ok_wnd_update",
    "click_eva_ok_wnd_prepare",
    "click_eva_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: EVA message name residual pack.
pub fn honesty_eva_message_names_residual_wave143() -> bool {
    EVA_MESSAGE_COUNT_WAVE143 == 53
        && EVA_MESSAGE_NAMES_CORE_WAVE143.len() == 8
        && residual_name_index(EVA_MESSAGE_NAMES_CORE_WAVE143, "LOWPOWER") == Some(0)
        && residual_name_index(EVA_MESSAGE_NAMES_CORE_WAVE143, "INSUFFICIENTFUNDS") == Some(1)
        && residual_name_index(EVA_MESSAGE_NAMES_CORE_WAVE143, "BUILDINGLOST") == Some(5)
        && EVA_MESSAGE_NAMES_CORE_WAVE143
            .iter()
            .all(|n| n.chars().all(|c| c.is_ascii_uppercase() || c == '_'))
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_eva_nav_commands_residual_wave143() -> bool {
    EVA_NAV_STEPS_WAVE143.len() == 7
        && residual_name_index(EVA_NAV_STEPS_WAVE143, "SET_SHOULD_PLAY_LOWPOWER") == Some(2)
        && residual_name_index(EVA_NAV_STEPS_WAVE143, "RESET_EVA_SYSTEM") == Some(6)
        && RUNTIME_HOST_EVA_CMD_NAMES_WAVE143.len() == 7
        && residual_name_index(
            RUNTIME_HOST_EVA_CMD_NAMES_WAVE143,
            "click_eva_ok_wnd_prepare",
        ) == Some(5)
}

/// Wave 143 composite residual honesty pack.
pub fn honesty_eva_residual_pack_wave143() -> bool {
    honesty_eva_message_names_residual_wave143() && honesty_eva_nav_commands_residual_wave143()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_names_residual() {
        assert!(honesty_eva_message_names_residual_wave143());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_eva_nav_commands_residual_wave143());
    }

    #[test]
    fn wave143_composite_pack() {
        assert!(honesty_eva_residual_pack_wave143());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_eva_prepare_low_power_residual_live() {
        use game_client::eva::{
            ResidualEvaAction, residual_eva_is_enabled, residual_eva_last_action,
            residual_eva_last_message_index, simulate_eva_disable,
            simulate_eva_prepare_low_power_alert,
        };
        assert!(
            simulate_eva_prepare_low_power_alert(),
            "enable+LOWPOWER residual must latch"
        );
        assert!(residual_eva_is_enabled());
        assert_eq!(residual_eva_last_message_index(), Some(0));
        assert_eq!(residual_eva_last_action(), ResidualEvaAction::ShouldPlay);
        assert!(simulate_eva_disable());
        assert!(!residual_eva_is_enabled());
        assert_eq!(residual_eva_last_action(), ResidualEvaAction::Disable);
    }
}
