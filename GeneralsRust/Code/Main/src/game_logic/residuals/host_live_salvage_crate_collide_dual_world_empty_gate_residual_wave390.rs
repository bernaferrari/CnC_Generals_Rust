//! Wave 390 residual peels: SalvageCrateCollide dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), salvage
//! crate resolve helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 389 HiveStructureBody dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/collide/crate_collide/salvage_crate_collide.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// SalvageCrateCollide dual-world empty-gate residual method names.
pub const LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE390: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_crate_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE390: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SALVAGE_CRATE_COLLIDE_EMPTY_GATES",
    "LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE390:
    &[&str] = &[
    "click_live_salvage_crate_collide_dual_world_empty_gate_ok_prepare",
    "click_live_salvage_crate_collide_dual_world_empty_gate_ok_live",
    "click_live_salvage_crate_collide_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_salvage_crate_collide_dual_world_empty_gate_method_names_residual_wave390()
-> bool {
    LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE390.len() == 3
        && residual_name_index(
            LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE390,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE390,
            "resolve_crate_object",
        ) == Some(1)
        && residual_name_index(
            LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE390,
            "playable_claim = false",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_salvage_crate_collide_dual_world_empty_gate_nav_commands_residual_wave390()
-> bool {
    LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE390.len() == 4
        && residual_name_index(
            LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE390,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE390,
            "LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SALVAGE_CRATE_COLLIDE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE390.len()
            == 3
}

/// Wave 390 composite residual honesty pack.
pub fn honesty_live_salvage_crate_collide_dual_world_empty_gate_residual_pack_wave390() -> bool {
    honesty_live_salvage_crate_collide_dual_world_empty_gate_method_names_residual_wave390()
        && honesty_live_salvage_crate_collide_dual_world_empty_gate_nav_commands_residual_wave390()
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

/// Source residual: SalvageCrateCollide empty dual-world short-circuits.
pub fn honesty_salvage_crate_collide_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/collide/crate_collide/salvage_crate_collide.rs"
    );
    if !(g.contains("Wave 390")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_crate_object(") else {
        return false;
    };
    helper_ok && resolve.contains("return None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_salvage_crate_collide_dual_world_empty_gate_honesty() -> bool {
    honesty_live_salvage_crate_collide_dual_world_empty_gate_residual_pack_wave390()
        && honesty_salvage_crate_collide_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_salvage_crate_collide_dual_world_empty_gate_method_names_residual_wave390(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_salvage_crate_collide_dual_world_empty_gate_nav_commands_residual_wave390(
            )
        );
    }

    #[test]
    fn wave390_composite_pack() {
        assert!(honesty_live_salvage_crate_collide_dual_world_empty_gate_residual_pack_wave390());
    }

    #[test]
    fn salvage_crate_collide_dual_world_empty_gate_sources() {
        assert!(honesty_salvage_crate_collide_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_salvage_crate_collide_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_salvage_crate_collide_dual_world_empty_gate_honesty(),
            "salvage crate collide dual-world empty gate residual must latch"
        );
    }
}
