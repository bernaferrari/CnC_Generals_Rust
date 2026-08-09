//! Wave 116 residual peels: Skirmish slot / faction / AI combo residual
//! (host-testable shell match config path; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 114 MainMenu→Skirmish, Wave 115 map-select,
//! Wave 106 WindowLayout. Host residual only — network deferred.
//!
//! Sources (retail ZH C++):
//! - SkirmishGameOptionsMenu.cpp ComboBoxPlayer/Color/Team/PlayerTemplate
//! - SlotState Open/Closed/EasyAI/MedAI/BrutalAI/Player
//! - Init seeds slot 0 human + slot 1 Medium AI
//!
//! Fail-closed:
//! - Not full live GadgetComboBox selection UI residual
//! - Not full player-template store populate residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// Slot residual tables
// ---------------------------------------------------------------------------

/// Retail `MAX_SLOTS` residual.
pub const SKIRMISH_MAX_SLOTS_WAVE116: usize = 8;

/// Retail `SlotState` residual names (enum order).
pub const SLOT_STATE_NAMES_WAVE116: &[&str] =
    &["Open", "Closed", "EasyAI", "MedAI", "BrutalAI", "Player"];

/// Retail ComboBoxPlayer AI label residual (GUI: tokens).
pub const SKIRMISH_SLOT_AI_LABELS_WAVE116: &[&str] = &[
    "GUI:Open",
    "GUI:Closed",
    "GUI:EasyAI",
    "GUI:MediumAI",
    "GUI:HardAI",
];

/// Retail slot combo control name stems (format with slot index).
pub const SKIRMISH_SLOT_COMBO_STEMS_WAVE116: &[&str] = &[
    "ComboBoxPlayer",
    "ComboBoxColor",
    "ComboBoxPlayerTemplate",
    "ComboBoxTeam",
];

/// Ordered slot-config residual navigation steps.
pub const SKIRMISH_SLOT_CONFIG_NAV_STEPS_WAVE116: &[&str] = &[
    "ENSURE_DEFAULT_SLOTS",
    "SET_SLOT0_HUMAN",
    "SET_SLOT1_MEDIUM_AI",
    "OPTIONAL_AI_DIFFICULTY_OVERRIDE",
    "OPTIONAL_FACTION_TEMPLATE",
    "OPTIONAL_COLOR_TEAM",
    "GBM_SELECTED_BUTTON_START",
];

/// Runtime-host command residual names including slot peels.
pub const RUNTIME_HOST_SLOT_CMD_NAMES_WAVE116: &[&str] = &[
    "click_skirmish_start_ok_wnd_via_slots",
    "click_skirmish_start_ok_wnd_via_map_select_slots",
    "click_skirmish_start_ok_wnd_via_map_select",
    "click_skirmish_start_ok_wnd",
];

/// Build residual combo control name for slot index.
pub fn skirmish_slot_combo_control_name_wave116(stem: &str, index: usize) -> Option<String> {
    if index >= SKIRMISH_MAX_SLOTS_WAVE116 {
        return None;
    }
    if !SKIRMISH_SLOT_COMBO_STEMS_WAVE116.contains(&stem) {
        return None;
    }
    Some(format!("SkirmishGameOptionsMenu.wnd:{stem}{index}"))
}

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: SlotState + AI label residual pack.
pub fn honesty_skirmish_slot_state_residual_wave116() -> bool {
    SLOT_STATE_NAMES_WAVE116.len() == 6
        && residual_name_index(SLOT_STATE_NAMES_WAVE116, "Open") == Some(0)
        && residual_name_index(SLOT_STATE_NAMES_WAVE116, "EasyAI") == Some(2)
        && residual_name_index(SLOT_STATE_NAMES_WAVE116, "MedAI") == Some(3)
        && residual_name_index(SLOT_STATE_NAMES_WAVE116, "BrutalAI") == Some(4)
        && residual_name_index(SLOT_STATE_NAMES_WAVE116, "Player") == Some(5)
        && SKIRMISH_SLOT_AI_LABELS_WAVE116.len() == 5
        && residual_name_index(SKIRMISH_SLOT_AI_LABELS_WAVE116, "GUI:EasyAI") == Some(2)
        && residual_name_index(SKIRMISH_SLOT_AI_LABELS_WAVE116, "GUI:MediumAI") == Some(3)
        && residual_name_index(SKIRMISH_SLOT_AI_LABELS_WAVE116, "GUI:HardAI") == Some(4)
        && SKIRMISH_MAX_SLOTS_WAVE116 == 8
}

