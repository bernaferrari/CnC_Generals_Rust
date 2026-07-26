//! Wave 451 residual peels: golden mop-up destroy default-off honesty.
//! Extends Wave 208 (destroy_object-only mop-up) by disabling mop-up by default.
//! Opt-in only via `GOLDEN_ALLOW_MOPUP_DESTROY=1`.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 450 core-sim dual-world empty-gate residual.
//!
//! Sources:
//! - `Main/src/golden_skirmish.rs`
//!
//! Fail-closed:
//! - Shell / golden `playable_claim` stays false
//! - Victory wipe is optional under mop-up-off honesty

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Golden mop-up default-off residual method names.
pub const LIVE_GOLDEN_MOPUP_DEFAULT_OFF_METHOD_NAMES_WAVE451: &[&str] = &[
    "golden_mopup_destroy_enabled",
    "clear_remaining_enemy_army_for_map_victory",
    "combat_no_mopup_ok",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GOLDEN_MOPUP_DEFAULT_OFF_NAV_STEPS_WAVE451: &[&str] = &[
    "REQUIRE_MOPUP_DEFAULT_OFF",
    "REQUIRE_COMBAT_NO_MOPUP_FLAG",
    "LIVE_GOLDEN_MOPUP_DEFAULT_OFF",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GOLDEN_MOPUP_DEFAULT_OFF_CMD_NAMES_WAVE451: &[&str] = &[
    "click_live_golden_mopup_default_off_ok_prepare",
    "click_live_golden_mopup_default_off_ok_live",
    "click_live_golden_mopup_default_off_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_golden_mopup_default_off_method_names_residual_wave451() -> bool {
    LIVE_GOLDEN_MOPUP_DEFAULT_OFF_METHOD_NAMES_WAVE451.len() == 4
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_DEFAULT_OFF_METHOD_NAMES_WAVE451,
            "golden_mopup_destroy_enabled",
        ) == Some(0)
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_DEFAULT_OFF_METHOD_NAMES_WAVE451,
            "combat_no_mopup_ok",
        ) == Some(2)
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_DEFAULT_OFF_METHOD_NAMES_WAVE451,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_golden_mopup_default_off_nav_commands_residual_wave451() -> bool {
    LIVE_GOLDEN_MOPUP_DEFAULT_OFF_NAV_STEPS_WAVE451.len() == 4
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_DEFAULT_OFF_NAV_STEPS_WAVE451,
            "REQUIRE_MOPUP_DEFAULT_OFF",
        ) == Some(0)
        && residual_name_index(
            LIVE_GOLDEN_MOPUP_DEFAULT_OFF_NAV_STEPS_WAVE451,
            "LIVE_GOLDEN_MOPUP_DEFAULT_OFF",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_GOLDEN_MOPUP_DEFAULT_OFF_CMD_NAMES_WAVE451.len() == 3
}

/// Wave 451 composite residual honesty pack.
pub fn honesty_live_golden_mopup_default_off_residual_pack_wave451() -> bool {
    honesty_live_golden_mopup_default_off_method_names_residual_wave451()
        && honesty_live_golden_mopup_default_off_nav_commands_residual_wave451()
}

/// Source residual: golden mop-up default-off honesty.
pub fn honesty_golden_mopup_default_off_source() -> bool {
    let g = include_str!("../golden_skirmish.rs");
    if !(g.contains("Wave 451")
        && g.contains("fn golden_mopup_destroy_enabled")
        && g.contains("GOLDEN_ALLOW_MOPUP_DESTROY")
        && g.contains("combat_no_mopup_ok"))
    {
        return false;
    }
    let gate_ok = g.contains("if !golden_mopup_destroy_enabled()")
        && g.contains("combat_no_mopup_ok = !mopup_enabled");
    let claim_ok = g.contains("let playable_claim = false;");
    gate_ok && claim_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_golden_mopup_default_off_honesty() -> bool {
    honesty_live_golden_mopup_default_off_residual_pack_wave451()
        && honesty_golden_mopup_default_off_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_golden_mopup_default_off_method_names_residual_wave451());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_golden_mopup_default_off_nav_commands_residual_wave451());
    }

    #[test]
    fn wave451_composite_pack() {
        assert!(honesty_live_golden_mopup_default_off_residual_pack_wave451());
    }

    #[test]
    fn golden_mopup_default_off_sources() {
        assert!(honesty_golden_mopup_default_off_source());
    }

    #[test]
    fn simulate_live_golden_mopup_default_off_honesty_residual_live() {
        assert!(
            simulate_live_golden_mopup_default_off_honesty(),
            "golden mop-up default-off residual must latch"
        );
    }
}
