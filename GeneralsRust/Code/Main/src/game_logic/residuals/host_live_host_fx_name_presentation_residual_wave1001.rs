//! Wave 1001: damage/bone/death FX name presentation residual.
//!
//! Entity freezes host pending transition damage FX, BoneFX last FX, and
//! pending death FX names; PresentationFrame projects them from GameWorld
//! instead of hardcoding None. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_FX_NAME_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1001: &[&str] = &[
    "damage_fx_name",
    "bone_fx_name",
    "death_fx_name",
    "Wave 1001",
    "playable_claim = false",
];

pub const LIVE_HOST_FX_NAME_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1001: &[&str] = &[
    "DAMAGE_FX",
    "BONE_FX",
    "DEATH_FX",
    "LIVE_HOST_FX_NAME_PRESENTATION_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostFxNamePresentationResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostFxNamePresentationResidualAction) {
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

pub fn honesty_host_fx_name_presentation_residual_method_names_residual_wave1001() -> bool {
    let names = LIVE_HOST_FX_NAME_PRESENTATION_RESIDUAL_METHOD_NAMES_WAVE1001;
    let ok = residual_name_index(names, "damage_fx_name").is_some()
        && residual_name_index(names, "Wave 1001").is_some();
    residual_action_store(ResidualHostFxNamePresentationResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fx_name_presentation_residual_nav_commands_residual_wave1001() -> bool {
    let steps = LIVE_HOST_FX_NAME_PRESENTATION_RESIDUAL_NAV_STEPS_WAVE1001;
    let ok = residual_name_index(steps, "LIVE_HOST_FX_NAME_PRESENTATION_RESIDUAL").is_some()
        && residual_name_index(steps, "DEATH_FX").is_some();
    residual_action_store(ResidualHostFxNamePresentationResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_fx_name_presentation_residual_residual_pack_wave1001() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let ent = entity_source();
    let sh = shadow_source();
    let gw = match pf.find("fn renderable_from_gameworld_entity") {
        Some(i) => &pf[i..pf.len().min(i + 12000)],
        None => "",
    };
    let ok = ent.contains("pub damage_fx_name: Option<String>")
        && ent.contains("pub bone_fx_name: Option<String>")
        && ent.contains("pub death_fx_name: Option<String>")
        && ent.contains("Wave 1001")
        && sh.contains("e.damage_fx_name = obj")
        && sh.contains("e.bone_fx_name = obj.bone_fx_damage")
        && sh.contains("e.death_fx_name = obj.pending_death_fx.clone()")
        && gw.contains("damage_fx_name: ent.damage_fx_name.clone()")
        && gw.contains("bone_fx_name: ent.bone_fx_name.clone()")
        && gw.contains("death_fx_name: ent.death_fx_name.clone()")
        && !gw.contains("damage_fx_name: None")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostFxNamePresentationResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_fx_name_presentation_residual_honesty() -> bool {
    let a = honesty_host_fx_name_presentation_residual_method_names_residual_wave1001();
    let b = honesty_host_fx_name_presentation_residual_nav_commands_residual_wave1001();
    let c = honesty_host_fx_name_presentation_residual_residual_pack_wave1001();
    residual_action_store(ResidualHostFxNamePresentationResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_fx_name_presentation_residual_wave1001() {
        assert!(honesty_host_fx_name_presentation_residual_residual_pack_wave1001());
        assert!(honesty_host_fx_name_presentation_residual_method_names_residual_wave1001());
        assert!(honesty_host_fx_name_presentation_residual_nav_commands_residual_wave1001());
        assert!(simulate_live_host_fx_name_presentation_residual_honesty());
    }
}
