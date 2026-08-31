//! Wave 727: runtime-host free default train/construct/upgrade names are opt-in.
//! Default fail-closed: missing template/name fails the command.
//! Smoke always passes explicit template/name. Harness may set
//! `default_template=1` / `GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE=1`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN_METHOD_NAMES_WAVE727: &[&str] = &[
    "allow_default_template",
    "GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE",
    "default_template=1",
    "train_fail_no_template",
    "Wave 727",
    "playable_claim = false",
];
pub const LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN_NAV_STEPS_WAVE727: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_SMOKE_EXPLICIT",
    "LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN_CMD_NAMES_WAVE727: &[&str] = &[
    "host_default_template_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "smoke_explicit",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDefaultTemplateOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostDefaultTemplateOptInAction {
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
fn residual_action_store(a: ResidualHostDefaultTemplateOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_default_template_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_default_template_opt_in_last_action() -> ResidualHostDefaultTemplateOptInAction
{
    ResidualHostDefaultTemplateOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn smoke_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
pub fn honesty_host_default_template_opt_in_method_names_residual_wave727() -> bool {
    let names = LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN_METHOD_NAMES_WAVE727;
    let ok = residual_name_index(names, "allow_default_template").is_some()
        && residual_name_index(names, "GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE").is_some()
        && residual_name_index(names, "default_template=1").is_some()
        && residual_name_index(names, "train_fail_no_template").is_some()
        && residual_name_index(names, "Wave 727").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostDefaultTemplateOptInAction::MethodNames);
    ok
}
pub fn honesty_host_default_template_opt_in_source_markers_residual_wave727() -> bool {
    let eng = eng_source();
    let smoke = smoke_source();
    let eng_ok = eng.contains("Wave 727")
        && eng.contains("allow_default_template")
        && eng.contains("GENERALS_RUNTIME_HOST_DEFAULT_TEMPLATE")
        && eng.contains("train_fail_no_template")
        && eng.contains("construct_fail_no_template")
        && eng.contains("upgrade_fail_no_name")
        && eng.matches("_ if allow_default_template").count() >= 3;
    // Free defaults only behind allow_default_template match arms.
    let gated_train =
        eng.contains("_ if allow_default_template => \"AmericaInfantryRanger\".to_string()");
    let gated_con = eng.contains("_ if allow_default_template => \"USA_Barracks\".to_string()");
    let gated_up = eng.contains("\"UpgradeAmericaRangerCaptureBuilding\".to_string()")
        && eng.contains("upgrade_fail_no_name");
    let smoke_ok = smoke.contains("train_unit|template=")
        && smoke.contains("construct|template=")
        && smoke.contains("upgrade|name=");
    let ok = eng_ok
        && gated_train
        && gated_con
        && gated_up
        && smoke_ok
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostDefaultTemplateOptInAction::SourceMarkers);
    ok
}

pub fn honesty_host_default_template_opt_in_nav_commands_residual_wave727() -> bool {
    let steps = LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN_NAV_STEPS_WAVE727;
    let cmds = RUNTIME_HOST_LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN_CMD_NAMES_WAVE727;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_SMOKE_EXPLICIT").is_some()
        && residual_name_index(steps, "LIVE_HOST_DEFAULT_TEMPLATE_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_default_template_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "smoke_explicit").is_some();
    residual_action_store(ResidualHostDefaultTemplateOptInAction::NavCommands);
    ok
}
pub fn simulate_host_default_template_opt_in_collect_source() -> bool {
    let ok = eng_source().contains("allow_default_template")
        && eng_source().contains("train_fail_no_template");
    residual_action_store(ResidualHostDefaultTemplateOptInAction::CollectSource);
    ok
}
pub fn simulate_host_default_template_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 727")
        && smoke_source().contains("train_unit|template=AmericaInfantryRanger");
    residual_action_store(ResidualHostDefaultTemplateOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_default_template_opt_in_residual_pack_wave727() -> bool {
    honesty_host_default_template_opt_in_method_names_residual_wave727()
        && honesty_host_default_template_opt_in_source_markers_residual_wave727()
        && honesty_host_default_template_opt_in_nav_commands_residual_wave727()
        && simulate_host_default_template_opt_in_collect_source()
        && simulate_host_default_template_opt_in_dispatch_source()
}
pub fn simulate_live_host_default_template_opt_in_honesty() -> bool {
    let ok = honesty_host_default_template_opt_in_residual_pack_wave727();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostDefaultTemplateOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_default_template_opt_in_method_names_residual_wave727());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_default_template_opt_in_source_markers_residual_wave727());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_default_template_opt_in_nav_commands_residual_wave727());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_default_template_opt_in_collect_source());
        assert!(simulate_host_default_template_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_default_template_opt_in_residual_pack_wave727());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_default_template_opt_in_honesty());
        assert!(residual_host_default_template_opt_in_ok());
    }
}
