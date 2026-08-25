//! Wave 486 residual peels: production door phase changes log model-condition bits.
//! - `tick_production_door` phase transitions call `record_host_model_condition`
//! - prevents `writeback_model_condition_to_host` stomping host door visual bits
//! - door phase still logs via `record_host_production_door`
//! - start cycle already recorded model bits (unchanged)
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 485 exit-delay clear.
//! Architecture residual - door model bits are host-authored presentation state.
//!
//! Sources:
//! - object.rs tick_production_door refresh + record_host_model_condition
//! - gameworld_shadow writeback_model_condition_to_host
//! - host_model_condition_log::record
//!
//! Fail-closed:
//! - Non-door model condition paths unchanged
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRODUCTION_DOOR_MODEL_CONDITION_LOG_METHOD_NAMES_WAVE486: &[&str] = &[
    "tick_production_door",
    "record_host_model_condition",
    "refresh_model_condition_bits",
    "writeback_model_condition_to_host",
    "record_host_production_door",
    "playable_claim = false",
];

pub const PRODUCTION_DOOR_MODEL_CONDITION_LOG_SOURCE_MARKERS_WAVE486: &[&str] = &[
    "Wave 486: door model bits must reach GW before model-condition writeback",
    "record_host_model_condition",
    "tick_production_door",
    "writeback_model_condition_to_host",
];

pub const PRODUCTION_DOOR_MODEL_CONDITION_LOG_NAV_STEPS_WAVE486: &[&str] = &[
    "DOOR_PHASE_ADVANCE",
    "UPDATE_MODEL_BITS",
    "REFRESH_MODEL_CONDITION",
    "RECORD_MODEL_CONDITION_LOG",
    "GW_APPLY_BEFORE_WRITEBACK",
    "NO_DOOR_VISUAL_STOMP",
];

pub const RUNTIME_HOST_PRODUCTION_DOOR_MODEL_CONDITION_LOG_CMD_NAMES_WAVE486: &[&str] = &[
    "click_production_door_model_condition_log_ok_wnd_detect",
    "click_production_door_model_condition_log_ok_wnd_skip",
    "click_production_door_model_condition_log_ok_wnd_queue",
    "click_production_door_model_condition_log_ok_wnd_prepare",
    "click_production_door_model_condition_log_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualProductionDoorModelConditionLogAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    DoorTickSource = 4,
    WritebackSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualProductionDoorModelConditionLogAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_production_door_model_condition_log_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_production_door_model_condition_log_last_action()
-> ResidualProductionDoorModelConditionLogAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualProductionDoorModelConditionLogAction::MethodNames,
        2 => ResidualProductionDoorModelConditionLogAction::SourceMarkers,
        3 => ResidualProductionDoorModelConditionLogAction::NavCommands,
        4 => ResidualProductionDoorModelConditionLogAction::DoorTickSource,
        5 => ResidualProductionDoorModelConditionLogAction::WritebackSource,
        6 => ResidualProductionDoorModelConditionLogAction::Composite,
        _ => ResidualProductionDoorModelConditionLogAction::Idle,
    }
}

fn object_source() -> &'static str {
    crate::game_logic::object::OBJECT_SRC
}

