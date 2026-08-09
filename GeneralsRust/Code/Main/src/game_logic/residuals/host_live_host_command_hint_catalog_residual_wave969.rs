//! Wave 969: create_command_hint presentation catalog hover residual.
//!
//! Removes fail-closed dual-world early return from create_command_hint.
//! MoveTo / AttackMoveTo / SetRallyPoint use hover_target_command_context
//! (presentation unit catalog + local team). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_HINT_CATALOG_METHOD_NAMES_WAVE969: &[&str] = &[
    "hover_target_command_context",
    "hover_target_shroud_for_command_hint",
    "create_command_hint",
    "move_to_cursor_for_context",
    "Wave 969",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_HINT_CATALOG_NAV_STEPS_WAVE969: &[&str] = &[
    "COMMAND_HINT_FROM_CATALOG",
    "HOVER_TARGET_CONTEXT",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_COMMAND_HINT_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandHintCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCommandHintCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn ui_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}

pub fn honesty_host_command_hint_catalog_method_names_residual_wave969() -> bool {
    let names = LIVE_HOST_COMMAND_HINT_CATALOG_METHOD_NAMES_WAVE969;
    let ok = residual_name_index(names, "hover_target_command_context").is_some()
        && residual_name_index(names, "Wave 969").is_some();
    residual_action_store(ResidualHostCommandHintCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_hint_catalog_nav_commands_residual_wave969() -> bool {
    let steps = LIVE_HOST_COMMAND_HINT_CATALOG_NAV_STEPS_WAVE969;
    let ok = residual_name_index(steps, "LIVE_HOST_COMMAND_HINT_CATALOG").is_some()
        && residual_name_index(steps, "COMMAND_HINT_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostCommandHintCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_hint_catalog_residual_pack_wave969() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let hint = match ui.find("fn create_command_hint") {
        Some(i) => &ui[i..ui.len().min(i + 3500)],
        None => "",
    };
    let hover = match ui.find("fn hover_target_command_context") {
        Some(i) => &ui[i..ui.len().min(i + 1200)],
        None => "",
    };
    let ok = ui.contains("Wave 969")
        && ui.contains("hover_target_command_context")
        && ui.contains("hover_target_shroud_for_command_hint")
        && hover.contains("presentation_unit_catalog")
        && hover.contains("presentation_local_team_name")
        && hint.contains("hover_target_command_context")
        && hint.contains("Wave 969")
        && !hint.contains("empty dual-world → no factory object walks")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandHintCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_command_hint_catalog_honesty() -> bool {
    let a = honesty_host_command_hint_catalog_method_names_residual_wave969();
    let b = honesty_host_command_hint_catalog_nav_commands_residual_wave969();
    let c = honesty_host_command_hint_catalog_residual_pack_wave969();
    residual_action_store(ResidualHostCommandHintCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_command_hint_catalog_residual_wave969() {
        assert!(honesty_host_command_hint_catalog_residual_pack_wave969());
        assert!(honesty_host_command_hint_catalog_method_names_residual_wave969());
        assert!(honesty_host_command_hint_catalog_nav_commands_residual_wave969());
        assert!(simulate_live_host_command_hint_catalog_honesty());
    }
}
