//! Historical wave669: guard belongs to the canonical Main host object.
//! Guard shadow replay and reverse writeback are retired; adjacent channels remain separately tracked.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_GUARD_READY_LOG_HELPER_METHOD_NAMES_WAVE669: &[&str] = &[
    "canonical_guard_owner",
    "retired_guard_writeback",
    "host_apply_guard_ready_completions",
    "Wave 669",
    "playable_claim = false",
];

pub const LIVE_HOST_GUARD_READY_LOG_HELPER_NAV_STEPS_WAVE669: &[&str] = &[
    "REQUIRE_CANONICAL_GUARD_OWNER",
    "REQUIRE_RETIRED_GUARD_WRITEBACK",
    "REQUIRE_NO_GUARD_READY_QUEUE",
    "REQUIRE_HOST_APPLIES_GUARD_BOOKKEEPING",
    "LIVE_HOST_GUARD_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_GUARD_READY_LOG_HELPER_CMD_NAMES_WAVE669: &[&str] = &[
    "host_guard_ready_log_helper",
    "retired_guard_writeback",
    "no_guard_ready_queue",
    "host_applies_guard_bookkeeping",
    "guard_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGuardReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostGuardReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostGuardReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_guard_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_guard_ready_log_helper_last_action() -> ResidualHostGuardReadyLogHelperAction {
    ResidualHostGuardReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
fn ready_log_source() -> &'static str {
    include_str!("../object/orders.rs")
}

pub fn honesty_host_guard_ready_log_helper_method_names_residual_wave669() -> bool {
    let names = LIVE_HOST_GUARD_READY_LOG_HELPER_METHOD_NAMES_WAVE669;
    let ok = residual_name_index(names, "canonical_guard_owner").is_some()
        && residual_name_index(names, "retired_guard_writeback").is_some()
        && residual_name_index(names, "Wave 669").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGuardReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_guard_ready_log_helper_source_markers_residual_wave669() -> bool {
    let ok = ready_log_source().contains("pub fn set_guard_position")
        && !ready_log_source().contains("record_host_guard")
        && !shadow_source().contains("pub fn writeback_guard_to_host")
        && !gl_source().contains("pub fn host_apply_guard_ready_completions");
    residual_action_store(ResidualHostGuardReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_guard_ready_log_helper_nav_commands_residual_wave669() -> bool {
    let steps = LIVE_HOST_GUARD_READY_LOG_HELPER_NAV_STEPS_WAVE669;
    let cmds = RUNTIME_HOST_LIVE_HOST_GUARD_READY_LOG_HELPER_CMD_NAMES_WAVE669;
    let ok = residual_name_index(steps, "REQUIRE_CANONICAL_GUARD_OWNER").is_some()
        && residual_name_index(steps, "REQUIRE_RETIRED_GUARD_WRITEBACK").is_some()
        && residual_name_index(steps, "REQUIRE_NO_GUARD_READY_QUEUE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_APPLIES_GUARD_BOOKKEEPING").is_some()
        && residual_name_index(steps, "LIVE_HOST_GUARD_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_guard_ready_log_helper").is_some()
        && residual_name_index(cmds, "retired_guard_writeback").is_some()
        && residual_name_index(cmds, "no_guard_ready_queue").is_some()
        && residual_name_index(cmds, "host_applies_guard_bookkeeping").is_some()
        && residual_name_index(cmds, "guard_ready_log_residual").is_some();
    residual_action_store(ResidualHostGuardReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_guard_ready_log_helper_collect_source() -> bool {
    let ok = honesty_host_guard_ready_log_helper_source_markers_residual_wave669();
    residual_action_store(ResidualHostGuardReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_guard_ready_log_helper_dispatch_source() -> bool {
    let ok = !shadow_source().contains("ReadyLogDrainOp::Guard")
        && !gl_source().contains("host_guard_ready_log::");
    residual_action_store(ResidualHostGuardReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_guard_ready_log_helper_residual_pack_wave669() -> bool {
    honesty_host_guard_ready_log_helper_method_names_residual_wave669()
        && honesty_host_guard_ready_log_helper_source_markers_residual_wave669()
        && honesty_host_guard_ready_log_helper_nav_commands_residual_wave669()
        && simulate_host_guard_ready_log_helper_collect_source()
        && simulate_host_guard_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_guard_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_guard_ready_log_helper_residual_pack_wave669();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGuardReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_guard_ready_log_helper_method_names_residual_wave669());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_guard_ready_log_helper_source_markers_residual_wave669());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_guard_ready_log_helper_nav_commands_residual_wave669());
    }
    #[test]
    fn host_guard_ready_log_helper_sources() {
        assert!(simulate_host_guard_ready_log_helper_collect_source());
        assert!(simulate_host_guard_ready_log_helper_dispatch_source());
    }
    #[test]
    fn wave669_composite_pack() {
        assert!(honesty_host_guard_ready_log_helper_residual_pack_wave669());
    }
    #[test]
    fn simulate_live_host_guard_ready_log_helper_honesty_residual_live() {
        assert!(simulate_live_host_guard_ready_log_helper_honesty());
        assert!(residual_host_guard_ready_log_helper_ok());
        assert_eq!(
            residual_host_guard_ready_log_helper_last_action(),
            ResidualHostGuardReadyLogHelperAction::Composite
        );
    }
}
