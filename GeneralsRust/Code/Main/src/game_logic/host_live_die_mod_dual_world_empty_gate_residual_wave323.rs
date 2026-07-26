//! Wave 323 residual peels: DieModule dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), die
//! resolve/on_die helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 322 TensileFormation dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/die/mod.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Die module dual-world empty-gate residual method names.
pub const LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE323: &[&str] = &[
    "dual_world_registry_unavailable",
    "with_object",
    "with_object_mut",
    "get_object",
    "on_die",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE323: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DIE_MOD_EMPTY_GATES",
    "LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE323: &[&str] = &[
    "click_live_die_mod_dual_world_empty_gate_ok_prepare",
    "click_live_die_mod_dual_world_empty_gate_ok_live",
    "click_live_die_mod_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_die_mod_dual_world_empty_gate_method_names_residual_wave323() -> bool {
    LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE323.len() == 6
        && residual_name_index(
            LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE323,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE323,
            "on_die",
        ) == Some(4)
        && residual_name_index(
            LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE323,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_die_mod_dual_world_empty_gate_nav_commands_residual_wave323() -> bool {
    LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE323.len() == 4
        && residual_name_index(
            LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE323,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE323,
            "LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DIE_MOD_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE323.len() == 3
}

/// Wave 323 composite residual honesty pack.
pub fn honesty_live_die_mod_dual_world_empty_gate_residual_pack_wave323() -> bool {
    honesty_live_die_mod_dual_world_empty_gate_method_names_residual_wave323()
        && honesty_live_die_mod_dual_world_empty_gate_nav_commands_residual_wave323()
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

/// Source residual: die module empty dual-world short-circuits.
pub fn honesty_die_mod_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/die/mod.rs");
    if !(g.contains("Wave 323")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let with_count = g.matches("fn with_object").count();
    let gated_with = g.matches("// Wave 323: empty dual-world → None.").count();
    let Some(on_die) = fn_body(g, "fn on_die(") else {
        return false;
    };
    helper_ok
        && with_count >= 2
        && gated_with >= 4
        && on_die.contains("Ok(())")
        && g.contains("fn get_object(")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_die_mod_dual_world_empty_gate_honesty() -> bool {
    honesty_live_die_mod_dual_world_empty_gate_residual_pack_wave323()
        && honesty_die_mod_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_die_mod_dual_world_empty_gate_method_names_residual_wave323());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_die_mod_dual_world_empty_gate_nav_commands_residual_wave323());
    }

    #[test]
    fn wave323_composite_pack() {
        assert!(honesty_live_die_mod_dual_world_empty_gate_residual_pack_wave323());
    }

    #[test]
    fn die_mod_dual_world_empty_gate_sources() {
        assert!(honesty_die_mod_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_die_mod_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_die_mod_dual_world_empty_gate_honesty(),
            "die mod dual-world empty gate residual must latch"
        );
    }
}
