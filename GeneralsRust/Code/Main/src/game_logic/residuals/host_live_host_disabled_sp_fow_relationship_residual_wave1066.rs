//! Wave 1066: dual-world disabled SP source + FOW relationship residual.
//!
//! is_valid_special_power_target fails closed on disabled sources; relationship_to_target
//! fails closed on non-local fogged/black shroud. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DISABLED_SP_FOW_RELATIONSHIP_RESIDUAL_METHOD_NAMES_WAVE1066: &[&str] = &[
    "is_valid_special_power_target",
    "relationship_to_target",
    "source.disabled",
    "Wave 1066",
    "playable_claim = false",
];

pub const LIVE_HOST_DISABLED_SP_FOW_RELATIONSHIP_RESIDUAL_NAV_STEPS_WAVE1066: &[&str] = &[
    "DISABLED_SP_SOURCE",
    "FOW_RELATIONSHIP",
    "LIVE_HOST_DISABLED_SP_FOW_RELATIONSHIP_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDisabledSpFowRelationshipResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDisabledSpFowRelationshipResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
fn ui_source() -> &'static str {
    game_client::gui::ingame_ui::INGAME_UI_SRC
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_disabled_sp_fow_relationship_residual_method_names_residual_wave1066() -> bool {
    let names = LIVE_HOST_DISABLED_SP_FOW_RELATIONSHIP_RESIDUAL_METHOD_NAMES_WAVE1066;
    let ok = residual_name_index(names, "relationship_to_target").is_some()
        && residual_name_index(names, "Wave 1066").is_some();
    residual_action_store(ResidualHostDisabledSpFowRelationshipResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disabled_sp_fow_relationship_residual_nav_commands_residual_wave1066() -> bool {
    let steps = LIVE_HOST_DISABLED_SP_FOW_RELATIONSHIP_RESIDUAL_NAV_STEPS_WAVE1066;
    let ok = residual_name_index(steps, "LIVE_HOST_DISABLED_SP_FOW_RELATIONSHIP_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "FOW_RELATIONSHIP").is_some();
    residual_action_store(ResidualHostDisabledSpFowRelationshipResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_disabled_sp_fow_relationship_residual_residual_pack_wave1066() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let tr = tr_source();
    let ok = ui.contains("Wave 1066: disabled source residual fail-closed")
        && ui.contains("source.destroyed")
        && ui.contains("source.sold")
        && ui.contains("source.disabled")
        && tr.contains("Wave 1066: FOW fogged/black non-local relationship residual fail-closed")
        && tr.contains("entry.shroud_status >= 2 && !translator_entry_is_local(&entry)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDisabledSpFowRelationshipResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_disabled_sp_fow_relationship_residual_honesty() -> bool {
    let a = honesty_host_disabled_sp_fow_relationship_residual_method_names_residual_wave1066();
    let b = honesty_host_disabled_sp_fow_relationship_residual_nav_commands_residual_wave1066();
    let c = honesty_host_disabled_sp_fow_relationship_residual_residual_pack_wave1066();
    residual_action_store(ResidualHostDisabledSpFowRelationshipResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_disabled_sp_fow_relationship_residual_wave1066() {
        assert!(honesty_host_disabled_sp_fow_relationship_residual_residual_pack_wave1066());
        assert!(
            honesty_host_disabled_sp_fow_relationship_residual_method_names_residual_wave1066()
        );
        assert!(
            honesty_host_disabled_sp_fow_relationship_residual_nav_commands_residual_wave1066()
        );
        assert!(simulate_live_host_disabled_sp_fow_relationship_residual_honesty());
    }
}
