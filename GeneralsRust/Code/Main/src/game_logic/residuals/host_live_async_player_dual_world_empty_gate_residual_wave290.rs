//! Historical Wave 290 async-player gate, retained for shell command/result compatibility.
//! The async-player implementation was an undeclared archive and is retired.
//! This source gate checks that classic AI and the live factory installer are
//! declared while the retired modules remain undeclared. It proves no async
//! runtime behavior or gameplay parity. Shell `playable_claim` stays false.
//!
//! Orthogonal to Wave 289 weapon.rs dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/mod.rs` compiled classic AI declarations
//! - `GameLogic/src/lib.rs` compiled live module-factory declaration
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - The remaining classic AI registry gates are checked independently

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Historical names from the retired source; compatibility labels only.
pub const LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_game_state",
    "evaluate_threats",
    "generate_tasks_from_strategy",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ASYNC_PLAYER_EMPTY_GATES",
    "LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE290: &[&str] = &[
    "click_live_async_player_dual_world_empty_gate_ok_prepare",
    "click_live_async_player_dual_world_empty_gate_ok_live",
    "click_live_async_player_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290() -> bool {
    LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290.len() == 5
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290,
            "generate_tasks_from_strategy",
        ) == Some(3)
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE290,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290() -> bool {
    LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290.len() == 4
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE290,
            "LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ASYNC_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE290.len() == 3
}

/// Wave 290 composite residual honesty pack.
pub fn honesty_live_async_player_dual_world_empty_gate_residual_pack_wave290() -> bool {
    honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290()
        && honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290()
}

fn module_declared(src: &str, name: &str) -> bool {
    let private = format!("mod {name};");
    let public = format!("pub mod {name};");
    src.lines().any(|line| {
        let declaration = line.split("//").next().unwrap_or("").trim();
        declaration == private || declaration == public
    })
}

/// Retirement source contract. The public name remains a compatibility alias;
/// the removed archive supplies no executable evidence.
pub fn honesty_async_player_dual_world_empty_gate_source() -> bool {
    let ai = include_str!("../../../../GameEngine/GameLogic/src/ai/mod.rs");
    let root = include_str!("../../../../GameEngine/GameLogic/src/lib.rs");
    module_declared(root, "ai")
        && module_declared(ai, "ai_core")
        && module_declared(ai, "ai_player")
        && !module_declared(ai, "async_player")
        && module_declared(root, "contain_module_overrides")
        && !module_declared(root, "module_overrides")
}

/// Historical shell smoke latch for the compatibility names and retirement contract.
pub fn simulate_live_async_player_dual_world_empty_gate_honesty() -> bool {
    honesty_live_async_player_dual_world_empty_gate_residual_pack_wave290()
        && honesty_async_player_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_async_player_dual_world_empty_gate_method_names_residual_wave290());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_async_player_dual_world_empty_gate_nav_commands_residual_wave290());
    }

    #[test]
    fn wave290_composite_pack() {
        assert!(honesty_live_async_player_dual_world_empty_gate_residual_pack_wave290());
    }

    #[test]
    fn async_player_dual_world_empty_gate_sources() {
        assert!(honesty_async_player_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_async_player_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_async_player_dual_world_empty_gate_honesty(),
            "historical async-player gate must attest retirement and retain compatibility names"
        );
    }
}
