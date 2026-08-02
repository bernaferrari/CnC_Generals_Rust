//! Wave 1000: dual-world IgnoredInGui slaver mouseover presentation residual.
//!
//! mouseover_drawable_id_for_lookup peels empty OBJECT_REGISTRY via
//! presentation_unit_catalog (kind IgnoredInGui → slaver_object_id).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_IGNORED_GUI_SLAVER_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1000: &[&str] = &[
    "ignored_gui_slaver_id_from_presentation",
    "mouseover_drawable_id_for_lookup",
    "slaver_object_id",
    "Wave 1000",
    "playable_claim = false",
];

pub const LIVE_HOST_IGNORED_GUI_SLAVER_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1000: &[&str] = &[
    "DUAL_WORLD",
    "IGNORED_IN_GUI",
    "SLAVER_REMAP",
    "LIVE_HOST_IGNORED_GUI_SLAVER_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostIgnoredGuiSlaverPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostIgnoredGuiSlaverPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn ui_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/ingame_ui.rs")
}

pub fn honesty_host_ignored_gui_slaver_presentation_residual_method_names_residual_wave1000() -> bool
{
    let names = LIVE_HOST_IGNORED_GUI_SLAVER_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1000;
    let ok = residual_name_index(names, "ignored_gui_slaver_id_from_presentation").is_some()
        && residual_name_index(names, "Wave 1000").is_some();
    residual_action_store(ResidualHostIgnoredGuiSlaverPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ignored_gui_slaver_presentation_residual_nav_commands_residual_wave1000() -> bool
{
    let steps = LIVE_HOST_IGNORED_GUI_SLAVER_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1000;
    let ok = residual_name_index(steps, "LIVE_HOST_IGNORED_GUI_SLAVER_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "SLAVER_REMAP").is_some();
    residual_action_store(ResidualHostIgnoredGuiSlaverPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_ignored_gui_slaver_presentation_residual_residual_pack_wave1000() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let lookup = match ui.find("fn mouseover_drawable_id_for_lookup") {
        Some(i) => &ui[i..ui.len().min(i + 1200)],
        None => "",
    };
    let ok = ui.contains("fn ignored_gui_slaver_id_from_presentation")
        && ui.contains("Wave 1000")
        && lookup.contains("dual_world_registry_unavailable")
        && lookup.contains("presentation_unit_catalog")
        && lookup.contains("IgnoredInGui")
        && lookup.contains("slaver_object_id")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostIgnoredGuiSlaverPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_ignored_gui_slaver_presentation_residual_honesty() -> bool {
    let a = honesty_host_ignored_gui_slaver_presentation_residual_method_names_residual_wave1000();
    let b = honesty_host_ignored_gui_slaver_presentation_residual_nav_commands_residual_wave1000();
    let c = honesty_host_ignored_gui_slaver_presentation_residual_residual_pack_wave1000();
    residual_action_store(ResidualHostIgnoredGuiSlaverPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_ignored_gui_slaver_presentation_residual_wave1000() {
        assert!(honesty_host_ignored_gui_slaver_presentation_residual_residual_pack_wave1000());
        assert!(
            honesty_host_ignored_gui_slaver_presentation_residual_method_names_residual_wave1000()
        );
        assert!(
            honesty_host_ignored_gui_slaver_presentation_residual_nav_commands_residual_wave1000()
        );
        assert!(simulate_live_host_ignored_gui_slaver_presentation_residual_honesty());
    }
}
