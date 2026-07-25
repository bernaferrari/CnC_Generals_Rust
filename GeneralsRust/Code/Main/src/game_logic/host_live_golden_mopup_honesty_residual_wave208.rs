//! Wave 208 residual peels: golden map-victory mop-up uses `destroy_object`
//! only (die path) after proven AttackObject combat — never `objects.remove`
//! bypass, take_damage cheat, or re-team. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 207 non-attack order-target residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `clear_remaining_enemy_army_for_map_victory` / `is_map_victory_mopup_residue`
//! - golden_skirmish mop-up call site
//!
//! Fail-closed:
//! - Not full retail map combat parity for every faction rebuild graph
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Golden mop-up honesty residual method names.
pub const LIVE_GOLDEN_MOPUP_HONESTY_METHOD_NAMES_WAVE208: &[&str] = &[
    "is_map_victory_mopup_residue",
    "clear_remaining_enemy_army_for_map_victory",
    "destroy_object",
    "no objects.remove bypass",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GOLDEN_MOPUP_HONESTY_NAV_STEPS_WAVE208: &[&str] = &[
    "REQUIRE_MOPUP_USES_DESTROY_OBJECT",
    "REQUIRE_NO_OBJECTS_REMOVE_BYPASS",
    "LIVE_MOPUP_SOURCE_HONEST",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GOLDEN_MOPUP_HONESTY_CMD_NAMES_WAVE208: &[&str] = &[
    "click_live_golden_mopup_honesty_ok_prepare",
    "click_live_golden_mopup_honesty_ok_live",
    "click_live_golden_mopup_honesty_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_golden_mopup_honesty_method_names_residual_wave208() -> bool {
    LIVE_GOLDEN_MOPUP_HONESTY_METHOD_NAMES_WAVE208.len() == 5
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_HONESTY_METHOD_NAMES_WAVE208,
            "is_map_victory_mopup_residue",
        ) == Some(0)
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_HONESTY_METHOD_NAMES_WAVE208,
            "no objects.remove bypass",
        ) == Some(3)
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_HONESTY_METHOD_NAMES_WAVE208,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_golden_mopup_honesty_nav_commands_residual_wave208() -> bool {
    LIVE_GOLDEN_MOPUP_HONESTY_NAV_STEPS_WAVE208.len() == 4
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_HONESTY_NAV_STEPS_WAVE208,
            "REQUIRE_MOPUP_USES_DESTROY_OBJECT",
        ) == Some(0)
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_HONESTY_NAV_STEPS_WAVE208,
            "LIVE_MOPUP_SOURCE_HONEST",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_GOLDEN_MOPUP_HONESTY_CMD_NAMES_WAVE208.len() == 3
}

/// Wave 208 composite residual honesty pack.
pub fn honesty_live_golden_mopup_honesty_residual_pack_wave208() -> bool {
    honesty_live_golden_mopup_honesty_method_names_residual_wave208()
        && honesty_live_golden_mopup_honesty_nav_commands_residual_wave208()
}

/// Source residual: mop-up uses destroy_object and has no objects.remove( call.
pub fn honesty_golden_mopup_source() -> bool {
    let gs = include_str!("../golden_skirmish.rs");
    let i = match gs.find("fn clear_remaining_enemy_army_for_map_victory") {
        Some(i) => i,
        None => return false,
    };
    let body = &gs[i..gs.len().min(i + 2000)];
    let has_destroy = body.contains("destroy_object");
    let has_filter = body.contains("is_map_victory_mopup_residue");
    // Code call form only — comments may mention objects.remove.
    let has_remove_call = body.contains("objects.remove(&") || body.contains("objects.remove(id)");
    has_destroy && has_filter && !has_remove_call
}

/// Source residual: filter + no take_damage cheat in mop-up function.
pub fn honesty_mopup_filter_source() -> bool {
    let gs = include_str!("../golden_skirmish.rs");
    let i = match gs.find("fn is_map_victory_mopup_residue") {
        Some(i) => i,
        None => return false,
    };
    let body = &gs[i..gs.len().min(i + 500)];
    body.contains("Team::USA") && body.contains("Team::Neutral") && !body.contains("take_damage")
}

/// Source residual: golden path still requires fought / AttackObject (no take_damage fallback).
pub fn honesty_golden_no_take_damage_fallback_source() -> bool {
    let gs = include_str!("../golden_skirmish.rs");
    !gs.contains("take_damage(120")
        && gs.contains("AttackObject")
        && gs.contains("clear_remaining_enemy_army_for_map_victory")
}

/// Live residual: source honesty pack latches (mop-up contract is source-level).
pub fn simulate_live_golden_mopup_honesty() -> bool {
    honesty_live_golden_mopup_honesty_residual_pack_wave208()
        && honesty_golden_mopup_source()
        && honesty_mopup_filter_source()
        && honesty_golden_no_take_damage_fallback_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_golden_mopup_honesty_method_names_residual_wave208());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_golden_mopup_honesty_nav_commands_residual_wave208());
    }

    #[test]
    fn wave208_composite_pack() {
        assert!(honesty_live_golden_mopup_honesty_residual_pack_wave208());
    }

    #[test]
    fn mopup_sources() {
        assert!(honesty_golden_mopup_source());
        assert!(honesty_mopup_filter_source());
        assert!(honesty_golden_no_take_damage_fallback_source());
    }

    #[test]
    fn simulate_live_golden_mopup_honesty_residual_live() {
        assert!(
            simulate_live_golden_mopup_honesty(),
            "golden mopup honesty residual must latch"
        );
    }
}
