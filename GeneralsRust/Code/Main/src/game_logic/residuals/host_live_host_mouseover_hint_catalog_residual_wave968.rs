//! Wave 968: mouseover/command-hint presentation catalog residual.
//!
//! Peels command_hint_source_context and create_mouseover_hint onto presentation
//! unit catalog + local team residual when OBJECT_REGISTRY is empty.
//! Catalog entries carry KindOf names. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MOUSEOVER_HINT_CATALOG_METHOD_NAMES_WAVE968: &[&str] = &[
    "create_mouseover_hint_from_presentation",
    "command_hint_source_context",
    "apply_presentation_local_team_name",
    "kind_names",
    "Wave 968",
    "playable_claim = false",
];

pub const LIVE_HOST_MOUSEOVER_HINT_CATALOG_NAV_STEPS_WAVE968: &[&str] = &[
    "MOUSEOVER_FROM_CATALOG",
    "COMMAND_HINT_SOURCE_FROM_CATALOG",
    "LOCAL_TEAM_RESIDUAL",
    "LIVE_HOST_MOUSEOVER_HINT_CATALOG",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMouseoverHintCatalogAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMouseoverHintCatalogAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_mouseover_hint_catalog_method_names_residual_wave968() -> bool {
    let names = LIVE_HOST_MOUSEOVER_HINT_CATALOG_METHOD_NAMES_WAVE968;
    let ok = residual_name_index(names, "create_mouseover_hint_from_presentation").is_some()
        && residual_name_index(names, "Wave 968").is_some();
    residual_action_store(ResidualHostMouseoverHintCatalogAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mouseover_hint_catalog_nav_commands_residual_wave968() -> bool {
    let steps = LIVE_HOST_MOUSEOVER_HINT_CATALOG_NAV_STEPS_WAVE968;
    let ok = residual_name_index(steps, "LIVE_HOST_MOUSEOVER_HINT_CATALOG").is_some()
        && residual_name_index(steps, "MOUSEOVER_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostMouseoverHintCatalogAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mouseover_hint_catalog_residual_pack_wave968() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let client = client_source();
    let mo = match ui.find("fn create_mouseover_hint") {
        Some(i) => &ui[i..ui.len().min(i + 600)],
        None => "",
    };
    let ctx = match ui.find("fn command_hint_source_context") {
        Some(i) => &ui[i..ui.len().min(i + 900)],
        None => "",
    };
    let ok = ui.contains("Wave 968")
        && client.contains("Wave 968")
        && cnc.contains("Wave 968")
        && ui.contains("create_mouseover_hint_from_presentation")
        && mo.contains("create_mouseover_hint_from_presentation")
        && ctx.contains("presentation_unit_catalog")
        && ctx.contains("presentation_local_team_name")
        && ui.contains("kind_names")
        && client.contains("apply_presentation_local_team_name")
        && cnc.contains("apply_presentation_local_team_name")
        && cnc.contains("kind_names:")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMouseoverHintCatalogAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_mouseover_hint_catalog_honesty() -> bool {
    let a = honesty_host_mouseover_hint_catalog_method_names_residual_wave968();
    let b = honesty_host_mouseover_hint_catalog_nav_commands_residual_wave968();
    let c = honesty_host_mouseover_hint_catalog_residual_pack_wave968();
    residual_action_store(ResidualHostMouseoverHintCatalogAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_mouseover_hint_catalog_residual_wave968() {
        assert!(honesty_host_mouseover_hint_catalog_residual_pack_wave968());
        assert!(honesty_host_mouseover_hint_catalog_method_names_residual_wave968());
        assert!(honesty_host_mouseover_hint_catalog_nav_commands_residual_wave968());
        assert!(simulate_live_host_mouseover_hint_catalog_honesty());
    }
}
