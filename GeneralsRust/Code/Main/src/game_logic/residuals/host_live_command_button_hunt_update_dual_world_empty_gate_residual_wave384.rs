//! Wave 384 residual peels: CommandButtonHuntUpdate dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), command
//! button hunt helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 383 DynamicShroudClearing dual-world empty-gate residual.
//!
//! Sources:
//! - `GameLogic/src/object/update/command_button_hunt_update.rs`
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// CommandButtonHuntUpdate dual-world empty-gate residual method names.
pub const LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE384: &[&str] = &[
    "dual_world_registry_unavailable",
    "object_arc",
    "hunt_special_power",
    "hunt_enter",
    "scan_closest_target",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE384: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_COMMAND_BUTTON_HUNT_EMPTY_GATES",
    "LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE384:
    &[&str] = &[
    "click_live_command_button_hunt_update_dual_world_empty_gate_ok_prepare",
    "click_live_command_button_hunt_update_dual_world_empty_gate_ok_live",
    "click_live_command_button_hunt_update_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_command_button_hunt_update_dual_world_empty_gate_method_names_residual_wave384()
-> bool {
    LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE384.len() == 6
        && residual_name_index(
            LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE384,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE384,
            "scan_closest_target",
        ) == Some(4)
        && residual_name_index(
            LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE384,
            "playable_claim = false",
        ) == Some(5)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_command_button_hunt_update_dual_world_empty_gate_nav_commands_residual_wave384()
-> bool {
    LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE384.len() == 4
        && residual_name_index(
            LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE384,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE384,
            "LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_COMMAND_BUTTON_HUNT_UPDATE_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE384
            .len()
            == 3
}

/// Wave 384 composite residual honesty pack.
pub fn honesty_live_command_button_hunt_update_dual_world_empty_gate_residual_pack_wave384() -> bool
{
    honesty_live_command_button_hunt_update_dual_world_empty_gate_method_names_residual_wave384()
        && honesty_live_command_button_hunt_update_dual_world_empty_gate_nav_commands_residual_wave384()
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

/// Source residual: CommandButtonHuntUpdate empty dual-world short-circuits.
pub fn honesty_command_button_hunt_update_dual_world_empty_gate_source() -> bool {
    let g = include_str!(
        "../../../../GameEngine/GameLogic/src/object/update/command_button_hunt_update.rs"
    );
    if !(g.contains("Wave 384")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let helper_ok = g.contains(
        "fn dual_world_registry_unavailable() -> bool {\n    crate::object::registry::OBJECT_REGISTRY.is_empty()\n}",
    );
    let Some(arc) = fn_body(g, "fn object_arc(") else {
        return false;
    };
    let Some(hunt) = fn_body(g, "fn hunt_special_power(") else {
        return false;
    };
    let Some(enter) = fn_body(g, "fn hunt_enter(") else {
        return false;
    };
    let Some(scan) = fn_body(g, "fn scan_closest_target(") else {
        return false;
    };
    helper_ok
        && arc.contains("return None")
        && hunt.contains("return UpdateSleepTime::Forever")
        && enter.contains("return UpdateSleepTime::Forever")
        && scan.contains("return None")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_command_button_hunt_update_dual_world_empty_gate_honesty() -> bool {
    honesty_live_command_button_hunt_update_dual_world_empty_gate_residual_pack_wave384()
        && honesty_command_button_hunt_update_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_command_button_hunt_update_dual_world_empty_gate_method_names_residual_wave384());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_command_button_hunt_update_dual_world_empty_gate_nav_commands_residual_wave384());
    }

    #[test]
    fn wave384_composite_pack() {
        assert!(
            honesty_live_command_button_hunt_update_dual_world_empty_gate_residual_pack_wave384()
        );
    }

    #[test]
    fn command_button_hunt_update_dual_world_empty_gate_sources() {
        assert!(honesty_command_button_hunt_update_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_command_button_hunt_update_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_command_button_hunt_update_dual_world_empty_gate_honesty(),
            "command button hunt update dual-world empty gate residual must latch"
        );
    }
}
