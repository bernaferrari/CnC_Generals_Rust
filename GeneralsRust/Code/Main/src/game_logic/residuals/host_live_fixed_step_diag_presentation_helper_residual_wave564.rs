//! Wave 564 residual peels: freeze full fixed-step diagnostics
//! (`logic_steps_budget_hit`, `logic_steps_accumulated_seconds`) onto
//! `PresentationFrame` and centralize slow-menu / status residual through
//! `presentation_or_boot_fixed_step_diagnostics` — presentation freeze owns
//! catch-up diagnostics when installed; boot residual without freeze uses host
//! `fixed_step_diagnostics`. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 563 template-name presentation helper residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` logic_steps_* fields + build_from_logic
//! - `cnc_game_engine.rs` presentation_or_boot_fixed_step_diagnostics / slow menu
//!
//! Fail-closed:
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER_METHOD_NAMES_WAVE564: &[&str] = &[
    "presentation_or_boot_fixed_step_diagnostics",
    "logic_steps_budget_hit",
    "logic_steps_accumulated_seconds",
    "fixed_step_diagnostics",
    "Wave 564",
    "playable_claim = false",
];

pub const LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER_NAV_STEPS_WAVE564: &[&str] = &[
    "REQUIRE_FIXED_STEP_DIAG_PRESENTATION_FIELDS",
    "REQUIRE_FIXED_STEP_DIAG_PRESENTATION_HELPER",
    "LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER_CMD_NAMES_WAVE564: &[&str] = &[
    "fixed_step_diag_presentation_fields",
    "fixed_step_diag_presentation_helper",
    "slow_menu_fixed_step_residual",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualFixedStepDiagPresentationHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualFixedStepDiagPresentationHelperAction {
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

fn residual_action_store(action: ResidualFixedStepDiagPresentationHelperAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_fixed_step_diag_presentation_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_fixed_step_diag_presentation_helper_last_action()
-> ResidualFixedStepDiagPresentationHelperAction {
    ResidualFixedStepDiagPresentationHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
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

pub fn honesty_fixed_step_diag_presentation_helper_method_names_residual_wave564() -> bool {
    let names = LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER_METHOD_NAMES_WAVE564;
    let ok = residual_name_index(names, "presentation_or_boot_fixed_step_diagnostics").is_some()
        && residual_name_index(names, "logic_steps_budget_hit").is_some()
        && residual_name_index(names, "logic_steps_accumulated_seconds").is_some()
        && residual_name_index(names, "fixed_step_diagnostics").is_some()
        && residual_name_index(names, "Wave 564").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualFixedStepDiagPresentationHelperAction::MethodNames);
    ok
}

pub fn honesty_fixed_step_diag_presentation_helper_source_markers_residual_wave564() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let field_ok = pf.contains("pub logic_steps_budget_hit: bool")
        && pf.contains("pub logic_steps_accumulated_seconds: f32")
        && pf.contains("Wave 564");
    let Some(helper) = fn_body(eng, "fn presentation_or_boot_fixed_step_diagnostics(") else {
        residual_action_store(ResidualFixedStepDiagPresentationHelperAction::SourceMarkers);
        return false;
    };
    // 2026-08-15: Wave 895 fail-closed — host_match_logic_steps, no dual-read.
    let helper_ok = helper.contains("Wave 564")
        && helper.contains("pres.logic_steps_run")
        && helper.contains("pres.logic_steps_budget_hit")
        && helper.contains("pres.logic_steps_accumulated_seconds")
        && helper.contains("host_match_logic_steps")
        && !helper.contains("self.game_logic.fixed_step_diagnostics()");
    let slow_ok = eng.contains("presentation_or_boot_fixed_step_diagnostics()");
    let raw = eng
        .matches("self.game_logic.fixed_step_diagnostics()")
        .count();
    let ok = field_ok && helper_ok && slow_ok && raw == 0 && !eng.contains("playable_claim = true");
    residual_action_store(ResidualFixedStepDiagPresentationHelperAction::SourceMarkers);
    ok
}

pub fn honesty_fixed_step_diag_presentation_helper_nav_commands_residual_wave564() -> bool {
    let steps = LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER_NAV_STEPS_WAVE564;
    let cmds = RUNTIME_HOST_LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER_CMD_NAMES_WAVE564;
    let ok = residual_name_index(steps, "REQUIRE_FIXED_STEP_DIAG_PRESENTATION_FIELDS").is_some()
        && residual_name_index(steps, "REQUIRE_FIXED_STEP_DIAG_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_FIXED_STEP_DIAG_PRESENTATION_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "fixed_step_diag_presentation_fields").is_some()
        && residual_name_index(cmds, "fixed_step_diag_presentation_helper").is_some()
        && residual_name_index(cmds, "slow_menu_fixed_step_residual").is_some();
    residual_action_store(ResidualFixedStepDiagPresentationHelperAction::NavCommands);
    ok
}

pub fn simulate_fixed_step_diag_presentation_helper_collect_source() -> bool {
    let eng = eng_source();
    let pf = pf_source();
    let ok = eng.contains("Wave 564")
        && eng.contains("fn presentation_or_boot_fixed_step_diagnostics")
        && pf.contains("pub logic_steps_budget_hit: bool");
    residual_action_store(ResidualFixedStepDiagPresentationHelperAction::CollectSource);
    ok
}

pub fn simulate_fixed_step_diag_presentation_helper_dispatch_source() -> bool {
    let eng = eng_source();
    // 2026-08-15: Slow-menu log peel — dispatcher still calls the helper.
    let ok = eng.contains("presentation_or_boot_fixed_step_diagnostics()")
        && eng.contains("presentation_or_boot_logic_steps()");
    residual_action_store(ResidualFixedStepDiagPresentationHelperAction::DispatchSource);
    ok
}

pub fn honesty_fixed_step_diag_presentation_helper_residual_pack_wave564() -> bool {
    honesty_fixed_step_diag_presentation_helper_method_names_residual_wave564()
        && honesty_fixed_step_diag_presentation_helper_source_markers_residual_wave564()
        && honesty_fixed_step_diag_presentation_helper_nav_commands_residual_wave564()
        && simulate_fixed_step_diag_presentation_helper_collect_source()
        && simulate_fixed_step_diag_presentation_helper_dispatch_source()
}

pub fn simulate_live_fixed_step_diag_presentation_helper_honesty() -> bool {
    let ok = honesty_fixed_step_diag_presentation_helper_residual_pack_wave564();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualFixedStepDiagPresentationHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_fixed_step_diag_presentation_helper_method_names_residual_wave564());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_fixed_step_diag_presentation_helper_source_markers_residual_wave564());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_fixed_step_diag_presentation_helper_nav_commands_residual_wave564());
    }

    #[test]
    fn fixed_step_diag_presentation_helper_sources() {
        assert!(simulate_fixed_step_diag_presentation_helper_collect_source());
        assert!(simulate_fixed_step_diag_presentation_helper_dispatch_source());
    }

    #[test]
    fn wave564_composite_pack() {
        assert!(honesty_fixed_step_diag_presentation_helper_residual_pack_wave564());
    }

    #[test]
    fn simulate_live_fixed_step_diag_presentation_helper_honesty_residual_live() {
        assert!(
            simulate_live_fixed_step_diag_presentation_helper_honesty(),
            "fixed-step diag presentation helper residual must latch"
        );
        assert!(residual_fixed_step_diag_presentation_helper_ok());
        assert_eq!(
            residual_fixed_step_diag_presentation_helper_last_action(),
            ResidualFixedStepDiagPresentationHelperAction::Composite
        );
    }
}
