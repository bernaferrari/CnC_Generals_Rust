//! Wave 344 residual peels: system GameLogic dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), core update/
//! cleanup/energy helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 343 script evaluator dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/system/game_logic.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - `register_object` intentionally ungated so first dual-world inserts remain possible

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// System GameLogic dual-world empty-gate residual method names.
pub const LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE344: &[&str] = &[
    "dual_world_registry_unavailable",
    "update",
    "cleanup_dead_objects",
    "energy_production",
    "energy_bonus",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE344: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SYSTEM_GAME_LOGIC_EMPTY_GATES",
    "LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE344: &[&str] = &[
    "click_live_system_game_logic_dual_world_empty_gate_ok_prepare",
    "click_live_system_game_logic_dual_world_empty_gate_ok_live",
    "click_live_system_game_logic_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_system_game_logic_dual_world_empty_gate_method_names_residual_wave344() -> bool
{
    LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE344.len() == 6
        && residual_name_index(
            LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE344,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE344,
            "energy_bonus",
        ) == Some(4)
        && residual_name_index(
            LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE344,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_system_game_logic_dual_world_empty_gate_nav_commands_residual_wave344() -> bool
{
    LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE344.len() == 4
        && residual_name_index(
            LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE344,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE344,
            "LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SYSTEM_GAME_LOGIC_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE344.len() == 3
}

/// Wave 344 composite residual honesty pack.
pub fn honesty_live_system_game_logic_dual_world_empty_gate_residual_pack_wave344() -> bool {
    honesty_live_system_game_logic_dual_world_empty_gate_method_names_residual_wave344()
        && honesty_live_system_game_logic_dual_world_empty_gate_nav_commands_residual_wave344()
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

/// Source residual: system GameLogic empty dual-world short-circuits.
pub fn honesty_system_game_logic_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/system/game_logic.rs");
    if !(g.contains("Wave 344")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    // register_object must remain ungated so empty registries can still accept inserts.
    if let Some(reg) = fn_body(g, "fn register_object(") {
        if reg.contains("dual_world_registry_unavailable") {
            return false;
        }
    }
    let Some(upd) = fn_body(g, "fn update(") else {
        return false;
    };
    let Some(clean) = fn_body(g, "fn cleanup_dead_objects(") else {
        return false;
    };
    let Some(prod) = fn_body(g, "fn energy_production(") else {
        return false;
    };
    let Some(bonus) = fn_body(g, "fn energy_bonus(") else {
        return false;
    };
    helper_ok
        && upd.contains("return Ok(())")
        && clean.contains("return Ok(())")
        && prod.contains("return 0")
        && bonus.contains("return 0")
        && g.contains("fn rebuild(")
        && g.contains("fn resolve_damage_and_physics(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_system_game_logic_dual_world_empty_gate_honesty() -> bool {
    honesty_live_system_game_logic_dual_world_empty_gate_residual_pack_wave344()
        && honesty_system_game_logic_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_system_game_logic_dual_world_empty_gate_method_names_residual_wave344()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_system_game_logic_dual_world_empty_gate_nav_commands_residual_wave344()
        );
    }

    #[test]
    fn wave344_composite_pack() {
        assert!(honesty_live_system_game_logic_dual_world_empty_gate_residual_pack_wave344());
    }

    #[test]
    fn system_game_logic_dual_world_empty_gate_sources() {
        assert!(honesty_system_game_logic_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_system_game_logic_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_system_game_logic_dual_world_empty_gate_honesty(),
            "system game logic dual-world empty gate residual must latch"
        );
    }
}
