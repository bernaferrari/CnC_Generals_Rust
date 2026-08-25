//! Wave 279 residual peels: CaveContain dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), cave
//! contain helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 278 Selection dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/contain/cave_contain.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// CaveContain dual-world empty-gate residual method names.
pub const LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE279: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_object",
    "on_containing",
    "on_removing",
    "add_to_contain",
    "recalc_apparent_controlling_player",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE279: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_CAVE_CONTAIN_EMPTY_GATES",
    "LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE279: &[&str] = &[
    "click_live_cave_contain_dual_world_empty_gate_ok_prepare",
    "click_live_cave_contain_dual_world_empty_gate_ok_live",
    "click_live_cave_contain_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_cave_contain_dual_world_empty_gate_method_names_residual_wave279() -> bool {
    LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE279.len() == 7
        && residual_name_index(
            LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE279,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE279,
            "recalc_apparent_controlling_player",
        ) == Some(5)
        && residual_name_index(
            LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE279,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_cave_contain_dual_world_empty_gate_nav_commands_residual_wave279() -> bool {
    LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE279.len() == 4
        && residual_name_index(
            LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE279,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE279,
            "LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CAVE_CONTAIN_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE279.len() == 3
}

/// Wave 279 composite residual honesty pack.
pub fn honesty_live_cave_contain_dual_world_empty_gate_residual_pack_wave279() -> bool {
    honesty_live_cave_contain_dual_world_empty_gate_method_names_residual_wave279()
        && honesty_live_cave_contain_dual_world_empty_gate_nav_commands_residual_wave279()
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

/// Source residual: CaveContain empty dual-world short-circuits.
pub fn honesty_cave_contain_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/contain/cave_contain.rs");
    if !(g.contains("Wave 279")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    // 2026-08-15: helper probes emptiness but returns false (C++ does not skip-close).
    let helper_ok = g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()")
        && g.contains("false");
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(add) = fn_body(g, "fn add_to_contain(") else {
        return false;
    };
    let Some(recalc) = fn_body(g, "fn recalc_apparent_controlling_player(") else {
        return false;
    };
    helper_ok
        && get.contains("dual_world_registry_unavailable")
        && add.contains("dual_world_registry_unavailable")
        && recalc.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_cave_contain_dual_world_empty_gate_honesty() -> bool {
    honesty_live_cave_contain_dual_world_empty_gate_residual_pack_wave279()
        && honesty_cave_contain_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_cave_contain_dual_world_empty_gate_method_names_residual_wave279());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_cave_contain_dual_world_empty_gate_nav_commands_residual_wave279());
    }

    #[test]
    fn wave279_composite_pack() {
        assert!(honesty_live_cave_contain_dual_world_empty_gate_residual_pack_wave279());
    }

    #[test]
    fn cave_contain_dual_world_empty_gate_sources() {
        assert!(honesty_cave_contain_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_cave_contain_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_cave_contain_dual_world_empty_gate_honesty(),
            "cave contain dual-world empty gate residual must latch"
        );
    }
}
