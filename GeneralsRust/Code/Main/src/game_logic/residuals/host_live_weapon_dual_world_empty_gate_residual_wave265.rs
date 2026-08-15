//! Wave 265 residual peels: Weapon module dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), weapon
//! range/LOS/damage helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 264 Object mod dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/weapon/mod.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Weapon dual-world empty-gate residual method names.
pub const LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE265: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_within_attack_range",
    "is_clear_firing_line_of_sight_terrain",
    "deal_damage",
    "find_objects_in_radius",
    "get_attack_distance",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE265: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_WEAPON_EMPTY_GATES",
    "LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE265: &[&str] = &[
    "click_live_weapon_dual_world_empty_gate_ok_prepare",
    "click_live_weapon_dual_world_empty_gate_ok_live",
    "click_live_weapon_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_weapon_dual_world_empty_gate_method_names_residual_wave265() -> bool {
    LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE265.len() == 7
        && residual_name_index(
            LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE265,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE265,
            "get_attack_distance",
        ) == Some(5)
        && residual_name_index(
            LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE265,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_weapon_dual_world_empty_gate_nav_commands_residual_wave265() -> bool {
    LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE265.len() == 4
        && residual_name_index(
            LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE265,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE265,
            "LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_WEAPON_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE265.len() == 3
}

/// Wave 265 composite residual honesty pack.
pub fn honesty_live_weapon_dual_world_empty_gate_residual_pack_wave265() -> bool {
    honesty_live_weapon_dual_world_empty_gate_method_names_residual_wave265()
        && honesty_live_weapon_dual_world_empty_gate_nav_commands_residual_wave265()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let short = name.trim_start_matches("fn ").trim_end_matches('(');
    crate::game_logic::residuals::harness::last_rust_fn_body(src, short)
}

/// Source residual: Weapon module empty dual-world short-circuits.
pub fn honesty_weapon_dual_world_empty_gate_source() -> bool {
    // 2026-08-15: weapon god-file split — scan WEAPON_SRC, not the facade.
    let g = crate::game_logic::residuals::WEAPON_SRC;
    if !(g.contains("Wave 265")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(range) = fn_body(g, "fn is_within_attack_range(") else {
        return false;
    };
    let Some(damage) = fn_body(g, "fn deal_damage(") else {
        return false;
    };
    let Some(find) = fn_body(g, "fn find_objects_in_radius(") else {
        return false;
    };
    range.contains("dual_world_registry_unavailable")
        && damage.contains("dual_world_registry_unavailable")
        && damage.contains("return 0.0")
        && find.contains("dual_world_registry_unavailable")
        && find.contains("Ok(Vec::new())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_weapon_dual_world_empty_gate_honesty() -> bool {
    honesty_live_weapon_dual_world_empty_gate_residual_pack_wave265()
        && honesty_weapon_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_weapon_dual_world_empty_gate_method_names_residual_wave265());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_weapon_dual_world_empty_gate_nav_commands_residual_wave265());
    }

    #[test]
    fn wave265_composite_pack() {
        assert!(honesty_live_weapon_dual_world_empty_gate_residual_pack_wave265());
    }

    #[test]
    fn weapon_dual_world_empty_gate_sources() {
        assert!(honesty_weapon_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_weapon_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_weapon_dual_world_empty_gate_honesty(),
            "weapon dual-world empty gate residual must latch"
        );
    }
}
