//! Wave 266 residual peels: PartitionFilter dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), partition
//! allow() queries fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 265 Weapon dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/collide/partition_filters.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Partition filters dual-world empty-gate residual method names.
pub const LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE266: &[&str] = &[
    "dual_world_registry_unavailable",
    "allow",
    "source_player",
    "PartitionFilterRelationship",
    "PartitionFilterSamePlayer",
    "PartitionFilterAcceptOnTeam",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE266: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PARTITION_FILTER_EMPTY_GATES",
    "LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE266: &[&str] = &[
    "click_live_partition_filters_dual_world_empty_gate_ok_prepare",
    "click_live_partition_filters_dual_world_empty_gate_ok_live",
    "click_live_partition_filters_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_partition_filters_dual_world_empty_gate_method_names_residual_wave266() -> bool
{
    LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE266.len() == 7
        && residual_name_index(
            LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE266,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE266,
            "source_player",
        ) == Some(2)
        && residual_name_index(
            LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE266,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_partition_filters_dual_world_empty_gate_nav_commands_residual_wave266() -> bool
{
    LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE266.len() == 4
        && residual_name_index(
            LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE266,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE266,
            "LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PARTITION_FILTERS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE266.len() == 3
}

/// Wave 266 composite residual honesty pack.
pub fn honesty_live_partition_filters_dual_world_empty_gate_residual_pack_wave266() -> bool {
    honesty_live_partition_filters_dual_world_empty_gate_method_names_residual_wave266()
        && honesty_live_partition_filters_dual_world_empty_gate_nav_commands_residual_wave266()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: PartitionFilter empty dual-world short-circuits.
pub fn honesty_partition_filters_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/collide/partition_filters.rs");
    if !(g.contains("Wave 266")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(player) = fn_body(g, "fn source_player(") else {
        return false;
    };
    // at least one allow path gated
    let allow_gated = g.contains("Wave 266: empty dual-world → reject object.");
    player.contains("dual_world_registry_unavailable")
        && player.contains("return None")
        && allow_gated
        && g.matches("dual_world_registry_unavailable").count() >= 10
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_partition_filters_dual_world_empty_gate_honesty() -> bool {
    honesty_live_partition_filters_dual_world_empty_gate_residual_pack_wave266()
        && honesty_partition_filters_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_partition_filters_dual_world_empty_gate_method_names_residual_wave266()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_partition_filters_dual_world_empty_gate_nav_commands_residual_wave266()
        );
    }

    #[test]
    fn wave266_composite_pack() {
        assert!(honesty_live_partition_filters_dual_world_empty_gate_residual_pack_wave266());
    }

    #[test]
    fn partition_filters_dual_world_empty_gate_sources() {
        assert!(honesty_partition_filters_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_partition_filters_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_partition_filters_dual_world_empty_gate_honesty(),
            "partition filters dual-world empty gate residual must latch"
        );
    }
}
