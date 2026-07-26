//! Wave 324 residual peels: PartitionManager dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), partition
//! query helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 323 DieModule dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/collide/partition_manager.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Partition manager dual-world empty-gate residual method names.
pub const LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE324: &[&str] = &[
    "dual_world_registry_unavailable",
    "find_objects_in_radius",
    "try_position",
    "is_clear_line_of_sight_terrain",
    "get_ground_or_structure_height",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE324: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PARTITION_MANAGER_EMPTY_GATES",
    "LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE324: &[&str] = &[
    "click_live_partition_manager_dual_world_empty_gate_ok_prepare",
    "click_live_partition_manager_dual_world_empty_gate_ok_live",
    "click_live_partition_manager_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_partition_manager_dual_world_empty_gate_method_names_residual_wave324() -> bool
{
    LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE324.len() == 6
        && residual_name_index(
            LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE324,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE324,
            "get_ground_or_structure_height",
        ) == Some(4)
        && residual_name_index(
            LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE324,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_partition_manager_dual_world_empty_gate_nav_commands_residual_wave324() -> bool
{
    LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE324.len() == 4
        && residual_name_index(
            LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE324,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE324,
            "LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PARTITION_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE324.len() == 3
}

/// Wave 324 composite residual honesty pack.
pub fn honesty_live_partition_manager_dual_world_empty_gate_residual_pack_wave324() -> bool {
    honesty_live_partition_manager_dual_world_empty_gate_method_names_residual_wave324()
        && honesty_live_partition_manager_dual_world_empty_gate_nav_commands_residual_wave324()
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

/// Source residual: partition manager empty dual-world short-circuits.
pub fn honesty_partition_manager_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/collide/partition_manager.rs");
    if !(g.contains("Wave 324")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(find) = fn_body(g, "fn find_objects_in_radius(") else {
        return false;
    };
    let Some(try_pos) = fn_body(g, "fn try_position(") else {
        return false;
    };
    let Some(los) = fn_body(g, "fn is_clear_line_of_sight_terrain(") else {
        return false;
    };
    let Some(height) = fn_body(g, "fn get_ground_or_structure_height(") else {
        return false;
    };
    helper_ok
        && find.contains("Vec::new()")
        && try_pos.contains("return None")
        && los.contains("return false")
        && height.contains("0.0")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_partition_manager_dual_world_empty_gate_honesty() -> bool {
    honesty_live_partition_manager_dual_world_empty_gate_residual_pack_wave324()
        && honesty_partition_manager_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_partition_manager_dual_world_empty_gate_method_names_residual_wave324()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_partition_manager_dual_world_empty_gate_nav_commands_residual_wave324()
        );
    }

    #[test]
    fn wave324_composite_pack() {
        assert!(honesty_live_partition_manager_dual_world_empty_gate_residual_pack_wave324());
    }

    #[test]
    fn partition_manager_dual_world_empty_gate_sources() {
        assert!(honesty_partition_manager_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_partition_manager_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_partition_manager_dual_world_empty_gate_honesty(),
            "partition manager dual-world empty gate residual must latch"
        );
    }
}
