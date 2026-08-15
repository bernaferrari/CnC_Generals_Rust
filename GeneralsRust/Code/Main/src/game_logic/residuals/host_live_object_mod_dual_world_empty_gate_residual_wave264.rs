//! Wave 264 residual peels: Object module dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), object
//! combat/contain/lookup helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 263 AI mod dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/mod.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Object mod dual-world empty-gate residual method names.
pub const LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE264: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_current_victim",
    "find_enemy_ids_in_radius",
    "fire_current_weapon_at_target",
    "look",
    "defect",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE264: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_OBJECT_MOD_EMPTY_GATES",
    "LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE264: &[&str] = &[
    "click_live_object_mod_dual_world_empty_gate_ok_prepare",
    "click_live_object_mod_dual_world_empty_gate_ok_live",
    "click_live_object_mod_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_object_mod_dual_world_empty_gate_method_names_residual_wave264() -> bool {
    LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE264.len() == 7
        && residual_name_index(
            LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE264,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE264,
            "defect",
        ) == Some(5)
        && residual_name_index(
            LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE264,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_object_mod_dual_world_empty_gate_nav_commands_residual_wave264() -> bool {
    LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE264.len() == 4
        && residual_name_index(
            LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE264,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE264,
            "LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OBJECT_MOD_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE264.len() == 3
}

/// Wave 264 composite residual honesty pack.
pub fn honesty_live_object_mod_dual_world_empty_gate_residual_pack_wave264() -> bool {
    honesty_live_object_mod_dual_world_empty_gate_method_names_residual_wave264()
        && honesty_live_object_mod_dual_world_empty_gate_nav_commands_residual_wave264()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let short = name.trim_start_matches("fn ").trim_end_matches('(');
    crate::game_logic::residuals::harness::last_rust_fn_body(src, short)
}

/// Source residual: Object module empty dual-world short-circuits.
pub fn honesty_object_mod_dual_world_empty_gate_source() -> bool {
    // 2026-08-15: object god-file split — scan OBJECT_SPLIT_SRC.
    let g = crate::game_logic::residuals::OBJECT_SPLIT_SRC;
    if !(g.contains("Wave 264")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(victim) = fn_body(g, "fn get_current_victim(") else {
        return false;
    };
    let Some(enemies) = fn_body(g, "fn find_enemy_ids_in_radius(") else {
        return false;
    };
    let Some(look) = fn_body(g, "fn look(") else {
        return false;
    };
    victim.contains("dual_world_registry_unavailable")
        && enemies.contains("dual_world_registry_unavailable")
        && enemies.contains("Ok(Vec::new())")
        && look.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_object_mod_dual_world_empty_gate_honesty() -> bool {
    honesty_live_object_mod_dual_world_empty_gate_residual_pack_wave264()
        && honesty_object_mod_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_object_mod_dual_world_empty_gate_method_names_residual_wave264());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_object_mod_dual_world_empty_gate_nav_commands_residual_wave264());
    }

    #[test]
    fn wave264_composite_pack() {
        assert!(honesty_live_object_mod_dual_world_empty_gate_residual_pack_wave264());
    }

    #[test]
    fn object_mod_dual_world_empty_gate_sources() {
        assert!(honesty_object_mod_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_object_mod_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_object_mod_dual_world_empty_gate_honesty(),
            "object mod dual-world empty gate residual must latch"
        );
    }
}
