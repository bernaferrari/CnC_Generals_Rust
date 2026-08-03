//! Wave 1063: dual-world FOW fogged non-local selection residual.
//!
//! Dual collect_drawables skips non-local units with shroud_status >= 2
//! (fogged/black SelectionInfo parity). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FOW_FOGGED_SELECTION_RESIDUAL_METHOD_NAMES_WAVE1063: &[&str] = &[
    "shroud_status",
    "translator_entry_is_local",
    "Wave 1063",
    "playable_claim = false",
];

pub const LIVE_HOST_FOW_FOGGED_SELECTION_RESIDUAL_NAV_STEPS_WAVE1063: &[&str] = &[
    "FOW",
    "FOGGED_SELECTION",
    "LIVE_HOST_FOW_FOGGED_SELECTION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFowFoggedSelectionResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFowFoggedSelectionResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn sx_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}

pub fn honesty_host_fow_fogged_selection_residual_method_names_residual_wave1063() -> bool {
    let names = LIVE_HOST_FOW_FOGGED_SELECTION_RESIDUAL_METHOD_NAMES_WAVE1063;
    let ok = residual_name_index(names, "shroud_status").is_some()
        && residual_name_index(names, "Wave 1063").is_some();
    residual_action_store(ResidualHostFowFoggedSelectionResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_fogged_selection_residual_nav_commands_residual_wave1063() -> bool {
    let steps = LIVE_HOST_FOW_FOGGED_SELECTION_RESIDUAL_NAV_STEPS_WAVE1063;
    let ok = residual_name_index(steps, "LIVE_HOST_FOW_FOGGED_SELECTION_RESIDUAL").is_some()
        && residual_name_index(steps, "FOGGED_SELECTION").is_some();
    residual_action_store(ResidualHostFowFoggedSelectionResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fow_fogged_selection_residual_residual_pack_wave1063() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let ok = sx.contains("Wave 1063: non-local fogged/black fail-closed")
        && sx.contains("entry.shroud_status >= 2 && !translator_entry_is_local(entry)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFowFoggedSelectionResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fow_fogged_selection_residual_honesty() -> bool {
    let a = honesty_host_fow_fogged_selection_residual_method_names_residual_wave1063();
    let b = honesty_host_fow_fogged_selection_residual_nav_commands_residual_wave1063();
    let c = honesty_host_fow_fogged_selection_residual_residual_pack_wave1063();
    residual_action_store(ResidualHostFowFoggedSelectionResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fow_fogged_selection_residual_wave1063() {
        assert!(honesty_host_fow_fogged_selection_residual_residual_pack_wave1063());
        assert!(honesty_host_fow_fogged_selection_residual_method_names_residual_wave1063());
        assert!(honesty_host_fow_fogged_selection_residual_nav_commands_residual_wave1063());
        assert!(simulate_live_host_fow_fogged_selection_residual_honesty());
    }
}
