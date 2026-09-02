//! Wave 178 residual peels: GameWorld sole-tick coupling honesty residual
//! (coupled tick depth gates production sole-tick; movement authority default-on;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 177 production authority residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `begin_shadow_coupled_tick` / `end_shadow_coupled_tick`
//! - `gameworld_production_sole_tick_enabled` requires coupled depth
//! - `gameworld_movement_authority_enabled` default-on
//!
//! Fail-closed:
//! - Not full multi-subsystem sole-tick of every host phase
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Sole-tick coupling residual method names.
pub const GAMEWORLD_SOLE_TICK_COUPLING_METHOD_NAMES_WAVE178: &[&str] = &[
    "begin_shadow_coupled_tick",
    "end_shadow_coupled_tick",
    "gameworld_production_sole_tick_enabled",
    "gameworld_movement_authority_enabled",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const GAMEWORLD_SOLE_TICK_COUPLING_NAV_STEPS_WAVE178: &[&str] = &[
    "REQUIRE_COUPLED_TICK_API",
    "REQUIRE_SOLE_TICK_NEEDS_COUPLING",
    "REQUIRE_MOVEMENT_AUTH_DEFAULT_ON",
    "LIVE_COUPLE_ENABLES_SOLE_TICK",
    "LIVE_UNCOUPLE_DISABLES_SOLE_TICK",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_GAMEWORLD_SOLE_TICK_COUPLING_CMD_NAMES_WAVE178: &[&str] = &[
    "click_gameworld_sole_tick_coupling_ok_couple",
    "click_gameworld_sole_tick_coupling_ok_uncouple",
    "click_gameworld_sole_tick_coupling_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_gameworld_sole_tick_coupling_method_names_residual_wave178() -> bool {
    GAMEWORLD_SOLE_TICK_COUPLING_METHOD_NAMES_WAVE178.len() == 5
        && residual_name_index(
            GAMEWORLD_SOLE_TICK_COUPLING_METHOD_NAMES_WAVE178,
            "begin_shadow_coupled_tick",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_SOLE_TICK_COUPLING_METHOD_NAMES_WAVE178,
            "gameworld_movement_authority_enabled",
        ) == Some(3)
        && residual_name_index(
            GAMEWORLD_SOLE_TICK_COUPLING_METHOD_NAMES_WAVE178,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_gameworld_sole_tick_coupling_nav_commands_residual_wave178() -> bool {
    GAMEWORLD_SOLE_TICK_COUPLING_NAV_STEPS_WAVE178.len() == 5
        && residual_name_index(
            GAMEWORLD_SOLE_TICK_COUPLING_NAV_STEPS_WAVE178,
            "REQUIRE_COUPLED_TICK_API",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_SOLE_TICK_COUPLING_NAV_STEPS_WAVE178,
            "LIVE_COUPLE_ENABLES_SOLE_TICK",
        ) == Some(3)
        && RUNTIME_HOST_GAMEWORLD_SOLE_TICK_COUPLING_CMD_NAMES_WAVE178.len() == 3
}

/// Wave 178 composite residual honesty pack.
pub fn honesty_gameworld_sole_tick_coupling_residual_pack_wave178() -> bool {
    honesty_gameworld_sole_tick_coupling_method_names_residual_wave178()
        && honesty_gameworld_sole_tick_coupling_nav_commands_residual_wave178()
}

/// Source residual: coupled tick enter/exit + sole-tick predicate.
pub fn honesty_coupled_tick_api_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    src.contains("pub fn begin_shadow_coupled_tick")
        && src.contains("pub fn end_shadow_coupled_tick")
        && src.contains("pub fn shadow_coupled_tick_active")
        && src.contains("pub fn gameworld_production_sole_tick_enabled")
}

/// Source residual: sole-tick requires shadow enabled + coupled depth + production auth.
pub fn honesty_sole_tick_requires_coupling_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let i = match src.find("pub fn gameworld_production_sole_tick_enabled") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 350)];
    body.contains("gameworld_production_authority_enabled")
        && body.contains("gameworld_shadow_enabled")
        && body.contains("shadow_coupled_tick_active")
}

/// Source residual: movement last-writer is a per-`GameLogic` context field
/// (hq-e84zk retired the `GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY` env flag);
/// default off — host integrates pose (C++ single store).
pub fn honesty_movement_authority_default_on_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let ctx = include_str!("../../game_logic/game_logic/gameworld_authority.rs");
    let i = match src.find("pub fn gameworld_movement_authority_enabled") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 300)];
    body.contains("current_gameworld_authority().movement")
        && ctx.contains("pub const DEFAULT_OFF: GameWorldAuthority")
        && ctx.contains("pub movement: bool")
        && ctx.contains("movement: false")
}

/// Live residual: last-writer off; sole-tick stays off even when coupled.
pub fn simulate_gameworld_sole_tick_coupling_honesty() -> bool {
    use crate::gameworld_shadow::{
        begin_shadow_coupled_tick, end_shadow_coupled_tick, ensure_gate_damage_authority,
        gameworld_movement_authority_enabled, gameworld_production_authority_enabled,
        gameworld_production_sole_tick_enabled, gameworld_shadow_enabled,
        shadow_coupled_tick_active,
    };

    if !honesty_gameworld_sole_tick_coupling_residual_pack_wave178() {
        return false;
    }
    if !honesty_coupled_tick_api_source() {
        return false;
    }
    if !honesty_sole_tick_requires_coupling_source() {
        return false;
    }
    if !honesty_movement_authority_default_on_source() {
        return false;
    }

    ensure_gate_damage_authority();
    // Deep-reader barrier: this harness owns no GameLogic instance, so
    // publish the all-off context explicitly instead of trusting whatever a
    // prior test left on the thread (fresh-instance parity).
    crate::game_logic::game_logic::gameworld_authority::publish_gameworld_authority(
        crate::game_logic::game_logic::gameworld_authority::GameWorldAuthority::DEFAULT_OFF,
    );
    if gameworld_production_authority_enabled() {
        return false;
    }
    if gameworld_movement_authority_enabled() {
        return false;
    }

    for _ in 0..8 {
        if !shadow_coupled_tick_active() {
            break;
        }
        end_shadow_coupled_tick();
    }
    if shadow_coupled_tick_active() {
        return false;
    }
    if gameworld_production_sole_tick_enabled() {
        return false;
    }

    begin_shadow_coupled_tick();
    let coupled_ok = shadow_coupled_tick_active();
    let sole_when_coupled = gameworld_production_sole_tick_enabled();
    end_shadow_coupled_tick();

    if !coupled_ok {
        return false;
    }
    // Last-writer off: coupling must not claim sole-tick.
    if sole_when_coupled {
        return false;
    }
    if shadow_coupled_tick_active() {
        return false;
    }
    if gameworld_production_sole_tick_enabled() {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_gameworld_sole_tick_coupling_method_names_residual_wave178());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_gameworld_sole_tick_coupling_nav_commands_residual_wave178());
    }

    #[test]
    fn wave178_composite_pack() {
        assert!(honesty_gameworld_sole_tick_coupling_residual_pack_wave178());
    }

    #[test]
    fn sole_tick_coupling_sources() {
        assert!(honesty_coupled_tick_api_source());
        assert!(honesty_sole_tick_requires_coupling_source());
        assert!(honesty_movement_authority_default_on_source());
    }

    #[test]
    fn simulate_gameworld_sole_tick_coupling_honesty_residual_live() {
        assert!(
            simulate_gameworld_sole_tick_coupling_honesty(),
            "coupled tick must gate production sole-tick residual"
        );
    }
}
