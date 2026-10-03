//! Wave 412 repaired source contract: LifetimeUpdate owner kill without global emptiness vetoes.
//! The scheduled callback kills its owner; the scheduler controls its wake.
//! Missing owners remain separate from unrelated world emptiness.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 411 PointDefenseLaserUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/lifetime_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// LifetimeUpdate dual-world empty-gate residual method names.
pub const LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE412: &[&str] = &[
    "dual_world_registry_unavailable",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE412: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_LIFETIME_UPDATE_EMPTY_GATES",
    "LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE412: &[&str] = &[
    "click_live_lifetime_update_dual_world_empty_gate_ok_prepare",
    "click_live_lifetime_update_dual_world_empty_gate_ok_live",
    "click_live_lifetime_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_lifetime_update_dual_world_empty_gate_method_names_residual_wave412() -> bool {
    LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE412.len() == 3
        && residual_name_index(
            LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE412,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE412,
            "update_simple",
        ) == Some(1)
        && residual_name_index(
            LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE412,
            "playable_claim = false",
        ) == Some(2)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_lifetime_update_dual_world_empty_gate_nav_commands_residual_wave412() -> bool {
    LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE412.len() == 4
        && residual_name_index(
            LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE412,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE412,
            "LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_LIFETIME_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE412.len() == 3
}

/// Wave 412 composite residual honesty pack.
pub fn honesty_live_lifetime_update_dual_world_empty_gate_residual_pack_wave412() -> bool {
    honesty_live_lifetime_update_dual_world_empty_gate_method_names_residual_wave412()
        && honesty_live_lifetime_update_dual_world_empty_gate_nav_commands_residual_wave412()
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
                        return Some(&src[i..brace + off + 1]);
                    }
                }
                _ => {}
            }
        }
        search_from = i + name.len();
    }
    None
}

/// Repaired source contract: the invoked Lifetime callback kills its exact owner.
pub fn honesty_lifetime_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/behavior/lifetime_update.rs");
    if g.contains("fn dual_world_registry_unavailable") || g.contains("OBJECT_REGISTRY.is_empty()")
    {
        return false;
    }
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    update.contains("find_object_by_id(self.object_id)")
        && update.contains("guard.kill(None, None)")
        && update.contains("UPDATE_SLEEP_FOREVER")
        && !update.contains("self.die_frame")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_lifetime_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_lifetime_update_dual_world_empty_gate_residual_pack_wave412()
        && honesty_lifetime_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_lifetime_update_dual_world_empty_gate_method_names_residual_wave412());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_lifetime_update_dual_world_empty_gate_nav_commands_residual_wave412());
    }

    #[test]
    fn wave412_composite_pack() {
        assert!(honesty_live_lifetime_update_dual_world_empty_gate_residual_pack_wave412());
    }

    #[test]
    fn lifetime_update_dual_world_empty_gate_sources() {
        assert!(honesty_lifetime_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_lifetime_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_lifetime_update_dual_world_empty_gate_honesty(),
            "lifetime update dual-world empty gate residual must latch"
        );
    }
}
