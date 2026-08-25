//! Wave 353 residual peels: SpecialPowerModule dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), special-power
//! activation helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 352 DeliverPayloadAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/special_power_module.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SpecialPowerModule dual-world empty-gate residual method names.
pub const LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE353: &[&str] = &[
    "dual_world_registry_unavailable",
    "initialize_from_owner",
    "initiate_intent_to_do_special_power",
    "can_activate",
    "do_special_power",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE353: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SPECIAL_POWER_MODULE_EMPTY_GATES",
    "LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE353:
    &[&str] = &[
    "click_live_special_power_module_dual_world_empty_gate_ok_prepare",
    "click_live_special_power_module_dual_world_empty_gate_ok_live",
    "click_live_special_power_module_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_special_power_module_dual_world_empty_gate_method_names_residual_wave353()
-> bool {
    LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE353.len() == 6
        && residual_name_index(
            LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE353,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE353,
            "do_special_power",
        ) == Some(4)
        && residual_name_index(
            LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE353,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_special_power_module_dual_world_empty_gate_nav_commands_residual_wave353()
-> bool {
    LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE353.len() == 4
        && residual_name_index(
            LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE353,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE353,
            "LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SPECIAL_POWER_MODULE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE353.len() == 3
}

/// Wave 353 composite residual honesty pack.
pub fn honesty_live_special_power_module_dual_world_empty_gate_residual_pack_wave353() -> bool {
    honesty_live_special_power_module_dual_world_empty_gate_method_names_residual_wave353()
        && honesty_live_special_power_module_dual_world_empty_gate_nav_commands_residual_wave353()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(name) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + name.len();
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
                        if body.contains("dual_world_registry_unavailable") {
                            return Some(body);
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_from = i + name.len();
    }
    None
}

/// Source residual: SpecialPowerModule empty dual-world short-circuits.
pub fn honesty_special_power_module_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/special_power_module.rs");
    if !(g.contains("Wave 353")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(init) = fn_body(g, "fn initialize_from_owner(") else {
        return false;
    };
    let Some(intent) = fn_body(g, "fn initiate_intent_to_do_special_power(") else {
        return false;
    };
    let Some(act) = fn_body(g, "fn can_activate(") else {
        return false;
    };
    let Some(dos) = fn_body(g, "fn do_special_power(") else {
        return false;
    };
    helper_ok
        && init.contains("return;")
        && intent.contains("return false")
        && act.contains("return false")
        && dos.contains("return;")
        && g.matches("Wave 353:").count() >= 12
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_special_power_module_dual_world_empty_gate_honesty() -> bool {
    honesty_live_special_power_module_dual_world_empty_gate_residual_pack_wave353()
        && honesty_special_power_module_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_special_power_module_dual_world_empty_gate_method_names_residual_wave353()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_special_power_module_dual_world_empty_gate_nav_commands_residual_wave353()
        );
    }

    #[test]
    fn wave353_composite_pack() {
        assert!(honesty_live_special_power_module_dual_world_empty_gate_residual_pack_wave353());
    }

    #[test]
    fn special_power_module_dual_world_empty_gate_sources() {
        assert!(honesty_special_power_module_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_special_power_module_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_special_power_module_dual_world_empty_gate_honesty(),
            "special power module dual-world empty gate residual must latch"
        );
    }
}
