//! Wave 278 residual peels: Selection dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), selection
//! object lookup helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 277 RiderChangeContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/commands/selection.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Selection dual-world empty-gate residual method names.
pub const LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE278: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object_info",
    "get_all_objects",
    "is_object_alive",
    "can_player_control",
    "resolve_selection_target",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE278: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SELECTION_EMPTY_GATES",
    "LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE278: &[&str] = &[
    "click_live_selection_dual_world_empty_gate_ok_prepare",
    "click_live_selection_dual_world_empty_gate_ok_live",
    "click_live_selection_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_selection_dual_world_empty_gate_method_names_residual_wave278() -> bool {
    LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE278.len() == 7
        && residual_name_index(
            LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE278,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE278,
            "resolve_selection_target",
        ) == Some(5)
        && residual_name_index(
            LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE278,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_selection_dual_world_empty_gate_nav_commands_residual_wave278() -> bool {
    LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE278.len() == 4
        && residual_name_index(
            LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE278,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE278,
            "LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SELECTION_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE278.len() == 3
}

/// Wave 278 composite residual honesty pack.
pub fn honesty_live_selection_dual_world_empty_gate_residual_pack_wave278() -> bool {
    honesty_live_selection_dual_world_empty_gate_method_names_residual_wave278()
        && honesty_live_selection_dual_world_empty_gate_nav_commands_residual_wave278()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    // Prefer impl body (has dual-world gate), not trait default.
    let mut search_from = 0usize;
    while let Some(rel) = src[search_from..].find(name) {
        let i = search_from + rel;
        let brace = match src[i..].find('{') {
            Some(b) => i + b,
            None => {
                search_from = i + name.len();
                continue;
            }
        };
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

/// Source residual: Selection empty dual-world short-circuits.
pub fn honesty_selection_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/commands/selection.rs");
    if !(g.contains("Wave 278")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    // helper must not recurse
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(info) = fn_body(g, "fn get_object_info(") else {
        return false;
    };
    let Some(all) = fn_body(g, "fn get_all_objects(") else {
        return false;
    };
    let Some(resolve) = fn_body(g, "fn resolve_selection_target(") else {
        return false;
    };
    helper_ok
        && info.contains("dual_world_registry_unavailable")
        && all.contains("dual_world_registry_unavailable")
        && all.contains("Vec::new()")
        && resolve.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_selection_dual_world_empty_gate_honesty() -> bool {
    honesty_live_selection_dual_world_empty_gate_residual_pack_wave278()
        && honesty_selection_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_selection_dual_world_empty_gate_method_names_residual_wave278());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_selection_dual_world_empty_gate_nav_commands_residual_wave278());
    }

    #[test]
    fn wave278_composite_pack() {
        assert!(honesty_live_selection_dual_world_empty_gate_residual_pack_wave278());
    }

    #[test]
    fn selection_dual_world_empty_gate_sources() {
        assert!(honesty_selection_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_selection_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_selection_dual_world_empty_gate_honesty(),
            "selection dual-world empty gate residual must latch"
        );
    }
}
