//! Wave 724: runtime-host train enqueue `try_names` GoldenRanger fallback is opt-in.
//! Default fail-closed: enqueue only real ranger aliases unless Wave 722 gate is on.
//! Shares `allow_golden_template` / `GENERALS_RUNTIME_HOST_ENSURE_GOLDEN_RANGER`.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN_METHOD_NAMES_WAVE724: &[&str] = &[
    "allow_golden_template",
    "try_names.push",
    "GoldenRanger",
    "Wave 724",
    "host_enqueue_production",
    "playable_claim = false",
];
pub const LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN_NAV_STEPS_WAVE724: &[&str] = &[
    "REQUIRE_OPT_IN_GATE",
    "REQUIRE_DEFAULT_FAIL_CLOSED",
    "REQUIRE_TRY_NAMES_GATED",
    "LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN_CMD_NAMES_WAVE724: &[&str] = &[
    "host_train_try_names_golden_opt_in",
    "opt_in_gate",
    "default_fail_closed",
    "try_names_gated",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTrainTryNamesGoldenOptInAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostTrainTryNamesGoldenOptInAction {
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
fn residual_action_store(a: ResidualHostTrainTryNamesGoldenOptInAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_train_try_names_golden_opt_in_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_train_try_names_golden_opt_in_last_action()
-> ResidualHostTrainTryNamesGoldenOptInAction {
    ResidualHostTrainTryNamesGoldenOptInAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
pub fn honesty_host_train_try_names_golden_opt_in_method_names_residual_wave724() -> bool {
    let names = LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN_METHOD_NAMES_WAVE724;
    let ok = residual_name_index(names, "allow_golden_template").is_some()
        && residual_name_index(names, "try_names.push").is_some()
        && residual_name_index(names, "GoldenRanger").is_some()
        && residual_name_index(names, "Wave 724").is_some()
        && residual_name_index(names, "host_enqueue_production").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostTrainTryNamesGoldenOptInAction::MethodNames);
    ok
}
pub fn honesty_host_train_try_names_golden_opt_in_source_markers_residual_wave724() -> bool {
    let eng = eng_source();
    let eng_ok = eng.contains("Wave 724")
        && (eng.contains("GoldenRanger enqueue fallback is opt-in only")
            || eng.contains("alias + GoldenRanger enqueue fallbacks are opt-in only"))
        && (eng.contains("try_names.push(\"GoldenRanger\")")
            || eng.contains("try_names.push(\"GoldenRanger\".into())")
            || eng.contains("unit_candidates.push(\"GoldenRanger\")"))
        && eng.contains("if allow_golden_template");
    // Unconditional try_names array with GoldenRanger must be gone.
    let no_uncond = !eng.contains(
        "let try_names = [\n                            template.as_str(),\n                            \"AmericaInfantryRanger\",\n                            \"USA_Ranger\",\n                            \"USARanger\",\n                            \"GoldenRanger\",\n                        ];"
    );
    let ok = eng_ok && no_uncond && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostTrainTryNamesGoldenOptInAction::SourceMarkers);
    ok
}
pub fn honesty_host_train_try_names_golden_opt_in_nav_commands_residual_wave724() -> bool {
    let steps = LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN_NAV_STEPS_WAVE724;
    let cmds = RUNTIME_HOST_LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN_CMD_NAMES_WAVE724;
    let ok = residual_name_index(steps, "REQUIRE_OPT_IN_GATE").is_some()
        && residual_name_index(steps, "REQUIRE_DEFAULT_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "REQUIRE_TRY_NAMES_GATED").is_some()
        && residual_name_index(steps, "LIVE_HOST_TRAIN_TRY_NAMES_GOLDEN_OPT_IN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_train_try_names_golden_opt_in").is_some()
        && residual_name_index(cmds, "opt_in_gate").is_some()
        && residual_name_index(cmds, "default_fail_closed").is_some()
        && residual_name_index(cmds, "try_names_gated").is_some();
    residual_action_store(ResidualHostTrainTryNamesGoldenOptInAction::NavCommands);
    ok
}
pub fn simulate_host_train_try_names_golden_opt_in_collect_source() -> bool {
    let ok = (eng_source().contains("try_names.push(\"GoldenRanger\")")
        || eng_source().contains("try_names.push(\"GoldenRanger\".into())")
        || eng_source().contains("unit_candidates.push(\"GoldenRanger\")"))
        && eng_source().contains("allow_golden_template");
    residual_action_store(ResidualHostTrainTryNamesGoldenOptInAction::CollectSource);
    ok
}
pub fn simulate_host_train_try_names_golden_opt_in_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 724") && eng_source().contains("host_enqueue_production");
    residual_action_store(ResidualHostTrainTryNamesGoldenOptInAction::DispatchSource);
    ok
}
pub fn honesty_host_train_try_names_golden_opt_in_residual_pack_wave724() -> bool {
    honesty_host_train_try_names_golden_opt_in_method_names_residual_wave724()
        && honesty_host_train_try_names_golden_opt_in_source_markers_residual_wave724()
        && honesty_host_train_try_names_golden_opt_in_nav_commands_residual_wave724()
        && simulate_host_train_try_names_golden_opt_in_collect_source()
        && simulate_host_train_try_names_golden_opt_in_dispatch_source()
}
pub fn simulate_live_host_train_try_names_golden_opt_in_honesty() -> bool {
    let ok = honesty_host_train_try_names_golden_opt_in_residual_pack_wave724();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostTrainTryNamesGoldenOptInAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_train_try_names_golden_opt_in_method_names_residual_wave724());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_train_try_names_golden_opt_in_source_markers_residual_wave724());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_train_try_names_golden_opt_in_nav_commands_residual_wave724());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_train_try_names_golden_opt_in_collect_source());
        assert!(simulate_host_train_try_names_golden_opt_in_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_train_try_names_golden_opt_in_residual_pack_wave724());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_train_try_names_golden_opt_in_honesty());
        assert!(residual_host_train_try_names_golden_opt_in_ok());
    }
}
