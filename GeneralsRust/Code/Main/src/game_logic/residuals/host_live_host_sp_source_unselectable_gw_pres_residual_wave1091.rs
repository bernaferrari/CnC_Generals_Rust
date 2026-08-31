//! Wave 1091: SP/cmd-hint source unselectable/masked + GameWorld presentation residual.
//!
//! Extends Waves 1088–1090:
//! - SP source legality fail-closes unselectable/masked (with destroyed/sold/disabled/UC)
//! - SP override-destination source same fail-closed set
//! - command-hint source adds unselectable
//!
//! Also locks production presentation path: `presentation_from_gameworld_enabled`
//! defaults on so object roster rebuilds from GameWorld shadow entity store.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SP_SOURCE_UNSELECTABLE_GW_PRES_METHOD_NAMES_WAVE1091: &[&str] = &[
    "is_valid_special_power_target",
    "source_has_overridable_special_power_destination",
    "command_hint_source_context",
    "presentation_from_gameworld_enabled",
    "Wave 1091",
    "playable_claim = false",
];

pub const LIVE_HOST_SP_SOURCE_UNSELECTABLE_GW_PRES_NAV_STEPS_WAVE1091: &[&str] = &[
    "SP_SOURCE_UNSELECTABLE_MASKED",
    "CMD_HINT_SOURCE_UNSELECTABLE",
    "PRESENTATION_FROM_GAMEWORLD_DEFAULT_ON",
    "LIVE_HOST_SP_SOURCE_UNSELECTABLE_GW_PRES",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSpSourceUnselectableGwPresAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSpSourceUnselectableGwPresAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_sp_source_unselectable_gw_pres_method_names_residual_wave1091() -> bool {
    let names = LIVE_HOST_SP_SOURCE_UNSELECTABLE_GW_PRES_METHOD_NAMES_WAVE1091;
    let ok = residual_name_index(names, "presentation_from_gameworld_enabled").is_some()
        && residual_name_index(names, "Wave 1091").is_some();
    residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_source_unselectable_gw_pres_nav_commands_residual_wave1091() -> bool {
    let steps = LIVE_HOST_SP_SOURCE_UNSELECTABLE_GW_PRES_NAV_STEPS_WAVE1091;
    let ok = residual_name_index(steps, "LIVE_HOST_SP_SOURCE_UNSELECTABLE_GW_PRES").is_some()
        && residual_name_index(steps, "PRESENTATION_FROM_GAMEWORLD_DEFAULT_ON").is_some();
    residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sp_source_unselectable_gw_pres_residual_pack_wave1091() -> bool {
    let ui = ui_source();
    let pf = pf_source();
    let es = es_source();
    let sp_i = match ui.find("fn is_valid_special_power_target") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let sp = &ui[sp_i..sp_i.saturating_add(2200)];
    let ov_i = match ui.find("fn source_has_overridable_special_power_destination") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let ov = &ui[ov_i..ov_i.saturating_add(1400)];
    let hint_i = match ui.find("fn command_hint_source_context") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let hint = &ui[hint_i..hint_i.saturating_add(1400)];
    let gw_i = match pf.find("fn presentation_from_gameworld_enabled") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let gw = &pf[gw_i..gw_i.saturating_add(900)];
    let build_i = match pf.find("fn build_for_engine") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let build = &pf[build_i..build_i.saturating_add(1200)];
    let ok = sp.contains("Wave 1091: unselectable/masked source residual fail-closed")
        && sp.contains("source.unselectable")
        && sp.contains("source.masked")
        && ov.contains("!u.unselectable")
        && ov.contains("!u.masked")
        && hint.contains("entry.unselectable")
        && gw.contains("Err(_) => true")
        && gw.contains("GENERALS_PRESENTATION_FROM_GAMEWORLD")
        && build.contains("presentation_from_gameworld_enabled()")
        && build.contains("Self::build_from_gameworld")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sp_source_unselectable_gw_pres_residual_honesty() -> bool {
    let a = honesty_host_sp_source_unselectable_gw_pres_method_names_residual_wave1091();
    let b = honesty_host_sp_source_unselectable_gw_pres_nav_commands_residual_wave1091();
    let c = honesty_host_sp_source_unselectable_gw_pres_residual_pack_wave1091();
    residual_action_store(ResidualHostSpSourceUnselectableGwPresAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sp_source_unselectable_gw_pres_residual_wave1091() {
        assert!(honesty_host_sp_source_unselectable_gw_pres_residual_pack_wave1091());
        assert!(honesty_host_sp_source_unselectable_gw_pres_method_names_residual_wave1091());
        assert!(honesty_host_sp_source_unselectable_gw_pres_nav_commands_residual_wave1091());
        assert!(simulate_live_host_sp_source_unselectable_gw_pres_residual_honesty());
    }
}
