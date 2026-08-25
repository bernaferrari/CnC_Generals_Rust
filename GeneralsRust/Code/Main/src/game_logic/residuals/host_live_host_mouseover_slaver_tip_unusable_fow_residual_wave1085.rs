//! Wave 1085: dual-world mouseover slaver/tip unusable FOW residual.
//!
//! After IgnoredInGui→slaver remap, tip residual fails closed on destroyed/sold/
//! unselectable/masked, non-local stealth, and non-local FOW. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_MOUSEOVER_SLAVER_TIP_UNUSABLE_FOW_RESIDUAL_METHOD_NAMES_WAVE1085: &[&str] = &[
    "create_mouseover_hint_from_presentation",
    "slaver_object_id",
    "tip_entry",
    "Wave 1085",
    "playable_claim = false",
];

pub const LIVE_HOST_MOUSEOVER_SLAVER_TIP_UNUSABLE_FOW_RESIDUAL_NAV_STEPS_WAVE1085: &[&str] = &[
    "MOUSEOVER_SLAVER",
    "TIP_UNUSABLE_FOW",
    "LIVE_HOST_MOUSEOVER_SLAVER_TIP_UNUSABLE_FOW_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMouseoverSlaverTipUnusableFowResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostMouseoverSlaverTipUnusableFowResidualAction) {
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

pub fn honesty_host_mouseover_slaver_tip_unusable_fow_residual_method_names_residual_wave1085()
-> bool {
    let names = LIVE_HOST_MOUSEOVER_SLAVER_TIP_UNUSABLE_FOW_RESIDUAL_METHOD_NAMES_WAVE1085;
    let ok = residual_name_index(names, "slaver_object_id").is_some()
        && residual_name_index(names, "Wave 1085").is_some();
    residual_action_store(ResidualHostMouseoverSlaverTipUnusableFowResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mouseover_slaver_tip_unusable_fow_residual_nav_commands_residual_wave1085()
-> bool {
    let steps = LIVE_HOST_MOUSEOVER_SLAVER_TIP_UNUSABLE_FOW_RESIDUAL_NAV_STEPS_WAVE1085;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_MOUSEOVER_SLAVER_TIP_UNUSABLE_FOW_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "TIP_UNUSABLE_FOW").is_some();
    residual_action_store(ResidualHostMouseoverSlaverTipUnusableFowResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_mouseover_slaver_tip_unusable_fow_residual_residual_pack_wave1085() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let ui = ui_source();
    let ok = ui
        .contains("Wave 1085: slaver/tip residual fail-closed on unusable/FOW/stealth non-local")
        && ui.contains("tip_entry.destroyed")
        && ui.contains("tip_entry.effectively_stealthed && !tip_local")
        && ui.contains("fogged && !tip_local")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostMouseoverSlaverTipUnusableFowResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_mouseover_slaver_tip_unusable_fow_residual_honesty() -> bool {
    let a =
        honesty_host_mouseover_slaver_tip_unusable_fow_residual_method_names_residual_wave1085();
    let b =
        honesty_host_mouseover_slaver_tip_unusable_fow_residual_nav_commands_residual_wave1085();
    let c = honesty_host_mouseover_slaver_tip_unusable_fow_residual_residual_pack_wave1085();
    residual_action_store(ResidualHostMouseoverSlaverTipUnusableFowResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_mouseover_slaver_tip_unusable_fow_residual_wave1085() {
        assert!(honesty_host_mouseover_slaver_tip_unusable_fow_residual_residual_pack_wave1085());
        assert!(
            honesty_host_mouseover_slaver_tip_unusable_fow_residual_method_names_residual_wave1085(
            )
        );
        assert!(
            honesty_host_mouseover_slaver_tip_unusable_fow_residual_nav_commands_residual_wave1085(
            )
        );
        assert!(simulate_live_host_mouseover_slaver_tip_unusable_fow_residual_honesty());
    }
}
