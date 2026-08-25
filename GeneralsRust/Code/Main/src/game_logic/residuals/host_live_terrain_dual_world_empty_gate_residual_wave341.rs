//! Wave 341 residual peels: terrain dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), bridge/water/
//! wall helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 340 modules dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/terrain.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Terrain dual-world empty-gate residual method names.
pub const LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE341: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_damage_state",
    "apply_water_rise_damage",
    "is_point_on_wall_fallback",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE341: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TERRAIN_EMPTY_GATES",
    "LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE341: &[&str] = &[
    "click_live_terrain_dual_world_empty_gate_ok_prepare",
    "click_live_terrain_dual_world_empty_gate_ok_live",
    "click_live_terrain_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_terrain_dual_world_empty_gate_method_names_residual_wave341() -> bool {
    LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE341.len() == 5
        && residual_name_index(
            LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE341,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE341,
            "is_point_on_wall_fallback",
        ) == Some(3)
        && residual_name_index(
            LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE341,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_terrain_dual_world_empty_gate_nav_commands_residual_wave341() -> bool {
    LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE341.len() == 4
        && residual_name_index(
            LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE341,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE341,
            "LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TERRAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE341.len() == 3
}

/// Wave 341 composite residual honesty pack.
pub fn honesty_live_terrain_dual_world_empty_gate_residual_pack_wave341() -> bool {
    honesty_live_terrain_dual_world_empty_gate_method_names_residual_wave341()
        && honesty_live_terrain_dual_world_empty_gate_nav_commands_residual_wave341()
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

/// Source residual: terrain empty dual-world short-circuits.
pub fn honesty_terrain_dual_world_empty_gate_source() -> bool {
    let g = concat!(
        include_str!("../../../../GameEngine/GameLogic/src/terrain/mod.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/terrain/water.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/terrain/map_height.rs"),
    );
    if !(g.contains("Wave 341")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(upd) = fn_body(g, "fn update_damage_state(") else {
        return false;
    };
    let Some(water) = fn_body(g, "fn apply_water_rise_damage(") else {
        return false;
    };
    let Some(wall) = fn_body(g, "fn is_point_on_wall_fallback(") else {
        return false;
    };
    helper_ok
        && upd.contains("return;")
        && water.contains("return;")
        && wall.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_terrain_dual_world_empty_gate_honesty() -> bool {
    honesty_live_terrain_dual_world_empty_gate_residual_pack_wave341()
        && honesty_terrain_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_terrain_dual_world_empty_gate_method_names_residual_wave341());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_terrain_dual_world_empty_gate_nav_commands_residual_wave341());
    }

    #[test]
    fn wave341_composite_pack() {
        assert!(honesty_live_terrain_dual_world_empty_gate_residual_pack_wave341());
    }

    #[test]
    fn terrain_dual_world_empty_gate_sources() {
        assert!(honesty_terrain_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_terrain_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_terrain_dual_world_empty_gate_honesty(),
            "terrain dual-world empty gate residual must latch"
        );
    }
}
