//! Wave 291 residual peels: ActiveBody dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), damage/owner
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 290 async player dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/body/active_body.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ActiveBody dual-world empty-gate residual method names.
pub const LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE291: &[&str] = &[
    "dual_world_registry_unavailable",
    "snapshot_object_for_damage_fx",
    "get_owner",
    "owner_handle",
    "attempt_damage",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE291: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ACTIVE_BODY_EMPTY_GATES",
    "LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE291: &[&str] = &[
    "click_live_active_body_dual_world_empty_gate_ok_prepare",
    "click_live_active_body_dual_world_empty_gate_ok_live",
    "click_live_active_body_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_active_body_dual_world_empty_gate_method_names_residual_wave291() -> bool {
    LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE291.len() == 6
        && residual_name_index(
            LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE291,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE291,
            "attempt_damage",
        ) == Some(4)
        && residual_name_index(
            LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE291,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_active_body_dual_world_empty_gate_nav_commands_residual_wave291() -> bool {
    LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE291.len() == 4
        && residual_name_index(
            LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE291,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE291,
            "LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ACTIVE_BODY_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE291.len() == 3
}

/// Wave 291 composite residual honesty pack.
pub fn honesty_live_active_body_dual_world_empty_gate_residual_pack_wave291() -> bool {
    honesty_live_active_body_dual_world_empty_gate_method_names_residual_wave291()
        && honesty_live_active_body_dual_world_empty_gate_nav_commands_residual_wave291()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    crate::game_logic::residuals::harness::last_rust_fn_body(
        src,
        name.trim_start_matches("fn ").trim_end_matches('('),
    )
    .or_else(|| {
        crate::game_logic::residuals::harness::rust_fn_body(
            src,
            name.trim_start_matches("fn ").trim_end_matches('('),
        )
    })
}

/// Source residual: ActiveBody empty-registry helper plus C++ hull path.
/// 2026-08-15: `attempt_damage` no longer no-ops on an empty dual-world
/// registry — C++ `ActiveBody::attemptDamage` (ActiveBody.cpp:321-339)
/// always validates armor, bails only on null/indestructible/already-dead,
/// then applies local HP. Owner lookup may still fail-closed.
pub fn honesty_active_body_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/body/active_body.rs");
    if !(g.contains("Wave 291")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(damage) = fn_body(g, "fn attempt_damage(") else {
        return false;
    };
    let Some(owner) = fn_body(g, "fn get_owner(") else {
        return false;
    };
    helper_ok
        && !damage.contains("dual_world_registry_unavailable")
        && damage.contains("validate_armor_and_damage_fx")
        && damage.contains("sync_from_input")
        && owner.contains("return None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_active_body_dual_world_empty_gate_honesty() -> bool {
    honesty_live_active_body_dual_world_empty_gate_residual_pack_wave291()
        && honesty_active_body_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_active_body_dual_world_empty_gate_method_names_residual_wave291());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_active_body_dual_world_empty_gate_nav_commands_residual_wave291());
    }

    #[test]
    fn wave291_composite_pack() {
        assert!(honesty_live_active_body_dual_world_empty_gate_residual_pack_wave291());
    }

    #[test]
    fn active_body_dual_world_empty_gate_sources() {
        assert!(honesty_active_body_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_active_body_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_active_body_dual_world_empty_gate_honesty(),
            "active body dual-world empty gate residual must latch"
        );
    }
}
