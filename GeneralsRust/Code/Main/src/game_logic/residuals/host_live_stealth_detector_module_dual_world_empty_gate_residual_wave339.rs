//! Wave 339 residual peels: stealth::detector dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), detector
//! scan/team helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 297 StealthDetectorUpdate dual-world empty-gate residual
//! and Wave 338 TurretAI dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/stealth/detector.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Stealth detector module dual-world empty-gate residual method names.
pub const LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE339: &[&str] = &[
    "dual_world_registry_unavailable",
    "scan_for_stealth",
    "get_detector_position",
    "is_enemy",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE339: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_STEALTH_DETECTOR_MODULE_EMPTY_GATES",
    "LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE339:
    &[&str] = &[
    "click_live_stealth_detector_module_dual_world_empty_gate_ok_prepare",
    "click_live_stealth_detector_module_dual_world_empty_gate_ok_live",
    "click_live_stealth_detector_module_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_stealth_detector_module_dual_world_empty_gate_method_names_residual_wave339()
-> bool {
    LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE339.len() == 5
        && residual_name_index(
            LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE339,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE339,
            "is_enemy",
        ) == Some(3)
        && residual_name_index(
            LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE339,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_stealth_detector_module_dual_world_empty_gate_nav_commands_residual_wave339()
-> bool {
    LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE339.len() == 4
        && residual_name_index(
            LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE339,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE339,
            "LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_STEALTH_DETECTOR_MODULE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE339.len()
            == 3
}

/// Wave 339 composite residual honesty pack.
pub fn honesty_live_stealth_detector_module_dual_world_empty_gate_residual_pack_wave339() -> bool {
    honesty_live_stealth_detector_module_dual_world_empty_gate_method_names_residual_wave339()
        && honesty_live_stealth_detector_module_dual_world_empty_gate_nav_commands_residual_wave339(
        )
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

/// Source residual: stealth detector module empty dual-world short-circuits.
pub fn honesty_stealth_detector_module_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/stealth/detector.rs");
    if !(g.contains("Wave 339")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(scan) = fn_body(g, "fn scan_for_stealth(") else {
        return false;
    };
    let Some(pos) = fn_body(g, "fn get_detector_position(") else {
        return false;
    };
    let Some(enemy) = fn_body(g, "fn is_enemy(") else {
        return false;
    };
    helper_ok
        && scan.contains("return;")
        && pos.contains("return None")
        && enemy.contains("return false")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_stealth_detector_module_dual_world_empty_gate_honesty() -> bool {
    honesty_live_stealth_detector_module_dual_world_empty_gate_residual_pack_wave339()
        && honesty_stealth_detector_module_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_stealth_detector_module_dual_world_empty_gate_method_names_residual_wave339()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_stealth_detector_module_dual_world_empty_gate_nav_commands_residual_wave339()
        );
    }

    #[test]
    fn wave339_composite_pack() {
        assert!(honesty_live_stealth_detector_module_dual_world_empty_gate_residual_pack_wave339());
    }

    #[test]
    fn stealth_detector_module_dual_world_empty_gate_sources() {
        assert!(honesty_stealth_detector_module_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_stealth_detector_module_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_stealth_detector_module_dual_world_empty_gate_honesty(),
            "stealth detector module dual-world empty gate residual must latch"
        );
    }
}
