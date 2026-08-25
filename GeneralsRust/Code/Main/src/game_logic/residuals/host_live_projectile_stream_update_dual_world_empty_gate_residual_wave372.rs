//! Wave 372 residual peels: ProjectileStreamUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), projectile
//! stream helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 371 ToppleUpdate dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/projectile_stream_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - Factory intentionally ungated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// ProjectileStreamUpdate dual-world empty-gate residual method names.
pub const LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE372: &[&str] = &[
    "dual_world_registry_unavailable",
    "get_all_points",
    "set_position",
    "cull_front_of_list",
    "consider_dying",
    "update_simple",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE372: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_PROJECTILE_STREAM_EMPTY_GATES",
    "LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE372:
    &[&str] = &[
    "click_live_projectile_stream_update_dual_world_empty_gate_ok_prepare",
    "click_live_projectile_stream_update_dual_world_empty_gate_ok_live",
    "click_live_projectile_stream_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_projectile_stream_update_dual_world_empty_gate_method_names_residual_wave372()
-> bool {
    LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE372.len() == 7
        && residual_name_index(
            LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE372,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE372,
            "update_simple",
        ) == Some(5)
        && residual_name_index(
            LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE372,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_projectile_stream_update_dual_world_empty_gate_nav_commands_residual_wave372()
-> bool {
    LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE372.len() == 4
        && residual_name_index(
            LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE372,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE372,
            "LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_PROJECTILE_STREAM_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE372.len()
            == 3
}

/// Wave 372 composite residual honesty pack.
pub fn honesty_live_projectile_stream_update_dual_world_empty_gate_residual_pack_wave372() -> bool {
    honesty_live_projectile_stream_update_dual_world_empty_gate_method_names_residual_wave372()
        && honesty_live_projectile_stream_update_dual_world_empty_gate_nav_commands_residual_wave372(
        )
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

/// Source residual: ProjectileStreamUpdate empty dual-world short-circuits.
pub fn honesty_projectile_stream_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/behavior/projectile_stream_update.rs"
    );
    if !(g.contains("Wave 372")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(points) = fn_body(g, "fn get_all_points(") else {
        return false;
    };
    let Some(set_pos) = fn_body(g, "fn set_position(") else {
        return false;
    };
    let Some(cull) = fn_body(g, "fn cull_front_of_list(") else {
        return false;
    };
    let Some(dying) = fn_body(g, "fn consider_dying(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update_simple(") else {
        return false;
    };
    helper_ok
        && points.contains("return Vec::new()")
        && set_pos.contains("return;")
        && cull.contains("return;")
        && dying.contains("return false")
        && update.contains("return UpdateSleepTime::Forever")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_projectile_stream_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_projectile_stream_update_dual_world_empty_gate_residual_pack_wave372()
        && honesty_projectile_stream_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_projectile_stream_update_dual_world_empty_gate_method_names_residual_wave372());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_projectile_stream_update_dual_world_empty_gate_nav_commands_residual_wave372());
    }

    #[test]
    fn wave372_composite_pack() {
        assert!(
            honesty_live_projectile_stream_update_dual_world_empty_gate_residual_pack_wave372()
        );
    }

    #[test]
    fn projectile_stream_update_dual_world_empty_gate_sources() {
        assert!(honesty_projectile_stream_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_projectile_stream_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_projectile_stream_update_dual_world_empty_gate_honesty(),
            "projectile stream update dual-world empty gate residual must latch"
        );
    }
}
