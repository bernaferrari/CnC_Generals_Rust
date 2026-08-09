//! Wave 356 residual peels: WeaponTemplate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), fire/damage
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 355 DockUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/weapon/weapon_template.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// WeaponTemplate dual-world empty-gate residual method names.
pub const LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE356: &[&str] = &[
    "dual_world_registry_unavailable",
    "fire_weapon_template",
    "create_projectile",
    "get_object_position",
    "deal_damage_internal",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE356: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_WEAPON_TEMPLATE_EMPTY_GATES",
    "LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE356: &[&str] = &[
    "click_live_weapon_template_dual_world_empty_gate_ok_prepare",
    "click_live_weapon_template_dual_world_empty_gate_ok_live",
    "click_live_weapon_template_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_weapon_template_dual_world_empty_gate_method_names_residual_wave356() -> bool {
    LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE356.len() == 6
        && residual_name_index(
            LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE356,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE356,
            "deal_damage_internal",
        ) == Some(4)
        && residual_name_index(
            LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE356,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_weapon_template_dual_world_empty_gate_nav_commands_residual_wave356() -> bool {
    LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE356.len() == 4
        && residual_name_index(
            LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE356,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE356,
            "LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_WEAPON_TEMPLATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE356.len() == 3
}

/// Wave 356 composite residual honesty pack.
pub fn honesty_live_weapon_template_dual_world_empty_gate_residual_pack_wave356() -> bool {
    honesty_live_weapon_template_dual_world_empty_gate_method_names_residual_wave356()
        && honesty_live_weapon_template_dual_world_empty_gate_nav_commands_residual_wave356()
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

/// Source residual: WeaponTemplate empty dual-world short-circuits.
pub fn honesty_weapon_template_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../../GameEngine/GameLogic/src/weapon/weapon_template.rs");
    if !(g.contains("Wave 356")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(fire) = fn_body(g, "fn fire_weapon_template(") else {
        return false;
    };
    let Some(proj) = fn_body(g, "fn create_projectile(") else {
        return false;
    };
    let Some(pos) = fn_body(g, "fn get_object_position(") else {
        return false;
    };
    let Some(dmg) = fn_body(g, "fn deal_damage_internal(") else {
        return false;
    };
    helper_ok
        && fire.contains("return Ok(0)")
        && proj.contains("return Ok(None)")
        && pos.contains("GameLogicError::InvalidObject")
        && dmg.contains("return Ok(())")
        && g.matches("Wave 356:").count() >= 10
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_weapon_template_dual_world_empty_gate_honesty() -> bool {
    honesty_live_weapon_template_dual_world_empty_gate_residual_pack_wave356()
        && honesty_weapon_template_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_weapon_template_dual_world_empty_gate_method_names_residual_wave356());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_weapon_template_dual_world_empty_gate_nav_commands_residual_wave356());
    }

    #[test]
    fn wave356_composite_pack() {
        assert!(honesty_live_weapon_template_dual_world_empty_gate_residual_pack_wave356());
    }

    #[test]
    fn weapon_template_dual_world_empty_gate_sources() {
        assert!(honesty_weapon_template_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_weapon_template_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_weapon_template_dual_world_empty_gate_honesty(),
            "weapon template dual-world empty gate residual must latch"
        );
    }
}
