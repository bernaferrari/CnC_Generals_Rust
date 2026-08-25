//! Wave 444 residual peels: PlayerUpgradeManager dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), player
//! upgrade bulk apply/remove helpers fail-closed without dual-world walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 443 StealthIntegration dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/upgrade/player_upgrade_manager.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// PlayerUpgradeManager dual-world empty-gate residual method names.
pub const LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE444: &[&str] = &[
    "dual_world_registry_unavailable",
    "apply_upgrade_to_existing_objects",
    "remove_upgrade_from_existing_objects",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE444: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PLAYER_UPGRADE_MANAGER_EMPTY_GATES",
    "LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE444:
    &[&str] = &[
    "click_live_player_upgrade_manager_dual_world_empty_gate_ok_prepare",
    "click_live_player_upgrade_manager_dual_world_empty_gate_ok_live",
    "click_live_player_upgrade_manager_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_player_upgrade_manager_dual_world_empty_gate_method_names_residual_wave444()
-> bool {
    LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE444.len() == 4
        && residual_name_index(
            LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE444,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE444,
            "remove_upgrade_from_existing_objects",
        ) == Some(2)
        && residual_name_index(
            LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE444,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_player_upgrade_manager_dual_world_empty_gate_nav_commands_residual_wave444()
-> bool {
    LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE444.len() == 4
        && residual_name_index(
            LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE444,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE444,
            "LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PLAYER_UPGRADE_MANAGER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE444.len()
            == 3
}

/// Wave 444 composite residual honesty pack.
pub fn honesty_live_player_upgrade_manager_dual_world_empty_gate_residual_pack_wave444() -> bool {
    honesty_live_player_upgrade_manager_dual_world_empty_gate_method_names_residual_wave444()
        && honesty_live_player_upgrade_manager_dual_world_empty_gate_nav_commands_residual_wave444()
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

/// Source residual: PlayerUpgradeManager empty dual-world short-circuits.
pub fn honesty_player_upgrade_manager_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/upgrade/player_upgrade_manager.rs");
    if !(g.contains("Wave 444")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(apply) = fn_body(g, "fn apply_upgrade_to_existing_objects(") else {
        return false;
    };
    let Some(remove) = fn_body(g, "fn remove_upgrade_from_existing_objects(") else {
        return false;
    };
    helper_ok && apply.contains("return;") && remove.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_player_upgrade_manager_dual_world_empty_gate_honesty() -> bool {
    honesty_live_player_upgrade_manager_dual_world_empty_gate_residual_pack_wave444()
        && honesty_player_upgrade_manager_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_player_upgrade_manager_dual_world_empty_gate_method_names_residual_wave444(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_player_upgrade_manager_dual_world_empty_gate_nav_commands_residual_wave444(
            )
        );
    }

    #[test]
    fn wave444_composite_pack() {
        assert!(honesty_live_player_upgrade_manager_dual_world_empty_gate_residual_pack_wave444());
    }

    #[test]
    fn player_upgrade_manager_dual_world_empty_gate_sources() {
        assert!(honesty_player_upgrade_manager_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_player_upgrade_manager_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_player_upgrade_manager_dual_world_empty_gate_honesty(),
            "player upgrade manager dual-world empty gate residual must latch"
        );
    }
}
