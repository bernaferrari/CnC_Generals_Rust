//! Wave 276 residual peels: Turret AI dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), turret
//! target/scan helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 275 CommandProcessor dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/turret.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - set_current_target* keep local ID writes (registry optional)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Turret dual-world empty-gate residual method names.
pub const LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE276: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_current_target",
    "is_target_in_weapon_range",
    "scan_for_targets",
    "find_best_target",
    "classic_on_update",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE276: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TURRET_EMPTY_GATES",
    "LIVE_TURRET_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE276: &[&str] = &[
    "click_live_turret_dual_world_empty_gate_ok_prepare",
    "click_live_turret_dual_world_empty_gate_ok_live",
    "click_live_turret_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_turret_dual_world_empty_gate_method_names_residual_wave276() -> bool {
    LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE276.len() == 7
        && residual_name_index(
            LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE276,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE276,
            "classic_on_update",
        ) == Some(5)
        && residual_name_index(
            LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE276,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_turret_dual_world_empty_gate_nav_commands_residual_wave276() -> bool {
    LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE276.len() == 4
        && residual_name_index(
            LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE276,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE276,
            "LIVE_TURRET_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TURRET_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE276.len() == 3
}

/// Wave 276 composite residual honesty pack.
pub fn honesty_live_turret_dual_world_empty_gate_residual_pack_wave276() -> bool {
    honesty_live_turret_dual_world_empty_gate_method_names_residual_wave276()
        && honesty_live_turret_dual_world_empty_gate_nav_commands_residual_wave276()
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

/// Source residual: Turret empty dual-world short-circuits.
pub fn honesty_turret_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/ai/turret.rs");
    if !(g.contains("Wave 276")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(get) = fn_body(g, "fn get_current_target(") else {
        return false;
    };
    let Some(scan) = fn_body(g, "fn scan_for_targets(") else {
        return false;
    };
    // set_current_target must NOT early-return solely on empty dual-world
    let Some(set) = fn_body(g, "fn set_current_target(") else {
        return false;
    };
    let classic_gated = g.contains(
        "fn classic_on_update(&mut self) -> Result<StateReturnType, String> {
        // Wave 276:",
    ) || g.contains(
        "dual_world_registry_unavailable() {
            return Ok(StateReturnType::Failure);",
    );
    get.contains("dual_world_registry_unavailable")
        && get.contains("return None")
        && scan.contains("dual_world_registry_unavailable")
        && scan.contains("Vec::new()")
        && classic_gated
        && !set.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_turret_dual_world_empty_gate_honesty() -> bool {
    honesty_live_turret_dual_world_empty_gate_residual_pack_wave276()
        && honesty_turret_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_turret_dual_world_empty_gate_method_names_residual_wave276());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_turret_dual_world_empty_gate_nav_commands_residual_wave276());
    }

    #[test]
    fn wave276_composite_pack() {
        assert!(honesty_live_turret_dual_world_empty_gate_residual_pack_wave276());
    }

    #[test]
    fn turret_dual_world_empty_gate_sources() {
        assert!(honesty_turret_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_turret_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_turret_dual_world_empty_gate_honesty(),
            "turret dual-world empty gate residual must latch"
        );
    }
}
