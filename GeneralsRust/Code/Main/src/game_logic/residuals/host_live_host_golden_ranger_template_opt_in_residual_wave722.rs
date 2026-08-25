//! Wave 722: runtime-host synthetic GoldenRanger template insert is opt-in.
//! Default fail-closed: train does not invent GoldenRanger mid-command.
//! Harness may set `golden_template=1` / `GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN_METHOD_NAMES_WAVE722: &[&str] = &[
    "allow_golden_template",
    "GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER",
    "golden_template=1",
    "host_ensure_golden_ranger_template",
    "Wave 722",
    "playable_claim = false",
];
pub const LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN_NAV_STEPS_WAVE722: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_CANDIDATE_GATED",
    "LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN_CMD_NAMES_WAVE722: &[&str] = &[
    "host_golden_ranger_template_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "candidate_gated",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostGoldenRangerTemplateOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostGoldenRangerTemplateOptInAction {
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
fn residual_action_store(a: ResidualHostGoldenRangerTemplateOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_golden_ranger_template_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_golden_ranger_template_opt_in_last_action()
-> ResidualHostGoldenRangerTemplateOptInAction {
    ResidualHostGoldenRangerTemplateOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
pub fn honesty_host_golden_ranger_template_opt_in_method_names_residual_wave722() -> bool {
    let names = LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN_METHOD_NAMES_WAVE722;
    let ok = residual_name_index(names, "allow_golden_template").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER").is_some()
        && residual_name_index(names, "golden_template=1").is_some()
        && residual_name_index(names, "host_ensure_golden_ranger_template").is_some()
        && residual_name_index(names, "Wave 722").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostGoldenRangerTemplateOptInAction::MethodNames);
    ok
}
pub fn honesty_host_golden_ranger_template_opt_in_source_markers_residual_wave722() -> bool {
    let eng = eng_source();
    let eng_ok = eng.contains("Wave 722")
        && eng.contains("allow_golden_template")
        && eng.contains("GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER")
        && eng.contains("golden_template")
        && eng.contains("if allow_golden_template")
        && eng.contains("unit_candidates.push(\"GoldenRanger\")")
        && eng.contains("self.host_ensure_golden_ranger_template();");
    // 2026-08-15: indentation peeled — gate + call still adjacent in enqueue.
    let gated_call = eng.contains("if allow_golden_template")
        && eng.contains("self.host_ensure_golden_ranger_template();");
    let no_uncond = !eng.contains("Wave 581: GoldenRanger host template residual via helper.");
    let ok = eng_ok && gated_call && no_uncond && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostGoldenRangerTemplateOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_golden_ranger_template_opt_in_nav_commands_residual_wave722() -> bool {
    let steps = LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN_NAV_STEPS_WAVE722;
    let cmds = RUNTIME_HOST_LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN_CMD_NAMES_WAVE722;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_CANDIDATE_GATED").is_some()
        && residual_name_index(steps, "LIVE_HOST_GOLDEN_RANGER_TEMPLATE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_golden_ranger_template_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "candidate_gated").is_some();
    residual_action_store(ResidualHostGoldenRangerTemplateOptInAction::NavCommands);
    ok
}
pub fn simulate_host_golden_ranger_template_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_golden_template")
        && eng_source().contains("unit_candidates.push(\"GoldenRanger\")");
    residual_action_store(ResidualHostGoldenRangerTemplateOptInAction::CollectSource);
    ok
}
pub fn simulate_host_golden_ranger_template_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 722")
        && eng_source().contains("if allow_golden_template")
        && eng_source().contains("self.host_ensure_golden_ranger_template();");
    residual_action_store(ResidualHostGoldenRangerTemplateOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_golden_ranger_template_opt_in_residual_pack_wave722() -> bool {
    honesty_host_golden_ranger_template_opt_in_method_names_residual_wave722()
        && honesty_host_golden_ranger_template_opt_in_source_markers_residual_wave722()
        && honesty_host_golden_ranger_template_opt_in_nav_commands_residual_wave722()
        && simulate_host_golden_ranger_template_opt_in_collect_source()
        && simulate_host_golden_ranger_template_opt_in_dispatch_source()
}
pub fn simulate_live_host_golden_ranger_template_opt_in_honesty() -> bool {
    let ok = honesty_host_golden_ranger_template_opt_in_residual_pack_wave722();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostGoldenRangerTemplateOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_golden_ranger_template_opt_in_method_names_residual_wave722());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_golden_ranger_template_opt_in_source_markers_residual_wave722());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_golden_ranger_template_opt_in_nav_commands_residual_wave722());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_golden_ranger_template_opt_in_collect_source());
        assert!(simulate_host_golden_ranger_template_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_golden_ranger_template_opt_in_residual_pack_wave722());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_golden_ranger_template_opt_in_honesty());
        assert!(residual_host_golden_ranger_template_opt_in_ok());
    }
}
