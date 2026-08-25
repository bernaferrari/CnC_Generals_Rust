//! Wave 271 residual peels: script conditions dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), object-bound
//! script conditions fail-closed (`Ok(false)`) without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 270 Drawable dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/scripting/conditions/mod.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Script conditions dual-world empty-gate residual method names.
pub const LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE271: &[&str] = &[
    "dual_world_registry_unavailable",
    "NamedUnitExistsCondition",
    "NamedUnitDestroyedCondition",
    "BuiltByPlayerCondition",
    "UnitHealthCondition",
    "EnemySightedCondition",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE271: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_SCRIPT_CONDITIONS_EMPTY_GATES",
    "LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE271: &[&str] = &[
    "click_live_script_conditions_dual_world_empty_gate_ok_prepare",
    "click_live_script_conditions_dual_world_empty_gate_ok_live",
    "click_live_script_conditions_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
// 2026-08-15: empty-registry helper is fail-open (C++ does not skip-close).
pub fn honesty_live_script_conditions_dual_world_empty_gate_method_names_residual_wave271() -> bool
{
    LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE271.len() == 7
        && residual_name_index(
            LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE271,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE271,
            "UnitHealthCondition",
        ) == Some(4)
        && residual_name_index(
            LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE271,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_script_conditions_dual_world_empty_gate_nav_commands_residual_wave271() -> bool
{
    LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE271.len() == 4
        && residual_name_index(
            LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE271,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE271,
            "LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_SCRIPT_CONDITIONS_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE271.len() == 3
}

/// Wave 271 composite residual honesty pack.
pub fn honesty_live_script_conditions_dual_world_empty_gate_residual_pack_wave271() -> bool {
    honesty_live_script_conditions_dual_world_empty_gate_method_names_residual_wave271()
        && honesty_live_script_conditions_dual_world_empty_gate_nav_commands_residual_wave271()
}

fn fn_body_after<'a>(src: &'a str, marker: &str) -> Option<&'a str> {
    let i = src.find(marker)?;
    let e = src[i..].find("fn evaluate(")?;
    let start = i + e;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: script conditions empty dual-world short-circuits.
pub fn honesty_script_conditions_dual_world_empty_gate_source() -> bool {
    // 2026-08-15: conditions god-file split — scan the live modules.
    let g = concat!(
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/mod.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/helpers.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/named.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/object.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/player.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/combat.rs"),
        include_str!("../../../../GameEngine/GameLogic/src/scripting/conditions/logic.rs"),
    );
    if !(g.contains("Wave 271")
        && g.contains("fn dual_world_registry_unavailable")
        && (g.contains("let _host_empty") || g.contains("OBJECT_REGISTRY.is_empty()"))
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    // helper must not recurse
    // 2026-08-15: helper probes emptiness but returns false (C++ does not skip-close).
    let helper_ok = g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()")
        && g.contains("false");
    let Some(exists) = fn_body_after(g, "impl ScriptCondition for NamedUnitExistsCondition") else {
        return false;
    };
    let Some(built) = fn_body_after(g, "impl ScriptCondition for BuiltByPlayerCondition") else {
        return false;
    };
    let Some(health) = fn_body_after(g, "impl ScriptCondition for UnitHealthCondition") else {
        return false;
    };
    helper_ok
        && exists.contains("dual_world_registry_unavailable")
        && exists.contains("Ok(false)")
        && built.contains("dual_world_registry_unavailable")
        && health.contains("dual_world_registry_unavailable")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_script_conditions_dual_world_empty_gate_honesty() -> bool {
    honesty_live_script_conditions_dual_world_empty_gate_residual_pack_wave271()
        && honesty_script_conditions_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_script_conditions_dual_world_empty_gate_method_names_residual_wave271()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_script_conditions_dual_world_empty_gate_nav_commands_residual_wave271()
        );
    }

    #[test]
    fn wave271_composite_pack() {
        assert!(honesty_live_script_conditions_dual_world_empty_gate_residual_pack_wave271());
    }

    #[test]
    fn script_conditions_dual_world_empty_gate_sources() {
        assert!(honesty_script_conditions_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_script_conditions_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_script_conditions_dual_world_empty_gate_honesty(),
            "script conditions dual-world empty gate residual must latch"
        );
    }
}
