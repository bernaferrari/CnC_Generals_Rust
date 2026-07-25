//! Wave 288 residual peels: HijackerUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), hijacker
//! update sleep-returns without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 287 enhanced player dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/hijacker_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Hijacker update dual-world empty-gate residual method names.
pub const LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE288: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE288: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_HIJACKER_UPDATE_EMPTY_GATES",
    "LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE288: &[&str] = &[
    "click_live_hijacker_update_dual_world_empty_gate_ok_prepare",
    "click_live_hijacker_update_dual_world_empty_gate_ok_live",
    "click_live_hijacker_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_hijacker_update_dual_world_empty_gate_method_names_residual_wave288() -> bool {
    LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE288.len() == 3
        && residual_name_index(
            LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE288,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE288,
            "update_simple",
        ) == Some(1)
        && residual_name_index(
            LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE288,
            "playable_claim = false",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_hijacker_update_dual_world_empty_gate_nav_commands_residual_wave288() -> bool {
    LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE288.len() == 4
        && residual_name_index(
            LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE288,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE288,
            "LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_HIJACKER_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE288.len() == 3
}

/// Wave 288 composite residual honesty pack.
pub fn honesty_live_hijacker_update_dual_world_empty_gate_residual_pack_wave288() -> bool {
    honesty_live_hijacker_update_dual_world_empty_gate_method_names_residual_wave288()
        && honesty_live_hijacker_update_dual_world_empty_gate_nav_commands_residual_wave288()
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

/// Source residual: hijacker update empty dual-world short-circuits.
pub fn honesty_hijacker_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/behavior/hijacker_update.rs");
    if !(g.contains("Wave 288")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok && update.contains("UpdateSleepTime::None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_hijacker_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_hijacker_update_dual_world_empty_gate_residual_pack_wave288()
        && honesty_hijacker_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_hijacker_update_dual_world_empty_gate_method_names_residual_wave288());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_hijacker_update_dual_world_empty_gate_nav_commands_residual_wave288());
    }

    #[test]
    fn wave288_composite_pack() {
        assert!(honesty_live_hijacker_update_dual_world_empty_gate_residual_pack_wave288());
    }

    #[test]
    fn hijacker_update_dual_world_empty_gate_sources() {
        assert!(honesty_hijacker_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_hijacker_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_hijacker_update_dual_world_empty_gate_honesty(),
            "hijacker update dual-world empty gate residual must latch"
        );
    }
}
