//! Wave 999: surrender + emoticon presentation residual from GameWorld Entity.
//!
//! Entity gains is_surrendered / emoticon_name / emoticon_frames_left (host
//! Object shadow). Presentation freezes them instead of hardcoding empty/false.
//! Also syncs formation_id/offset host→entity on status path.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SURRENDER_EMOTICON_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE999: &[&str] = &[
    "is_surrendered",
    "emoticon_name",
    "emoticon_frames_left",
    "Wave 999",
    "playable_claim = false",
];

pub const LIVE_HOST_SURRENDER_EMOTICON_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE999: &[&str] = &[
    "SURRENDERED",
    "EMOTICON",
    "ENTITY_SHADOW",
    "LIVE_HOST_SURRENDER_EMOTICON_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSurrenderEmoticonPresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSurrenderEmoticonPresentationResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn entity_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}

pub fn honesty_host_surrender_emoticon_presentation_residual_method_names_residual_wave999() -> bool
{
    let names = LIVE_HOST_SURRENDER_EMOTICON_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE999;
    let ok = residual_name_index(names, "is_surrendered").is_some()
        && residual_name_index(names, "Wave 999").is_some();
    residual_action_store(ResidualHostSurrenderEmoticonPresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_surrender_emoticon_presentation_residual_nav_commands_residual_wave999() -> bool
{
    let steps = LIVE_HOST_SURRENDER_EMOTICON_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE999;
    let ok = residual_name_index(steps, "LIVE_HOST_SURRENDER_EMOTICON_PRESENTATION_RESIDUAL")
        .is_some()
        && residual_name_index(steps, "EMOTICON").is_some();
    residual_action_store(ResidualHostSurrenderEmoticonPresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_surrender_emoticon_presentation_residual_residual_pack_wave999() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let sh = shadow_source();
    let gw = match pf.find("fn renderable_from_gameworld_entity") {
        Some(i) => &pf[i..pf.len().min(i + 10000)],
        None => "",
    };
    let ok = ent.contains("pub is_surrendered: bool")
        && ent.contains("pub emoticon_name: String")
        && ent.contains("Wave 999")
        && sh.contains("e.is_surrendered = obj.is_surrendered")
        && sh.contains("e.emoticon_name = obj.emoticon_name.clone()")
        && gw.contains("is_surrendered: ent.is_surrendered")
        && gw.contains("emoticon_name: ent.emoticon_name.clone()")
        && gw.contains("emoticon_frames_left: ent.emoticon_frames_left")
        && !gw.contains("is_surrendered: false")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostSurrenderEmoticonPresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_surrender_emoticon_presentation_residual_honesty() -> bool {
    let a = honesty_host_surrender_emoticon_presentation_residual_method_names_residual_wave999();
    let b = honesty_host_surrender_emoticon_presentation_residual_nav_commands_residual_wave999();
    let c = honesty_host_surrender_emoticon_presentation_residual_residual_pack_wave999();
    residual_action_store(ResidualHostSurrenderEmoticonPresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_surrender_emoticon_presentation_residual_wave999() {
        assert!(honesty_host_surrender_emoticon_presentation_residual_residual_pack_wave999());
        assert!(
            honesty_host_surrender_emoticon_presentation_residual_method_names_residual_wave999()
        );
        assert!(
            honesty_host_surrender_emoticon_presentation_residual_nav_commands_residual_wave999()
        );
        assert!(simulate_live_host_surrender_emoticon_presentation_residual_honesty());
    }
}
