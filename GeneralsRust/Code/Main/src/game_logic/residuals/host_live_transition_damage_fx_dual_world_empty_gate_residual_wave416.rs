//! Wave 416 residual peels: TransitionDamageFX dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), transition
//! damage FX helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 415 DamageModule dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/damage/transition_damage_fx.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// TransitionDamageFX dual-world empty-gate residual method names.
pub const LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE416: &[&str] = &[
    "dual_world_registry_unavailable",
    "resolve_damage_source_pos",
    "on_body_damage_state_change",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE416: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_TRANSITION_DAMAGE_FX_EMPTY_GATES",
    "LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE416:
    &[&str] = &[
    "click_live_transition_damage_fx_dual_world_empty_gate_ok_prepare",
    "click_live_transition_damage_fx_dual_world_empty_gate_ok_live",
    "click_live_transition_damage_fx_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_transition_damage_fx_dual_world_empty_gate_method_names_residual_wave416()
-> bool {
    LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE416.len() == 4
        && residual_name_index(
            LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE416,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE416,
            "on_body_damage_state_change",
        ) == Some(2)
        && residual_name_index(
            LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE416,
            "playable_claim = false",
        ) == Some(3)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_transition_damage_fx_dual_world_empty_gate_nav_commands_residual_wave416()
-> bool {
    LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE416.len() == 4
        && residual_name_index(
            LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE416,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE416,
            "LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_TRANSITION_DAMAGE_FX_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE416.len() == 3
}

/// Wave 416 composite residual honesty pack.
pub fn honesty_live_transition_damage_fx_dual_world_empty_gate_residual_pack_wave416() -> bool {
    honesty_live_transition_damage_fx_dual_world_empty_gate_method_names_residual_wave416()
        && honesty_live_transition_damage_fx_dual_world_empty_gate_nav_commands_residual_wave416()
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

/// Source residual: TransitionDamageFX empty dual-world short-circuits.
pub fn honesty_transition_damage_fx_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../../GameEngine/GameLogic/src/object/damage/transition_damage_fx.rs");
    if !(g.contains("Wave 416")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(resolve) = fn_body(g, "fn resolve_damage_source_pos(") else {
        return false;
    };
    let Some(on_body) = fn_body(g, "fn on_body_damage_state_change(") else {
        return false;
    };
    helper_ok && resolve.contains("return None") && on_body.contains("return Ok(())")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_transition_damage_fx_dual_world_empty_gate_honesty() -> bool {
    honesty_live_transition_damage_fx_dual_world_empty_gate_residual_pack_wave416()
        && honesty_transition_damage_fx_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_transition_damage_fx_dual_world_empty_gate_method_names_residual_wave416()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_transition_damage_fx_dual_world_empty_gate_nav_commands_residual_wave416()
        );
    }

    #[test]
    fn wave416_composite_pack() {
        assert!(honesty_live_transition_damage_fx_dual_world_empty_gate_residual_pack_wave416());
    }

    #[test]
    fn transition_damage_fx_dual_world_empty_gate_sources() {
        assert!(honesty_transition_damage_fx_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_transition_damage_fx_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_transition_damage_fx_dual_world_empty_gate_honesty(),
            "transition damage fx dual-world empty gate residual must latch"
        );
    }
}
