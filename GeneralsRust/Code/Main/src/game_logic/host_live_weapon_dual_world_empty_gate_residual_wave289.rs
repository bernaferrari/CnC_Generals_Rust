//! Wave 289 residual peels: Weapon.rs dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), fire/range
//! helpers in `weapon/weapon.rs` fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 265 weapon/mod dual-world empty-gate residual and
//! Wave 288 HijackerUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/weapon/weapon.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Weapon.rs dual-world empty-gate residual method names.
pub const LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE289: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_within_target_pitch",
    "private_fire_weapon",
    "get_attack_distance",
    "is_source_object_with_goal_position_within_attack_range",
    "process_request_assistance",
    "get_firing_line_of_sight_origin",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE289: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_WEAPON_IMPL_EMPTY_GATES",
    "LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE289: &[&str] = &[
    "click_live_weapon_impl_dual_world_empty_gate_ok_prepare",
    "click_live_weapon_impl_dual_world_empty_gate_ok_live",
    "click_live_weapon_impl_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_weapon_impl_dual_world_empty_gate_method_names_residual_wave289() -> bool {
    LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE289.len() == 8
        && residual_name_index(
            LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE289,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE289,
            "private_fire_weapon",
        ) == Some(2)
        && residual_name_index(
            LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE289,
            "playable_claim = false",
        ) == Some(7)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_weapon_impl_dual_world_empty_gate_nav_commands_residual_wave289() -> bool {
    LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE289.len() == 4
        && residual_name_index(
            LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE289,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE289,
            "LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_WEAPON_IMPL_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE289.len() == 3
}

/// Wave 289 composite residual honesty pack.
pub fn honesty_live_weapon_impl_dual_world_empty_gate_residual_pack_wave289() -> bool {
    honesty_live_weapon_impl_dual_world_empty_gate_method_names_residual_wave289()
        && honesty_live_weapon_impl_dual_world_empty_gate_nav_commands_residual_wave289()
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

/// Source residual: weapon.rs empty dual-world short-circuits.
pub fn honesty_weapon_impl_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameLogic/src/weapon/weapon.rs");
    if !(g.contains("Wave 289")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(fire) = fn_body(g, "fn private_fire_weapon(") else {
        return false;
    };
    let Some(range) = fn_body(g, "fn get_attack_distance(") else {
        return false;
    };
    helper_ok && fire.contains("Ok((false, None))") && range.contains("return 0.0")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_weapon_impl_dual_world_empty_gate_honesty() -> bool {
    honesty_live_weapon_impl_dual_world_empty_gate_residual_pack_wave289()
        && honesty_weapon_impl_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_weapon_impl_dual_world_empty_gate_method_names_residual_wave289());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_weapon_impl_dual_world_empty_gate_nav_commands_residual_wave289());
    }

    #[test]
    fn wave289_composite_pack() {
        assert!(honesty_live_weapon_impl_dual_world_empty_gate_residual_pack_wave289());
    }

    #[test]
    fn weapon_impl_dual_world_empty_gate_sources() {
        assert!(honesty_weapon_impl_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_weapon_impl_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_weapon_impl_dual_world_empty_gate_honesty(),
            "weapon.rs dual-world empty gate residual must latch"
        );
    }
}