/// Honesty: combo control name residual pack.
pub fn honesty_skirmish_slot_combo_names_residual_wave116() -> bool {
    SKIRMISH_SLOT_COMBO_STEMS_WAVE116.len() == 4
        && residual_name_index(SKIRMISH_SLOT_COMBO_STEMS_WAVE116, "ComboBoxPlayer") == Some(0)
        && residual_name_index(SKIRMISH_SLOT_COMBO_STEMS_WAVE116, "ComboBoxTeam") == Some(3)
        && skirmish_slot_combo_control_name_wave116("ComboBoxPlayer", 0)
            == Some("SkirmishGameOptionsMenu.wnd:ComboBoxPlayer0".into())
        && skirmish_slot_combo_control_name_wave116("ComboBoxColor", 1)
            == Some("SkirmishGameOptionsMenu.wnd:ComboBoxColor1".into())
        && skirmish_slot_combo_control_name_wave116("ComboBoxPlayerTemplate", 7)
            == Some("SkirmishGameOptionsMenu.wnd:ComboBoxPlayerTemplate7".into())
        && skirmish_slot_combo_control_name_wave116("ComboBoxPlayer", 8).is_none()
        && skirmish_slot_combo_control_name_wave116("NoSuch", 0).is_none()
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_skirmish_slot_nav_commands_residual_wave116() -> bool {
    SKIRMISH_SLOT_CONFIG_NAV_STEPS_WAVE116.len() == 7
        && residual_name_index(SKIRMISH_SLOT_CONFIG_NAV_STEPS_WAVE116, "SET_SLOT0_HUMAN") == Some(1)
        && residual_name_index(
            SKIRMISH_SLOT_CONFIG_NAV_STEPS_WAVE116,
            "SET_SLOT1_MEDIUM_AI",
        ) == Some(2)
        && residual_name_index(
            SKIRMISH_SLOT_CONFIG_NAV_STEPS_WAVE116,
            "GBM_SELECTED_BUTTON_START",
        ) == Some(6)
        && RUNTIME_HOST_SLOT_CMD_NAMES_WAVE116.len() == 4
        && residual_name_index(
            RUNTIME_HOST_SLOT_CMD_NAMES_WAVE116,
            "click_skirmish_start_ok_wnd_via_slots",
        ) == Some(0)
        && residual_name_index(
            RUNTIME_HOST_SLOT_CMD_NAMES_WAVE116,
            "click_skirmish_start_ok_wnd_via_map_select_slots",
        ) == Some(1)
}

/// Wave 116 composite residual honesty pack.
pub fn honesty_skirmish_slot_config_residual_pack_wave116() -> bool {
    honesty_skirmish_slot_state_residual_wave116()
        && honesty_skirmish_slot_combo_names_residual_wave116()
        && honesty_skirmish_slot_nav_commands_residual_wave116()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_state_residual() {
        assert!(honesty_skirmish_slot_state_residual_wave116());
    }

    #[test]
    fn slot_combo_names_residual() {
        assert!(honesty_skirmish_slot_combo_names_residual_wave116());
    }

    #[test]
    fn slot_nav_commands_residual() {
        assert!(honesty_skirmish_slot_nav_commands_residual_wave116());
    }

    #[test]
    fn wave116_composite_pack() {
        assert!(honesty_skirmish_slot_config_residual_pack_wave116());
    }

    #[cfg(feature = "game_client")]
    #[test]
    fn simulate_default_human_medium_ai_residual_live() {
        assert!(
            game_client::gui::callbacks::simulate_skirmish_default_human_and_medium_ai(),
            "default human+MedAI residual must commit"
        );
        let setup = game_client::gui::get_skirmish_setup();
        let s0 = setup.game_info().game_info().get_slot(0).unwrap();
        let s1 = setup.game_info().game_info().get_slot(1).unwrap();
        assert!(s0.is_human() || matches!(s0.get_state(), game_client::SlotState::Player));
        assert!(s1.is_ai());
        assert!(matches!(s1.get_state(), game_client::SlotState::MedAI));
    }
}
