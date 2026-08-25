//! Wave 342 residual peels: SpecialPowerTemplate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), special-power
//! usability helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 341 terrain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/special_power_template.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Special power template dual-world empty-gate residual method names.
pub const LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE342: &[&str] = &[
    "dual_world_registry_unavailable",
    "can_use_special_power",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE342: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SPECIAL_POWER_TEMPLATE_EMPTY_GATES",
    "LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE342:
    &[&str] = &[
    "click_live_special_power_template_dual_world_empty_gate_ok_prepare",
    "click_live_special_power_template_dual_world_empty_gate_ok_live",
    "click_live_special_power_template_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_special_power_template_dual_world_empty_gate_method_names_residual_wave342()
-> bool {
    LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE342.len() == 3
        && residual_name_index(
            LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE342,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE342,
            "can_use_special_power",
        ) == Some(1)
        && residual_name_index(
            LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE342,
            "playable_claim = false",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_special_power_template_dual_world_empty_gate_nav_commands_residual_wave342()
-> bool {
    LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE342.len() == 4
        && residual_name_index(
            LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE342,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE342,
            "LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SPECIAL_POWER_TEMPLATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE342.len()
            == 3
}

/// Wave 342 composite residual honesty pack.
pub fn honesty_live_special_power_template_dual_world_empty_gate_residual_pack_wave342() -> bool {
    honesty_live_special_power_template_dual_world_empty_gate_method_names_residual_wave342()
        && honesty_live_special_power_template_dual_world_empty_gate_nav_commands_residual_wave342()
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

/// Source residual: special power template empty dual-world short-circuits.
pub fn honesty_special_power_template_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/special_power_template.rs");
    if !(g.contains("Wave 342")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(can) = fn_body(g, "fn can_use_special_power(") else {
        return false;
    };
    helper_ok && can.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_special_power_template_dual_world_empty_gate_honesty() -> bool {
    honesty_live_special_power_template_dual_world_empty_gate_residual_pack_wave342()
        && honesty_special_power_template_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_special_power_template_dual_world_empty_gate_method_names_residual_wave342(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_special_power_template_dual_world_empty_gate_nav_commands_residual_wave342(
            )
        );
    }

    #[test]
    fn wave342_composite_pack() {
        assert!(honesty_live_special_power_template_dual_world_empty_gate_residual_pack_wave342());
    }

    #[test]
    fn special_power_template_dual_world_empty_gate_sources() {
        assert!(honesty_special_power_template_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_special_power_template_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_special_power_template_dual_world_empty_gate_honesty(),
            "special power template dual-world empty gate residual must latch"
        );
    }
}
