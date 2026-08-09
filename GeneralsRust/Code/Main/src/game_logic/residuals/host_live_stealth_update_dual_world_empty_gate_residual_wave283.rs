//! Wave 283 residual peels: StealthUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), stealth
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 282 AIUpdateInterface dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/stealth_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - disguise_as_template keeps local template/state writes

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// StealthUpdate dual-world empty-gate residual method names.
pub const LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE283: &[&str] = &[
    "dual_world_registry_unavailable",
    "current_status",
    "update_stealth",
    "receive_grant",
    "is_too_close_to_current_target",
    "register_with_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE283: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_STEALTH_UPDATE_EMPTY_GATES",
    "LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE283: &[&str] = &[
    "click_live_stealth_update_dual_world_empty_gate_ok_prepare",
    "click_live_stealth_update_dual_world_empty_gate_ok_live",
    "click_live_stealth_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_stealth_update_dual_world_empty_gate_method_names_residual_wave283() -> bool {
    LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE283.len() == 7
        && residual_name_index(
            LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE283,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE283,
            "register_with_object",
        ) == Some(5)
        && residual_name_index(
            LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE283,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_stealth_update_dual_world_empty_gate_nav_commands_residual_wave283() -> bool {
    LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE283.len() == 4
        && residual_name_index(
            LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE283,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE283,
            "LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_STEALTH_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE283.len() == 3
}

/// Wave 283 composite residual honesty pack.
pub fn honesty_live_stealth_update_dual_world_empty_gate_residual_pack_wave283() -> bool {
    honesty_live_stealth_update_dual_world_empty_gate_method_names_residual_wave283()
        && honesty_live_stealth_update_dual_world_empty_gate_nav_commands_residual_wave283()
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

/// Source residual: StealthUpdate empty dual-world short-circuits.
pub fn honesty_stealth_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/stealth_update.rs");
    if !(g.contains("Wave 283")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(status) = fn_body(g, "fn current_status(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_stealth(") else {
        return false;
    };
    let Some(reg) = fn_body(g, "fn register_with_object(") else {
        return false;
    };
    // disguise_as_template must not early-return solely on empty dual-world
    let disguise_ok = !g.contains(
        "fn disguise_as_template(\n        &mut self,\n        template_name: Option<String>,\n        _current_frame: UnsignedInt,\n    ) {\n        // Wave 283:",
    );
    helper_ok
        && status.contains("dual-world registry unavailable")
        && update.contains("dual_world_registry_unavailable")
        && update.contains("if !self.enabled")
        && reg.contains("dual_world_registry_unavailable")
        && disguise_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_stealth_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_stealth_update_dual_world_empty_gate_residual_pack_wave283()
        && honesty_stealth_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_stealth_update_dual_world_empty_gate_method_names_residual_wave283());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_stealth_update_dual_world_empty_gate_nav_commands_residual_wave283());
    }

    #[test]
    fn wave283_composite_pack() {
        assert!(honesty_live_stealth_update_dual_world_empty_gate_residual_pack_wave283());
    }

    #[test]
    fn stealth_update_dual_world_empty_gate_sources() {
        assert!(honesty_stealth_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_stealth_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_stealth_update_dual_world_empty_gate_honesty(),
            "stealth update dual-world empty gate residual must latch"
        );
    }
}
