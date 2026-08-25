//! Wave 383 residual peels: DynamicShroudClearingRangeUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), shroud
//! range helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 382 MissileLauncherBuilding dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/dynamic_shroud_clearing_range_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// DynamicShroudClearingRangeUpdate dual-world empty-gate residual method names.
pub const LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE383:
    &[&str] = &[
    "dual_world_registry_unavailable",
    "create_grid_decals",
    "animate_grid_decals",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE383:
    &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DYNAMIC_SHROUD_EMPTY_GATES",
    "LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE383:
    &[&str] = &[
    "click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_ok_prepare",
    "click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_ok_live",
    "click_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_residual_wave383()
-> bool {
    LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE383.len() == 5
        && residual_name_index(
            LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE383,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE383,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE383,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_residual_wave383()
-> bool {
    LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE383.len() == 4
        && residual_name_index(
            LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE383,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE383,
            "LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DYNAMIC_SHROUD_CLEARING_RANGE_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE383
            .len()
            == 3
}

/// Wave 383 composite residual honesty pack.
pub fn honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_residual_pack_wave383()
-> bool {
    honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_residual_wave383()
        && honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_residual_wave383()
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

/// Source residual: DynamicShroudClearingRangeUpdate empty dual-world short-circuits.
pub fn honesty_dynamic_shroud_clearing_range_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/dynamic_shroud_clearing_range_update.rs"
    );
    if !(g.contains("Wave 383")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(create) = fn_body(g, "fn create_grid_decals(") else {
        return false;
    };
    let Some(animate) = fn_body(g, "fn animate_grid_decals(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok
        && create.contains("return;")
        && animate.contains("return;")
        && update.contains("return UpdateSleepTime::Forever")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_residual_pack_wave383()
        && honesty_dynamic_shroud_clearing_range_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_method_names_residual_wave383()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_nav_commands_residual_wave383()
        );
    }

    #[test]
    fn wave383_composite_pack() {
        assert!(
            honesty_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_residual_pack_wave383()
        );
    }

    #[test]
    fn dynamic_shroud_clearing_range_update_dual_world_empty_gate_sources() {
        assert!(honesty_dynamic_shroud_clearing_range_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_honesty_residual_live()
     {
        assert!(
            simulate_live_dynamic_shroud_clearing_range_update_dual_world_empty_gate_honesty(),
            "dynamic shroud clearing range update dual-world empty gate residual must latch"
        );
    }
}
