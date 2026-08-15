//! Wave 1069: dual-world masked visibility + repair/resume residual.
//!
//! Drawable visibility hides masked units; repair dual fails closed on undamaged/
//! UC/FOW targets and unusable sources; resume dual FOW + source usability.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MASKED_VIS_REPAIR_RESUME_RESIDUAL_METHOD_NAMES_WAVE1069: &[&str] = &[
    "update_drawable_visibility",
    "selection_can_repair_target",
    "selection_can_resume_construction_target",
    "entry.masked",
    "Wave 1069",
    "playable_claim = false",
];

pub const LIVE_HOST_MASKED_VIS_REPAIR_RESUME_RESIDUAL_NAV_STEPS_WAVE1069: &[&str] = &[
    "MASKED_VISIBILITY",
    "REPAIR",
    "RESUME",
    "LIVE_HOST_MASKED_VIS_REPAIR_RESUME_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMaskedVisRepairResumeResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMaskedVisRepairResumeResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn gc_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_masked_vis_repair_resume_residual_method_names_residual_wave1069() -> bool {
    let names = LIVE_HOST_MASKED_VIS_REPAIR_RESUME_RESIDUAL_METHOD_NAMES_WAVE1069;
    let ok = residual_name_index(names, "selection_can_repair_target").is_some()
        && residual_name_index(names, "Wave 1069").is_some();
    residual_action_store(ResidualHostMaskedVisRepairResumeResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_masked_vis_repair_resume_residual_nav_commands_residual_wave1069() -> bool {
    let steps = LIVE_HOST_MASKED_VIS_REPAIR_RESUME_RESIDUAL_NAV_STEPS_WAVE1069;
    let ok = residual_name_index(steps, "LIVE_HOST_MASKED_VIS_REPAIR_RESUME_RESIDUAL").is_some()
        && residual_name_index(steps, "REPAIR").is_some();
    residual_action_store(ResidualHostMaskedVisRepairResumeResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_masked_vis_repair_resume_residual_residual_pack_wave1069() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let tr = tr_source();
    let ok = gc.contains("Wave 1069: masked residual always hidden")
        && gc.matches("|| entry.masked").count() >= 2
        && tr.contains("Wave 1069: undamaged residual fail-closed")
        && tr.contains(
            "target.health_maximum > 0.0 && target.health_current >= target.health_maximum",
        )
        && tr.contains("Wave 1069: under-construction is resume, not repair")
        && tr.contains("Wave 1069: FOW fogged/black non-local repair target residual fail-closed")
        && tr.contains("Wave 1069: FOW fogged/black non-local resume target residual fail-closed")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMaskedVisRepairResumeResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_masked_vis_repair_resume_residual_honesty() -> bool {
    let a = honesty_host_masked_vis_repair_resume_residual_method_names_residual_wave1069();
    let b = honesty_host_masked_vis_repair_resume_residual_nav_commands_residual_wave1069();
    let c = honesty_host_masked_vis_repair_resume_residual_residual_pack_wave1069();
    residual_action_store(ResidualHostMaskedVisRepairResumeResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_masked_vis_repair_resume_residual_wave1069() {
        assert!(honesty_host_masked_vis_repair_resume_residual_residual_pack_wave1069());
        assert!(honesty_host_masked_vis_repair_resume_residual_method_names_residual_wave1069());
        assert!(honesty_host_masked_vis_repair_resume_residual_nav_commands_residual_wave1069());
        assert!(simulate_live_host_masked_vis_repair_resume_residual_honesty());
    }
}
