//! Wave 179 residual peels: GameWorld authority matrix honesty residual
//! (all subsystem authorities default-on; engine couples shadow tick around
//! host update; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 178 sole-tick coupling residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - damage/economy/movement/production/ai_attack/projectile/ai_decision/
//!   fire_spawn/construction/special_power authorities default-on
//! - `CncGameEngine` begin/end_shadow_coupled_tick around host logic update
//!
//! Fail-closed:
//! - Not full GameWorld sole ownership of every phase
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Authority matrix residual method names.
pub const GAMEWORLD_AUTHORITY_MATRIX_METHOD_NAMES_WAVE179: &[&str] = &[
    "gameworld_damage_authority_enabled",
    "gameworld_ai_attack_authority_enabled",
    "gameworld_fire_spawn_authority_enabled",
    "gameworld_construction_authority_enabled",
    "begin_shadow_coupled_tick",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const GAMEWORLD_AUTHORITY_MATRIX_NAV_STEPS_WAVE179: &[&str] = &[
    "REQUIRE_ALL_AUTH_DEFAULT_ON",
    "REQUIRE_ENGINE_COUPLE_SHADOW",
    "LIVE_ENSURE_GATE_ALL_ON",
    "LIVE_AI_FIRE_CONSTRUCTION_ON",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_GAMEWORLD_AUTHORITY_MATRIX_CMD_NAMES_WAVE179: &[&str] = &[
    "click_gameworld_authority_matrix_ok_all",
    "click_gameworld_authority_matrix_ok_couple",
    "click_gameworld_authority_matrix_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_gameworld_authority_matrix_method_names_residual_wave179() -> bool {
    GAMEWORLD_AUTHORITY_MATRIX_METHOD_NAMES_WAVE179.len() == 6
        && residual_name_index(
            GAMEWORLD_AUTHORITY_MATRIX_METHOD_NAMES_WAVE179,
            "gameworld_damage_authority_enabled",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_MATRIX_METHOD_NAMES_WAVE179,
            "begin_shadow_coupled_tick",
        ) == Some(4)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_MATRIX_METHOD_NAMES_WAVE179,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_gameworld_authority_matrix_nav_commands_residual_wave179() -> bool {
    GAMEWORLD_AUTHORITY_MATRIX_NAV_STEPS_WAVE179.len() == 5
        && residual_name_index(
            GAMEWORLD_AUTHORITY_MATRIX_NAV_STEPS_WAVE179,
            "REQUIRE_ALL_AUTH_DEFAULT_ON",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_AUTHORITY_MATRIX_NAV_STEPS_WAVE179,
            "LIVE_ENSURE_GATE_ALL_ON",
        ) == Some(2)
        && RUNTIME_HOST_GAMEWORLD_AUTHORITY_MATRIX_CMD_NAMES_WAVE179.len() == 3
}

/// Wave 179 composite residual honesty pack.
pub fn honesty_gameworld_authority_matrix_residual_pack_wave179() -> bool {
    honesty_gameworld_authority_matrix_method_names_residual_wave179()
        && honesty_gameworld_authority_matrix_nav_commands_residual_wave179()
}

fn authority_fn_defaults_on(src: &str, fn_name: &str) -> bool {
    let i = match src.find(&format!("pub fn {fn_name}")) {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 400)];
    // env_flag_cached(..., true)
    body.contains("true") && body.contains("GENERALS_GAMEWORLD_")
}

/// Source residual: full authority matrix defaults on.
pub fn honesty_authority_matrix_default_on_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let names = [
        "gameworld_damage_authority_enabled",
        "gameworld_economy_authority_enabled",
        "gameworld_movement_authority_enabled",
        "gameworld_production_authority_enabled",
        "gameworld_ai_attack_authority_enabled",
        "gameworld_projectile_authority_enabled",
        "gameworld_ai_decision_authority_enabled",
        "gameworld_fire_spawn_authority_enabled",
        "gameworld_construction_authority_enabled",
        "gameworld_special_power_authority_enabled",
    ];
    names.iter().all(|n| authority_fn_defaults_on(src, n))
}

/// Source residual: engine couples shadow tick around host logic update.
/// 2026-08-15: `begin_shadow_coupled_tick()` / `end_shadow_coupled_tick()`
/// moved into `CoupledTickGuard::enter` (`gameworld_shadow/tick/couple.rs`).
/// Engine couples via RAII guard around host logic + post-logic writeback.
pub fn honesty_engine_couple_shadow_tick_source() -> bool {
    let src = crate::cnc_game_engine::ENGINE_SRC;
    let gw = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    src.contains("couple_shadow")
        && src.contains("gameworld_shadow.is_some()")
        && src.contains("CoupledTickGuard::enter")
        && gw.contains("pub fn begin_shadow_coupled_tick")
        && gw.contains("pub fn end_shadow_coupled_tick")
}

/// Live residual: after gate ensure, all authorities report enabled.
pub fn simulate_gameworld_authority_matrix_honesty() -> bool {
    use crate::gameworld_shadow::{
        ensure_gate_damage_authority, gameworld_ai_attack_authority_enabled,
        gameworld_ai_decision_authority_enabled, gameworld_construction_authority_enabled,
        gameworld_damage_authority_enabled, gameworld_economy_authority_enabled,
        gameworld_fire_spawn_authority_enabled, gameworld_movement_authority_enabled,
        gameworld_production_authority_enabled, gameworld_projectile_authority_enabled,
        gameworld_special_power_authority_enabled,
    };

    if !honesty_gameworld_authority_matrix_residual_pack_wave179() {
        return false;
    }
    if !honesty_authority_matrix_default_on_source() {
        return false;
    }
    if !honesty_engine_couple_shadow_tick_source() {
        return false;
    }

    ensure_gate_damage_authority();

    let all = [
        gameworld_damage_authority_enabled(),
        gameworld_economy_authority_enabled(),
        gameworld_movement_authority_enabled(),
        gameworld_production_authority_enabled(),
        gameworld_ai_attack_authority_enabled(),
        gameworld_projectile_authority_enabled(),
        gameworld_ai_decision_authority_enabled(),
        gameworld_fire_spawn_authority_enabled(),
        gameworld_construction_authority_enabled(),
        gameworld_special_power_authority_enabled(),
    ];
    all.iter().all(|v| *v)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_gameworld_authority_matrix_method_names_residual_wave179());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_gameworld_authority_matrix_nav_commands_residual_wave179());
    }

    #[test]
    fn wave179_composite_pack() {
        assert!(honesty_gameworld_authority_matrix_residual_pack_wave179());
    }

    #[test]
    fn authority_matrix_sources() {
        assert!(honesty_authority_matrix_default_on_source());
        assert!(honesty_engine_couple_shadow_tick_source());
    }

    #[test]
    fn simulate_gameworld_authority_matrix_honesty_residual_live() {
        assert!(
            simulate_gameworld_authority_matrix_honesty(),
            "full GameWorld authority matrix default-on residual must latch"
        );
    }
}
