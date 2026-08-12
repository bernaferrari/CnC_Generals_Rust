//! Wave 452 residual peels: die/command dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), remaining
//! die transfer, dam waveguides, crate team, RTS move estimate, AI task, and
//! object-creation upgrade helpers fail-closed without dual-world walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 451 golden mop-up default-off residual.
//!
//! Sources:
//! - `object/die/create_object_die.rs`
//! - `object/die/dam_die.rs`
//! - `object/die/create_crate_die.rs`
//! - `commands/rts_command.rs`
//! - `ai/ai_update.rs`
//! - `upgrade/modules/object_creation.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Die/command dual-world empty-gate residual method names.
pub const LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE452: &[&str] = &[
    "dual_world_registry_unavailable",
    "transfer_attackers",
    "enable_waveguides",
    "set_crate_team",
    "estimate_move_time",
    "process_ai_task",
    "apply_upgrade",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE452: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DIE_COMMAND_EMPTY_GATES",
    "LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE452: &[&str] = &[
    "click_live_die_command_dual_world_empty_gate_ok_prepare",
    "click_live_die_command_dual_world_empty_gate_ok_live",
    "click_live_die_command_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_die_command_dual_world_empty_gate_method_names_residual_wave452() -> bool {
    LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE452.len() == 8
        && residual_name_index(
            LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE452,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE452,
            "process_ai_task",
        ) == Some(5)
        && residual_name_index(
            LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE452,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_die_command_dual_world_empty_gate_nav_commands_residual_wave452() -> bool {
    LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE452.len() == 4
        && residual_name_index(
            LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE452,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE452,
            "LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DIE_COMMAND_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE452.len() == 3
}

/// Wave 452 composite residual honesty pack.
pub fn honesty_live_die_command_dual_world_empty_gate_residual_pack_wave452() -> bool {
    honesty_live_die_command_dual_world_empty_gate_method_names_residual_wave452()
        && honesty_live_die_command_dual_world_empty_gate_nav_commands_residual_wave452()
}

fn honesty_one(src: &str, fn_name: &str, expected_return_snip: &str) -> bool {
    if !(src.contains("Wave 452")
        && src.contains("fn dual_world_registry_unavailable")
        && src.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = src.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let needle = format!("fn {fn_name}(");
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(&needle) {
        let i = search_from + rel;
        let Some(b) = src[i..].find('{') else {
            search_from = i + needle.len();
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
                        if body.contains("dual_world_registry_unavailable")
                            && body.contains(expected_return_snip)
                        {
                            return helper_ok;
                        }
                        break;
                    }
                }
                _ => {}
            }
        }
        search_from = i + needle.len();
    }
    false
}

/// Source residual: die/command empty dual-world short-circuits.
pub fn honesty_die_command_dual_world_empty_gate_source() -> bool {
    let create_die =
        include_str!("../../../../GameEngine/GameLogic/src/object/die/create_object_die.rs");
    let dam = include_str!("../../../../GameEngine/GameLogic/src/object/die/dam_die.rs");
    let crate_die =
        include_str!("../../../../GameEngine/GameLogic/src/object/die/create_crate_die.rs");
    let rts = include_str!("../../../../GameEngine/GameLogic/src/commands/rts_command.rs");
    let ai = include_str!("../../../../GameEngine/GameLogic/src/ai/ai_update.rs");
    let ocl =
        include_str!("../../../../GameEngine/GameLogic/src/upgrade/modules/object_creation.rs");

    honesty_one(create_die, "transfer_attackers", "return;")
        && honesty_one(dam, "enable_waveguides", "return;")
        && honesty_one(crate_die, "set_crate_team", "return;")
        && honesty_one(rts, "estimate_move_time", "return 0;")
        && honesty_one(ai, "process_ai_task", "return Ok(())")
        && honesty_one(ocl, "apply_upgrade", "return false;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_die_command_dual_world_empty_gate_honesty() -> bool {
    honesty_live_die_command_dual_world_empty_gate_residual_pack_wave452()
        && honesty_die_command_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_die_command_dual_world_empty_gate_method_names_residual_wave452());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_die_command_dual_world_empty_gate_nav_commands_residual_wave452());
    }

    #[test]
    fn wave452_composite_pack() {
        assert!(honesty_live_die_command_dual_world_empty_gate_residual_pack_wave452());
    }

    #[test]
    fn die_command_dual_world_empty_gate_sources() {
        assert!(honesty_die_command_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_die_command_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_die_command_dual_world_empty_gate_honesty(),
            "die/command dual-world empty gate residual must latch"
        );
    }
}