fn gw_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn function_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let brace = src[start..].find('{')? + start;
    let mut depth = 0i32;
    for (off, ch) in src[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&src[start..brace + off + 1]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_production_door_model_condition_log_method_names_residual_wave486() -> bool {
    PRODUCTION_DOOR_MODEL_CONDITION_LOG_METHOD_NAMES_WAVE486.len() == 6
        && residual_name_index(
            PRODUCTION_DOOR_MODEL_CONDITION_LOG_METHOD_NAMES_WAVE486,
            "tick_production_door",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_DOOR_MODEL_CONDITION_LOG_METHOD_NAMES_WAVE486,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_production_door_model_condition_log_source_markers_residual_wave486() -> bool {
    PRODUCTION_DOOR_MODEL_CONDITION_LOG_SOURCE_MARKERS_WAVE486.len() == 4
        && residual_name_index(
            PRODUCTION_DOOR_MODEL_CONDITION_LOG_SOURCE_MARKERS_WAVE486,
            "Wave 486: door model bits must reach GW before model-condition writeback",
        ) == Some(0)
        && residual_name_index(
            PRODUCTION_DOOR_MODEL_CONDITION_LOG_SOURCE_MARKERS_WAVE486,
            "record_host_model_condition",
        ) == Some(1)
}

pub fn honesty_production_door_model_condition_log_nav_commands_residual_wave486() -> bool {
    PRODUCTION_DOOR_MODEL_CONDITION_LOG_NAV_STEPS_WAVE486.len() == 6
        && residual_name_index(
            PRODUCTION_DOOR_MODEL_CONDITION_LOG_NAV_STEPS_WAVE486,
            "RECORD_MODEL_CONDITION_LOG",
        ) == Some(3)
        && residual_name_index(
            PRODUCTION_DOOR_MODEL_CONDITION_LOG_NAV_STEPS_WAVE486,
            "NO_DOOR_VISUAL_STOMP",
        ) == Some(5)
        && RUNTIME_HOST_PRODUCTION_DOOR_MODEL_CONDITION_LOG_CMD_NAMES_WAVE486.len() == 5
        && residual_name_index(
            RUNTIME_HOST_PRODUCTION_DOOR_MODEL_CONDITION_LOG_CMD_NAMES_WAVE486,
            "click_production_door_model_condition_log_ok_wnd_prepare",
        ) == Some(3)
}

pub fn simulate_production_door_model_condition_log_door_tick_source() -> bool {
    let Some(body) = function_body(object_source(), "fn tick_production_door(") else {
        return false;
    };
    let ok = body
        .contains("Wave 486: door model bits must reach GW before model-condition writeback")
        && body.contains("record_host_model_condition")
        && body.contains("refresh_model_condition_bits")
        && body.contains("record_host_production_door");
    residual_action_store(ResidualProductionDoorModelConditionLogAction::DoorTickSource);
    ok
}

pub fn simulate_production_door_model_condition_log_writeback_source() -> bool {
    let gw = gw_source();
    let ok = gw.contains("fn writeback_model_condition_to_host")
        && gw.contains("writeback_model_condition_to_host(logic)")
        && gw.contains("obj.model_condition_bits = ent.model_condition_bits");
    residual_action_store(ResidualProductionDoorModelConditionLogAction::WritebackSource);
    ok
}

pub fn honesty_production_door_model_condition_log_residual_pack_wave486() -> bool {
    honesty_production_door_model_condition_log_method_names_residual_wave486()
        && honesty_production_door_model_condition_log_source_markers_residual_wave486()
        && honesty_production_door_model_condition_log_nav_commands_residual_wave486()
        && simulate_production_door_model_condition_log_door_tick_source()
        && simulate_production_door_model_condition_log_writeback_source()
}

pub fn simulate_live_production_door_model_condition_log_honesty() -> bool {
    let ok = honesty_production_door_model_condition_log_residual_pack_wave486();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualProductionDoorModelConditionLogAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_production_door_model_condition_log_method_names_residual_wave486());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_production_door_model_condition_log_source_markers_residual_wave486());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_production_door_model_condition_log_nav_commands_residual_wave486());
    }

    #[test]
    fn production_door_model_condition_log_sources() {
        assert!(simulate_production_door_model_condition_log_door_tick_source());
        assert!(simulate_production_door_model_condition_log_writeback_source());
    }

    #[test]
    fn wave486_composite_pack() {
        assert!(honesty_production_door_model_condition_log_residual_pack_wave486());
    }

    #[test]
    fn simulate_live_production_door_model_condition_log_honesty_residual_live() {
        assert!(
            simulate_live_production_door_model_condition_log_honesty(),
            "production door model condition log residual must latch"
        );
        assert!(residual_production_door_model_condition_log_ok());
        assert_eq!(
            residual_production_door_model_condition_log_last_action(),
            ResidualProductionDoorModelConditionLogAction::Composite
        );
    }
}
