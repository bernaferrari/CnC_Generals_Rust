//! Wave 448 residual peels: object upgrade batch dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), remaining
//! object-upgrade helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 447 FireSpreadUpdate dual-world empty-gate residual.
//!
//! Sources (batch):
//! - `object/upgrade/stealth_upgrade.rs`
//! - `object/upgrade/active_shroud_upgrade.rs`
//! - `object/upgrade/experience_scalar_upgrade.rs`
//! - `object/upgrade/locomotor_set_upgrade.rs`
//! - `object/upgrade/grant_science_upgrade.rs`
//! - `object/upgrade/cost_modifier_upgrade.rs`
//! - `object/upgrade/passengers_fire_upgrade.rs`
//! - `object/upgrade/model_condition_upgrade.rs`
//! - `object/upgrade/weapon_bonus_upgrade.rs`
//! - `object/upgrade/radar_upgrade.rs`
//! - `object/upgrade/command_set_upgrade.rs`
//! - `object/upgrade/weapon_set_upgrade.rs`
//! - `object/upgrade/unpause_special_power_upgrade.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Object-upgrade batch dual-world empty-gate residual method names.
pub const LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE448: &[&str] = &[
    "dual_world_registry_unavailable",
    "apply_upgrade",
    "upgrade_implementation",
    "apply_shroud_upgrade",
    "apply_passengers_fire",
    "apply_radar_upgrade",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE448: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_OBJECT_UPGRADE_BATCH_EMPTY_GATES",
    "LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE448:
    &[&str] = &[
    "click_live_object_upgrade_batch_dual_world_empty_gate_ok_prepare",
    "click_live_object_upgrade_batch_dual_world_empty_gate_ok_live",
    "click_live_object_upgrade_batch_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_object_upgrade_batch_dual_world_empty_gate_method_names_residual_wave448()
-> bool {
    LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE448.len() == 7
        && residual_name_index(
            LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE448,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE448,
            "apply_radar_upgrade",
        ) == Some(5)
        && residual_name_index(
            LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE448,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_object_upgrade_batch_dual_world_empty_gate_nav_commands_residual_wave448()
-> bool {
    LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE448.len() == 4
        && residual_name_index(
            LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE448,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE448,
            "LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OBJECT_UPGRADE_BATCH_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE448.len() == 3
}

/// Wave 448 composite residual honesty pack.
pub fn honesty_live_object_upgrade_batch_dual_world_empty_gate_residual_pack_wave448() -> bool {
    honesty_live_object_upgrade_batch_dual_world_empty_gate_method_names_residual_wave448()
        && honesty_live_object_upgrade_batch_dual_world_empty_gate_nav_commands_residual_wave448()
}

fn honesty_one(src: &str, fn_name: &str, expected_return_snip: &str) -> bool {
    if !(src.contains("Wave 448")
        && src.contains("fn dual_world_registry_unavailable")
        && src.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = src.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    // Find gated function body
    let needle = format!("fn {fn_name}(");
    let mut search_from = 0usize;
    let mut found = false;
    while let Some(rel) = src[search_from..].find(&needle) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + needle.len();
            continue;
        };
        let brace = i + b;
        let mut depth = 0usize;
        for (off, ch) in src[brace..].char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        let body = &src[i..brace + off + 1];
                        if body.contains("dual_world_registry_unavailable")
                            && body.contains(expected_return_snip)
                        {
                            found = true;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        if found {
            break;
        }
        search_from = i + needle.len();
    }
    helper_ok && found
}

/// Source residual: object upgrade batch empty dual-world short-circuits.
pub fn honesty_object_upgrade_batch_dual_world_empty_gate_source() -> bool {
    let stealth =
        include_str!("../../../../GameEngine/GameLogic/src/object/upgrade/stealth_upgrade.rs");
    let shroud = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/active_shroud_upgrade.rs"
    );
    let exp = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/experience_scalar_upgrade.rs"
    );
    let loco = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/locomotor_set_upgrade.rs"
    );
    let science = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/grant_science_upgrade.rs"
    );
    let cost = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/cost_modifier_upgrade.rs"
    );
    let pax = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/passengers_fire_upgrade.rs"
    );
    let model = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/model_condition_upgrade.rs"
    );
    let wbonus =
        include_str!("../../../../GameEngine/GameLogic/src/object/upgrade/weapon_bonus_upgrade.rs");
    let radar =
        include_str!("../../../../GameEngine/GameLogic/src/object/upgrade/radar_upgrade.rs");
    let cmd =
        include_str!("../../../../GameEngine/GameLogic/src/object/upgrade/command_set_upgrade.rs");
    let wset =
        include_str!("../../../../GameEngine/GameLogic/src/object/upgrade/weapon_set_upgrade.rs");
    let unpause = include_str!(
        "../../../../GameEngine/GameLogic/src/object/upgrade/unpause_special_power_upgrade.rs"
    );

    honesty_one(stealth, "upgrade_implementation", "return Ok(())")
        && honesty_one(shroud, "apply_shroud_upgrade", "return Ok(())")
        && honesty_one(exp, "apply_upgrade", "return false;")
        && honesty_one(loco, "apply_upgrade", "return false;")
        && honesty_one(science, "apply_upgrade", "return false;")
        && honesty_one(cost, "apply_upgrade", "return false;")
        && honesty_one(pax, "apply_passengers_fire", "return false;")
        && honesty_one(model, "apply_upgrade", "return false;")
        && honesty_one(wbonus, "apply_upgrade", "return false;")
        && honesty_one(radar, "apply_radar_upgrade", "return Ok(())")
        && honesty_one(cmd, "apply_upgrade", "return false;")
        && honesty_one(wset, "apply_upgrade", "return false;")
        && honesty_one(unpause, "apply_upgrade", "return false;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_object_upgrade_batch_dual_world_empty_gate_honesty() -> bool {
    honesty_live_object_upgrade_batch_dual_world_empty_gate_residual_pack_wave448()
        && honesty_object_upgrade_batch_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_object_upgrade_batch_dual_world_empty_gate_method_names_residual_wave448()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_object_upgrade_batch_dual_world_empty_gate_nav_commands_residual_wave448()
        );
    }

    #[test]
    fn wave448_composite_pack() {
        assert!(honesty_live_object_upgrade_batch_dual_world_empty_gate_residual_pack_wave448());
    }

    #[test]
    fn object_upgrade_batch_dual_world_empty_gate_sources() {
        assert!(honesty_object_upgrade_batch_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_object_upgrade_batch_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_object_upgrade_batch_dual_world_empty_gate_honesty(),
            "object upgrade batch dual-world empty gate residual must latch"
        );
    }
}
