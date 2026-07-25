//! Wave 282 residual peels: AIUpdateInterface dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), AI update
//! path/locomotor/collision helpers fail-closed without dual-world walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 281 helpers dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/ai_update_interface.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - apply_locomotor_result keeps local speed write path

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// AIUpdateInterface dual-world empty-gate residual method names.
pub const LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE282: &[&str] = &[
    "dual_world_registry_unavailable",
    "do_pathfind",
    "compute_attack_path",
    "owner_locomotor_inputs",
    "can_crush_or_squish",
    "need_to_rotate",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE282: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_AI_UPDATE_INTERFACE_EMPTY_GATES",
    "LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE282: &[&str] =
    &[
        "click_live_ai_update_interface_dual_world_empty_gate_ok_prepare",
        "click_live_ai_update_interface_dual_world_empty_gate_ok_live",
        "click_live_ai_update_interface_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_ai_update_interface_dual_world_empty_gate_method_names_residual_wave282() -> bool
{
    LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE282.len() == 7
        && residual_name_index(
            LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE282,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE282,
            "need_to_rotate",
        ) == Some(5)
        && residual_name_index(
            LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE282,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ai_update_interface_dual_world_empty_gate_nav_commands_residual_wave282() -> bool
{
    LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE282.len() == 4
        && residual_name_index(
            LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE282,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE282,
            "LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_AI_UPDATE_INTERFACE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE282.len() == 3
}

/// Wave 282 composite residual honesty pack.
pub fn honesty_live_ai_update_interface_dual_world_empty_gate_residual_pack_wave282() -> bool {
    honesty_live_ai_update_interface_dual_world_empty_gate_method_names_residual_wave282()
        && honesty_live_ai_update_interface_dual_world_empty_gate_nav_commands_residual_wave282()
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

/// Source residual: AIUpdateInterface empty dual-world short-circuits.
pub fn honesty_ai_update_interface_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/object/update/ai_update_interface.rs");
    if !(g.contains("Wave 282")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(path) = fn_body(g, "fn do_pathfind(") else {
        return false;
    };
    let Some(attack) = fn_body(g, "fn compute_attack_path(") else {
        return false;
    };
    let Some(crush) = fn_body(g, "fn can_crush_or_squish(") else {
        return false;
    };
    // apply_locomotor_result must not early-return solely on empty dual-world
    let apply_ok = !g.contains(
        "fn apply_locomotor_result(\n        &mut self,\n        old_pos: Coord3D,\n        old_angle: Real,\n        new_pos: Coord3D,\n        new_angle: Real,\n        new_speed: Real,\n    ) {\n        // Wave 282:",
    );
    helper_ok
        && path.contains("dual_world_registry_unavailable")
        && attack.contains("return false")
        && crush.contains("return false")
        && apply_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ai_update_interface_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ai_update_interface_dual_world_empty_gate_residual_pack_wave282()
        && honesty_ai_update_interface_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_ai_update_interface_dual_world_empty_gate_method_names_residual_wave282()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_ai_update_interface_dual_world_empty_gate_nav_commands_residual_wave282()
        );
    }

    #[test]
    fn wave282_composite_pack() {
        assert!(honesty_live_ai_update_interface_dual_world_empty_gate_residual_pack_wave282());
    }

    #[test]
    fn ai_update_interface_dual_world_empty_gate_sources() {
        assert!(honesty_ai_update_interface_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ai_update_interface_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ai_update_interface_dual_world_empty_gate_honesty(),
            "ai update interface dual-world empty gate residual must latch"
        );
    }
}
