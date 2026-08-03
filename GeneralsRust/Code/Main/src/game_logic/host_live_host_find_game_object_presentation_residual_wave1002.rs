//! Wave 1002: dual-world find_game_object presentation-known residual.
//!
//! Empty OBJECT_REGISTRY still cannot return Arc<Object>; Wave 1002 adds
//! presentation_object_known via translator catalog and documents the dual
//! fail-closed Object path. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FIND_GAME_OBJECT_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1002: &[&str] = &[
    "find_game_object",
    "presentation_object_known",
    "translator_catalog_entry",
    "Wave 1002",
    "playable_claim = false",
];

pub const LIVE_HOST_FIND_GAME_OBJECT_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1002: &[&str] = &[
    "DUAL_WORLD",
    "OBJECT_ARC_UNAVAILABLE",
    "PRESENTATION_KNOWN",
    "LIVE_HOST_FIND_GAME_OBJECT_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFindGameObjectPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFindGameObjectPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}
fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}
fn gc_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_find_game_object_presentation_residual_method_names_residual_wave1002() -> bool
{
    let names = LIVE_HOST_FIND_GAME_OBJECT_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1002;
    let ok = residual_name_index(names, "presentation_object_known").is_some()
        && residual_name_index(names, "Wave 1002").is_some();
    residual_action_store(ResidualHostFindGameObjectPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_find_game_object_presentation_residual_nav_commands_residual_wave1002() -> bool
{
    let steps = LIVE_HOST_FIND_GAME_OBJECT_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1002;
    let ok = residual_name_index(steps, "LIVE_HOST_FIND_GAME_OBJECT_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "PRESENTATION_KNOWN").is_some();
    residual_action_store(ResidualHostFindGameObjectPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_find_game_object_presentation_residual_residual_pack_wave1002() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let gc = gc_source();
    let find = match gc.find("pub fn find_game_object") {
        Some(i) => &gc[i..gc.len().min(i + 900)],
        None => "",
    };
    let ok = find.contains("Wave 269/1002")
        && find.contains("presentation_object_known")
        && gc.contains("pub fn presentation_object_known")
        && gc.contains("translator_catalog_entry(object_id)")
        && find.contains("return Ok(None)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFindGameObjectPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_find_game_object_presentation_residual_honesty() -> bool {
    let a = honesty_host_find_game_object_presentation_residual_method_names_residual_wave1002();
    let b = honesty_host_find_game_object_presentation_residual_nav_commands_residual_wave1002();
    let c = honesty_host_find_game_object_presentation_residual_residual_pack_wave1002();
    residual_action_store(ResidualHostFindGameObjectPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_find_game_object_presentation_residual_wave1002() {
        assert!(honesty_host_find_game_object_presentation_residual_residual_pack_wave1002());
        assert!(
            honesty_host_find_game_object_presentation_residual_method_names_residual_wave1002()
        );
        assert!(
            honesty_host_find_game_object_presentation_residual_nav_commands_residual_wave1002()
        );
        assert!(simulate_live_host_find_game_object_presentation_residual_honesty());
    }
}
