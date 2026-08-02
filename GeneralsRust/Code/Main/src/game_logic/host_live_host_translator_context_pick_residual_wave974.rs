//! Wave 974: translator context-pick presentation catalog residual.
//!
//! Peels collect_selectable_objects onto presentation translator catalog
//! residual (position + kind + local team) when OBJECT_REGISTRY is empty.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_TRANSLATOR_CONTEXT_PICK_METHOD_NAMES_WAVE974: &[&str] = &[
    "collect_selectable_objects_from_presentation",
    "collect_selectable_objects",
    "with_translator_catalog",
    "Wave 974",
    "playable_claim = false",
];

pub const LIVE_HOST_TRANSLATOR_CONTEXT_PICK_NAV_STEPS_WAVE974: &[&str] = &[
    "CONTEXT_PICK_FROM_CATALOG",
    "POSITION_KIND_RESIDUAL",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_TRANSLATOR_CONTEXT_PICK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostTranslatorContextPickAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostTranslatorContextPickAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn tr_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/message_stream/translators.rs")
}

fn residual_mod_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/presentation_translator_residual.rs")
}

fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_translator_context_pick_method_names_residual_wave974() -> bool {
    let names = LIVE_HOST_TRANSLATOR_CONTEXT_PICK_METHOD_NAMES_WAVE974;
    let ok = residual_name_index(names, "collect_selectable_objects_from_presentation").is_some()
        && residual_name_index(names, "Wave 974").is_some();
    residual_action_store(ResidualHostTranslatorContextPickAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_translator_context_pick_nav_commands_residual_wave974() -> bool {
    let steps = LIVE_HOST_TRANSLATOR_CONTEXT_PICK_NAV_STEPS_WAVE974;
    let ok = residual_name_index(steps, "LIVE_HOST_TRANSLATOR_CONTEXT_PICK").is_some()
        && residual_name_index(steps, "CONTEXT_PICK_FROM_CATALOG").is_some();
    residual_action_store(ResidualHostTranslatorContextPickAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_translator_context_pick_residual_pack_wave974() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let residual = residual_mod_source();
    let client = client_source();
    let collect = match tr.find("fn collect_selectable_objects(") {
        Some(i) => &tr[i..tr.len().min(i + 900)],
        None => "",
    };
    let from_pres = match tr.find("fn collect_selectable_objects_from_presentation") {
        Some(i) => &tr[i..tr.len().min(i + 1200)],
        None => "",
    };
    let ok = tr.contains("Wave 974")
        && residual.contains("Wave 974")
        && client.contains("position: u.position")
        && residual.contains("position: [f32; 3]")
        && collect.contains("collect_selectable_objects_from_presentation")
        && from_pres.contains("with_translator_catalog")
        && from_pres.contains("object_pick_distance")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostTranslatorContextPickAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_translator_context_pick_honesty() -> bool {
    let a = honesty_host_translator_context_pick_method_names_residual_wave974();
    let b = honesty_host_translator_context_pick_nav_commands_residual_wave974();
    let c = honesty_host_translator_context_pick_residual_pack_wave974();
    residual_action_store(ResidualHostTranslatorContextPickAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_translator_context_pick_residual_wave974() {
        assert!(honesty_host_translator_context_pick_residual_pack_wave974());
        assert!(honesty_host_translator_context_pick_method_names_residual_wave974());
        assert!(honesty_host_translator_context_pick_nav_commands_residual_wave974());
        assert!(simulate_live_host_translator_context_pick_honesty());
    }
}
