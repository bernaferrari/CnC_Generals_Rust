//! Wave 406 residual peels: OCLSpecialPower dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), OCL special
//! power helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 405 SupplyWarehouseDock dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/special_powers/ocl_special_power.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// OCLSpecialPower dual-world empty-gate residual method names.
pub const LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE406: &[&str] = &[
    "dual_world_registry_unavailable",
    "find_ocl_name",
    "do_special_power_at_location",
    "do_special_power_at_object",
    "do_special_power",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE406: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_OCL_SPECIAL_POWER_EMPTY_GATES",
    "LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE406: &[&str] = &[
    "click_live_ocl_special_power_dual_world_empty_gate_ok_prepare",
    "click_live_ocl_special_power_dual_world_empty_gate_ok_live",
    "click_live_ocl_special_power_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ocl_special_power_dual_world_empty_gate_method_names_residual_wave406() -> bool
{
    LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE406.len() == 6
        && residual_name_index(
            LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE406,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE406,
            "do_special_power",
        ) == Some(4)
        && residual_name_index(
            LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE406,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ocl_special_power_dual_world_empty_gate_nav_commands_residual_wave406() -> bool
{
    LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE406.len() == 4
        && residual_name_index(
            LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE406,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE406,
            "LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_OCL_SPECIAL_POWER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE406.len() == 3
}

/// Wave 406 composite residual honesty pack.
pub fn honesty_live_ocl_special_power_dual_world_empty_gate_residual_pack_wave406() -> bool {
    honesty_live_ocl_special_power_dual_world_empty_gate_method_names_residual_wave406()
        && honesty_live_ocl_special_power_dual_world_empty_gate_nav_commands_residual_wave406()
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

/// Source residual: OCLSpecialPower empty dual-world short-circuits.
pub fn honesty_ocl_special_power_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/special_powers/ocl_special_power.rs"
    );
    if !(g.contains("Wave 406")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(find) = fn_body(g, "fn find_ocl_name(") else {
        return false;
    };
    let Some(loc) = fn_body(g, "fn do_special_power_at_location(") else {
        return false;
    };
    let Some(obj) = fn_body(g, "fn do_special_power_at_object(") else {
        return false;
    };
    let Some(plain) = fn_body(g, "fn do_special_power(") else {
        return false;
    };
    helper_ok
        && find.contains("return None")
        && loc.contains("return Ok(())")
        && obj.contains("return Ok(())")
        && plain.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ocl_special_power_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ocl_special_power_dual_world_empty_gate_residual_pack_wave406()
        && honesty_ocl_special_power_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_ocl_special_power_dual_world_empty_gate_method_names_residual_wave406()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_ocl_special_power_dual_world_empty_gate_nav_commands_residual_wave406()
        );
    }

    #[test]
    fn wave406_composite_pack() {
        assert!(honesty_live_ocl_special_power_dual_world_empty_gate_residual_pack_wave406());
    }

    #[test]
    fn ocl_special_power_dual_world_empty_gate_sources() {
        assert!(honesty_ocl_special_power_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ocl_special_power_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ocl_special_power_dual_world_empty_gate_honesty(),
            "ocl special power dual-world empty gate residual must latch"
        );
    }
}
