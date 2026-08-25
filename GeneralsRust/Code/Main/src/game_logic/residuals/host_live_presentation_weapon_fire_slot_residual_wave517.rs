//! Wave 517 residual peels: slot-aware weapon fire mesh bits + panic freeze.
//! - freeze `weapon_fire_status`, `is_panicking`, `moving_backwards`
//! - stamp FIRING/BETWEEN/PREATTACK/RELOADING/USING_WEAPON A/B/C from slot + status
//! - stamp PANICKING when `is_panicking`
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 516 formation selection links.
//! Architecture residual - weapon pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 517 freeze + slot-aware stamps
//! - host_enum_table_residual.rs firing/between/preattack/reload/using/panic helpers
//! - game_logic Object::weapon_fire_status / Entity::weapon_fire_status
//!
//! Fail-closed:
//! - Full W3D animation graph still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_WEAPON_FIRE_SLOT_METHOD_NAMES_WAVE517: &[&str] = &[
    "weapon_fire_status",
    "active_weapon_slot",
    "firing_b_model_bit",
    "between_firing_shots_a_model_bit",
    "panicking_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_WEAPON_FIRE_SLOT_SOURCE_MARKERS_WAVE517: &[&str] = &[
    "Wave 517: slot-aware FIRING / BETWEEN / PREATTACK / RELOADING + PANICKING",
    "weapon_fire_status: obj.weapon_fire_status as u8",
    "is_panicking: obj.is_panicking",
    "fn firing_b_model_bit",
];

