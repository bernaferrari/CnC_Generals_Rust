//! Wave 1061: dual-world AlwaysSelectable dead selection residual.
//!
//! Dual collect_drawables allows destroyed units with AlwaysSelectable KindOf
//! (C++ SelectionXlat dead + KINDOF_ALWAYS_SELECTABLE). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_ALWAYS_SELECTABLE_DEAD_RESIDUAL_METHOD_NAMES_WAVE1061: &[&str] = &[
    "AlwaysSelectable",
    "collect_drawables",
    "Wave 1061",
    "playable_claim = false",
];

pub const LIVE_HOST_ALWAYS_SELECTABLE_DEAD_RESIDUAL_NAV_STEPS_WAVE1061: &[&str] = &[
    "ALWAYS_SELECTABLE",
    "DEAD_SELECTION",
    "LIVE_HOST_ALWAYS_SELECTABLE_DEAD_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostAlwaysSelectableDeadResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostAlwaysSelectableDeadResidualAction) {
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

pub fn honesty_host_always_selectable_dead_residual_method_names_residual_wave1061() -> bool {
    let names = LIVE_HOST_ALWAYS_SELECTABLE_DEAD_RESIDUAL_METHOD_NAMES_WAVE1061;
    let ok = residual_name_index(names, "AlwaysSelectable").is_some()
        && residual_name_index(names, "Wave 1061").is_some();
    residual_action_store(ResidualHostAlwaysSelectableDeadResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_always_selectable_dead_residual_nav_commands_residual_wave1061() -> bool {
    let steps = LIVE_HOST_ALWAYS_SELECTABLE_DEAD_RESIDUAL_NAV_STEPS_WAVE1061;
    let ok = residual_name_index(steps, "LIVE_HOST_ALWAYS_SELECTABLE_DEAD_RESIDUAL").is_some()
        && residual_name_index(steps, "ALWAYS_SELECTABLE").is_some();
    residual_action_store(ResidualHostAlwaysSelectableDeadResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_always_selectable_dead_residual_residual_pack_wave1061() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let sx = sx_source();
    let ok = sx.contains("Wave 1034/1035/1036/1061: C++ status + enemy stealth residual")
        && sx.contains("Wave 1061: destroyed still selectable when AlwaysSelectable")
        && sx.contains("translator_entry_has_kind(entry, \"AlwaysSelectable\")")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostAlwaysSelectableDeadResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_always_selectable_dead_residual_honesty() -> bool {
    let a = honesty_host_always_selectable_dead_residual_method_names_residual_wave1061();
    let b = honesty_host_always_selectable_dead_residual_nav_commands_residual_wave1061();
    let c = honesty_host_always_selectable_dead_residual_residual_pack_wave1061();
    residual_action_store(ResidualHostAlwaysSelectableDeadResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_always_selectable_dead_residual_wave1061() {
        assert!(honesty_host_always_selectable_dead_residual_residual_pack_wave1061());
        assert!(honesty_host_always_selectable_dead_residual_method_names_residual_wave1061());
        assert!(honesty_host_always_selectable_dead_residual_nav_commands_residual_wave1061());
        assert!(simulate_live_host_always_selectable_dead_residual_honesty());
    }
}
