//! Wave 265 residual peels: Weapon module dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), weapon
//! ID-based range/damage/radius helpers fail-closed without dual-world factory walks.
//! An explicitly borrowed range source is accepted before the registry gate.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 264 Object mod dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/weapon/helpers.rs` registry availability
//! - `GameLogic/src/weapon/weapon_range.rs` range source resolution
//! - `GameLogic/src/weapon/weapon_instance_combat.rs` damage and radius queries
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

/// Source contract for the canonical ID adapters and borrowed range source.
/// This checks declaration/delegation paths, not executable gameplay parity.
fn weapon_empty_gate_source_contract(g: &str) -> bool {
    let Some(helper) = fn_body(g, "fn dual_world_registry_unavailable(") else {
        return false;
    };
    let Some(range) = fn_body(g, "fn is_within_attack_range(") else {
        return false;
    };
    let Some(source) = fn_body(g, "fn range_source(") else {
        return false;
    };
    let Some(damage) = fn_body(g, "fn deal_damage(") else {
        return false;
    };
    let Some(find) = fn_body(g, "fn find_objects_in_radius(") else {
        return false;
    };
    let Some(find_with_source) = fn_body(g, "fn find_objects_in_radius_with_source(") else {
        return false;
    };
    let Some(borrowed_at) = source.find("self.caller_source(id)") else {
        return false;
    };
    let Some(empty_at) = source.find("dual_world_registry_unavailable()") else {
        return false;
    };
    g.contains("mod weapon_range;")
        && g.contains("mod weapon_instance_combat;")
        && helper.contains("OBJECT_REGISTRY.is_empty()")
        && range.contains("self.range_source(source_obj)")
        && range.contains("return false")
        && range.contains("self.is_within_attack_range_from_source(")
        && borrowed_at < empty_at
        && source.contains("return Some((source.position, source.geometry))")
        && source.contains("return None")
        && source.contains("OBJECT_REGISTRY.with_object(id")
        && damage.contains("dual_world_registry_unavailable()")
        && damage.contains("return 0.0")
        && find.contains(
            "self.find_objects_in_radius_with_source(source_obj_id, center, radius, None)",
        )
        && find_with_source.contains("dual_world_registry_unavailable()")
        && find_with_source.contains("Ok(Vec::new())")
}

/// Source residual: canonical Weapon adapters preserve empty-registry results.
pub fn honesty_weapon_dual_world_empty_gate_source() -> bool {
    weapon_empty_gate_source_contract(crate::game_logic::residuals::WEAPON_SRC)
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
    fn canonical_range_contract_requires_borrowed_source_before_registry_gate() {
        let source = crate::game_logic::residuals::WEAPON_SRC;
        let without_borrow = source.replace("self.caller_source(id)", "None");
        assert!(!weapon_empty_gate_source_contract(&without_borrow));
        let without_range_module = source.replace("mod weapon_range;", "");
        assert!(!weapon_empty_gate_source_contract(&without_range_module));
    }

    #[test]
    fn canonical_radius_contract_requires_wrapper_and_gated_implementation() {
        let source = crate::game_logic::residuals::WEAPON_SRC;
        let without_delegate = source.replace(
            "self.find_objects_in_radius_with_source(source_obj_id, center, radius, None)",
            "Ok(Vec::new())",
        );
        assert!(!weapon_empty_gate_source_contract(&without_delegate));
        let without_empty_result = source.replace("return Ok(Vec::new());", "return Err(error);");
        assert!(!weapon_empty_gate_source_contract(&without_empty_result));
    }

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
