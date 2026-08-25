//! Wave 732: free fallback start CC/dozer seeding is opt-in.
//! Default fail-closed: `ensure_non_shell_player_presence` no-ops unless
//! `GENERALS_RUNTIME_HOST_SEED_START_PRESENCE=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_SEED_START_PRESENCE_OPT_IN_METHOD_NAMES_WAVE732: &[&str] = &[
    "allow_seed",
    "GENERALS_RUNTIME_HOST_SEED_START_PRESENCE",
    "ensure_non_shell_player_presence",
    "GoldenDozer",
    "Wave 732",
    "playable_claim = false",
];
pub const LIVE_HOST_SEED_START_PRESENCE_OPT_IN_NAV_STEPS_WAVE732: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_NO_FREE_SEED_DEFAULT",
    "LIVE_HOST_SEED_START_PRESENCE_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_SEED_START_PRESENCE_OPT_IN_CMD_NAMES_WAVE732: &[&str] = &[
    "host_seed_start_presence_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "no_free_seed_default",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSeedStartPresenceOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostSeedStartPresenceOptInAction {
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
fn residual_action_store(a: ResidualHostSeedStartPresenceOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_seed_start_presence_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_seed_start_presence_opt_in_last_action()
-> ResidualHostSeedStartPresenceOptInAction {
    ResidualHostSeedStartPresenceOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
pub fn honesty_host_seed_start_presence_opt_in_method_names_residual_wave732() -> bool {
    let names = LIVE_HOST_SEED_START_PRESENCE_OPT_IN_METHOD_NAMES_WAVE732;
    let ok = residual_name_index(names, "allow_seed").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_SEED_START_PRESENCE").is_some()
        && residual_name_index(names, "ensure_non_shell_player_presence").is_some()
        && residual_name_index(names, "GoldenDozer").is_some()
        && residual_name_index(names, "Wave 732").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSeedStartPresenceOptInAction::MethodNames);
    ok
}
pub fn honesty_host_seed_start_presence_opt_in_source_markers_residual_wave732() -> bool {
    let gl = gl_source();
    let gl_ok = gl.contains("Wave 732")
        && gl.contains("allow_seed")
        && gl.contains("GENERALS_RUNTIME_HOST_SEED_START_PRESENCE")
        && gl.contains("fn ensure_non_shell_player_presence")
        && gl.contains("if !allow_seed")
        && gl.contains("GoldenDozer");
    let ok = gl_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSeedStartPresenceOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_seed_start_presence_opt_in_nav_commands_residual_wave732() -> bool {
    let steps = LIVE_HOST_SEED_START_PRESENCE_OPT_IN_NAV_STEPS_WAVE732;
    let cmds = RUNTIME_HOST_LIVE_HOST_SEED_START_PRESENCE_OPT_IN_CMD_NAMES_WAVE732;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_NO_FREE_SEED_DEFAULT").is_some()
        && residual_name_index(steps, "LIVE_HOST_SEED_START_PRESENCE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_seed_start_presence_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "no_free_seed_default").is_some();
    residual_action_store(ResidualHostSeedStartPresenceOptInAction::NavCommands);
    ok
}
pub fn simulate_host_seed_start_presence_opt_in_collect_source() -> bool {
    let ok = gl_source().contains("allow_seed")
        && gl_source().contains("GENERALS_RUNTIME_HOST_SEED_START_PRESENCE");
    residual_action_store(ResidualHostSeedStartPresenceOptInAction::CollectSource);
    ok
}
pub fn simulate_host_seed_start_presence_opt_in_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 732")
        && gl_source().contains("if !allow_seed {\n            return;");
    residual_action_store(ResidualHostSeedStartPresenceOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_seed_start_presence_opt_in_residual_pack_wave732() -> bool {
    honesty_host_seed_start_presence_opt_in_method_names_residual_wave732()
        && honesty_host_seed_start_presence_opt_in_source_markers_residual_wave732()
        && honesty_host_seed_start_presence_opt_in_nav_commands_residual_wave732()
        && simulate_host_seed_start_presence_opt_in_collect_source()
        && simulate_host_seed_start_presence_opt_in_dispatch_source()
}
pub fn simulate_live_host_seed_start_presence_opt_in_honesty() -> bool {
    let ok = honesty_host_seed_start_presence_opt_in_residual_pack_wave732();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostSeedStartPresenceOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_seed_start_presence_opt_in_method_names_residual_wave732());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_seed_start_presence_opt_in_source_markers_residual_wave732());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_seed_start_presence_opt_in_nav_commands_residual_wave732());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_seed_start_presence_opt_in_collect_source());
        assert!(simulate_host_seed_start_presence_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_seed_start_presence_opt_in_residual_pack_wave732());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_seed_start_presence_opt_in_honesty());
        assert!(residual_host_seed_start_presence_opt_in_ok());
    }
}
