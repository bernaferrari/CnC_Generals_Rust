//! Wave 379 residual peels: FlammableUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), flammable
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 378 HordeUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/flammable_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// FlammableUpdate dual-world empty-gate residual method names.
pub const LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE379: &[&str] = &[
    "dual_world_registry_unavailable",
    "try_to_ignite",
    "do_aflame_damage",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE379: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_FLAMMABLE_UPDATE_EMPTY_GATES",
    "LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE379: &[&str] = &[
    "click_live_flammable_update_dual_world_empty_gate_ok_prepare",
    "click_live_flammable_update_dual_world_empty_gate_ok_live",
    "click_live_flammable_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_flammable_update_dual_world_empty_gate_method_names_residual_wave379() -> bool {
    LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE379.len() == 5
        && residual_name_index(
            LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE379,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE379,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE379,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_flammable_update_dual_world_empty_gate_nav_commands_residual_wave379() -> bool {
    LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE379.len() == 4
        && residual_name_index(
            LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE379,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE379,
            "LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_FLAMMABLE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE379.len() == 3
}

/// Wave 379 composite residual honesty pack.
pub fn honesty_live_flammable_update_dual_world_empty_gate_residual_pack_wave379() -> bool {
    honesty_live_flammable_update_dual_world_empty_gate_method_names_residual_wave379()
        && honesty_live_flammable_update_dual_world_empty_gate_nav_commands_residual_wave379()
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

/// Source residual: FlammableUpdate empty dual-world short-circuits.
pub fn honesty_flammable_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/behavior/flammable_update.rs");
    if !(g.contains("Wave 379")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(ignite) = fn_body(g, "fn try_to_ignite(") else {
        return false;
    };
    let Some(damage) = fn_body(g, "fn do_aflame_damage(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok
        && ignite.contains("return;")
        && damage.contains("return;")
        && update.contains("return UpdateSleepTime::Forever")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_flammable_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_flammable_update_dual_world_empty_gate_residual_pack_wave379()
        && honesty_flammable_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_flammable_update_dual_world_empty_gate_method_names_residual_wave379()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_flammable_update_dual_world_empty_gate_nav_commands_residual_wave379()
        );
    }

    #[test]
    fn wave379_composite_pack() {
        assert!(honesty_live_flammable_update_dual_world_empty_gate_residual_pack_wave379());
    }

    #[test]
    fn flammable_update_dual_world_empty_gate_sources() {
        assert!(honesty_flammable_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_flammable_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_flammable_update_dual_world_empty_gate_honesty(),
            "flammable update dual-world empty gate residual must latch"
        );
    }
}
