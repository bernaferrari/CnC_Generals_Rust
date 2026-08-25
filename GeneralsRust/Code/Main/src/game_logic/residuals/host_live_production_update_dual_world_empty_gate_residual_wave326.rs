//! Wave 326 residual peels: ProductionUpdateBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), production
//! queue/door helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 325 SpectreGunship dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/production_update_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Production update dual-world empty-gate residual method names.
pub const LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE326: &[&str] = &[
    "dual_world_registry_unavailable",
    "can_queue_create_unit",
    "set_hold_door_open",
    "update",
    "update_doors",
    "add_to_production_queue",
    "remove_from_production_queue_internal",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE326: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PRODUCTION_UPDATE_EMPTY_GATES",
    "LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE326: &[&str] = &[
    "click_live_production_update_dual_world_empty_gate_ok_prepare",
    "click_live_production_update_dual_world_empty_gate_ok_live",
    "click_live_production_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave326() -> bool
{
    LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE326.len() == 8
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE326,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE326,
            "update",
        ) == Some(3)
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE326,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave326() -> bool
{
    LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE326.len() == 4
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE326,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE326,
            "LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PRODUCTION_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE326.len() == 3
}

/// Wave 326 composite residual honesty pack.
pub fn honesty_live_production_update_dual_world_empty_gate_residual_pack_wave326() -> bool {
    honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave326()
        && honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave326()
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

/// Source residual: production update empty dual-world short-circuits.
pub fn honesty_production_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/production_update_behavior.rs"
    );
    if !(g.contains("Wave 326")
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
    let Some(can) = fn_body(g, "fn can_queue_create_unit(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(") else {
        return false;
    };
    helper_ok
        && can.contains("CanMakeType::Other")
        && update.contains("UpdateResult::Sleep")
        && g.contains("fn set_hold_door_open")
        && g.contains("fn update_doors")
        && g.contains("fn add_to_production_queue")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_production_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_production_update_dual_world_empty_gate_residual_pack_wave326()
        && honesty_production_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_production_update_dual_world_empty_gate_method_names_residual_wave326()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_production_update_dual_world_empty_gate_nav_commands_residual_wave326()
        );
    }

    #[test]
    fn wave326_composite_pack() {
        assert!(honesty_live_production_update_dual_world_empty_gate_residual_pack_wave326());
    }

    #[test]
    fn production_update_dual_world_empty_gate_sources() {
        assert!(honesty_production_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_production_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_production_update_dual_world_empty_gate_honesty(),
            "production update dual-world empty gate residual must latch"
        );
    }
}
