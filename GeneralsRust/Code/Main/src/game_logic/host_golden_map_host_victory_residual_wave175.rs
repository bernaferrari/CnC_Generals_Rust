//! Wave 175 residual peels: golden map-host victory honesty residual
//! (Wave 451: mop-up default-off; map_host_playable_ok requires proven map
//! combat + combat_no_mopup_ok + combat_no_teleport_ok; never flips shell
//! `playable_claim`).
//!
//! Orthogonal to Wave 174 presentation/GameClient boundary residual.
//! Host residual only — network deferred.
//!
//! Sources (golden_skirmish map path):
//! - `clear_remaining_enemy_army_for_map_victory` gated by mop-up env (default off)
//! - `map_host_playable_ok` fail-closed formula
//! - `synthetic_combat=false` when map combat proven (victory wipe optional)
//!
//! Fail-closed:
//! - Does not run full golden combat inside shell_smoke (gate owns that)
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Golden map-host victory residual method names.
pub const GOLDEN_MAP_HOST_VICTORY_METHOD_NAMES_WAVE175: &[&str] = &[
    "clear_remaining_enemy_army_for_map_victory",
    "map_host_playable_ok",
    "synthetic_combat",
    "combat_no_teleport_ok",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const GOLDEN_MAP_HOST_VICTORY_NAV_STEPS_WAVE175: &[&str] = &[
    "REQUIRE_CLEAR_REMAINING_AFTER_COMBAT",
    "REQUIRE_MAP_HOST_FORMULA",
    "REQUIRE_SYNTHETIC_FALSE_ON_MAP_VICTORY",
    "LIVE_SOURCE_MARKERS",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_GOLDEN_MAP_HOST_VICTORY_CMD_NAMES_WAVE175: &[&str] = &[
    "click_golden_map_host_victory_ok_clear",
    "click_golden_map_host_victory_ok_formula",
    "click_golden_map_host_victory_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_golden_map_host_victory_method_names_residual_wave175() -> bool {
    GOLDEN_MAP_HOST_VICTORY_METHOD_NAMES_WAVE175.len() == 5
        && residual_name_index(
            GOLDEN_MAP_HOST_VICTORY_METHOD_NAMES_WAVE175,
            "clear_remaining_enemy_army_for_map_victory",
        ) == Some(0)
        && residual_name_index(
            GOLDEN_MAP_HOST_VICTORY_METHOD_NAMES_WAVE175,
            "map_host_playable_ok",
        ) == Some(1)
        && residual_name_index(
            GOLDEN_MAP_HOST_VICTORY_METHOD_NAMES_WAVE175,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_golden_map_host_victory_nav_commands_residual_wave175() -> bool {
    GOLDEN_MAP_HOST_VICTORY_NAV_STEPS_WAVE175.len() == 5
        && residual_name_index(
            GOLDEN_MAP_HOST_VICTORY_NAV_STEPS_WAVE175,
            "REQUIRE_CLEAR_REMAINING_AFTER_COMBAT",
        ) == Some(0)
        && residual_name_index(
            GOLDEN_MAP_HOST_VICTORY_NAV_STEPS_WAVE175,
            "LIVE_SOURCE_MARKERS",
        ) == Some(3)
        && RUNTIME_HOST_GOLDEN_MAP_HOST_VICTORY_CMD_NAMES_WAVE175.len() == 3
}

/// Wave 175 composite residual honesty pack.
pub fn honesty_golden_map_host_victory_residual_pack_wave175() -> bool {
    honesty_golden_map_host_victory_method_names_residual_wave175()
        && honesty_golden_map_host_victory_nav_commands_residual_wave175()
}

/// Source residual: mop-up helper exists and is opt-in gated (Wave 451 default off).
pub fn honesty_clear_remaining_enemy_army_source() -> bool {
    let src = include_str!("../golden_skirmish.rs");
    src.contains("fn clear_remaining_enemy_army_for_map_victory")
        && src.contains("maybe_spawn_rebuild_hole")
        && src.contains("GLAHole")
        && src.contains("clear_remaining_enemy_army_for_map_victory(logic)")
        && src.contains("golden_mopup_destroy_enabled")
}

/// Source residual: map_host_playable_ok formula is fail-closed (Wave 451 mop-up-off).
pub fn honesty_map_host_playable_formula_source() -> bool {
    let src = include_str!("../golden_skirmish.rs");
    let i = match src.find("let map_host_playable_ok =") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 900)];
    body.contains("map_loaded")
        && body.contains("outcome.fought")
        && body.contains("outcome.map_combat_ok")
        && body.contains("outcome.combat_no_mopup_ok")
        && body.contains("outcome.combat_no_teleport_ok")
        && body.contains("outcome.combat_realistic_speed_ok")
}

/// Source residual: synthetic_combat false after proven map combat (victory wipe optional).
pub fn honesty_synthetic_combat_false_on_map_victory_source() -> bool {
    let src = include_str!("../golden_skirmish.rs");
    let i = match src.find("let synthetic_combat =") {
        Some(i) => i,
        None => return false,
    };
    let body = &src[i..src.len().min(i + 400)];
    body.contains("map_loaded")
        && body.contains("outcome.map_combat_ok")
        && body.contains("outcome.same_world_production_ok")
        && body.contains("outcome.fought")
}

/// Live residual: source honesty only (full golden owned by golden_skirmish_gate).
pub fn simulate_golden_map_host_victory_honesty() -> bool {
    if !honesty_golden_map_host_victory_residual_pack_wave175() {
        return false;
    }
    if !honesty_clear_remaining_enemy_army_source() {
        return false;
    }
    if !honesty_map_host_playable_formula_source() {
        return false;
    }
    if !honesty_synthetic_combat_false_on_map_victory_source() {
        return false;
    }
    // playable_claim remains forced false in golden source.
    let src = include_str!("../golden_skirmish.rs");
    if !(src.contains("let playable_claim = false") || src.contains("playable_claim: false")) {
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_golden_map_host_victory_method_names_residual_wave175());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_golden_map_host_victory_nav_commands_residual_wave175());
    }

    #[test]
    fn wave175_composite_pack() {
        assert!(honesty_golden_map_host_victory_residual_pack_wave175());
    }

    #[test]
    fn golden_map_host_sources() {
        assert!(honesty_clear_remaining_enemy_army_source());
        assert!(honesty_map_host_playable_formula_source());
        assert!(honesty_synthetic_combat_false_on_map_victory_source());
    }

    #[test]
    fn simulate_golden_map_host_victory_honesty_residual_live() {
        assert!(
            simulate_golden_map_host_victory_honesty(),
            "golden map-host victory mop-up + formula residual must latch"
        );
    }
}
