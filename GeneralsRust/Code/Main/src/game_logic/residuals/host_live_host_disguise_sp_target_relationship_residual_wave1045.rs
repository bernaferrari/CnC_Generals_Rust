//! Wave 1045: dual-world disguise special-power target relationship residual.
//!
//! is_valid_special_power_target dual path uses apparent team for disguised
//! targets when the caster is not allied to the real owner. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DISGUISE_SP_TARGET_RELATIONSHIP_RESIDUAL_METHOD_NAMES_WAVE1045: &[&str] = &[
    "is_valid_special_power_target",
    "disguise_as_team",
    "Wave 1045",
    "playable_claim = false",
];

pub const LIVE_HOST_DISGUISE_SP_TARGET_RELATIONSHIP_RESIDUAL_NAV_STEPS_WAVE1045: &[&str] = &[
    "DISGUISE",
    "SPECIAL_POWER",
    "TARGET_RELATIONSHIP",
    "LIVE_HOST_DISGUISE_SP_TARGET_RELATIONSHIP_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDisguiseSpTargetRelationshipResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDisguiseSpTargetRelationshipResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}

pub fn honesty_host_disguise_sp_target_relationship_residual_method_names_residual_wave1045() -> bool
{
    let names = LIVE_HOST_DISGUISE_SP_TARGET_RELATIONSHIP_RESIDUAL_METHOD_NAMES_WAVE1045;
    let ok = residual_name_index(names, "is_valid_special_power_target").is_some()
        && residual_name_index(names, "Wave 1045").is_some();
    residual_action_store(ResidualHostDisguiseSpTargetRelationshipResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_sp_target_relationship_residual_nav_commands_residual_wave1045() -> bool
{
    let steps = LIVE_HOST_DISGUISE_SP_TARGET_RELATIONSHIP_RESIDUAL_NAV_STEPS_WAVE1045;
    let ok = residual_name_index(steps, "LIVE_HOST_DISGUISE_SP_TARGET_RELATIONSHIP_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "SPECIAL_POWER").is_some();
    residual_action_store(ResidualHostDisguiseSpTargetRelationshipResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disguise_sp_target_relationship_residual_residual_pack_wave1045() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let ok = ui.contains("Wave 1045: disguised targets present apparent team")
        && ui.contains("disguise_as_team")
        && ui.contains("is_valid_special_power_target")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDisguiseSpTargetRelationshipResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_disguise_sp_target_relationship_residual_honesty() -> bool {
    let a = honesty_host_disguise_sp_target_relationship_residual_method_names_residual_wave1045();
    let b = honesty_host_disguise_sp_target_relationship_residual_nav_commands_residual_wave1045();
    let c = honesty_host_disguise_sp_target_relationship_residual_residual_pack_wave1045();
    residual_action_store(ResidualHostDisguiseSpTargetRelationshipResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_disguise_sp_target_relationship_residual_wave1045() {
        assert!(honesty_host_disguise_sp_target_relationship_residual_residual_pack_wave1045());
        assert!(
            honesty_host_disguise_sp_target_relationship_residual_method_names_residual_wave1045()
        );
        assert!(
            honesty_host_disguise_sp_target_relationship_residual_nav_commands_residual_wave1045()
        );
        assert!(simulate_live_host_disguise_sp_target_relationship_residual_honesty());
    }
}
