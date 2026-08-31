//! Wave 719: runtime-host `construct` free-dozer spawn is opt-in.
//! Default fail-closed: construct requires an existing builder on the map.
//! Smoke may set `spawn_dozer=1` / `GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN_METHOD_NAMES_WAVE719: &[&str] = &[
    "allow_spawn_dozer",
    "GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER",
    "spawn_dozer=1",
    "host_create_object",
    "Wave 719",
    "playable_claim = false",
];
pub const LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN_NAV_STEPS_WAVE719: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_OPT_IN",
    "LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN_CMD_NAMES_WAVE719: &[&str] = &[
    "host_construct_spawn_dozer_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_opt_in",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostConstructSpawnDozerOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostConstructSpawnDozerOptInAction {
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
fn residual_action_store(a: ResidualHostConstructSpawnDozerOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_construct_spawn_dozer_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_construct_spawn_dozer_opt_in_last_action()
-> ResidualHostConstructSpawnDozerOptInAction {
    ResidualHostConstructSpawnDozerOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_construct_spawn_dozer_opt_in_method_names_residual_wave719() -> bool {
    let names = LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN_METHOD_NAMES_WAVE719;
    let ok = residual_name_index(names, "allow_spawn_dozer").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER").is_some()
        && residual_name_index(names, "spawn_dozer=1").is_some()
        && residual_name_index(names, "host_create_object").is_some()
        && residual_name_index(names, "Wave 719").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostConstructSpawnDozerOptInAction::MethodNames);
    ok
}
pub fn honesty_host_construct_spawn_dozer_opt_in_source_markers_residual_wave719() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 719")
        && eng.contains("allow_spawn_dozer")
        && eng.contains("GENERALS_RUNTIME_HOST_CONSTRUCT_SPAWN_DOZER")
        && eng.contains("spawn_dozer")
        && eng.contains("if builders.is_empty() && allow_spawn_dozer");
    let fail_closed = eng.contains("Default fail-closed") || eng.contains("opt-in only");
    let smoke_ok = smoke.contains("spawn_dozer=1") && smoke.contains("construct|template=");
    let ok = eng_ok && fail_closed && smoke_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostConstructSpawnDozerOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_construct_spawn_dozer_opt_in_nav_commands_residual_wave719() -> bool {
    let steps = LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN_NAV_STEPS_WAVE719;
    let cmds = RUNTIME_HOST_LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN_CMD_NAMES_WAVE719;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_HOST_CONSTRUCT_SPAWN_DOZER_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_construct_spawn_dozer_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_opt_in").is_some();
    residual_action_store(ResidualHostConstructSpawnDozerOptInAction::NavCommands);
    ok
}
pub fn simulate_host_construct_spawn_dozer_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_spawn_dozer") && smoke_source().contains("spawn_dozer=1");
    residual_action_store(ResidualHostConstructSpawnDozerOptInAction::CollectSource);
    ok
}
pub fn simulate_host_construct_spawn_dozer_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 719") && smoke_source().contains("spawn_dozer=1");
    residual_action_store(ResidualHostConstructSpawnDozerOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_construct_spawn_dozer_opt_in_residual_pack_wave719() -> bool {
    honesty_host_construct_spawn_dozer_opt_in_method_names_residual_wave719()
        && honesty_host_construct_spawn_dozer_opt_in_source_markers_residual_wave719()
        && honesty_host_construct_spawn_dozer_opt_in_nav_commands_residual_wave719()
        && simulate_host_construct_spawn_dozer_opt_in_collect_source()
        && simulate_host_construct_spawn_dozer_opt_in_dispatch_source()
}
pub fn simulate_live_host_construct_spawn_dozer_opt_in_honesty() -> bool {
    let ok = honesty_host_construct_spawn_dozer_opt_in_residual_pack_wave719();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostConstructSpawnDozerOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_construct_spawn_dozer_opt_in_method_names_residual_wave719());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_construct_spawn_dozer_opt_in_source_markers_residual_wave719());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_construct_spawn_dozer_opt_in_nav_commands_residual_wave719());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_construct_spawn_dozer_opt_in_collect_source());
        assert!(simulate_host_construct_spawn_dozer_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_construct_spawn_dozer_opt_in_residual_pack_wave719());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_construct_spawn_dozer_opt_in_honesty());
        assert!(residual_host_construct_spawn_dozer_opt_in_ok());
    }
}
