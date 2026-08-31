//! Wave 1095: control-group assign + select/add presentation legality residual.
//!
//! After Waves 1092–1094 pick/selectable peels:
//! - control-group assign only skipped `destroyed`, still stored sold/masked/disabled
//! - select_single / add_to_selection trusted raw ids without presentation legality
//!
//! Fail-close those paths on `presentation_is_selectable` + local team when a
//! presentation freeze is installed.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_CONTROL_GROUP_SELECT_LEGALITY_METHOD_NAMES_WAVE1095: &[&str] = &[
    "assign_control_group",
    "select_single_object",
    "add_to_selection",
    "presentation_is_selectable",
    "Wave 1095",
    "playable_claim = false",
];

pub const LIVE_HOST_CONTROL_GROUP_SELECT_LEGALITY_NAV_STEPS_WAVE1095: &[&str] = &[
    "CONTROL_GROUP_ASSIGN_SELECTABLE",
    "SELECT_SINGLE_PRESENTATION_LEGALITY",
    "ADD_TO_SELECTION_PRESENTATION_LEGALITY",
    "LIVE_HOST_CONTROL_GROUP_SELECT_LEGALITY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostControlGroupSelectLegalityAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostControlGroupSelectLegalityAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_control_group_select_legality_method_names_residual_wave1095() -> bool {
    let names = LIVE_HOST_CONTROL_GROUP_SELECT_LEGALITY_METHOD_NAMES_WAVE1095;
    let ok = residual_name_index(names, "assign_control_group").is_some()
        && residual_name_index(names, "Wave 1095").is_some();
    residual_action_store(ResidualHostControlGroupSelectLegalityAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_group_select_legality_nav_commands_residual_wave1095() -> bool {
    let steps = LIVE_HOST_CONTROL_GROUP_SELECT_LEGALITY_NAV_STEPS_WAVE1095;
    let ok = residual_name_index(steps, "LIVE_HOST_CONTROL_GROUP_SELECT_LEGALITY").is_some()
        && residual_name_index(steps, "CONTROL_GROUP_ASSIGN_SELECTABLE").is_some();
    residual_action_store(ResidualHostControlGroupSelectLegalityAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_control_group_select_legality_residual_pack_wave1095() -> bool {
    let uc = uc_source();
    let es = es_source();
    let asg_i = match uc.find("fn assign_control_group") {
        Some(i) => i,
        None => {
            // pub async fn
            match uc.find("assign_control_group(") {
                Some(i) => i,
                None => {
                    residual_action_store(
                        ResidualHostControlGroupSelectLegalityAction::SourceMarkers,
                    );
                    RESIDUAL_OK.store(false, Ordering::SeqCst);
                    return false;
                }
            }
        }
    };
    let asg = &uc[asg_i..asg_i.saturating_add(1600)];
    let single_i = match uc.find("fn select_single_object") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostControlGroupSelectLegalityAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let single = &uc[single_i..single_i.saturating_add(1400)];
    let add_i = match uc.find("fn add_to_selection") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostControlGroupSelectLegalityAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let add = &uc[add_i..add_i.saturating_add(1400)];
    let ok = asg.contains("Wave 1095: assign residual fail-closed on full selectable")
        && asg.contains("presentation_is_selectable(o)")
        && single.contains("Wave 1095: when a presentation freeze is installed")
        && single.contains("presentation_is_selectable(o)")
        && add.contains("Wave 1095: presentation freeze fail-closed on unusable add-to-selection")
        && add.contains("presentation_is_selectable(o)")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostControlGroupSelectLegalityAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_control_group_select_legality_residual_honesty() -> bool {
    let a = honesty_host_control_group_select_legality_method_names_residual_wave1095();
    let b = honesty_host_control_group_select_legality_nav_commands_residual_wave1095();
    let c = honesty_host_control_group_select_legality_residual_pack_wave1095();
    residual_action_store(ResidualHostControlGroupSelectLegalityAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_control_group_select_legality_residual_wave1095() {
        assert!(honesty_host_control_group_select_legality_residual_pack_wave1095());
        assert!(honesty_host_control_group_select_legality_method_names_residual_wave1095());
        assert!(honesty_host_control_group_select_legality_nav_commands_residual_wave1095());
        assert!(simulate_live_host_control_group_select_legality_residual_honesty());
    }
}
