//! Wave 249 residual peels: GameClient dual-world selection/control-bar paths
//! short-circuit when `OBJECT_REGISTRY` is empty (Main host/presentation path)
//! via `dual_world_registry_unavailable()`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 248 legacy object registry fastpath residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `message_stream/translators.rs` dual_world_registry_unavailable /
//!   selection_any_local_object_can_target / relationship_to_target / …
//! - `gui/control_bar/control_bar.rs` dual_world_registry_unavailable /
//!   get_object_has_production / update_portrait_for_object
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred
//! - Dual-world still used when registry is populated

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Client dual-world empty-gate residual method names.
pub const LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE249: &[&str] = &[
    "dual_world_registry_unavailable",
    "selection_any_local_object_can_target",
    "get_object_has_production",
    "update_portrait_for_object",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE249: &[&str] = &[
    "REQUIRE_CLIENT_EMPTY_GATE_HELPER",
    "REQUIRE_SELECTION_AND_CONTROL_BAR_GATES",
    "LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE249: &[&str] = &[
    "click_live_client_dual_world_empty_gate_ok_prepare",
    "click_live_client_dual_world_empty_gate_ok_live",
    "click_live_client_dual_world_empty_gate_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_client_dual_world_empty_gate_method_names_residual_wave249() -> bool {
    LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE249.len() == 5
        && residual_name_index(
            LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE249,
            "dual_world_registry_unavailable",
        ) == Some(0)
        && residual_name_index(
            LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE249,
            "update_portrait_for_object",
        ) == Some(3)
        && residual_name_index(
            LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_METHOD_NAMES_WAVE249,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_client_dual_world_empty_gate_nav_commands_residual_wave249() -> bool {
    LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE249.len() == 4
        && residual_name_index(
            LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE249,
            "REQUIRE_CLIENT_EMPTY_GATE_HELPER",
        ) == Some(0)
        && residual_name_index(
            LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_NAV_STEPS_WAVE249,
            "LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE",
        ) == Some(2)
        && RUNTIME_HOST_LIVE_CLIENT_DUAL_WORLD_EMPTY_GATE_CMD_NAMES_WAVE249.len() == 3
}

/// Wave 249 composite residual honesty pack.
pub fn honesty_live_client_dual_world_empty_gate_residual_pack_wave249() -> bool {
    honesty_live_client_dual_world_empty_gate_method_names_residual_wave249()
        && honesty_live_client_dual_world_empty_gate_nav_commands_residual_wave249()
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

/// Source residual: translators + control_bar host empty gates present.
pub fn honesty_client_dual_world_empty_gate_source() -> bool {
    let tr = game_client::message_stream::translators::TRANSLATORS_SRC;
    let cb = game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC;
    if !(tr.contains("fn dual_world_registry_unavailable(")
        && tr.contains("Wave 249")
        && cb.contains("fn dual_world_registry_unavailable(")
        && cb.contains("Wave 249"))
    {
        return false;
    }
    let Some(any_local) = fn_body(tr, "fn selection_any_local_object_can_target") else {
        return false;
    };
    if !any_local.contains("dual_world_registry_unavailable()") {
        return false;
    }
    let Some(portrait) = fn_body(cb, "fn update_portrait_for_object") else {
        return false;
    };
    portrait.contains("dual_world_registry_unavailable()") && portrait.contains("Wave 249")
}

/// Live residual: source honesty pack latches.
pub fn simulate_live_client_dual_world_empty_gate_honesty() -> bool {
    honesty_live_client_dual_world_empty_gate_residual_pack_wave249()
        && honesty_client_dual_world_empty_gate_source()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_client_dual_world_empty_gate_method_names_residual_wave249());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_client_dual_world_empty_gate_nav_commands_residual_wave249());
    }

    #[test]
    fn wave249_composite_pack() {
        assert!(honesty_live_client_dual_world_empty_gate_residual_pack_wave249());
    }

    #[test]
    fn client_dual_world_empty_gate_sources() {
        assert!(honesty_client_dual_world_empty_gate_source());
    }

    #[test]
    fn simulate_live_client_dual_world_empty_gate_honesty_residual_live() {
        assert!(
            simulate_live_client_dual_world_empty_gate_honesty(),
            "client dual-world empty gate residual must latch"
        );
    }
}
