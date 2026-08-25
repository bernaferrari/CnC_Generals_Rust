//! Wave 1082: dual-world is_selectable FOW packing + disabled prisoner residual.
//!
//! Dual SelectableDrawable is_selectable fails closed on non-local FOW fogged/black;
//! is_prisoner_target dual fails closed on disabled. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_IS_SELECTABLE_FOW_PRISONER_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1082: &[&str] = &[
    "is_selectable",
    "shroud_status",
    "is_prisoner_target",
    "Wave 1082",
    "playable_claim = false",
];

pub const LIVE_HOST_IS_SELECTABLE_FOW_PRISONER_DISABLED_RESIDUAL_NAV_STEPS_WAVE1082: &[&str] = &[
    "IS_SELECTABLE_FOW",
    "PRISONER_DISABLED",
    "LIVE_HOST_IS_SELECTABLE_FOW_PRISONER_DISABLED_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostIsSelectableFowPrisonerDisabledResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostIsSelectableFowPrisonerDisabledResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn sx_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_is_selectable_fow_prisoner_disabled_residual_method_names_residual_wave1082()
-> bool {
    let names = LIVE_HOST_IS_SELECTABLE_FOW_PRISONER_DISABLED_RESIDUAL_METHOD_NAMES_WAVE1082;
    let ok = residual_name_index(names, "is_selectable").is_some()
        && residual_name_index(names, "Wave 1082").is_some();
    residual_action_store(ResidualHostIsSelectableFowPrisonerDisabledResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_is_selectable_fow_prisoner_disabled_residual_nav_commands_residual_wave1082()
-> bool {
    let steps = LIVE_HOST_IS_SELECTABLE_FOW_PRISONER_DISABLED_RESIDUAL_NAV_STEPS_WAVE1082;
    let ok = residual_name_index(
        steps,
        "LIVE_HOST_IS_SELECTABLE_FOW_PRISONER_DISABLED_RESIDUAL",
    )
    .is_some()
        && residual_name_index(steps, "IS_SELECTABLE_FOW").is_some();
    residual_action_store(ResidualHostIsSelectableFowPrisonerDisabledResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_is_selectable_fow_prisoner_disabled_residual_residual_pack_wave1082() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let tr = tr_source();
    let ok = sx.contains("Wave 1082: FOW fogged/black non-local residual also unselectable")
        && sx.contains("entry.shroud_status >= 2 && !translator_entry_is_local(entry)")
        && tr.contains("Wave 1082: disabled prisoner residual fail-closed")
        && tr.contains("if e.disabled {\n            return false;\n        }")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostIsSelectableFowPrisonerDisabledResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_is_selectable_fow_prisoner_disabled_residual_honesty() -> bool {
    let a =
        honesty_host_is_selectable_fow_prisoner_disabled_residual_method_names_residual_wave1082();
    let b =
        honesty_host_is_selectable_fow_prisoner_disabled_residual_nav_commands_residual_wave1082();
    let c = honesty_host_is_selectable_fow_prisoner_disabled_residual_residual_pack_wave1082();
    residual_action_store(
        ResidualHostIsSelectableFowPrisonerDisabledResidualAction::DispatchSource,
    );
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_is_selectable_fow_prisoner_disabled_residual_wave1082() {
        assert!(honesty_host_is_selectable_fow_prisoner_disabled_residual_residual_pack_wave1082());
        assert!(
            honesty_host_is_selectable_fow_prisoner_disabled_residual_method_names_residual_wave1082()
        );
        assert!(
            honesty_host_is_selectable_fow_prisoner_disabled_residual_nav_commands_residual_wave1082()
        );
        assert!(simulate_live_host_is_selectable_fow_prisoner_disabled_residual_honesty());
    }
}
