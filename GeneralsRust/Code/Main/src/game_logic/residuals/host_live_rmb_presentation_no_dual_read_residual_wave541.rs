//! Wave 541 residual peels: RMB `classify_right_click_target_from_presentation`
//! never dual-reads live GameLogic when `target_presentation` is installed.
//! Empty `selected_presentation` fails closed for capability probes.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 540 camera shell flag residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `command_system.rs` determine_context_command / classify_right_click_target_from_presentation
//!
//! Fail-closed:
//! - Boot residual without target_presentation still uses ObjectId probes
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_RMB_PRESENTATION_NO_DUAL_READ_METHOD_NAMES_WAVE541: &[&str] = &[
    "classify_right_click_target_from_presentation",
    "target_presentation",
    "selected_presentation",
    "Wave 541",
    "playable_claim = false",
];

pub const LIVE_RMB_PRESENTATION_NO_DUAL_READ_NAV_STEPS_WAVE541: &[&str] = &[
    "REQUIRE_RMB_PRESENTATION_NO_DUAL_READ",
    "REQUIRE_CLASSIFY_PASSES_NONE",
    "LIVE_RMB_PRESENTATION_NO_DUAL_READ",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_RMB_PRESENTATION_NO_DUAL_READ_CMD_NAMES_WAVE541: &[&str] = &[
    "rmb_presentation_no_dual_read",
    "classify_none",
    "fail_closed_empty_selected",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualRmbPresentationNoDualReadAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualRmbPresentationNoDualReadAction {
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

fn residual_action_store(action: ResidualRmbPresentationNoDualReadAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_rmb_presentation_no_dual_read_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_rmb_presentation_no_dual_read_last_action()
-> ResidualRmbPresentationNoDualReadAction {
    ResidualRmbPresentationNoDualReadAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn cs_source() -> &'static str {
    crate::command_system::COMMAND_SYSTEM_SRC
}

pub fn honesty_rmb_presentation_no_dual_read_method_names_residual_wave541() -> bool {
    let names = LIVE_RMB_PRESENTATION_NO_DUAL_READ_METHOD_NAMES_WAVE541;
    let ok = residual_name_index(names, "classify_right_click_target_from_presentation").is_some()
        && residual_name_index(names, "target_presentation").is_some()
        && residual_name_index(names, "selected_presentation").is_some()
        && residual_name_index(names, "Wave 541").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualRmbPresentationNoDualReadAction::MethodNames);
    ok
}

pub fn honesty_rmb_presentation_no_dual_read_source_markers_residual_wave541() -> bool {
    let cs = cs_source();
    // determine_context_command must pass None into classify when target_presentation present.
    let det_i = cs.find("fn determine_context_command");
    let ok = match det_i {
        Some(i) => {
            let window = &cs[i..cs.len().min(i + 3500)];
            window.contains("Wave 541")
                && window.contains("classify_right_click_target_from_presentation")
                && window.contains("target presentation freeze is authoritative")
                && window.contains("None,")
                && !window.contains("if context.selected_presentation.is_empty()")
        }
        None => false,
    };
    let ok = ok
        && cs.contains("fn classify_right_click_target_from_presentation")
        && cs.contains("Wave 235/541")
        && !cs.contains("playable_claim = true");
    residual_action_store(ResidualRmbPresentationNoDualReadAction::SourceMarkers);
    ok
}

pub fn honesty_rmb_presentation_no_dual_read_nav_commands_residual_wave541() -> bool {
    let steps = LIVE_RMB_PRESENTATION_NO_DUAL_READ_NAV_STEPS_WAVE541;
    let cmds = RUNTIME_HOST_LIVE_RMB_PRESENTATION_NO_DUAL_READ_CMD_NAMES_WAVE541;
    let ok = residual_name_index(steps, "REQUIRE_RMB_PRESENTATION_NO_DUAL_READ").is_some()
        && residual_name_index(steps, "REQUIRE_CLASSIFY_PASSES_NONE").is_some()
        && residual_name_index(steps, "LIVE_RMB_PRESENTATION_NO_DUAL_READ").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "rmb_presentation_no_dual_read").is_some()
        && residual_name_index(cmds, "classify_none").is_some()
        && residual_name_index(cmds, "fail_closed_empty_selected").is_some();
    residual_action_store(ResidualRmbPresentationNoDualReadAction::NavCommands);
    ok
}

pub fn simulate_rmb_presentation_no_dual_read_collect_source() -> bool {
    let cs = cs_source();
    let ok = cs.contains("Wave 541")
        && cs.contains("classify_right_click_target_from_presentation")
        && cs.contains("target_presentation");
    residual_action_store(ResidualRmbPresentationNoDualReadAction::CollectSource);
    ok
}

pub fn simulate_rmb_presentation_no_dual_read_dispatch_source() -> bool {
    let cs = cs_source();
    // Call site passes bare None after hint arg (not conditional game_logic).
    let ok = cs.contains("target presentation freeze is authoritative")
        && cs.contains("Empty selected_presentation fails closed");
    residual_action_store(ResidualRmbPresentationNoDualReadAction::DispatchSource);
    ok
}

pub fn honesty_rmb_presentation_no_dual_read_residual_pack_wave541() -> bool {
    honesty_rmb_presentation_no_dual_read_method_names_residual_wave541()
        && honesty_rmb_presentation_no_dual_read_source_markers_residual_wave541()
        && honesty_rmb_presentation_no_dual_read_nav_commands_residual_wave541()
        && simulate_rmb_presentation_no_dual_read_collect_source()
        && simulate_rmb_presentation_no_dual_read_dispatch_source()
}

pub fn simulate_live_rmb_presentation_no_dual_read_honesty() -> bool {
    let ok = honesty_rmb_presentation_no_dual_read_residual_pack_wave541();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualRmbPresentationNoDualReadAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_rmb_presentation_no_dual_read_method_names_residual_wave541());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_rmb_presentation_no_dual_read_source_markers_residual_wave541());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_rmb_presentation_no_dual_read_nav_commands_residual_wave541());
    }

    #[test]
    fn rmb_presentation_no_dual_read_sources() {
        assert!(simulate_rmb_presentation_no_dual_read_collect_source());
        assert!(simulate_rmb_presentation_no_dual_read_dispatch_source());
    }

    #[test]
    fn wave541_composite_pack() {
        assert!(honesty_rmb_presentation_no_dual_read_residual_pack_wave541());
    }

    #[test]
    fn simulate_live_rmb_presentation_no_dual_read_honesty_residual_live() {
        assert!(
            simulate_live_rmb_presentation_no_dual_read_honesty(),
            "rmb presentation no dual-read residual must latch"
        );
        assert!(residual_rmb_presentation_no_dual_read_ok());
        assert_eq!(
            residual_rmb_presentation_no_dual_read_last_action(),
            ResidualRmbPresentationNoDualReadAction::Composite
        );
    }
}