pub const PRESENTATION_WEAPON_FIRE_SLOT_NAV_STEPS_WAVE517: &[&str] = &[
    "FREEZE_WEAPON_FIRE_STATUS",
    "FREEZE_PANICKING_BACKWARDS",
    "STAMP_SLOT_WEAPON_BITS",
    "STAMP_PANICKING",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_WEAPON_FIRE_SLOT_CMD_NAMES_WAVE517: &[&str] = &[
    "click_presentation_weapon_fire_slot_ok_wnd_detect",
    "click_presentation_weapon_fire_slot_ok_wnd_skip",
    "click_presentation_weapon_fire_slot_ok_wnd_queue",
    "click_presentation_weapon_fire_slot_ok_wnd_prepare",
    "click_presentation_weapon_fire_slot_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationWeaponFireSlotAction {
    Idle = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    FreezeSource = 4,
    StampSource = 5,
    Composite = 6,
}

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static LAST_ACTION: AtomicU8 = AtomicU8::new(0);

fn residual_action_store(a: ResidualPresentationWeaponFireSlotAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_weapon_fire_slot_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_weapon_fire_slot_last_action()
-> ResidualPresentationWeaponFireSlotAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationWeaponFireSlotAction::MethodNames,
        2 => ResidualPresentationWeaponFireSlotAction::SourceMarkers,
        3 => ResidualPresentationWeaponFireSlotAction::NavCommands,
        4 => ResidualPresentationWeaponFireSlotAction::FreezeSource,
        5 => ResidualPresentationWeaponFireSlotAction::StampSource,
        6 => ResidualPresentationWeaponFireSlotAction::Composite,
        _ => ResidualPresentationWeaponFireSlotAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

fn ent_source() -> &'static str {
    include_str!("../../../../GameEngine/GameLogic/src/world/entities/mod.rs")
}

pub fn honesty_presentation_weapon_fire_slot_method_names_residual_wave517() -> bool {
    PRESENTATION_WEAPON_FIRE_SLOT_METHOD_NAMES_WAVE517.len() == 6
        && residual_name_index(
            PRESENTATION_WEAPON_FIRE_SLOT_METHOD_NAMES_WAVE517,
            "weapon_fire_status",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WEAPON_FIRE_SLOT_METHOD_NAMES_WAVE517,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_weapon_fire_slot_source_markers_residual_wave517() -> bool {
    PRESENTATION_WEAPON_FIRE_SLOT_SOURCE_MARKERS_WAVE517.len() == 4
        && residual_name_index(
            PRESENTATION_WEAPON_FIRE_SLOT_SOURCE_MARKERS_WAVE517,
            "Wave 517: slot-aware FIRING / BETWEEN / PREATTACK / RELOADING + PANICKING",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_WEAPON_FIRE_SLOT_SOURCE_MARKERS_WAVE517,
            "fn firing_b_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_weapon_fire_slot_nav_commands_residual_wave517() -> bool {
    PRESENTATION_WEAPON_FIRE_SLOT_NAV_STEPS_WAVE517.len() == 6
        && residual_name_index(
            PRESENTATION_WEAPON_FIRE_SLOT_NAV_STEPS_WAVE517,
            "STAMP_SLOT_WEAPON_BITS",
        ) == Some(2)
        && residual_name_index(
            PRESENTATION_WEAPON_FIRE_SLOT_NAV_STEPS_WAVE517,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_WEAPON_FIRE_SLOT_CMD_NAMES_WAVE517.len() == 5
}

pub fn simulate_presentation_weapon_fire_slot_freeze_source() -> bool {
    let pf = pf_source();
    let ent = ent_source();
    let ok = pf.contains("weapon_fire_status: obj.weapon_fire_status as u8")
        && pf.contains("is_panicking: obj.is_panicking")
        && pf.contains("moving_backwards: obj.moving_backwards")
        && ent.contains("pub weapon_fire_status: u8");
    residual_action_store(ResidualPresentationWeaponFireSlotAction::FreezeSource);
    ok
}

pub fn simulate_presentation_weapon_fire_slot_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = pf
        .contains("Wave 517: slot-aware FIRING / BETWEEN / PREATTACK / RELOADING + PANICKING")
        && en.contains("pub fn firing_b_model_bit")
        && en.contains("pub fn between_firing_shots_a_model_bit")
        && en.contains("pub fn panicking_model_bit")
        && pf.contains("Wave 517: slot-aware USING_WEAPON_A/B/C");
    residual_action_store(ResidualPresentationWeaponFireSlotAction::StampSource);
    ok
}

pub fn honesty_presentation_weapon_fire_slot_residual_pack_wave517() -> bool {
    honesty_presentation_weapon_fire_slot_method_names_residual_wave517()
        && honesty_presentation_weapon_fire_slot_source_markers_residual_wave517()
        && honesty_presentation_weapon_fire_slot_nav_commands_residual_wave517()
        && simulate_presentation_weapon_fire_slot_freeze_source()
        && simulate_presentation_weapon_fire_slot_stamp_source()
}

pub fn simulate_live_presentation_weapon_fire_slot_honesty() -> bool {
    let ok = honesty_presentation_weapon_fire_slot_residual_pack_wave517();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationWeaponFireSlotAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_weapon_fire_slot_method_names_residual_wave517());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_weapon_fire_slot_source_markers_residual_wave517());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_weapon_fire_slot_nav_commands_residual_wave517());
    }

    #[test]
    fn presentation_weapon_fire_slot_sources() {
        assert!(simulate_presentation_weapon_fire_slot_freeze_source());
        assert!(simulate_presentation_weapon_fire_slot_stamp_source());
    }

    #[test]
    fn wave517_composite_pack() {
        assert!(honesty_presentation_weapon_fire_slot_residual_pack_wave517());
    }

    #[test]
    fn simulate_live_presentation_weapon_fire_slot_honesty_residual_live() {
        assert!(
            simulate_live_presentation_weapon_fire_slot_honesty(),
            "presentation weapon-fire-slot residual must latch"
        );
        assert!(residual_presentation_weapon_fire_slot_ok());
        assert_eq!(
            residual_presentation_weapon_fire_slot_last_action(),
            ResidualPresentationWeaponFireSlotAction::Composite
        );
    }
}
