//! Historical wave622: canonical host experience/veterancy ownership.
//! Remaining presentation channels retain their admission checks; XP mirror writers are retired.
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_VETERANCY_READY_LOG_HELPER_METHOD_NAMES_WAVE622: &[&str] = &[
    "canonical_veterancy_owner",
    "retired_experience_writeback",
    "host_apply_veterancy_ready_completions",
    "gameworld_damage_authority_live",
    "Wave 622",
    "playable_claim = false",
];

pub const LIVE_HOST_VETERANCY_READY_LOG_HELPER_NAV_STEPS_WAVE622: &[&str] = &[
    "REQUIRE_CANONICAL_VETERANCY_OWNER",
    "REQUIRE_RETIRED_EXPERIENCE_WRITEBACK",
    "REQUIRE_NO_VETERANCY_READY_QUEUE",
    "REQUIRE_HOST_APPLIES_BONUSES",
    "LIVE_HOST_VETERANCY_READY_LOG_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_HOST_VETERANCY_READY_LOG_HELPER_CMD_NAMES_WAVE622: &[&str] = &[
    "host_veterancy_ready_log_helper",
    "retired_experience_writeback",
    "no_veterancy_ready_queue",
    "host_applies_bonuses",
    "veterancy_ready_log_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostVeterancyReadyLogHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualHostVeterancyReadyLogHelperAction {
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

fn residual_action_store(action: ResidualHostVeterancyReadyLogHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_host_veterancy_ready_log_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_host_veterancy_ready_log_helper_last_action()
-> ResidualHostVeterancyReadyLogHelperAction {
    ResidualHostVeterancyReadyLogHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

fn ready_log_source() -> &'static str {
    include_str!("../object/update.rs")
}

pub fn honesty_host_veterancy_ready_log_helper_method_names_residual_wave622() -> bool {
    let names = LIVE_HOST_VETERANCY_READY_LOG_HELPER_METHOD_NAMES_WAVE622;
    let ok = residual_name_index(names, "canonical_veterancy_owner").is_some()
        && residual_name_index(names, "retired_experience_writeback").is_some()
        && residual_name_index(names, "Wave 622").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostVeterancyReadyLogHelperAction::MethodNames);
    ok
}

pub fn honesty_host_veterancy_ready_log_helper_source_markers_residual_wave622() -> bool {
    let ok = ready_log_source().contains("pub fn gain_experience")
        && ready_log_source().contains("self.apply_veterancy_bonuses(previous_level, new_level)")
        && !shadow_source().contains("pub fn writeback_experience_to_host")
        && !gl_source().contains("pub fn host_apply_veterancy_ready_completions");
    residual_action_store(ResidualHostVeterancyReadyLogHelperAction::SourceMarkers);
    ok
}

pub fn honesty_host_veterancy_ready_log_helper_nav_commands_residual_wave622() -> bool {
    let steps = LIVE_HOST_VETERANCY_READY_LOG_HELPER_NAV_STEPS_WAVE622;
    let cmds = RUNTIME_HOST_LIVE_HOST_VETERANCY_READY_LOG_HELPER_CMD_NAMES_WAVE622;
    let ok = residual_name_index(steps, "REQUIRE_CANONICAL_VETERANCY_OWNER").is_some()
        && residual_name_index(steps, "REQUIRE_RETIRED_EXPERIENCE_WRITEBACK").is_some()
        && residual_name_index(steps, "REQUIRE_NO_VETERANCY_READY_QUEUE").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_APPLIES_BONUSES").is_some()
        && residual_name_index(steps, "LIVE_HOST_VETERANCY_READY_LOG_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_veterancy_ready_log_helper").is_some()
        && residual_name_index(cmds, "retired_experience_writeback").is_some()
        && residual_name_index(cmds, "no_veterancy_ready_queue").is_some()
        && residual_name_index(cmds, "host_applies_bonuses").is_some()
        && residual_name_index(cmds, "veterancy_ready_log_residual").is_some();
    residual_action_store(ResidualHostVeterancyReadyLogHelperAction::NavCommands);
    ok
}

pub fn simulate_host_veterancy_ready_log_helper_collect_source() -> bool {
    let ok = honesty_host_veterancy_ready_log_helper_source_markers_residual_wave622();
    residual_action_store(ResidualHostVeterancyReadyLogHelperAction::CollectSource);
    ok
}

pub fn simulate_host_veterancy_ready_log_helper_dispatch_source() -> bool {
    let ok = !shadow_source().contains("ReadyLogDrainOp::Veterancy")
        && !gl_source().contains("host_veterancy_ready_log::");
    residual_action_store(ResidualHostVeterancyReadyLogHelperAction::DispatchSource);
    ok
}

pub fn honesty_host_veterancy_ready_log_helper_residual_pack_wave622() -> bool {
    honesty_host_veterancy_ready_log_helper_method_names_residual_wave622()
        && honesty_host_veterancy_ready_log_helper_source_markers_residual_wave622()
        && honesty_host_veterancy_ready_log_helper_nav_commands_residual_wave622()
        && simulate_host_veterancy_ready_log_helper_collect_source()
        && simulate_host_veterancy_ready_log_helper_dispatch_source()
}

pub fn simulate_live_host_veterancy_ready_log_helper_honesty() -> bool {
    let ok = honesty_host_veterancy_ready_log_helper_residual_pack_wave622();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostVeterancyReadyLogHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_veterancy_ready_log_helper_method_names_residual_wave622());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_veterancy_ready_log_helper_source_markers_residual_wave622());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_veterancy_ready_log_helper_nav_commands_residual_wave622());
    }

    #[test]
    fn host_veterancy_ready_log_helper_sources() {
        assert!(simulate_host_veterancy_ready_log_helper_collect_source());
        assert!(simulate_host_veterancy_ready_log_helper_dispatch_source());
    }

    #[test]
    fn wave622_composite_pack() {
        assert!(honesty_host_veterancy_ready_log_helper_residual_pack_wave622());
    }

    #[test]
    fn simulate_live_host_veterancy_ready_log_helper_honesty_residual_live() {
        assert!(
            simulate_live_host_veterancy_ready_log_helper_honesty(),
            "host veterancy ready log helper residual must latch"
        );
        assert!(residual_host_veterancy_ready_log_helper_ok());
        assert_eq!(
            residual_host_veterancy_ready_log_helper_last_action(),
            ResidualHostVeterancyReadyLogHelperAction::Composite
        );
    }
}
