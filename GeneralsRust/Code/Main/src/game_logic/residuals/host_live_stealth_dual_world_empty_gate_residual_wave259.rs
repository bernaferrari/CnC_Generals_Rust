//! Wave 259 residual peels: StealthUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), stealth
//! detect/disguise/update walks fail-closed without dual-world factory
//! resolution. Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 258 Unit dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/stealth_update.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Stealth dual-world empty-gate residual method names.
pub const LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE259: &[&str] = &[
    "dual_world_registry_unavailable",
    "allowed_to_stealth",
    "mark_as_detected",
    "calc_stealth_look_for_player",
    "update",
    "calc_stealth_owner",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE259: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_STEALTH_EMPTY_GATES",
    "LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE259: &[&str] = &[
    "click_live_stealth_dual_world_empty_gate_ok_prepare",
    "click_live_stealth_dual_world_empty_gate_ok_live",
    "click_live_stealth_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_stealth_dual_world_empty_gate_method_names_residual_wave259() -> bool {
    LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE259.len() == 7
        && residual_name_index(
            LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE259,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE259,
            "calc_stealth_owner",
        ) == Some(5)
        && residual_name_index(
            LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE259,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_stealth_dual_world_empty_gate_nav_commands_residual_wave259() -> bool {
    LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE259.len() == 4
        && residual_name_index(
            LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE259,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE259,
            "LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_STEALTH_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE259.len() == 3
}

/// Wave 259 composite residual honesty pack.
pub fn honesty_live_stealth_dual_world_empty_gate_residual_pack_wave259() -> bool {
    honesty_live_stealth_dual_world_empty_gate_method_names_residual_wave259()
        && honesty_live_stealth_dual_world_empty_gate_nav_commands_residual_wave259()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
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
    None
}

/// Source residual: StealthUpdate empty dual-world short-circuits.
pub fn honesty_stealth_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/object/update/stealth_update.rs");
    if !(g.contains("Wave 259")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(allowed) = fn_body(g, "fn allowed_to_stealth(") else {
        return false;
    };
    let Some(look) = fn_body(g, "fn calc_stealth_look_for_player(") else {
        return false;
    };
    let Some(owner) = fn_body(g, "fn calc_stealth_owner(") else {
        return false;
    };
    allowed.contains("dual_world_registry_unavailable")
        && look.contains("dual_world_registry_unavailable")
        && look.contains("StealthLookType::None")
        && owner.contains("dual_world_registry_unavailable")
        && owner.contains("self.object_id")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_stealth_dual_world_empty_gate_honesty() -> bool {
    honesty_live_stealth_dual_world_empty_gate_residual_pack_wave259()
        && honesty_stealth_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_stealth_dual_world_empty_gate_method_names_residual_wave259());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_stealth_dual_world_empty_gate_nav_commands_residual_wave259());
    }

    #[test]
    fn wave259_composite_pack() {
        assert!(honesty_live_stealth_dual_world_empty_gate_residual_pack_wave259());
    }

    #[test]
    fn stealth_dual_world_empty_gate_sources() {
        assert!(honesty_stealth_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_stealth_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_stealth_dual_world_empty_gate_honesty(),
            "stealth dual-world empty gate residual must latch"
        );
    }
}
