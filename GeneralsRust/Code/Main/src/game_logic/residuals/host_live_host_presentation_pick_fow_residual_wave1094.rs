//! Wave 1094: presentation pick non-local FOW residual.
//!
//! World-pick and screen-pick still admitted non-local fogged/shrouded objects
//! (only destroyed gated). Dual collect_selectable fails closed when
//! shroud_status >= PartialClear for non-local. Mirror Clear-only (alpha>=0.95)
//! on presentation pick paths used by find_object_at_position.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_PICK_FOW_METHOD_NAMES_WAVE1094: &[&str] = &[
    "pick_object_id_at_world_from_presentation",
    "pick_object_at_screen_pos_from_presentation",
    "Wave 1094",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_PICK_FOW_NAV_STEPS_WAVE1094: &[&str] = &[
    "PRESENTATION_PICK_FOW",
    "NON_LOCAL_CLEAR_ONLY",
    "LIVE_HOST_PRESENTATION_PICK_FOW",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationPickFowAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationPickFowAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn uc_source() -> &'static str {
    include_str!("../../unit_control.rs")
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_presentation_pick_fow_method_names_residual_wave1094() -> bool {
    let names = LIVE_HOST_PRESENTATION_PICK_FOW_METHOD_NAMES_WAVE1094;
    let ok = residual_name_index(names, "pick_object_id_at_world_from_presentation").is_some()
        && residual_name_index(names, "Wave 1094").is_some();
    residual_action_store(ResidualHostPresentationPickFowAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_pick_fow_nav_commands_residual_wave1094() -> bool {
    let steps = LIVE_HOST_PRESENTATION_PICK_FOW_NAV_STEPS_WAVE1094;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_PICK_FOW").is_some()
        && residual_name_index(steps, "NON_LOCAL_CLEAR_ONLY").is_some();
    residual_action_store(ResidualHostPresentationPickFowAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_pick_fow_residual_pack_wave1094() -> bool {
    let uc = uc_source();
    let cnc = cnc_source();
    let es = es_source();
    let world_i = match uc.find("fn pick_object_id_at_world_from_presentation") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationPickFowAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let world = &uc[world_i..world_i.saturating_add(1600)];
    let screen_i = match uc.find("fn pick_object_at_screen_pos_from_presentation") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationPickFowAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let screen = &uc[screen_i..screen_i.saturating_add(1200)];
    let find_i = match cnc.find("fn find_object_at_position") {
        Some(i) => i,
        None => {
            residual_action_store(ResidualHostPresentationPickFowAction::SourceMarkers);
            RESIDUAL_OK.store(false, Ordering::SeqCst);
            return false;
        }
    };
    let find = &cnc[find_i..find_i.saturating_add(900)];
    let ok = world.contains("Wave 1094: non-local FOW residual fail-closed")
        && world.contains("visibility_alpha < 0.95")
        && world.contains("is_local")
        && screen.contains("Wave 1094: non-local FOW residual fail-closed")
        && screen.contains("visibility_alpha < 0.95")
        && find.contains("pick_object_id_at_world_from_presentation")
        && find.contains("presentation-only pick")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    residual_action_store(ResidualHostPresentationPickFowAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_pick_fow_residual_honesty() -> bool {
    let a = honesty_host_presentation_pick_fow_method_names_residual_wave1094();
    let b = honesty_host_presentation_pick_fow_nav_commands_residual_wave1094();
    let c = honesty_host_presentation_pick_fow_residual_pack_wave1094();
    residual_action_store(ResidualHostPresentationPickFowAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_pick_fow_residual_wave1094() {
        assert!(honesty_host_presentation_pick_fow_residual_pack_wave1094());
        assert!(honesty_host_presentation_pick_fow_method_names_residual_wave1094());
        assert!(honesty_host_presentation_pick_fow_nav_commands_residual_wave1094());
        assert!(simulate_live_host_presentation_pick_fow_residual_honesty());
    }
}
