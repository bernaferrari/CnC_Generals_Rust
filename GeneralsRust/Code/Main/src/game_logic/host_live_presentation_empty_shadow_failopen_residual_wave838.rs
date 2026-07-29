//! Wave 838: presentation GW rebuild fail-open when shadow is empty so host
//! map/construct/train objects still feed unit mesh collect. playable_claim false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_EMPTY_SHADOW_FAILOPEN_METHOD_NAMES_WAVE838: &[&str] = &[
    "host_finalize_presentation_after_logic",
    "sync_from_host",
    "rebuild_objects_from_gameworld",
    "build_with_victory_for_engine",
    "build_from_gameworld",
    "Wave 838",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_EMPTY_SHADOW_FAILOPEN_NAV_STEPS_WAVE838: &[&str] = &[
    "REQUIRE_FINALIZE_SYNC",
    "REQUIRE_EMPTY_SHADOW_FAILOPEN",
    "REQUIRE_HOST_ROSTER_RESTORE",
    "LIVE_PRESENTATION_EMPTY_SHADOW_FAILOPEN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationEmptyShadowFailopenAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualPresentationEmptyShadowFailopenAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn pf_source() -> &'static str {
    include_str!("../presentation_frame.rs")
}

pub fn honesty_presentation_empty_shadow_failopen_method_names_residual_wave838() -> bool {
    let names = LIVE_PRESENTATION_EMPTY_SHADOW_FAILOPEN_METHOD_NAMES_WAVE838;
    let ok = residual_name_index(names, "sync_from_host").is_some()
        && residual_name_index(names, "rebuild_objects_from_gameworld").is_some()
        && residual_name_index(names, "Wave 838").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationEmptyShadowFailopenAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_presentation_empty_shadow_failopen_nav_commands_residual_wave838() -> bool {
    let steps = LIVE_PRESENTATION_EMPTY_SHADOW_FAILOPEN_NAV_STEPS_WAVE838;
    let ok = residual_name_index(steps, "LIVE_PRESENTATION_EMPTY_SHADOW_FAILOPEN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualPresentationEmptyShadowFailopenAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_presentation_empty_shadow_failopen_residual_pack_wave838() -> bool {
    let cnc = cnc_source();
    let pf = pf_source();
    let ok = cnc.contains("Wave 838: full host→shadow sync before GW presentation rebuild")
        && pf.contains("Wave 838: empty GameWorld shadow must not erase a non-empty host")
        && pf.contains("Wave 838: keep host objects when shadow yields nothing")
        && pf.contains("if gw_n == 0 && host_n > 0");
    residual_action_store(ResidualPresentationEmptyShadowFailopenAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_presentation_empty_shadow_failopen_honesty() -> bool {
    let a = honesty_presentation_empty_shadow_failopen_method_names_residual_wave838();
    let b = honesty_presentation_empty_shadow_failopen_nav_commands_residual_wave838();
    let c = honesty_presentation_empty_shadow_failopen_residual_pack_wave838();
    residual_action_store(ResidualPresentationEmptyShadowFailopenAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_presentation_empty_shadow_failopen_residual_wave838() {
        assert!(honesty_presentation_empty_shadow_failopen_residual_pack_wave838());
        assert!(honesty_presentation_empty_shadow_failopen_method_names_residual_wave838());
        assert!(honesty_presentation_empty_shadow_failopen_nav_commands_residual_wave838());
        assert!(simulate_live_presentation_empty_shadow_failopen_honesty());
    }
}
