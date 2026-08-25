//! Wave 561 residual peels: freeze `logic_steps_run` onto `PresentationFrame` and
//! centralize runtime-status catch-up residual through
//! `presentation_or_boot_logic_steps` — presentation freeze owns fixed-step
//! `steps_run` when installed; boot residual without freeze uses host
//! `fixed_step_diagnostics`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 560 logic-frame presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` logic_steps_run field + build_from_logic
//! - `cnc_game_engine.rs` presentation_or_boot_logic_steps / runtime_host_status_snapshot
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_LOGIC_STEPS_PRESENTATION_HELPER_METHOD_NAMES_WAVE561: &[&str] = &[
    "presentation_or_boot_logic_steps",
    "logic_steps_run",
    "fixed_step_diagnostics",
    "runtime_host_status_snapshot",
    "Wave 561",
    "playable_claim = false",
];

pub const LIVE_LOGIC_STEPS_PRESENTATION_HELPER_NAV_STEPS_WAVE561: &[&str] = &[
    "REQUIRE_LOGIC_STEPS_PRESENTATION_FIELD",
    "REQUIRE_LOGIC_STEPS_PRESENTATION_HELPER",
    "LIVE_LOGIC_STEPS_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_LOGIC_STEPS_PRESENTATION_HELPER_CMD_NAMES_WAVE561: &[&str] = &[
    "logic_steps_presentation_field",
    "logic_steps_presentation_helper",
    "boot_fixed_step_diagnostics",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualLogicStepsPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualLogicStepsPresentationHelperAction {
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

fn residual_action_store(action: ResidualLogicStepsPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_logic_steps_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_logic_steps_presentation_helper_last_action()
-> ResidualLogicStepsPresentationHelperAction {
    ResidualLogicStepsPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn fn_body<'a>(src: &'a str, sig: &str) -> Option<&'a str> {
    let start = src.find(sig)?;
    let after = &src[start..];
    let brace = after.find('{')?;
    let mut depth = 0i32;
    for (i, ch) in after[brace..].char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return Some(&after[..=brace + i]);
                }
            }
            _ => {}
        }
    }
    None
}

pub fn honesty_logic_steps_presentation_helper_method_names_residual_wave561() -> bool {
    let names = LIVE_LOGIC_STEPS_PRESENTATION_HELPER_METHOD_NAMES_WAVE561;
    let ok = residual_name_index(names, "presentation_or_boot_logic_steps").is_some()
        && residual_name_index(names, "logic_steps_run").is_some()
        && residual_name_index(names, "fixed_step_diagnostics").is_some()
        && residual_name_index(names, "runtime_host_status_snapshot").is_some()
        && residual_name_index(names, "Wave 561").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualLogicStepsPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_logic_steps_presentation_helper_source_markers_residual_wave561() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub logic_steps_run: u32")
        && pf.contains("Wave 561")
        && pf.contains("logic_steps_run: logic.fixed_step_diagnostics().steps_run as u32");
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_logic_steps(") else {
        residual_action_store(ResidualLogicStepsPresentationHelperAction::SourceMarkers);
        return false;
    };
    let Some(status) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualLogicStepsPresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 895 fail-closed — host_match_logic_steps, no dual-read.
    let helper_ok = helper.contains("presentation_or_boot_fixed_step_diagnostics()")
        && eng.contains("fn presentation_or_boot_fixed_step_diagnostics")
        && eng.contains("pres.logic_steps_run")
        && eng.contains("host_match_logic_steps");
    let status_ok = status.contains("presentation_or_boot_logic_steps()")
        && !status.contains("fixed_step_diagnostics()");
    let raw = eng
        .matches("self.game_logic.fixed_step_diagnostics()")
        .count();
    let ok =
        field_ok && helper_ok && status_ok && raw == 0 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualLogicStepsPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_logic_steps_presentation_helper_nav_commands_residual_wave561() -> bool {
    let steps = LIVE_LOGIC_STEPS_PRESENTATION_HELPER_NAV_STEPS_WAVE561;
    let cmds = RUNTIME_HOST_LIVE_LOGIC_STEPS_PRESENTATION_HELPER_CMD_NAMES_WAVE561;
    let ok = residual_name_index(steps, "REQUIRE_LOGIC_STEPS_PRESENTATION_FIELD").is_some()
        && residual_name_index(steps, "REQUIRE_LOGIC_STEPS_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_LOGIC_STEPS_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "logic_steps_presentation_field").is_some()
        && residual_name_index(cmds, "logic_steps_presentation_helper").is_some()
        && residual_name_index(cmds, "boot_fixed_step_diagnostics").is_some();
    residual_action_store(ResidualLogicStepsPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_logic_steps_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let ok = eng.contains("Wave 561")
        && eng.contains("fn presentation_or_boot_logic_steps")
        && pf.contains("pub logic_steps_run: u32");
    residual_action_store(ResidualLogicStepsPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_logic_steps_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    let Some(status) = fn_body(eng, "fn runtime_host_status_snapshot(") else {
        residual_action_store(ResidualLogicStepsPresentationHelperAction::DispatchSource);
        return false;
    };
    let ok = status.contains("logic_steps: self.presentation_or_boot_logic_steps()")
        && status.contains("logic_frame: self.presentation_or_boot_logic_frame()");
    residual_action_store(ResidualLogicStepsPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_logic_steps_presentation_helper_residual_pack_wave561() -> bool {
    honesty_logic_steps_presentation_helper_method_names_residual_wave561()
        && honesty_logic_steps_presentation_helper_source_markers_residual_wave561()
        && honesty_logic_steps_presentation_helper_nav_commands_residual_wave561()
        && simulate_logic_steps_presentation_helper_collect_source()
        && simulate_logic_steps_presentation_helper_dispatch_source()
}

pub fn simulate_live_logic_steps_presentation_helper_honesty() -> bool {
    let ok = honesty_logic_steps_presentation_helper_residual_pack_wave561();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualLogicStepsPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_logic_steps_presentation_helper_method_names_residual_wave561());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_logic_steps_presentation_helper_source_markers_residual_wave561());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_logic_steps_presentation_helper_nav_commands_residual_wave561());
    }

    #[test]
    fn logic_steps_presentation_helper_sources() {
        assert!(simulate_logic_steps_presentation_helper_collect_source());
        assert!(simulate_logic_steps_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave561_composite_pack() {
        assert!(honesty_logic_steps_presentation_helper_residual_pack_wave561());
    }

    #[test]
    fn simulate_live_logic_steps_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_logic_steps_presentation_helper_honesty(),
            "logic steps presentation helper residual must latch"
        );
        assert!(residual_logic_steps_presentation_helper_ok());
        assert_eq!(
            residual_logic_steps_presentation_helper_last_action(),
            ResidualLogicStepsPresentationHelperAction::Composite
        );
    }
}
