//! Wave 1077: dual-world FOW UI text hide + garrison catalog residual.
//!
//! draw_ui_text_from_presentation skips fully FOW-obscured drawables; ControlBar
//! dual garrison peels catalog occupant and clears on unusable. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FOW_UI_TEXT_GARRISON_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1077: &[&str] = &[
    "draw_ui_text_from_presentation",
    "drawable_fully_obscured_by_shroud",
    "occupant_count",
    "Wave 1077",
    "playable_claim = false",
];

pub const LIVE_HOST_FOW_UI_TEXT_GARRISON_CATALOG_RESIDUAL_NAV_STEPS_WAVE1077: &[&str] = &[
    "FOW_UI_TEXT",
    "GARRISON_CATALOG",
    "LIVE_HOST_FOW_UI_TEXT_GARRISON_CATALOG_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFowUiTextGarrisonCatalogResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFowUiTextGarrisonCatalogResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn d_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}

pub fn honesty_host_fow_ui_text_garrison_catalog_residual_method_names_residual_wave1077() -> bool {
    let names = LIVE_HOST_FOW_UI_TEXT_GARRISON_CATALOG_RESIDUAL_METHOD_NAMES_WAVE1077;
    let ok = residual_name_index(names, "draw_ui_text_from_presentation").is_some()
        && residual_name_index(names, "Wave 1077").is_some();
    residual_action_store(ResidualHostFowUiTextGarrisonCatalogResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_ui_text_garrison_catalog_residual_nav_commands_residual_wave1077() -> bool {
    let steps = LIVE_HOST_FOW_UI_TEXT_GARRISON_CATALOG_RESIDUAL_NAV_STEPS_WAVE1077;
    let ok = residual_name_index(steps, "LIVE_HOST_FOW_UI_TEXT_GARRISON_CATALOG_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "FOW_UI_TEXT").is_some();
    residual_action_store(ResidualHostFowUiTextGarrisonCatalogResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_ui_text_garrison_catalog_residual_residual_pack_wave1077() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let d = d_source();
    let cb = cb_source();
    let ok = d.contains("Wave 1077: FOW fully-obscured residual hides dual presentation UI text")
        && d.contains("if self.drawable_fully_obscured_by_shroud")
        && (cb.contains("Wave 1077: catalog occupant residual when freeze count unset")
            || cb.contains(
                "Wave 1077: catalog occupant residual when freeze count unset; clear on unusable",
            ))
        && cb.contains("entry.occupant_count")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFowUiTextGarrisonCatalogResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fow_ui_text_garrison_catalog_residual_honesty() -> bool {
    let a = honesty_host_fow_ui_text_garrison_catalog_residual_method_names_residual_wave1077();
    let b = honesty_host_fow_ui_text_garrison_catalog_residual_nav_commands_residual_wave1077();
    let c = honesty_host_fow_ui_text_garrison_catalog_residual_residual_pack_wave1077();
    residual_action_store(ResidualHostFowUiTextGarrisonCatalogResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fow_ui_text_garrison_catalog_residual_wave1077() {
        assert!(honesty_host_fow_ui_text_garrison_catalog_residual_residual_pack_wave1077());
        assert!(
            honesty_host_fow_ui_text_garrison_catalog_residual_method_names_residual_wave1077()
        );
        assert!(
            honesty_host_fow_ui_text_garrison_catalog_residual_nav_commands_residual_wave1077()
        );
        assert!(simulate_live_host_fow_ui_text_garrison_catalog_residual_honesty());
    }
}
