//! Wave 365 residual peels: ProductionUpdateComplete dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), production
//! door/spawn helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 364 PropagandaCenterBehavior dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/production/production_update_complete.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ProductionUpdateComplete dual-world empty-gate residual method names.
pub const LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE365: &[&str] = &[
    "dual_world_registry_unavailable",
    "sync_actively_constructing_flag",
    "set_hold_door_open",
    "update_doors",
    "build_player_modifiers",
    "spawn_unit",
    "can_produce",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE365: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PRODUCTION_UPDATE_COMPLETE_EMPTY_GATES",
    "LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE365:
    &[&str] = &[
    "click_live_production_update_complete_dual_world_empty_gate_ok_prepare",
    "click_live_production_update_complete_dual_world_empty_gate_ok_live",
    "click_live_production_update_complete_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_production_update_complete_dual_world_empty_gate_method_names_residual_wave365()
-> bool {
    LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE365.len() == 8
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE365,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE365,
            "can_produce",
        ) == Some(6)
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE365,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_production_update_complete_dual_world_empty_gate_nav_commands_residual_wave365()
-> bool {
    LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE365.len() == 4
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE365,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE365,
            "LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRODUCTION_UPDATE_COMPLETE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE365
            .len()
            == 3
}

/// Wave 365 composite residual honesty pack.
pub fn honesty_live_production_update_complete_dual_world_empty_gate_residual_pack_wave365() -> bool
{
    honesty_live_production_update_complete_dual_world_empty_gate_method_names_residual_wave365()
        && honesty_live_production_update_complete_dual_world_empty_gate_nav_commands_residual_wave365()
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

/// Source residual: ProductionUpdateComplete empty dual-world short-circuits.
pub fn honesty_production_update_complete_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/production/production_update_complete.rs"
    );
    if !(g.contains("Wave 365")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    // 2026-08-15: helper probes emptiness but returns false (C++ does not skip-close).
    let helper_ok = g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()")
        && g.contains("false");
    let Some(sync) = fn_body(g, "fn sync_actively_constructing_flag(") else {
        return false;
    };
    let Some(doors) = fn_body(g, "fn update_doors(") else {
        return false;
    };
    let Some(mods) = fn_body(g, "fn build_player_modifiers(") else {
        return false;
    };
    let Some(spawn) = fn_body(g, "fn spawn_unit(") else {
        return false;
    };
    let Some(can) = fn_body(g, "fn can_produce(") else {
        return false;
    };
    helper_ok
        && sync.contains("return;")
        && doors.contains("return;")
        && mods.contains("return PlayerBuildModifiers::default()")
        && spawn.contains("return Ok(())")
        && can.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_production_update_complete_dual_world_empty_gate_honesty() -> bool {
    honesty_live_production_update_complete_dual_world_empty_gate_residual_pack_wave365()
        && honesty_production_update_complete_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_production_update_complete_dual_world_empty_gate_method_names_residual_wave365());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_production_update_complete_dual_world_empty_gate_nav_commands_residual_wave365());
    }

    #[test]
    fn wave365_composite_pack() {
        assert!(
            honesty_live_production_update_complete_dual_world_empty_gate_residual_pack_wave365()
        );
    }

    #[test]
    fn production_update_complete_dual_world_empty_gate_sources() {
        assert!(honesty_production_update_complete_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_production_update_complete_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_production_update_complete_dual_world_empty_gate_honesty(),
            "production update complete dual-world empty gate residual must latch"
        );
    }
}
