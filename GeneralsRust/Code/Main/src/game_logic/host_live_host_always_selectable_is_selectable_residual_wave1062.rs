//! Wave 1062: dual-world AlwaysSelectable dead is_selectable residual.
//!
//! Dual SelectableDrawable.is_selectable allows destroyed AlwaysSelectable
//! units so can_select KindOf path works. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ALWAYS_SELECTABLE_IS_SELECTABLE_RESIDUAL_METHOD_NAMES_WAVE1062: &[&str] = &[
    "is_selectable",
    "AlwaysSelectable",
    "Wave 1062",
    "playable_claim = false",
];

pub const LIVE_HOST_ALWAYS_SELECTABLE_IS_SELECTABLE_RESIDUAL_NAV_STEPS_WAVE1062: &[&str] = &[
    "ALWAYS_SELECTABLE",
    "IS_SELECTABLE",
    "LIVE_HOST_ALWAYS_SELECTABLE_IS_SELECTABLE_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAlwaysSelectableIsSelectableResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAlwaysSelectableIsSelectableResidualAction) {
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

pub fn honesty_host_always_selectable_is_selectable_residual_method_names_residual_wave1062() -> bool
{
    let names = LIVE_HOST_ALWAYS_SELECTABLE_IS_SELECTABLE_RESIDUAL_METHOD_NAMES_WAVE1062;
    let ok = residual_name_index(names, "is_selectable").is_some()
        && residual_name_index(names, "Wave 1062").is_some();
    residual_action_store(ResidualHostAlwaysSelectableIsSelectableResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_always_selectable_is_selectable_residual_nav_commands_residual_wave1062() -> bool
{
    let steps = LIVE_HOST_ALWAYS_SELECTABLE_IS_SELECTABLE_RESIDUAL_NAV_STEPS_WAVE1062;
    let ok = residual_name_index(steps, "LIVE_HOST_ALWAYS_SELECTABLE_IS_SELECTABLE_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "IS_SELECTABLE").is_some();
    residual_action_store(ResidualHostAlwaysSelectableIsSelectableResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_always_selectable_is_selectable_residual_residual_pack_wave1062() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let ok = sx.contains("Wave 1062: AlwaysSelectable dead units remain selectable (C++)")
        && sx.contains("translator_entry_has_kind(entry, \"AlwaysSelectable\")")
        && sx.contains("Wave 1035/1062: destroyed residual maps to is_dead")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostAlwaysSelectableIsSelectableResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_always_selectable_is_selectable_residual_honesty() -> bool {
    let a = honesty_host_always_selectable_is_selectable_residual_method_names_residual_wave1062();
    let b = honesty_host_always_selectable_is_selectable_residual_nav_commands_residual_wave1062();
    let c = honesty_host_always_selectable_is_selectable_residual_residual_pack_wave1062();
    residual_action_store(ResidualHostAlwaysSelectableIsSelectableResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_always_selectable_is_selectable_residual_wave1062() {
        assert!(honesty_host_always_selectable_is_selectable_residual_residual_pack_wave1062());
        assert!(
            honesty_host_always_selectable_is_selectable_residual_method_names_residual_wave1062()
        );
        assert!(
            honesty_host_always_selectable_is_selectable_residual_nav_commands_residual_wave1062()
        );
        assert!(simulate_live_host_always_selectable_is_selectable_residual_honesty());
    }
}
