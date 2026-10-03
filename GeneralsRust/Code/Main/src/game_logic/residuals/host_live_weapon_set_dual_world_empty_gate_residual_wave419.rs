//! Wave 419 residual peels: WeaponSet dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), weapon set
//! targeting helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 418 BuildPlacement dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/weapon/weapon_set.rs` selection
//! - `GameLogic/src/weapon/weapon_set_able.rs` target legality and usability
//! - `GameLogic/src/weapon/helpers.rs` child-module registry availability
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// WeaponSet dual-world empty-gate residual method names.
pub const LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE419: &[&str] = &[
    "dual_world_registry_unavailable",
    "choose_best_weapon_for_target",
    "get_able_to_use_weapon_against_target",
    "get_able_to_attack_specific_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE419: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_WEAPON_SET_EMPTY_GATES",
    "LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE419: &[&str] = &[
    "click_live_weapon_set_dual_world_empty_gate_ok_prepare",
    "click_live_weapon_set_dual_world_empty_gate_ok_live",
    "click_live_weapon_set_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_weapon_set_dual_world_empty_gate_method_names_residual_wave419() -> bool {
    LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE419.len() == 5
        && residual_name_index(
            LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE419,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE419,
            "get_able_to_attack_specific_object",
        ) == Some(3)
        && residual_name_index(
            LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE419,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_weapon_set_dual_world_empty_gate_nav_commands_residual_wave419() -> bool {
    LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE419.len() == 4
        && residual_name_index(
            LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE419,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE419,
            "LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_WEAPON_SET_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE419.len() == 3
}

/// Wave 419 composite residual honesty pack.
pub fn honesty_live_weapon_set_dual_world_empty_gate_residual_pack_wave419() -> bool {
    honesty_live_weapon_set_dual_world_empty_gate_method_names_residual_wave419()
        && honesty_live_weapon_set_dual_world_empty_gate_nav_commands_residual_wave419()
}

const WEAPON_SET_SOURCE: &str =
    include_str!("../../../../GameEngine/GameLogic/src/weapon/weapon_set.rs");
const WEAPON_SET_ABLE_SOURCE: &str =
    include_str!("../../../../GameEngine/GameLogic/src/weapon/weapon_set_able.rs");
const WEAPON_HELPERS_SOURCE: &str =
    include_str!("../../../../GameEngine/GameLogic/src/weapon/helpers.rs");

fn fn_body<'a>(source: &'a str, name: &str) -> Option<&'a str> {
    crate::game_logic::residuals::harness::last_rust_fn_body(source, name)
}

/// Check real canonical declarations and the imported child-module gate.
/// The retired evaluate_weapon_against_target helper is not a production owner.
fn weapon_set_empty_gate_source_contract(set: &str, able: &str, helpers: &str) -> bool {
    let Some(set_helper) = fn_body(set, "dual_world_registry_unavailable") else {
        return false;
    };
    let Some(child_helper) = fn_body(helpers, "dual_world_registry_unavailable") else {
        return false;
    };
    let Some(choose) = fn_body(set, "choose_best_weapon_for_target") else {
        return false;
    };
    let Some(use_target) = fn_body(able, "get_able_to_use_weapon_against_target") else {
        return false;
    };
    let Some(specific) = fn_body(able, "get_able_to_attack_specific_object") else {
        return false;
    };
    set.contains("mod weapon_set_able;")
        && set.contains("#[path = \"weapon_set_able.rs\"]")
        && able.contains("use super::super::helpers::dual_world_registry_unavailable;")
        && set_helper.contains("OBJECT_REGISTRY.is_empty()")
        && child_helper.contains("OBJECT_REGISTRY.is_empty()")
        && choose.contains("dual_world_registry_unavailable()")
        && choose.contains("return Ok(false)")
        && use_target.contains("dual_world_registry_unavailable()")
        && use_target.contains("return CanAttackResult::NotPossible")
        && specific.contains("dual_world_registry_unavailable()")
        && specific.contains("return CanAttackResult::NotPossible")
        && specific.contains("self.get_able_to_use_weapon_against_target(")
}

/// Source residual: canonical WeaponSet targeting adapters fail closed.
pub fn honesty_weapon_set_dual_world_empty_gate_source() -> bool {
    weapon_set_empty_gate_source_contract(
        WEAPON_SET_SOURCE,
        WEAPON_SET_ABLE_SOURCE,
        WEAPON_HELPERS_SOURCE,
    )
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_weapon_set_dual_world_empty_gate_honesty() -> bool {
    honesty_live_weapon_set_dual_world_empty_gate_residual_pack_wave419()
        && honesty_weapon_set_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_target_contract_requires_reachable_able_child() {
        assert!(!weapon_set_empty_gate_source_contract(
            WEAPON_SET_SOURCE,
            "",
            WEAPON_HELPERS_SOURCE,
        ));
        let without_declaration = WEAPON_SET_SOURCE.replace("mod weapon_set_able;", "");
        assert!(!weapon_set_empty_gate_source_contract(
            &without_declaration,
            WEAPON_SET_ABLE_SOURCE,
            WEAPON_HELPERS_SOURCE,
        ));
    }

    #[test]
    fn canonical_target_contract_requires_real_import_and_empty_gate() {
        let without_import = WEAPON_SET_ABLE_SOURCE.replace(
            "use super::super::helpers::dual_world_registry_unavailable;",
            "",
        );
        assert!(!weapon_set_empty_gate_source_contract(
            WEAPON_SET_SOURCE,
            &without_import,
            WEAPON_HELPERS_SOURCE,
        ));
        let without_gate =
            WEAPON_SET_ABLE_SOURCE.replace("if dual_world_registry_unavailable()", "if false");
        assert!(!weapon_set_empty_gate_source_contract(
            WEAPON_SET_SOURCE,
            &without_gate,
            WEAPON_HELPERS_SOURCE,
        ));
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_weapon_set_dual_world_empty_gate_method_names_residual_wave419());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_weapon_set_dual_world_empty_gate_nav_commands_residual_wave419());
    }

    #[test]
    fn wave419_composite_pack() {
        assert!(honesty_live_weapon_set_dual_world_empty_gate_residual_pack_wave419());
    }

    #[test]
    fn weapon_set_dual_world_empty_gate_sources() {
        assert!(honesty_weapon_set_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_weapon_set_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_weapon_set_dual_world_empty_gate_honesty(),
            "weapon set dual-world empty gate residual must latch"
        );
    }
}
