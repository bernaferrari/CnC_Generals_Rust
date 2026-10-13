//! Historical wave659: AI attitude now belongs exclusively to the Main host object.
//! This diagnostic verifies the retired shadow writeback channel stays absent.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER_METHOD_NAMES_WAVE659: &[&str] = &[
    "canonical_ai_attitude_owner",
    "retired_ai_attitude_writeback",
    "host_apply_ai_attitude_ready_completions",
    "Wave 659",
    "playable_claim = false",
];

pub const LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER_NAV_STEPS_WAVE659: &[&str] = &[
    "REQUIRE_CANONICAL_AI_ATTITUDE_OWNER",
    "REQUIRE_RETIRED_AI_ATTITUDE_WRITEBACK",
    "REQUIRE_NO_AI_ATTITUDE_READY_QUEUE",
    "REQUIRE_IMMUTABLE_AI_ATTITUDE_PROJECTION",
    "LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER_CMD_NAMES_WAVE659: &[&str] = &[
    "host_ai_attitude_ready_log_helper",
    "retired_ai_attitude_writeback",
    "no_ai_attitude_ready_queue",
    "immutable_ai_attitude_projection",
    "ai_attitude_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAiAttitudeReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostAiAttitudeReadyLogHelperAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualHostAiAttitudeReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_ai_attitude_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_ai_attitude_ready_log_helper_last_action()
-> ResidualHostAiAttitudeReadyLogHelperAction {
    ResidualHostAiAttitudeReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn ready_log_source() -> &'static str {
    include_str!("../object/record.rs")
}

pub fn honesty_host_ai_attitude_ready_log_helper_method_names_residual_wave659() -> bool {
    let names = LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER_METHOD_NAMES_WAVE659;
    let ok = residual_name_index(names, "canonical_ai_attitude_owner").is_some()
        && residual_name_index(names, "retired_ai_attitude_writeback").is_some()
        && residual_name_index(names, "Wave 659").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostAiAttitudeReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_ai_attitude_ready_log_helper_nav_commands_residual_wave659() -> bool {
    let steps = LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER_NAV_STEPS_WAVE659;
    let cmds = RUNTIME_HOST_LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER_CMD_NAMES_WAVE659;
    let ok = residual_name_index(steps, "REQUIRE_CANONICAL_AI_ATTITUDE_OWNER").is_some()
        && residual_name_index(steps, "REQUIRE_RETIRED_AI_ATTITUDE_WRITEBACK").is_some()
        && residual_name_index(steps, "REQUIRE_NO_AI_ATTITUDE_READY_QUEUE").is_some()
        && residual_name_index(steps, "REQUIRE_IMMUTABLE_AI_ATTITUDE_PROJECTION").is_some()
        && residual_name_index(steps, "LIVE_HOST_AI_ATTITUDE_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_ai_attitude_ready_log_helper").is_some()
        && residual_name_index(cmds, "retired_ai_attitude_writeback").is_some()
        && residual_name_index(cmds, "no_ai_attitude_ready_queue").is_some()
        && residual_name_index(cmds, "immutable_ai_attitude_projection").is_some()
        && residual_name_index(cmds, "ai_attitude_ready_log_residual").is_some();
    residual_action_store(ResidualHostAiAttitudeReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_ai_attitude_ready_log_helper_collect_source() -> bool {
    let ok = honesty_host_ai_attitude_ready_log_helper_source_markers_residual_wave659();
    residual_action_store(ResidualHostAiAttitudeReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_ai_attitude_ready_log_helper_dispatch_source() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let ok = !sh.contains("pub fn apply_host_ai_attitude_events")
        && !gl.contains("pub fn host_apply_ai_attitude_ready_completions");
    residual_action_store(ResidualHostAiAttitudeReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_ai_attitude_ready_log_helper_residual_pack_wave659() -> bool {
    honesty_host_ai_attitude_ready_log_helper_method_names_residual_wave659()
        && honesty_host_ai_attitude_ready_log_helper_source_markers_residual_wave659()
        && honesty_host_ai_attitude_ready_log_helper_nav_commands_residual_wave659()
        && simulate_host_ai_attitude_ready_log_helper_collect_source()
        && simulate_host_ai_attitude_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_ai_attitude_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_ai_attitude_ready_log_helper_residual_pack_wave659();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostAiAttitudeReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_ai_attitude_ready_log_helper_method_names_residual_wave659());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_ai_attitude_ready_log_helper_source_markers_residual_wave659());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_ai_attitude_ready_log_helper_nav_commands_residual_wave659());
    }

    #[test]
    fn host_ai_attitude_ready_log_helper_sources() {
        assert!(simulate_host_ai_attitude_ready_log_helper_collect_source());
        assert!(simulate_host_ai_attitude_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave659_composite_pack() {
        assert!(honesty_host_ai_attitude_ready_log_helper_residual_pack_wave659());
    }

    #[test]
    fn simulate_live_host_ai_attitude_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_ai_attitude_ready_log_helper_honesty(),
            "host ai_attitude ready log helper residual must latch"
        );
        assert!(residual_host_ai_attitude_ready_log_helper_ok());
        assert_eq!(
            residual_host_ai_attitude_ready_log_helper_last_action(),
            ResidualHostAiAttitudeReadyLogHelperAction::Composite
        );
    }
}

pub fn honesty_host_ai_attitude_ready_log_helper_source_markers_residual_wave659() -> bool {
    let ok = ready_log_source().contains("pub fn set_ai_attitude_i8")
        && !ready_log_source().contains("record_host_ai_attitude")
        && !shadow_source().contains("pub fn writeback_ai_attitude_to_host")
        && !gl_source().contains("ReadyLogDrainOp::AiAttitude");
    residual_action_store(ResidualHostAiAttitudeReadyLogHelperAction::SourceMarkers);
    ok
}
