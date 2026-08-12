//! Wave 256 residual peels: Team dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), team
//! member/target walks fail-closed without dual-world factory resolution.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 255 AIPlayer dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/team.rs` dual_world_registry_unavailable + early outs
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Team dual-world empty-gate residual method names.
pub const LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE256: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_team_target_object",
    "try_to_recruit",
    "update_state",
    "kill_team",
    "did_all_enter",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE256: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TEAM_EMPTY_GATES",
    "LIVE_TEAM_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE256: &[&str] = &[
    "click_live_team_dual_world_empty_gate_ok_prepare",
    "click_live_team_dual_world_empty_gate_ok_live",
    "click_live_team_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_team_dual_world_empty_gate_method_names_residual_wave256() -> bool {
    LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE256.len() == 7
        && residual_name_index(
            LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE256,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE256,
            "kill_team",
        ) == Some(4)
        && residual_name_index(
            LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE256,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_team_dual_world_empty_gate_nav_commands_residual_wave256() -> bool {
    LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE256.len() == 4
        && residual_name_index(
            LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE256,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE256,
            "LIVE_TEAM_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TEAM_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE256.len() == 3
}

/// Wave 256 composite residual honesty pack.
pub fn honesty_live_team_dual_world_empty_gate_residual_pack_wave256() -> bool {
    honesty_live_team_dual_world_empty_gate_method_names_residual_wave256()
        && honesty_live_team_dual_world_empty_gate_nav_commands_residual_wave256()
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

/// Source residual: Team empty dual-world short-circuits.
pub fn honesty_team_dual_world_empty_gate_source() -> bool {
    let g = gamelogic::team::TEAM_SRC;
    if !(g.contains("Wave 256")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(target) = fn_body(g, "fn get_team_target_object(") else {
        return false;
    };
    let Some(recruit) = fn_body(g, "fn try_to_recruit(") else {
        return false;
    };
    let Some(kill) = fn_body(g, "fn kill_team(") else {
        return false;
    };
    target.contains("dual_world_registry_unavailable")
        && target.contains("INVALID_ID")
        && recruit.contains("dual_world_registry_unavailable")
        && kill.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_team_dual_world_empty_gate_honesty() -> bool {
    honesty_live_team_dual_world_empty_gate_residual_pack_wave256()
        && honesty_team_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_team_dual_world_empty_gate_method_names_residual_wave256());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_team_dual_world_empty_gate_nav_commands_residual_wave256());
    }

    #[test]
    fn wave256_composite_pack() {
        assert!(honesty_live_team_dual_world_empty_gate_residual_pack_wave256());
    }

    #[test]
    fn team_dual_world_empty_gate_sources() {
        assert!(honesty_team_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_team_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_team_dual_world_empty_gate_honesty(),
            "team dual-world empty gate residual must latch"
        );
    }
}
