//! Wave 153 residual peels: GameWorld authority env residual
//! (shadow + subsystem authority flags; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 152 ChallengeGenerals residual.
//! Architecture residual — promotes awareness of GameWorld shadow path.
//!
//! Sources (retail ZH architecture + repo policy):
//! - GENERALS_GAMEWORLD_SHADOW / *_AUTHORITY env table
//! - gameworld_shadow.rs default-on authority flags
//!
//! Fail-closed:
//! - Not full GameWorld production authority cutover
//! - Not dual-tick removal residual
//! - Shell `playable_claim` stays false; network deferred

// ---------------------------------------------------------------------------
// Shared residual helpers
// ---------------------------------------------------------------------------

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

// ---------------------------------------------------------------------------
// GameWorld authority residual tables
// ---------------------------------------------------------------------------

/// Retail GENERALS_GAMEWORLD_* env names residual.
/// 2026-08-14: GENERALS_GAMEWORLD_WEAPON_AUTHORITY appended by the Phase 2
/// per-slot weapon authority channel (mirrors couple_guard.rs live table).
pub const GAMEWORLD_AUTHORITY_ENV_NAMES_WAVE153: &[&str] = &[
    "GENERALS_GAMEWORLD_SHADOW",
    "GENERALS_GAMEWORLD_DAMAGE_AUTHORITY",
    "GENERALS_GAMEWORLD_ECONOMY_AUTHORITY",
    "GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY",
    "GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY",
    "GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY",
    "GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY",
    "GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY",
    "GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY",
    "GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY",
    "GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY",
    "GENERALS_GAMEWORLD_WEAPON_AUTHORITY",
    "GENERALS_GAMEWORLD_ENTITY_MODULES",
];

/// Expected env-name table length residual (preview ENTITY_MODULES is default off).
pub const GAMEWORLD_AUTHORITY_DEFAULT_ON_COUNT_WAVE153: usize = 13;

/// GameWorld authority method residual names.
pub const GAMEWORLD_AUTHORITY_METHOD_NAMES_WAVE153: &[&str] = &[
    "gameworld_shadow_enabled",
    "gameworld_damage_authority_enabled",
    "gameworld_economy_authority_enabled",
    "probe_host_vs_gameworld",
    "refresh_gameworld_authority_env_caches",
    "overlay_gameworld_shadow",
];

/// Ordered GameWorld authority residual navigation steps.
pub const GAMEWORLD_AUTHORITY_NAV_STEPS_WAVE153: &[&str] = &[
    "REFRESH_AUTHORITY_ENV_CACHES",
    "CHECK_SHADOW_ENABLED",
    "CHECK_DAMAGE_AUTHORITY",
    "CHECK_ECONOMY_AUTHORITY",
    "PROBE_HOST_VS_GAMEWORLD",
    "OVERLAY_PRESENTATION_SHADOW",
    "SOLE_TICK_SUBSYSTEMS",
];

/// Runtime-host command residual names for GameWorld authority peels.
pub const RUNTIME_HOST_GAMEWORLD_AUTHORITY_CMD_NAMES_WAVE153: &[&str] = &[
    "click_gameworld_authority_ok_wnd_refresh",
    "click_gameworld_authority_ok_wnd_shadow",
    "click_gameworld_authority_ok_wnd_probe",
    "click_gameworld_authority_ok_wnd_prepare",
    "click_gameworld_authority_miss",
];

// ---------------------------------------------------------------------------
// Honesty packs
// ---------------------------------------------------------------------------

