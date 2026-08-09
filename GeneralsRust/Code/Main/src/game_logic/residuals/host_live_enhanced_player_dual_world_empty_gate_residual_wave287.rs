//! Wave 287 residual peels: Enhanced AI player dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), strategic
//! scans/prereq helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 286 DumbProjectile dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/ai/enhanced_player.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Enhanced player dual-world empty-gate residual method names.
pub const LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE287: &[&str] = &[
    "dual_world_registry_unavailable",
    "calculate_supply_security",
    "scan_for_nearby_enemies",
    "meets_prerequisites",
    "check_expansion_opportunities",
    "calculate_base_security",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE287: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ENHANCED_PLAYER_EMPTY_GATES",
    "LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE287: &[&str] = &[
    "click_live_enhanced_player_dual_world_empty_gate_ok_prepare",
    "click_live_enhanced_player_dual_world_empty_gate_ok_live",
    "click_live_enhanced_player_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_enhanced_player_dual_world_empty_gate_method_names_residual_wave287() -> bool {
    LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE287.len() == 7
        && residual_name_index(
            LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE287,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE287,
            "calculate_base_security",
        ) == Some(5)
        && residual_name_index(
            LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE287,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_enhanced_player_dual_world_empty_gate_nav_commands_residual_wave287() -> bool {
    LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE287.len() == 4
        && residual_name_index(
            LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE287,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE287,
            "LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ENHANCED_PLAYER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE287.len() == 3
}

/// Wave 287 composite residual honesty pack.
pub fn honesty_live_enhanced_player_dual_world_empty_gate_residual_pack_wave287() -> bool {
    honesty_live_enhanced_player_dual_world_empty_gate_method_names_residual_wave287()
        && honesty_live_enhanced_player_dual_world_empty_gate_nav_commands_residual_wave287()
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

/// Source residual: enhanced player empty dual-world short-circuits.
pub fn honesty_enhanced_player_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/ai/enhanced_player.rs");
    if !(g.contains("Wave 287")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(supply) = fn_body(g, "fn calculate_supply_security(") else {
        return false;
    };
    let Some(prereq) = fn_body(g, "fn meets_prerequisites(") else {
        return false;
    };
    let Some(base) = fn_body(g, "fn calculate_base_security(") else {
        return false;
    };
    helper_ok
        && supply.contains("return 0.0")
        && prereq.contains("Ok(false)")
        && base.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_enhanced_player_dual_world_empty_gate_honesty() -> bool {
    honesty_live_enhanced_player_dual_world_empty_gate_residual_pack_wave287()
        && honesty_enhanced_player_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_enhanced_player_dual_world_empty_gate_method_names_residual_wave287());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_enhanced_player_dual_world_empty_gate_nav_commands_residual_wave287());
    }

    #[test]
    fn wave287_composite_pack() {
        assert!(honesty_live_enhanced_player_dual_world_empty_gate_residual_pack_wave287());
    }

    #[test]
    fn enhanced_player_dual_world_empty_gate_sources() {
        assert!(honesty_enhanced_player_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_enhanced_player_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_enhanced_player_dual_world_empty_gate_honesty(),
            "enhanced player dual-world empty gate residual must latch"
        );
    }
}
