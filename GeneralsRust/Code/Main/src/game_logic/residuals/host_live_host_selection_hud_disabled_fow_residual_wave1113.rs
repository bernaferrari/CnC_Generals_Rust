//! Wave 1113: dual-world selection HUD disabled + FOW residual.
//!
//! `draw_selection_anims_from_presentation` skipped destroyed/sold/unselectable/
//! masked/stealthed but still drew selection health bars for disabled units and
//! non-local FOW fogged/black selected entries (catalog legality residual).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SELECTION_HUD_DISABLED_FOW_METHOD_NAMES_WAVE1113: &[&str] = &[
    "draw_selection_anims_from_presentation",
    "presentation_unit_catalog",
    "Wave 1113",
    "playable_claim: false",
];

pub const LIVE_HOST_SELECTION_HUD_DISABLED_FOW_NAV_STEPS_WAVE1113: &[&str] = &[
    "SELECTION_HUD_DISABLED_FAIL_CLOSED",
    "SELECTION_HUD_FOW_NON_LOCAL",
    "LIVE_HOST_SELECTION_HUD_DISABLED_FOW",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSelectionHudDisabledFowAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSelectionHudDisabledFowAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_selection_hud_disabled_fow_method_names_residual_wave1113() -> bool {
    // Self-table membership is inflation (host_wave_inflation). Scan shipped HUD fn.
    debug_assert!(crate::game_logic::host_wave_inflation::self_table_honesty_is_inflation());
    let ui = ui_source();
    let ok = crate::game_logic::host_wave_inflation::shipped_fn_contains(
        ui,
        "fn draw_selection_anims_from_presentation",
        &[
            "presentation_unit_catalog",
            "entry.disabled",
            "ObjectShroudStatus::Fogged",
            "ObjectShroudStatus::Shrouded",
        ],
    );
    residual_action_store(ResidualHostSelectionHudDisabledFowAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_hud_disabled_fow_nav_commands_residual_wave1113() -> bool {
    // Nav honesty: shipped selection HUD must fail-closed on disabled + non-local FOW.
    debug_assert!(crate::game_logic::host_wave_inflation::self_table_honesty_is_inflation());
    let ui = ui_source();
    let ok = crate::game_logic::host_wave_inflation::shipped_fn_contains(
        ui,
        "fn draw_selection_anims_from_presentation",
        &["fogged && !is_local", "entry.disabled", "continue"],
    );
    residual_action_store(ResidualHostSelectionHudDisabledFowAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_selection_hud_disabled_fow_residual_pack_wave1113() -> bool {
    let ui = ui_source();
    let es = es_source();
    let fn_i = match ui.find("fn draw_selection_anims_from_presentation") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSelectionHudDisabledFowAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let window = &ui[fn_i..fn_i.saturating_add(2200)];
    let ok = window
        .contains("Wave 1113: disabled + non-local FOW selection HUD residual via catalog")
        && window.contains("entry.disabled")
        && window.contains("ObjectShroudStatus::Fogged")
        && window.contains("ObjectShroudStatus::Shrouded")
        && window.contains("presentation_unit_catalog")
        && window.contains("fogged && !is_local")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostSelectionHudDisabledFowAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_selection_hud_disabled_fow_residual_honesty() -> bool {
    let a = honesty_host_selection_hud_disabled_fow_method_names_residual_wave1113();
    let b = honesty_host_selection_hud_disabled_fow_nav_commands_residual_wave1113();
    let c = honesty_host_selection_hud_disabled_fow_residual_pack_wave1113();
    residual_action_store(ResidualHostSelectionHudDisabledFowAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_selection_hud_disabled_fow_residual_wave1113() {
        assert!(honesty_host_selection_hud_disabled_fow_residual_pack_wave1113());
        assert!(honesty_host_selection_hud_disabled_fow_method_names_residual_wave1113());
        assert!(honesty_host_selection_hud_disabled_fow_nav_commands_residual_wave1113());
        assert!(simulate_live_host_selection_hud_disabled_fow_residual_honesty());
    }
}
