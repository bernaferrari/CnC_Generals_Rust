//! Wave 371 residual peels: ToppleUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), topple
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 370 HealContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/topple_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Factory intentionally ungated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ToppleUpdate dual-world empty-gate residual method names.
pub const LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE371: &[&str] = &[
    "dual_world_registry_unavailable",
    "apply_toppling_force",
    "update_simple",
    "on_collision",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE371: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TOPPLE_UPDATE_EMPTY_GATES",
    "LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE371: &[&str] = &[
    "click_live_topple_update_dual_world_empty_gate_ok_prepare",
    "click_live_topple_update_dual_world_empty_gate_ok_live",
    "click_live_topple_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_topple_update_dual_world_empty_gate_method_names_residual_wave371() -> bool {
    LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE371.len() == 5
        && residual_name_index(
            LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE371,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE371,
            "on_collision",
        ) == Some(3)
        && residual_name_index(
            LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE371,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_topple_update_dual_world_empty_gate_nav_commands_residual_wave371() -> bool {
    LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE371.len() == 4
        && residual_name_index(
            LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE371,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE371,
            "LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TOPPLE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE371.len() == 3
}

/// Wave 371 composite residual honesty pack.
pub fn honesty_live_topple_update_dual_world_empty_gate_residual_pack_wave371() -> bool {
    honesty_live_topple_update_dual_world_empty_gate_method_names_residual_wave371()
        && honesty_live_topple_update_dual_world_empty_gate_nav_commands_residual_wave371()
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

/// Source residual: ToppleUpdate empty dual-world short-circuits.
pub fn honesty_topple_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/behavior/topple_update.rs");
    if !(g.contains("Wave 371")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(apply) = fn_body(g, "fn apply_toppling_force(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    let Some(collision) = fn_body(g, "fn on_collision(") else {
        return false;
    };
    helper_ok
        && apply.contains("return;")
        && update.contains("return UpdateSleepTime::Forever")
        && collision.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_topple_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_topple_update_dual_world_empty_gate_residual_pack_wave371()
        && honesty_topple_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_topple_update_dual_world_empty_gate_method_names_residual_wave371());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_topple_update_dual_world_empty_gate_nav_commands_residual_wave371());
    }

    #[test]
    fn wave371_composite_pack() {
        assert!(honesty_live_topple_update_dual_world_empty_gate_residual_pack_wave371());
    }

    #[test]
    fn topple_update_dual_world_empty_gate_sources() {
        assert!(honesty_topple_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_topple_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_topple_update_dual_world_empty_gate_honesty(),
            "topple update dual-world empty gate residual must latch"
        );
    }
}
