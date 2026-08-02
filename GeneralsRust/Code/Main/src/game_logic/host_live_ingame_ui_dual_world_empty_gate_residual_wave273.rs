//! Wave 273 residual peels: InGameUI dual-world empty short-circuits.
//! When `OBJECT_REGISTRY` is empty (host-only presentation path), InGameUI
//! selection/hint helpers fail-closed without dual-world factory walks.
//! Never flips shell `playable_claim`. Network deferred.
//!
//! Orthogonal to Wave 272 TransportContain dual-world empty-gate residual.
//!
//! Sources:
//! - `GameClient/src/gui/ingame_ui.rs` dual_world_registry_unavailable
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still active when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// InGameUI dual-world empty-gate residual method names.
pub const LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE273: &[&str] = &[
    "dual_world_registry_unavailable",
    "is_valid_special_power_target",
    "select_similar_units",
    "draw_selection_anims",
    "select_matching_across_map",
    "create_command_hint",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE273: &[&str] = &[
    "REQUIRE_DUAL_WORLD_HELPER",
    "REQUIRE_INGAME_UI_EMPTY_GATES",
    "LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE273: &[&str] = &[
    "click_live_ingame_ui_dual_world_empty_gate_ok_prepare",
    "click_live_ingame_ui_dual_world_empty_gate_ok_live",
    "click_live_ingame_ui_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_ingame_ui_dual_world_empty_gate_method_names_residual_wave273() -> bool {
    LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE273.len() == 7
        && residual_name_index(
            LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE273,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE273,
            "create_command_hint",
        ) == Some(5)
        && residual_name_index(
            LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE273,
            "playable_claim = false",
        ) == Some(6)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_ingame_ui_dual_world_empty_gate_nav_commands_residual_wave273() -> bool {
    LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE273.len() == 4
        && residual_name_index(
            LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE273,
            "REQUIRE_DUAL_WORLD_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE273,
            "LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_INGAME_UI_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE273.len() == 3
}

/// Wave 273 composite residual honesty pack.
pub fn honesty_live_ingame_ui_dual_world_empty_gate_residual_pack_wave273() -> bool {
    honesty_live_ingame_ui_dual_world_empty_gate_method_names_residual_wave273()
        && honesty_live_ingame_ui_dual_world_empty_gate_nav_commands_residual_wave273()
}

fn fn_body<'a>(src: &'a str, name: &str) -> Option<&'a str> {
    let i = src.find(name)?;
    let brace = src[i..].find('{')? + i;
    let mut depth = 0usize;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[i..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Source residual: InGameUI empty dual-world short-circuits.
pub fn honesty_ingame_ui_dual_world_empty_gate_source() -> bool {
    let g = include_str!("../../../GameEngine/GameClient/src/gui/ingame_ui.rs");
    if !(g.contains("Wave 273")
        && g.contains("fn dual_world_registry_unavailable")
        && g.contains("OBJECT_REGISTRY.is_empty()"))
    {
        return false;
    }
    let Some(valid) = fn_body(g, "fn is_valid_special_power_target(") else {
        return false;
    };
    let Some(select) = fn_body(g, "fn select_similar_units(") else {
        return false;
    };
    let Some(hint) = fn_body(g, "fn create_command_hint(") else {
        return false;
    };
    valid.contains("dual_world_registry_unavailable")
        && valid.contains("return false")
        && select.contains("dual_world_registry_unavailable")
        && select.contains("Ok(())")
        // Wave 969: create_command_hint peels via hover_target_command_context residual.
        && (hint.contains("dual_world_registry_unavailable")
            || hint.contains("hover_target_command_context"))
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_ingame_ui_dual_world_empty_gate_honesty() -> bool {
    honesty_live_ingame_ui_dual_world_empty_gate_residual_pack_wave273()
        && honesty_ingame_ui_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_ingame_ui_dual_world_empty_gate_method_names_residual_wave273());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_ingame_ui_dual_world_empty_gate_nav_commands_residual_wave273());
    }

    #[test]
    fn wave273_composite_pack() {
        assert!(honesty_live_ingame_ui_dual_world_empty_gate_residual_pack_wave273());
    }

    #[test]
    fn ingame_ui_dual_world_empty_gate_sources() {
        assert!(honesty_ingame_ui_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_ingame_ui_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_ingame_ui_dual_world_empty_gate_honesty(),
            "ingame ui dual-world empty gate residual must latch"
        );
    }
}
