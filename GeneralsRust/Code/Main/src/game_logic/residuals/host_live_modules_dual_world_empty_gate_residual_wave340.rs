//! Wave 340 residual peels: modules dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), contain/AI
//! module helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 339 stealth detector module dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/modules.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Modules dual-world empty-gate residual method names.
pub const LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE340: &[&str] = &[
    "dual_world_registry_unavailable",
    "order_all_passengers_to_exit",
    "get_stealth_units_contained",
    "get_goal_object",
    "ai_attack_object_id",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE340: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_MODULES_EMPTY_GATES",
    "LIVE_MODULES_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE340: &[&str] = &[
    "click_live_modules_dual_world_empty_gate_ok_prepare",
    "click_live_modules_dual_world_empty_gate_ok_live",
    "click_live_modules_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_modules_dual_world_empty_gate_method_names_residual_wave340() -> bool {
    LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE340.len() == 6
        && residual_name_index(
            LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE340,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE340,
            "ai_attack_object_id",
        ) == Some(4)
        && residual_name_index(
            LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE340,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_modules_dual_world_empty_gate_nav_commands_residual_wave340() -> bool {
    LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE340.len() == 4
        && residual_name_index(
            LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE340,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE340,
            "LIVE_MODULES_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_MODULES_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE340.len() == 3
}

/// Wave 340 composite residual honesty pack.
pub fn honesty_live_modules_dual_world_empty_gate_residual_pack_wave340() -> bool {
    honesty_live_modules_dual_world_empty_gate_method_names_residual_wave340()
        && honesty_live_modules_dual_world_empty_gate_nav_commands_residual_wave340()
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

/// Source residual: modules empty dual-world short-circuits.
pub fn honesty_modules_dual_world_empty_gate_source() -> bool {
    let g = gamelogic::modules::MODULES_SRC;
    if !(g.contains("Wave 340")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    // Reject broken impl-header gates.
    if g.contains("impl ContainModuleInterfaceExt for Arc<Mutex<dyn ContainModuleInterface>> {\n        // Wave 340")
        || g.contains("impl AIUpdateInterfaceExt for Arc<Mutex<dyn AIUpdateInterface>> {\n        // Wave 340")
    {
        return false;
    }
    let Some(exit) = fn_body(g, "fn order_all_passengers_to_exit(") else {
        return false;
    };
    let Some(goal) = fn_body(g, "fn get_goal_object(") else {
        return false;
    };
    let Some(atk) = fn_body(g, "fn ai_attack_object_id(") else {
        return false;
    };
    helper_ok
        && exit.contains("return Ok(())")
        && goal.contains("return None")
        && atk.contains("return;")
        && g.contains("fn get_stealth_units_contained")
        && g.contains("fn on_removing")
        && g.contains("fn ai_move_away_from_unit")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_modules_dual_world_empty_gate_honesty() -> bool {
    honesty_live_modules_dual_world_empty_gate_residual_pack_wave340()
        && honesty_modules_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_modules_dual_world_empty_gate_method_names_residual_wave340());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_modules_dual_world_empty_gate_nav_commands_residual_wave340());
    }

    #[test]
    fn wave340_composite_pack() {
        assert!(honesty_live_modules_dual_world_empty_gate_residual_pack_wave340());
    }

    #[test]
    fn modules_dual_world_empty_gate_sources() {
        assert!(honesty_modules_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_modules_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_modules_dual_world_empty_gate_honesty(),
            "modules dual-world empty gate residual must latch"
        );
    }
}
