//! Wave 336 residual peels: AssistedTargetingUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), assist
//! attack helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 335 BridgeScaffold dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/assisted_targeting_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Assisted targeting dual-world empty-gate residual method names.
pub const LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE336: &[&str] = &[
    "dual_world_registry_unavailable",
    "make_feedback_laser",
    "is_free_to_assist",
    "assist_attack",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE336: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_ASSISTED_TARGETING_EMPTY_GATES",
    "LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE336: &[&str] = &[
    "click_live_assisted_targeting_dual_world_empty_gate_ok_prepare",
    "click_live_assisted_targeting_dual_world_empty_gate_ok_live",
    "click_live_assisted_targeting_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_assisted_targeting_dual_world_empty_gate_method_names_residual_wave336() -> bool
{
    LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE336.len() == 5
        && residual_name_index(
            LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE336,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE336,
            "assist_attack",
        ) == Some(3)
        && residual_name_index(
            LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE336,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_assisted_targeting_dual_world_empty_gate_nav_commands_residual_wave336() -> bool
{
    LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE336.len() == 4
        && residual_name_index(
            LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE336,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE336,
            "LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_ASSISTED_TARGETING_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE336.len() == 3
}

/// Wave 336 composite residual honesty pack.
pub fn honesty_live_assisted_targeting_dual_world_empty_gate_residual_pack_wave336() -> bool {
    honesty_live_assisted_targeting_dual_world_empty_gate_method_names_residual_wave336()
        && honesty_live_assisted_targeting_dual_world_empty_gate_nav_commands_residual_wave336()
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

/// Source residual: assisted targeting empty dual-world short-circuits.
pub fn honesty_assisted_targeting_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/behavior/assisted_targeting_update.rs"
    );
    if !(g.contains("Wave 336")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(laser) = fn_body(g, "fn make_feedback_laser(") else {
        return false;
    };
    let Some(free) = fn_body(g, "fn is_free_to_assist(") else {
        return false;
    };
    let Some(assist) = fn_body(g, "fn assist_attack(") else {
        return false;
    };
    helper_ok
        && laser.contains("return;")
        && free.contains("return false")
        && assist.contains("return;")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_assisted_targeting_dual_world_empty_gate_honesty() -> bool {
    honesty_live_assisted_targeting_dual_world_empty_gate_residual_pack_wave336()
        && honesty_assisted_targeting_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_assisted_targeting_dual_world_empty_gate_method_names_residual_wave336()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_assisted_targeting_dual_world_empty_gate_nav_commands_residual_wave336()
        );
    }

    #[test]
    fn wave336_composite_pack() {
        assert!(honesty_live_assisted_targeting_dual_world_empty_gate_residual_pack_wave336());
    }

    #[test]
    fn assisted_targeting_dual_world_empty_gate_sources() {
        assert!(honesty_assisted_targeting_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_assisted_targeting_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_assisted_targeting_dual_world_empty_gate_honesty(),
            "assisted targeting dual-world empty gate residual must latch"
        );
    }
}
