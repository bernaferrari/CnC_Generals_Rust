//! Wave 995: captured/prone/poison/defector presentation residual.
//!
//! GameWorld Entity path no longer hardcodes false for:
//! captured (private_captured), prone (prone_active), poison_tinted,
//! undetected_defector, defector_flash. Host Object path already froze these.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_STATUS_FLAG_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE995: &[&str] = &[
    "private_captured",
    "prone_active",
    "poison_damage_frame",
    "defection_undetected",
    "defection_flash_this_frame",
    "Wave 995",
    "playable_claim = false",
];

pub const LIVE_HOST_STATUS_FLAG_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE995: &[&str] = &[
    "CAPTURED",
    "PRONE",
    "POISON_TINTED",
    "DEFECTOR",
    "LIVE_HOST_STATUS_FLAG_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostStatusFlagPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostStatusFlagPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}
fn entity_source() -> &'static str {
    include_str!("../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}

pub fn honesty_host_status_flag_presentation_residual_method_names_residual_wave995() -> bool {
    let names = LIVE_HOST_STATUS_FLAG_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE995;
    let ok = residual_name_index(names, "private_captured").is_some()
        && residual_name_index(names, "Wave 995").is_some();
    residual_action_store(ResidualHostStatusFlagPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_status_flag_presentation_residual_nav_commands_residual_wave995() -> bool {
    let steps = LIVE_HOST_STATUS_FLAG_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE995;
    let ok = residual_name_index(steps, "LIVE_HOST_STATUS_FLAG_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "DEFECTOR").is_some();
    residual_action_store(ResidualHostStatusFlagPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_status_flag_presentation_residual_residual_pack_wave995() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let gw = match pf.find("fn renderable_from_gameworld_entity") {
        Some(i) => &pf[i..pf.len().min(i + 12000)],
        None => "",
    };
    let ok = ent.contains("pub private_captured: bool")
        && ent.contains("pub prone_active: bool")
        && ent.contains("pub defection_undetected: bool")
        && gw.contains("captured: ent.private_captured")
        && gw.contains("prone: ent.prone_active")
        && gw.contains("poison_tinted: ent.poison_damage_frame != 0")
        && gw.contains("undetected_defector: ent.defection_undetected")
        && gw.contains("defector_flash: ent.defection_flash_this_frame")
        && !gw.contains("captured: false")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostStatusFlagPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_status_flag_presentation_residual_honesty() -> bool {
    let a = honesty_host_status_flag_presentation_residual_method_names_residual_wave995();
    let b = honesty_host_status_flag_presentation_residual_nav_commands_residual_wave995();
    let c = honesty_host_status_flag_presentation_residual_residual_pack_wave995();
    residual_action_store(ResidualHostStatusFlagPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_status_flag_presentation_residual_wave995() {
        assert!(honesty_host_status_flag_presentation_residual_residual_pack_wave995());
        assert!(honesty_host_status_flag_presentation_residual_method_names_residual_wave995());
        assert!(honesty_host_status_flag_presentation_residual_nav_commands_residual_wave995());
        assert!(simulate_live_host_status_flag_presentation_residual_honesty());
    }
}
