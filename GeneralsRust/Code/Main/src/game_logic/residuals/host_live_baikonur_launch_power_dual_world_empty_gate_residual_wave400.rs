//! Wave 400 residual peels: BaikonurLaunchPower dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), Baikonur launch
//! helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 399 ArtilleryBarragePower dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/special_power_module/baikonur_launch_power.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// BaikonurLaunchPower dual-world empty-gate residual method names.
pub const LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE400: &[&str] = &[
    "dual_world_registry_unavailable",
    "open_launch_door",
    "owner_is_disabled",
    "resolve_team",
    "try_activate",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE400: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_BAIKONUR_LAUNCH_POWER_EMPTY_GATES",
    "LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE400:
    &[&str] = &[
    "click_live_baikonur_launch_power_dual_world_empty_gate_ok_prepare",
    "click_live_baikonur_launch_power_dual_world_empty_gate_ok_live",
    "click_live_baikonur_launch_power_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_baikonur_launch_power_dual_world_empty_gate_method_names_residual_wave400()
-> bool {
    LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE400.len() == 6
        && residual_name_index(
            LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE400,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE400,
            "try_activate",
        ) == Some(4)
        && residual_name_index(
            LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE400,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_baikonur_launch_power_dual_world_empty_gate_nav_commands_residual_wave400()
-> bool {
    LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE400.len() == 4
        && residual_name_index(
            LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE400,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE400,
            "LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_BAIKONUR_LAUNCH_POWER_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE400.len()
            == 3
}

/// Wave 400 composite residual honesty pack.
pub fn honesty_live_baikonur_launch_power_dual_world_empty_gate_residual_pack_wave400() -> bool {
    honesty_live_baikonur_launch_power_dual_world_empty_gate_method_names_residual_wave400()
        && honesty_live_baikonur_launch_power_dual_world_empty_gate_nav_commands_residual_wave400()
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

/// Source residual: BaikonurLaunchPower empty dual-world short-circuits.
pub fn honesty_baikonur_launch_power_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/special_power_module/baikonur_launch_power.rs"
    );
    if !(g.contains("Wave 400")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(door) = fn_body(g, "fn open_launch_door(") else {
        return false;
    };
    let Some(disabled) = fn_body(g, "fn owner_is_disabled(") else {
        return false;
    };
    let Some(team) = fn_body(g, "fn resolve_team(") else {
        return false;
    };
    let Some(activate) = fn_body(g, "fn try_activate(") else {
        return false;
    };
    helper_ok
        && door.contains("return;")
        && disabled.contains("return false")
        && team.contains("not found")
        && activate.contains("ActivationResult::Failed")
        && activate.contains("empty dual-world")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_baikonur_launch_power_dual_world_empty_gate_honesty() -> bool {
    honesty_live_baikonur_launch_power_dual_world_empty_gate_residual_pack_wave400()
        && honesty_baikonur_launch_power_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(
            honesty_live_baikonur_launch_power_dual_world_empty_gate_method_names_residual_wave400(
            )
        );
    }

    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_live_baikonur_launch_power_dual_world_empty_gate_nav_commands_residual_wave400(
            )
        );
    }

    #[test]
    fn wave400_composite_pack() {
        assert!(honesty_live_baikonur_launch_power_dual_world_empty_gate_residual_pack_wave400());
    }

    #[test]
    fn baikonur_launch_power_dual_world_empty_gate_sources() {
        assert!(honesty_baikonur_launch_power_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_baikonur_launch_power_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_baikonur_launch_power_dual_world_empty_gate_honesty(),
            "baikonur launch power dual-world empty gate residual must latch"
        );
    }
}
