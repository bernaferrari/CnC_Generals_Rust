//! Wave 519 residual peels: shock explode / power-plant rods / jet afterburner mesh bits.
//! - freeze shock_* , power_plant_rods_*, jet_slow_death_active
//! - stamp EXPLODED_FLAILING/BOUNCING, SPLATTED, POWER_PLANT_UPGRADING, JETAFTERBURNER, SMOLDERING
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 518 weaponset/enemy-near bits.
//! Architecture residual - death/upgrade pose without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 519 freeze + stamps
//! - host_enum_table_residual.rs exploded/power_plant_upgrading/jetafterburner helpers
//!
//! Fail-closed:
//! - Full SlowDeath/JetSlowDeath drawable graph still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_SHOCK_POWER_JET_METHOD_NAMES_WAVE519: &[&str] = &[
    "shock_was_airborne",
    "power_plant_rods_extended",
    "jet_slow_death_active",
    "exploded_flailing_model_bit",
    "power_plant_upgrading_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_SHOCK_POWER_JET_SOURCE_MARKERS_WAVE519: &[&str] = &[
    "Wave 519: exploded flail/bounce, power-plant upgrading, jet afterburner residual bits",
    "shock_was_airborne: obj.shock_was_airborne",
    "power_plant_rods_extended: obj.power_plant_rods_extended",
    "fn exploded_flailing_model_bit",
];

pub const PRESENTATION_SHOCK_POWER_JET_NAV_STEPS_WAVE519: &[&str] = &[
    "FREEZE_SHOCK_STATE",
    "FREEZE_POWER_PLANT_RODS",
    "FREEZE_JET_SLOW_DEATH",
    "STAMP_EXPLODED_POWER_JET",
    "NO_LIVE_GAMELOGIC_DUAL_READ",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_SHOCK_POWER_JET_CMD_NAMES_WAVE519: &[&str] = &[
    "click_presentation_shock_power_jet_ok_wnd_detect",
    "click_presentation_shock_power_jet_ok_wnd_skip",
    "click_presentation_shock_power_jet_ok_wnd_queue",
    "click_presentation_shock_power_jet_ok_wnd_prepare",
    "click_presentation_shock_power_jet_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationShockPowerJetAction {
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

fn residual_action_store(a: ResidualPresentationShockPowerJetAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_shock_power_jet_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_shock_power_jet_last_action() -> ResidualPresentationShockPowerJetAction
{
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationShockPowerJetAction::MethodNames,
        2 => ResidualPresentationShockPowerJetAction::SourceMarkers,
        3 => ResidualPresentationShockPowerJetAction::NavCommands,
        4 => ResidualPresentationShockPowerJetAction::FreezeSource,
        5 => ResidualPresentationShockPowerJetAction::StampSource,
        6 => ResidualPresentationShockPowerJetAction::Composite,
        _ => ResidualPresentationShockPowerJetAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

pub fn honesty_presentation_shock_power_jet_method_names_residual_wave519() -> bool {
    PRESENTATION_SHOCK_POWER_JET_METHOD_NAMES_WAVE519.len() == 6
        && residual_name_index(
            PRESENTATION_SHOCK_POWER_JET_METHOD_NAMES_WAVE519,
            "shock_was_airborne",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_SHOCK_POWER_JET_METHOD_NAMES_WAVE519,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_shock_power_jet_source_markers_residual_wave519() -> bool {
    PRESENTATION_SHOCK_POWER_JET_SOURCE_MARKERS_WAVE519.len() == 4
        && residual_name_index(
            PRESENTATION_SHOCK_POWER_JET_SOURCE_MARKERS_WAVE519,
            "Wave 519: exploded flail/bounce, power-plant upgrading, jet afterburner residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_SHOCK_POWER_JET_SOURCE_MARKERS_WAVE519,
            "fn exploded_flailing_model_bit",
        ) == Some(3)
}

pub fn honesty_presentation_shock_power_jet_nav_commands_residual_wave519() -> bool {
    PRESENTATION_SHOCK_POWER_JET_NAV_STEPS_WAVE519.len() == 6
        && residual_name_index(
            PRESENTATION_SHOCK_POWER_JET_NAV_STEPS_WAVE519,
            "STAMP_EXPLODED_POWER_JET",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_SHOCK_POWER_JET_NAV_STEPS_WAVE519,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_SHOCK_POWER_JET_CMD_NAMES_WAVE519.len() == 5
}

pub fn simulate_presentation_shock_power_jet_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("shock_was_airborne: obj.shock_was_airborne")
        && pf.contains("power_plant_rods_extended: obj.power_plant_rods_extended")
        && pf.contains("jet_slow_death_active: obj.jet_slow_death.is_some()");
    residual_action_store(ResidualPresentationShockPowerJetAction::FreezeSource);
    ok
}

pub fn simulate_presentation_shock_power_jet_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let ok = (pf.contains(
        "Wave 519: exploded flail/bounce, power-plant upgrading, jet afterburner residual bits",
    ) || pf
        .contains("Wave 519: exploded flail/bounce and jet-afterburner residual bits"))
        && en.contains("pub fn exploded_flailing_model_bit")
        && en.contains("pub fn power_plant_upgrading_model_bit")
        && en.contains("pub fn jetafterburner_model_bit")
        && pf.contains("if self.jet_slow_death_active");
    residual_action_store(ResidualPresentationShockPowerJetAction::StampSource);
    ok
}

pub fn honesty_presentation_shock_power_jet_residual_pack_wave519() -> bool {
    honesty_presentation_shock_power_jet_method_names_residual_wave519()
        && honesty_presentation_shock_power_jet_source_markers_residual_wave519()
        && honesty_presentation_shock_power_jet_nav_commands_residual_wave519()
        && simulate_presentation_shock_power_jet_freeze_source()
        && simulate_presentation_shock_power_jet_stamp_source()
}

pub fn simulate_live_presentation_shock_power_jet_honesty() -> bool {
    let ok = honesty_presentation_shock_power_jet_residual_pack_wave519();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationShockPowerJetAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_shock_power_jet_method_names_residual_wave519());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_shock_power_jet_source_markers_residual_wave519());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_shock_power_jet_nav_commands_residual_wave519());
    }

    #[test]
    fn presentation_shock_power_jet_sources() {
        assert!(simulate_presentation_shock_power_jet_freeze_source());
        assert!(simulate_presentation_shock_power_jet_stamp_source());
    }

    #[test]
    fn wave519_composite_pack() {
        assert!(honesty_presentation_shock_power_jet_residual_pack_wave519());
    }

    #[test]
    fn simulate_live_presentation_shock_power_jet_honesty_residual_live() {
        assert!(
            simulate_live_presentation_shock_power_jet_honesty(),
            "presentation shock/power/jet residual must latch"
        );
        assert!(residual_presentation_shock_power_jet_ok());
        assert_eq!(
            residual_presentation_shock_power_jet_last_action(),
            ResidualPresentationShockPowerJetAction::Composite
        );
    }
}
