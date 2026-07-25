//! Wave 286 residual peels: DumbProjectileBehavior dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), projectile
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 285 AI integration dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/behavior/dumb_projectile_behavior.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated
//! - projectile_launch keeps local launcher/victim state setup

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// DumbProjectile dual-world empty-gate residual method names.
pub const LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE286: &[&str] = &[
    "dual_world_registry_unavailable",
    "from_module_thing",
    "get_object",
    "check_collision",
    "update",
    "projectile_now_jammed",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE286: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_DUMB_PROJECTILE_EMPTY_GATES",
    "LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE286: &[&str] = &[
    "click_live_dumb_projectile_dual_world_empty_gate_ok_prepare",
    "click_live_dumb_projectile_dual_world_empty_gate_ok_live",
    "click_live_dumb_projectile_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_dumb_projectile_dual_world_empty_gate_method_names_residual_wave286() -> bool {
    LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE286.len() == 7
        && residual_name_index(
            LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE286,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE286,
            "projectile_now_jammed",
        ) == Some(5)
        && residual_name_index(
            LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE286,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_dumb_projectile_dual_world_empty_gate_nav_commands_residual_wave286() -> bool {
    LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE286.len() == 4
        && residual_name_index(
            LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE286,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE286,
            "LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_DUMB_PROJECTILE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE286.len() == 3
}

/// Wave 286 composite residual honesty pack.
pub fn honesty_live_dumb_projectile_dual_world_empty_gate_residual_pack_wave286() -> bool {
    honesty_live_dumb_projectile_dual_world_empty_gate_method_names_residual_wave286()
        && honesty_live_dumb_projectile_dual_world_empty_gate_nav_commands_residual_wave286()
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

/// Source residual: DumbProjectile empty dual-world short-circuits.
pub fn honesty_dumb_projectile_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../GameEngine/GameLogic/src/object/behavior/dumb_projectile_behavior.rs"
    );
    if !(g.contains("Wave 286")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(get) = fn_body(g, "fn get_object(") else {
        return false;
    };
    let Some(update) = fn_body(g, "fn update(&mut self)") else {
        return false;
    };
    let Some(jam) = fn_body(g, "fn projectile_now_jammed(") else {
        return false;
    };
    let launch_ok = !g.contains(
        "pub fn projectile_launch_at_object_or_position(\n        &mut self,\n        victim: Option<ObjectID>,\n        victim_pos: &Coord3D,\n        launcher: ObjectID,\n        detonation_weapon: Option<Arc<WeaponTemplate>>,\n    ) {\n        // Wave 286:",
    );
    helper_ok
        && get.contains("dual-world registry unavailable")
        && update.contains("UpdateSleepTime::None")
        && jam.contains("dual_world_registry_unavailable")
        && launch_ok
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_dumb_projectile_dual_world_empty_gate_honesty() -> bool {
    honesty_live_dumb_projectile_dual_world_empty_gate_residual_pack_wave286()
        && honesty_dumb_projectile_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_dumb_projectile_dual_world_empty_gate_method_names_residual_wave286());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_dumb_projectile_dual_world_empty_gate_nav_commands_residual_wave286());
    }

    #[test]
    fn wave286_composite_pack() {
        assert!(honesty_live_dumb_projectile_dual_world_empty_gate_residual_pack_wave286());
    }

    #[test]
    fn dumb_projectile_dual_world_empty_gate_sources() {
        assert!(honesty_dumb_projectile_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_dumb_projectile_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_dumb_projectile_dual_world_empty_gate_honesty(),
            "dumb projectile dual-world empty gate residual must latch"
        );
    }
}
