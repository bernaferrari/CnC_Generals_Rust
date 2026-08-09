//! Wave 275 residual peels: CommandProcessor dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), command
//! executors fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 274 HelixContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/commands/command_processor.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// CommandProcessor dual-world empty-gate residual method names.
pub const LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE275: &[&str] = &[
    "dual_world_registry_unavailable",
    "execute_force_attack_ground",
    "execute_evacuate_command",
    "execute_exit_command",
    "execute_weapon_target_command",
    "execute_create_formation",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE275: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_COMMAND_PROCESSOR_EMPTY_GATES",
    "LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE275: &[&str] = &[
    "click_live_command_processor_dual_world_empty_gate_ok_prepare",
    "click_live_command_processor_dual_world_empty_gate_ok_live",
    "click_live_command_processor_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_processor_dual_world_empty_gate_method_names_residual_wave275() -> bool
{
    LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE275.len() == 7
        && residual_name_index(
            LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE275,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE275,
            "execute_weapon_target_command",
        ) == Some(4)
        && residual_name_index(
            LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE275,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_processor_dual_world_empty_gate_nav_commands_residual_wave275() -> bool
{
    LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE275.len() == 4
        && residual_name_index(
            LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE275,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE275,
            "LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_PROCESSOR_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE275.len() == 3
}

/// Wave 275 composite residual honesty pack.
pub fn honesty_live_command_processor_dual_world_empty_gate_residual_pack_wave275() -> bool {
    honesty_live_command_processor_dual_world_empty_gate_method_names_residual_wave275()
        && honesty_live_command_processor_dual_world_empty_gate_nav_commands_residual_wave275()
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

/// Source residual: CommandProcessor empty dual-world short-circuits.
pub fn honesty_command_processor_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/commands/command_processor.rs");
    if !(g.contains("Wave 275")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(force) = fn_body(g, "fn execute_force_attack_ground(") else {
        return false;
    };
    let Some(evac) = fn_body(g, "fn execute_evacuate_command(") else {
        return false;
    };
    let Some(weapon) = fn_body(g, "fn execute_weapon_target_command(") else {
        return false;
    };
    force.contains("dual_world_registry_unavailable")
        && force.contains("InvalidGameState")
        && evac.contains("dual_world_registry_unavailable")
        && weapon.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_processor_dual_world_empty_gate_honesty() -> bool {
    honesty_live_command_processor_dual_world_empty_gate_residual_pack_wave275()
        && honesty_command_processor_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_command_processor_dual_world_empty_gate_method_names_residual_wave275()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_command_processor_dual_world_empty_gate_nav_commands_residual_wave275()
        );
    }

    #[test]
    fn wave275_composite_pack() {
        assert!(honesty_live_command_processor_dual_world_empty_gate_residual_pack_wave275());
    }

    #[test]
    fn command_processor_dual_world_empty_gate_sources() {
        assert!(honesty_command_processor_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_command_processor_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_command_processor_dual_world_empty_gate_honesty(),
            "command processor dual-world empty gate residual must latch"
        );
    }
}