/// Honesty: GameWorld authority env names residual pack.
pub fn honesty_gameworld_authority_env_names_residual_wave153() -> bool {
    GAMEWORLD_AUTHORITY_ENV_NAMES_WAVE153.len() == GAMEWORLD_AUTHORITY_DEFAULT_ON_COUNT_WAVE153
        && residual_name_index(
            GAMEWORLD_AUTHORITY_ENV_NAMES_WAVE153,
            "GENERALS_GAMEWORLD_SHADOW",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_ENV_NAMES_WAVE153,
            "GENERALS_GAMEWORLD_DAMAGE_AUTHORITY",
        ) == Some(1)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_ENV_NAMES_WAVE153,
            "GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY",
        ) == Some(10)
        && GAMEWORLD_AUTHORITY_ENV_NAMES_WAVE153
            .iter()
            .all(|n| n.starts_with("GENERALS_GAMEWORLD_"))
}

/// Honesty: method names residual pack.
pub fn honesty_gameworld_authority_method_names_residual_wave153() -> bool {
    GAMEWORLD_AUTHORITY_METHOD_NAMES_WAVE153.len() == 6
        && residual_name_index(
            GAMEWORLD_AUTHORITY_METHOD_NAMES_WAVE153,
            "gameworld_shadow_enabled",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_METHOD_NAMES_WAVE153,
            "probe_host_vs_gameworld",
        ) == Some(3)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_METHOD_NAMES_WAVE153,
            "overlay_gameworld_shadow",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_gameworld_authority_nav_commands_residual_wave153() -> bool {
    GAMEWORLD_AUTHORITY_NAV_STEPS_WAVE153.len() == 7
        && residual_name_index(
            GAMEWORLD_AUTHORITY_NAV_STEPS_WAVE153,
            "REFRESH_AUTHORITY_ENV_CACHES",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_NAV_STEPS_WAVE153,
            "PROBE_HOST_VS_GAMEWORLD",
        ) == Some(4)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_NAV_STEPS_WAVE153,
            "SOLE_TICK_SUBSYSTEMS",
        ) == Some(6)
        && RUNTIME_HOST_GAMEWORLD_AUTHORITY_CMD_NAMES_WAVE153.len() == 5
        && residual_name_index(
            RUNTIME_HOST_GAMEWORLD_AUTHORITY_CMD_NAMES_WAVE153,
            "click_gameworld_authority_ok_wnd_prepare",
        ) == Some(3)
}

/// Wave 153 composite residual honesty pack.
pub fn honesty_gameworld_authority_residual_pack_wave153() -> bool {
    honesty_gameworld_authority_env_names_residual_wave153()
        && honesty_gameworld_authority_method_names_residual_wave153()
        && honesty_gameworld_authority_nav_commands_residual_wave153()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_names_residual() {
        assert!(honesty_gameworld_authority_env_names_residual_wave153());
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_gameworld_authority_method_names_residual_wave153());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_gameworld_authority_nav_commands_residual_wave153());
    }

    #[test]
    fn wave153_composite_pack() {
        assert!(honesty_gameworld_authority_residual_pack_wave153());
    }

    #[test]
    fn simulate_gameworld_authority_prepare_defaults_residual_live() {
        use crate::gameworld_shadow::{
            GAMEWORLD_AUTHORITY_ENV_NAMES, ResidualGameWorldAuthorityAction,
            residual_gameworld_authority_enabled_count, residual_gameworld_authority_last_action,
            residual_gameworld_shadow_enabled_latch, simulate_gameworld_authority_prepare_defaults,
        };
        assert_eq!(
            GAMEWORLD_AUTHORITY_ENV_NAMES.len(),
            GAMEWORLD_AUTHORITY_DEFAULT_ON_COUNT_WAVE153
        );
        assert!(
            simulate_gameworld_authority_prepare_defaults(),
            "default-on authority env residual must latch"
        );
        assert!(residual_gameworld_shadow_enabled_latch());
        assert!(
            residual_gameworld_authority_enabled_count()
                >= GAMEWORLD_AUTHORITY_DEFAULT_ON_COUNT_WAVE153 - 1
        );
        assert_eq!(
            residual_gameworld_authority_last_action(),
            ResidualGameWorldAuthorityAction::ShadowEnableCheck
        );
    }
}
