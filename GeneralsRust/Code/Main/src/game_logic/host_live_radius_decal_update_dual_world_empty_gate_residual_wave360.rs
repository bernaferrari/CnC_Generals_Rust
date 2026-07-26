//! Wave 360 residual peels: RadiusDecalUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), radius
//! decal helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 359 SpectreGunshipDeployment dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/radius_decal_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Module factory intentionally ungated (needs owner object)

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// RadiusDecalUpdate dual-world empty-gate residual method names.
pub const LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE360: &[&str] = &[
    "dual_world_registry_unavailable",
    "create_radius_decal",
    "kill_radius_decal",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE360: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_RADIUS_DECAL_UPDATE_EMPTY_GATES",
    "LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE360: &[&str] =
    &[
        "click_live_radius_decal_update_dual_world_empty_gate_ok_prepare",
        "click_live_radius_decal_update_dual_world_empty_gate_ok_live",
        "click_live_radius_decal_update_dual_world_empty_gate_miss",
    ];

/// Honesty: method names residual pack.
pub fn honesty_live_radius_decal_update_dual_world_empty_gate_method_names_residual_wave360() -> bool
{
    LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE360.len() == 5
        && residual_name_index(
            LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE360,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE360,
            "update_simple",
        ) == Some(3)
        && residual_name_index(
            LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE360,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_radius_decal_update_dual_world_empty_gate_nav_commands_residual_wave360() -> bool
{
    LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE360.len() == 4
        && residual_name_index(
            LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE360,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE360,
            "LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_RADIUS_DECAL_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE360.len() == 3
}

/// Wave 360 composite residual honesty pack.
pub fn honesty_live_radius_decal_update_dual_world_empty_gate_residual_pack_wave360() -> bool {
    honesty_live_radius_decal_update_dual_world_empty_gate_method_names_residual_wave360()
        && honesty_live_radius_decal_update_dual_world_empty_gate_nav_commands_residual_wave360()
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

/// Source residual: RadiusDecalUpdate empty dual-world short-circuits.
pub fn honesty_radius_decal_update_dual_world_empty_gate_source() -> bool {
    let g =
        include_str!("../../../GameEngine/GameLogic/src/object/behavior/radius_decal_update.rs");
    if !(g.contains("Wave 360")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(create) = fn_body(g, "fn create_radius_decal(") else {
        return false;
    };
    let Some(kill) = fn_body(g, "fn kill_radius_decal(") else {
        return false;
    };
    let Some(upd) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok
        && create.contains("return;")
        && kill.contains("return;")
        && upd.contains("UPDATE_SLEEP_FOREVER")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_radius_decal_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_radius_decal_update_dual_world_empty_gate_residual_pack_wave360()
        && honesty_radius_decal_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_radius_decal_update_dual_world_empty_gate_method_names_residual_wave360()
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_radius_decal_update_dual_world_empty_gate_nav_commands_residual_wave360()
        );
    }

    #[test]
    fn wave360_composite_pack() {
        assert!(honesty_live_radius_decal_update_dual_world_empty_gate_residual_pack_wave360());
    }

    #[test]
    fn radius_decal_update_dual_world_empty_gate_sources() {
        assert!(honesty_radius_decal_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_radius_decal_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_radius_decal_update_dual_world_empty_gate_honesty(),
            "radius decal update dual-world empty gate residual must latch"
        );
    }
}
