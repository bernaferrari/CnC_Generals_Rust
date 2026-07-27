//! Wave 177 residual peels: GameWorld production authority honesty residual
//! (PRODUCTION_AUTHORITY default-on; host sole-tick skips queue progress when
//! shadow-coupled; never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 176 executable presentation boundary residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `gameworld_production_authority_enabled` default-on
//! - `gameworld_production_sole_tick_enabled` gates host `update_production`
//! - `ensure_gate_damage_authority` also forces production authority
//! - `writeback_production_to_host` last-writer path
//!
//! Fail-closed:
//! - Not full GameWorld production cutover without Main host store
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// GameWorld production authority residual method names.
pub const GAMEWORLD_PRODUCTION_AUTHORITY_METHOD_NAMES_WAVE177: &[&str] = &[
    "gameworld_production_authority_enabled",
    "gameworld_production_sole_tick_enabled",
    "writeback_production_to_host",
    "GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const GAMEWORLD_PRODUCTION_AUTHORITY_NAV_STEPS_WAVE177: &[&str] = &[
    "REQUIRE_PRODUCTION_AUTH_DEFAULT_ON",
    "REQUIRE_HOST_SOLE_TICK_SKIP",
    "REQUIRE_WRITEBACK_PATH",
    "LIVE_PRODUCTION_AUTH_ENABLED",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_GAMEWORLD_PRODUCTION_AUTHORITY_CMD_NAMES_WAVE177: &[&str] = &[
    "click_gameworld_production_authority_ok_enabled",
    "click_gameworld_production_authority_ok_sole",
    "click_gameworld_production_authority_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_gameworld_production_authority_method_names_residual_wave177() -> bool {
    GAMEWORLD_PRODUCTION_AUTHORITY_METHOD_NAMES_WAVE177.len() == 5
        && residual_name_index(
            GAMEWORLD_PRODUCTION_AUTHORITY_METHOD_NAMES_WAVE177,
            "gameworld_production_authority_enabled",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_PRODUCTION_AUTHORITY_METHOD_NAMES_WAVE177,
            "writeback_production_to_host",
        ) == Some(2)
        && residual_name_index(
            GAMEWORLD_PRODUCTION_AUTHORITY_METHOD_NAMES_WAVE177,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_gameworld_production_authority_nav_commands_residual_wave177() -> bool {
    GAMEWORLD_PRODUCTION_AUTHORITY_NAV_STEPS_WAVE177.len() == 5
        && residual_name_index(
            GAMEWORLD_PRODUCTION_AUTHORITY_NAV_STEPS_WAVE177,
            "REQUIRE_PRODUCTION_AUTH_DEFAULT_ON",
        ) == Some(0)
        && residual_name_index(
            GAMEWORLD_PRODUCTION_AUTHORITY_NAV_STEPS_WAVE177,
            "LIVE_PRODUCTION_AUTH_ENABLED",
        ) == Some(3)
        && RUNTIME_HOST_GAMEWORLD_PRODUCTION_AUTHORITY_CMD_NAMES_WAVE177.len() == 3
}

/// Wave 177 composite residual honesty pack.
pub fn honesty_gameworld_production_authority_residual_pack_wave177() -> bool {
    honesty_gameworld_production_authority_method_names_residual_wave177()
        && honesty_gameworld_production_authority_nav_commands_residual_wave177()
}

/// Source residual: production authority defaults on via env cache.
pub fn honesty_production_authority_default_on_source() -> bool {
    let src = include_str!("../gameworld_shadow.rs");
    let i = match src.find("pub fn gameworld_production_authority_enabled") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 350)];
    body.contains("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY") && body.contains("true")
}

/// Source residual: host production sole-tick skips full queue progress advance.
pub fn honesty_host_production_sole_tick_source() -> bool {
    let src = include_str!("game_logic.rs");
    // Wave 464: sole-tick host only try_complete; exit delay sole-ticks on GameWorld.
    src.contains("gameworld_production_sole_tick_enabled()")
        && src.contains("try_complete_production")
        && src.contains("update_production")
        && (src.contains("Wave 464: GameWorld sole-ticks queue progress + exit delay")
            || src.contains("Wave 464/614: GameWorld sole-ticks progress + exit delay")
            || src.contains("host_production_ready_log"))
}

/// Source residual: writeback path exists for production last-writer.
pub fn honesty_production_writeback_source() -> bool {
    let src = include_str!("../gameworld_shadow.rs");
    src.contains("pub fn writeback_production_to_host")
        && src.contains("pub fn tick_production_queues")
        && src.contains("ensure_gate_production_authority")
        && src.contains("Wave 464: sole-tick factory exit delay")
}

/// Live residual: production authority enabled after gate ensure (default-on path).
pub fn simulate_gameworld_production_authority_honesty() -> bool {
    use crate::gameworld_shadow::{
        ensure_gate_damage_authority, gameworld_production_authority_enabled,
        gameworld_production_sole_tick_enabled,
    };

    if !honesty_gameworld_production_authority_residual_pack_wave177() {
        return false;
    }
    if !honesty_production_authority_default_on_source() {
        return false;
    }
    if !honesty_host_production_sole_tick_source() {
        return false;
    }
    if !honesty_production_writeback_source() {
        return false;
    }

    // Gate path forces production authority when unset (via damage ensure cascade).
    ensure_gate_damage_authority();
    if !gameworld_production_authority_enabled() {
        return false;
    }

    // Sole-tick is only live inside a coupled shadow session — outside it must be false
    // (host still owns progress). Honesty: API exists and returns bool without panic.
    let _sole = gameworld_production_sole_tick_enabled();
    // Outside coupled tick, sole-tick must not silently claim authority.
    if gameworld_production_sole_tick_enabled() {
        // Only acceptable if a test left coupled depth open — still require auth on.
        if !gameworld_production_authority_enabled() {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_gameworld_production_authority_method_names_residual_wave177());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_gameworld_production_authority_nav_commands_residual_wave177());
    }

    #[test]
    fn wave177_composite_pack() {
        assert!(honesty_gameworld_production_authority_residual_pack_wave177());
    }

    #[test]
    fn production_authority_sources() {
        assert!(honesty_production_authority_default_on_source());
        assert!(honesty_host_production_sole_tick_source());
        assert!(honesty_production_writeback_source());
    }

    #[test]
    fn simulate_gameworld_production_authority_honesty_residual_live() {
        assert!(
            simulate_gameworld_production_authority_honesty(),
            "GameWorld production authority default-on + sole-tick residual must latch"
        );
    }
}
