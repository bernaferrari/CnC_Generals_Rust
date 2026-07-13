//! Host China Hacker DisableBuilding residual combat polish.
//!
//! Residual slice (playability):
//! - `ChinaInfantryHacker` / Tank_/Nuke_ / Test variants can issue
//!   `SpecialAbilityHackerDisableBuilding` residual:
//!   walk to enemy structure within StartAbilityRange **150** → apply
//!   DISABLED_HACKED for EffectDuration **2000**ms → **60** logic frames.
//! - Disabled structures count as `is_disabled()` so residual production stops
//!   (same path as microwave subdued / EMP).
//! - Internet cash residual remains in `host_hacker_income` (not re-opened).
//!
//! Fail-closed honesty:
//! - Not full SpecialAbilityUpdate UnpackTime 7300 / PackTime 5133 / PreparationTime
//!   3000 / PersistentPrepTime 333 continuous refresh stream matrix
//! - Not full BinaryDataStream special object / DisableFX particle interleave
//! - Not network disable-building replication (network deferred)

use crate::game_logic::host_hacker_income::is_hacker_template;

// Re-export template matcher for integration call sites.
pub use crate::game_logic::host_hacker_income::is_hacker_template as is_hacker_disable_unit;

/// Retail special power template.
pub const SPECIAL_ABILITY_HACKER_DISABLE_BUILDING: &str = "SpecialAbilityHackerDisableBuilding";

/// SpecialAbilityUpdate StartAbilityRange residual.
pub const HACKER_DISABLE_START_ABILITY_RANGE: f32 = 150.0;

/// C++ SpecialAbilityUpdate EffectDuration = 2000 ms for
/// SpecialAbilityHackerDisableBuilding (2 seconds at 30 FPS logic).
pub const HACKER_DISABLE_EFFECT_DURATION_MS: u32 = 2_000;

/// Logic-frame residual of EffectDuration (ms * 30 / 1000).
pub const HACKER_DISABLE_EFFECT_DURATION_FRAMES: u32 =
    (HACKER_DISABLE_EFFECT_DURATION_MS * 30) / 1000;

/// Residual audio when hacker disables a building.
pub const HACKER_DISABLE_BUILDING_AUDIO: &str = "HackerDisableBuilding";

/// Whether residual unit can issue DisableBuilding special.
pub fn can_activate_hacker_disable_building(is_hacker: bool, is_alive: bool) -> bool {
    is_hacker && is_alive
}

/// Whether target is within StartAbilityRange residual.
pub fn hacker_disable_in_start_range(distance: f32) -> bool {
    distance <= HACKER_DISABLE_START_ABILITY_RANGE
}

/// Legal residual DisableBuilding target (enemy structure, not under construction).
pub fn is_legal_hacker_disable_target(
    is_alive: bool,
    is_structure: bool,
    under_construction: bool,
    is_enemy: bool,
    already_hacked: bool,
) -> bool {
    is_alive && is_structure && !under_construction && is_enemy && !already_hacked
}

/// Absolute expiry frame for residual disable (now + EffectDuration frames).
pub fn hacker_disable_until_frame(current_frame: u32) -> u32 {
    current_frame.saturating_add(HACKER_DISABLE_EFFECT_DURATION_FRAMES)
}

/// Whether residual path should apply for this template.
pub fn should_apply_hacker_disable(template_name: &str) -> bool {
    is_hacker_template(template_name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_and_range() {
        assert_eq!(HACKER_DISABLE_EFFECT_DURATION_FRAMES, 60);
        assert!(hacker_disable_in_start_range(150.0));
        assert!(hacker_disable_in_start_range(0.0));
        assert!(!hacker_disable_in_start_range(150.1));
        assert_eq!(hacker_disable_until_frame(100), 160);
    }

    #[test]
    fn legal_target_matrix() {
        assert!(is_legal_hacker_disable_target(true, true, false, true, false));
        assert!(!is_legal_hacker_disable_target(false, true, false, true, false));
        assert!(!is_legal_hacker_disable_target(true, false, false, true, false));
        assert!(!is_legal_hacker_disable_target(true, true, true, true, false));
        assert!(!is_legal_hacker_disable_target(true, true, false, false, false));
        assert!(!is_legal_hacker_disable_target(true, true, false, true, true));
    }

    #[test]
    fn unit_names() {
        assert!(should_apply_hacker_disable("ChinaInfantryHacker"));
        assert!(should_apply_hacker_disable("Tank_ChinaInfantryHacker"));
        assert!(should_apply_hacker_disable("Nuke_ChinaInfantryHacker"));
        assert!(should_apply_hacker_disable("TestHacker"));
        assert!(!should_apply_hacker_disable("ChinaInfantryBlackLotus"));
        assert!(!should_apply_hacker_disable("ChinaTankBattleMaster"));
        assert!(can_activate_hacker_disable_building(true, true));
        assert!(!can_activate_hacker_disable_building(true, false));
        assert!(!can_activate_hacker_disable_building(false, true));
    }
}
