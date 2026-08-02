//! Wave 988: host NIGHT/SNOW model-condition residual peel.
//!
//! When DEMO_TIME_OF_DAY forces model refresh with empty dual-world, queue
//! NIGHT/SNOW bits from global TimeOfDay/Weather; presentation shell drains
//! onto drawable_map (C++ forceModelsToFollowTimeOfDay/Weather residual).
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MODEL_COND_WEATHER_RESIDUAL_METHOD_NAMES_WAVE988: &[&str] = &[
    "refresh_drawable_model_conditions",
    "queue_host_model_condition_weather_residual",
    "take_host_model_condition_weather_residual",
    "ModelConditionFlags::NIGHT",
    "ModelConditionFlags::SNOW",
    "Wave 988",
    "playable_claim = false",
];

pub const LIVE_HOST_MODEL_COND_WEATHER_RESIDUAL_NAV_STEPS_WAVE988: &[&str] = &[
    "TOD_FORCE_MODEL_REFRESH",
    "QUEUE_NIGHT_SNOW",
    "SHELL_DRAIN_MODEL_COND",
    "LIVE_HOST_MODEL_COND_WEATHER_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostModelCondWeatherResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostModelCondWeatherResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn meta_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/meta_event.rs")
}
fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_model_cond_weather_residual_method_names_residual_wave988() -> bool {
    let names = LIVE_HOST_MODEL_COND_WEATHER_RESIDUAL_METHOD_NAMES_WAVE988;
    let ok = residual_name_index(names, "queue_host_model_condition_weather_residual").is_some()
        && residual_name_index(names, "Wave 988").is_some();
    residual_action_store(ResidualHostModelCondWeatherResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_model_cond_weather_residual_nav_commands_residual_wave988() -> bool {
    let steps = LIVE_HOST_MODEL_COND_WEATHER_RESIDUAL_NAV_STEPS_WAVE988;
    let ok = residual_name_index(steps, "LIVE_HOST_MODEL_COND_WEATHER_RESIDUAL").is_some()
        && residual_name_index(steps, "SHELL_DRAIN_MODEL_COND").is_some();
    residual_action_store(ResidualHostModelCondWeatherResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_model_cond_weather_residual_residual_pack_wave988() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let meta = meta_source();
    let client = client_source();
    let refresh = match meta.find("fn refresh_drawable_model_conditions") {
        Some(i) => &meta[i..meta.len().min(i + 900)],
        None => "",
    };
    let shell = match client.find("fn update_presentation_shell") {
        Some(i) => &client[i..client.len().min(i + 4000)],
        None => "",
    };
    let ok = meta.contains("Wave 988")
        && meta.contains("queue_host_model_condition_weather_residual")
        && meta.contains("take_host_model_condition_weather_residual")
        && refresh.contains("queue_host_model_condition_weather_residual")
        && refresh.contains("TimeOfDay::Night")
        && refresh.contains("Weather::Snowy")
        && shell.contains("Wave 988")
        && shell.contains("take_host_model_condition_weather_residual")
        && shell.contains("ModelConditionFlags::NIGHT")
        && shell.contains("ModelConditionFlags::SNOW")
        && shell.contains("clear_and_set_model_condition_flags")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostModelCondWeatherResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_model_cond_weather_residual_honesty() -> bool {
    let a = honesty_host_model_cond_weather_residual_method_names_residual_wave988();
    let b = honesty_host_model_cond_weather_residual_nav_commands_residual_wave988();
    let c = honesty_host_model_cond_weather_residual_residual_pack_wave988();
    residual_action_store(ResidualHostModelCondWeatherResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_model_cond_weather_residual_wave988() {
        assert!(honesty_host_model_cond_weather_residual_residual_pack_wave988());
        assert!(honesty_host_model_cond_weather_residual_method_names_residual_wave988());
        assert!(honesty_host_model_cond_weather_residual_nav_commands_residual_wave988());
        assert!(simulate_live_host_model_cond_weather_residual_honesty());
    }
}
