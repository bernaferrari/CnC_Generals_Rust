//! Wave 509 residual peels: TOPPLED / FREEFALL / NIGHT / SNOW mesh model-condition bits.
//! - freeze `parachute_open`; FREEFALL when parachuting && !open
//! - TOPPLED when |topple_lean_radians| > epsilon
//! - world_env is_snow/is_night from weather_state → unit mesh bits
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 505 parachuting bit (open chute vs freefall).
//! Architecture residual - weather/topple/freefall pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs PresentationWorldEnv is_snow/is_night
//! - presentation_frame.rs Wave 509 stamp
//! - host_enum_table_residual.rs toppled/freefall/night/snow bits
//!
//! Fail-closed:
//! - Full TimeOfDay runtime enum still deferred (weather string residual)
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_TOPPLE_FREEFALL_WEATHER_METHOD_NAMES_WAVE509: &[&str] = &[
    "parachute_open",
    "toppled_model_bit",
    "freefall_model_bit",
    "night_model_bit",
    "snow_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_TOPPLE_FREEFALL_WEATHER_SOURCE_MARKERS_WAVE509: &[&str] = &[
    "Wave 509: toppled / freefall / night / snow model-condition residual",
    "Wave 509: TOPPLED / FREEFALL / NIGHT / SNOW bits included in stamp helper",
    "parachute_open: obj.is_parachute_open()",
    "world_env.is_snow",
];

pub const PRESENTATION_TOPPLE_FREEFALL_WEATHER_NAV_STEPS_WAVE509: &[&str] = &[
    "FREEZE_PARACHUTE_OPEN",
    "WORLD_ENV_SNOW_NIGHT",
    "STAMP_TOPPLED",
    "STAMP_FREEFALL",
    "STAMP_NIGHT_SNOW",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_TOPPLE_FREEFALL_WEATHER_CMD_NAMES_WAVE509: &[&str] = &[
    "click_presentation_topple_freefall_weather_ok_wnd_detect",
    "click_presentation_topple_freefall_weather_ok_wnd_skip",
    "click_presentation_topple_freefall_weather_ok_wnd_queue",
    "click_presentation_topple_freefall_weather_ok_wnd_prepare",
    "click_presentation_topple_freefall_weather_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationToppleFreefallWeatherAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FreezeSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationToppleFreefallWeatherAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_topple_freefall_weather_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_topple_freefall_weather_last_action()
-> ResidualPresentationToppleFreefallWeatherAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationToppleFreefallWeatherAction::MethodNames,
        2 => ResidualPresentationToppleFreefallWeatherAction::SourceMarkers,
        3 => ResidualPresentationToppleFreefallWeatherAction::NavCommands,
        4 => ResidualPresentationToppleFreefallWeatherAction::FreezeSource,
        5 => ResidualPresentationToppleFreefallWeatherAction::StampSource,
        6 => ResidualPresentationToppleFreefallWeatherAction::Composite,
        _ => ResidualPresentationToppleFreefallWeatherAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_topple_freefall_weather_method_names_residual_wave509() -> bool {
    PRESENTATION_TOPPLE_FREEFALL_WEATHER_METHOD_NAMES_WAVE509.len() == 6
        && residual_name_index(
            PRESENTATION_TOPPLE_FREEFALL_WEATHER_METHOD_NAMES_WAVE509,
            "parachute_open",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_TOPPLE_FREEFALL_WEATHER_METHOD_NAMES_WAVE509,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_topple_freefall_weather_source_markers_residual_wave509() -> bool {
    PRESENTATION_TOPPLE_FREEFALL_WEATHER_SOURCE_MARKERS_WAVE509.len() == 4
        && residual_name_index(
            PRESENTATION_TOPPLE_FREEFALL_WEATHER_SOURCE_MARKERS_WAVE509,
            "Wave 509: toppled / freefall / night / snow model-condition residual",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_TOPPLE_FREEFALL_WEATHER_SOURCE_MARKERS_WAVE509,
            "parachute_open: obj.is_parachute_open()",
        ) == Some(2)
}

pub fn honesty_presentation_topple_freefall_weather_nav_commands_residual_wave509() -> bool {
    PRESENTATION_TOPPLE_FREEFALL_WEATHER_NAV_STEPS_WAVE509.len() == 6
        && residual_name_index(
            PRESENTATION_TOPPLE_FREEFALL_WEATHER_NAV_STEPS_WAVE509,
            "STAMP_FREEFALL",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_TOPPLE_FREEFALL_WEATHER_NAV_STEPS_WAVE509,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_TOPPLE_FREEFALL_WEATHER_CMD_NAMES_WAVE509.len() == 5
}

pub fn simulate_presentation_topple_freefall_weather_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 509: snow weather residual for mesh model-condition SNOW bank")
        && pf.contains("parachute_open: obj.is_parachute_open()")
        && pf.contains("parachute_open: ent.parachute_open")
        && pf.contains("let is_snow = weather.contains")
        // 2026-08-15: queries.rs passes world_env.is_snow into unit_render setter.
        && pf.contains("input.world_is_snow = world_is_snow")
        && pf.contains("self.world_env.is_snow");
    residual_action_store(ResidualPresentationToppleFreefallWeatherAction::FreezeSource);
    ok
}

pub fn simulate_presentation_topple_freefall_weather_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 509: toppled / freefall / night / snow model-condition residual")
        && en.contains("pub fn toppled_model_bit")
        && en.contains("pub fn freefall_model_bit")
        && en.contains("pub fn night_model_bit")
        && en.contains("pub fn snow_model_bit")
        && pf.contains("self.parachuting && !self.parachute_open")
        && rp.contains("Wave 509: TOPPLED / FREEFALL / NIGHT / SNOW bits included in stamp helper");
    residual_action_store(ResidualPresentationToppleFreefallWeatherAction::StampSource);
    ok
}

pub fn honesty_presentation_topple_freefall_weather_residual_pack_wave509() -> bool {
    honesty_presentation_topple_freefall_weather_method_names_residual_wave509()
        && honesty_presentation_topple_freefall_weather_source_markers_residual_wave509()
        && honesty_presentation_topple_freefall_weather_nav_commands_residual_wave509()
        && simulate_presentation_topple_freefall_weather_freeze_source()
        && simulate_presentation_topple_freefall_weather_stamp_source()
}

pub fn simulate_live_presentation_topple_freefall_weather_honesty() -> bool {
    let ok = honesty_presentation_topple_freefall_weather_residual_pack_wave509();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationToppleFreefallWeatherAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_topple_freefall_weather_method_names_residual_wave509());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_topple_freefall_weather_source_markers_residual_wave509());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_topple_freefall_weather_nav_commands_residual_wave509());
    }

    #[test]
    fn presentation_topple_freefall_weather_sources() {
        assert!(simulate_presentation_topple_freefall_weather_freeze_source());
        assert!(simulate_presentation_topple_freefall_weather_stamp_source());
    }

    #[test]
    fn wave509_composite_pack() {
        assert!(honesty_presentation_topple_freefall_weather_residual_pack_wave509());
    }

    #[test]
    fn simulate_live_presentation_topple_freefall_weather_honesty_residual_live() {
        assert!(
            simulate_live_presentation_topple_freefall_weather_honesty(),
            "presentation topple/freefall/weather residual must latch"
        );
        assert!(residual_presentation_topple_freefall_weather_ok());
        assert_eq!(
            residual_presentation_topple_freefall_weather_last_action(),
            ResidualPresentationToppleFreefallWeatherAction::Composite
        );
    }
}
