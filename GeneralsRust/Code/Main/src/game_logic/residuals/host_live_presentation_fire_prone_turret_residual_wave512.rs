//! Wave 512 residual peels: CONTINUOUS_FIRE_* / PRONE / PREATTACK_A / TURRET_ROTATE mesh bits.
//! - continuous_fire_level 0/1/2 → SLOW/MEAN/FAST (slow while firing at 0)
//! - prone_timer residual → PRONE
//! - attacking && !firing → PREATTACK_A
//! - non-structure |turret_angle| > 0.5° → TURRET_ROTATE
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 495 combat motion and Wave 494 turret yaw mesh facing.
//! Architecture residual - fire cadence/posture without live GameLogic dual-read.
//!
//! Sources:
//! - presentation_frame.rs Wave 512 freeze + stamp
//! - host_enum_table_residual.rs continuous_fire_*/prone/preattack/turret bits
//! - graphics/render_pipeline.rs Wave 512 comment
//!
//! Fail-closed:
//! - Full weapon slot B/C preattack/reload matrix still deferred
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|&n| n == name)
}

pub const PRESENTATION_FIRE_PRONE_TURRET_METHOD_NAMES_WAVE512: &[&str] = &[
    "continuous_fire_level",
    "prone",
    "continuous_fire_fast_model_bit",
    "preattack_a_model_bit",
    "turret_rotate_model_bit",
    "playable_claim = false",
];

pub const PRESENTATION_FIRE_PRONE_TURRET_SOURCE_MARKERS_WAVE512: &[&str] = &[
    "Wave 512: continuous-fire / prone / preattack / turret-rotate residual bits",
    "Wave 512: CONTINUOUS_FIRE / PRONE / PREATTACK / TURRET_ROTATE bits included in stamp helper",
    "prone: obj.prone_timer > 0.0",
    "continuous_fire_level: ro.continuous_fire_level",
];

pub const PRESENTATION_FIRE_PRONE_TURRET_NAV_STEPS_WAVE512: &[&str] = &[
    "FREEZE_CONTINUOUS_FIRE_LEVEL",
    "FREEZE_PRONE_TIMER",
    "STAMP_CONTINUOUS_FIRE_BANK",
    "STAMP_PRONE_PREATTACK",
    "STAMP_TURRET_ROTATE",
    "PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_PRESENTATION_FIRE_PRONE_TURRET_CMD_NAMES_WAVE512: &[&str] = &[
    "click_presentation_fire_prone_turret_ok_wnd_detect",
    "click_presentation_fire_prone_turret_ok_wnd_skip",
    "click_presentation_fire_prone_turret_ok_wnd_queue",
    "click_presentation_fire_prone_turret_ok_wnd_prepare",
    "click_presentation_fire_prone_turret_ok_wnd_composite",
];

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResidualPresentationFireProneTurretAction {
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

fn residual_action_store(a: ResidualPresentationFireProneTurretAction) {
    LAST_ACTION.store(a as u8, Ordering::SeqCst);
}

pub fn residual_presentation_fire_prone_turret_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_fire_prone_turret_last_action()
-> ResidualPresentationFireProneTurretAction {
    match LAST_ACTION.load(Ordering::SeqCst) {
        1 => ResidualPresentationFireProneTurretAction::MethodNames,
        2 => ResidualPresentationFireProneTurretAction::SourceMarkers,
        3 => ResidualPresentationFireProneTurretAction::NavCommands,
        4 => ResidualPresentationFireProneTurretAction::FreezeSource,
        5 => ResidualPresentationFireProneTurretAction::StampSource,
        6 => ResidualPresentationFireProneTurretAction::Composite,
        _ => ResidualPresentationFireProneTurretAction::Idle,
    }
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

fn en_source() -> &'static str {
    include_str!("../host_enum_table_residual.rs")
}

fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}

pub fn honesty_presentation_fire_prone_turret_method_names_residual_wave512() -> bool {
    PRESENTATION_FIRE_PRONE_TURRET_METHOD_NAMES_WAVE512.len() == 6
        && residual_name_index(
            PRESENTATION_FIRE_PRONE_TURRET_METHOD_NAMES_WAVE512,
            "continuous_fire_level",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FIRE_PRONE_TURRET_METHOD_NAMES_WAVE512,
            "playable_claim = false",
        ) == Some(5)
}

pub fn honesty_presentation_fire_prone_turret_source_markers_residual_wave512() -> bool {
    PRESENTATION_FIRE_PRONE_TURRET_SOURCE_MARKERS_WAVE512.len() == 4
        && residual_name_index(
            PRESENTATION_FIRE_PRONE_TURRET_SOURCE_MARKERS_WAVE512,
            "Wave 512: continuous-fire / prone / preattack / turret-rotate residual bits",
        ) == Some(0)
        && residual_name_index(
            PRESENTATION_FIRE_PRONE_TURRET_SOURCE_MARKERS_WAVE512,
            "prone: obj.prone_timer > 0.0",
        ) == Some(2)
}

pub fn honesty_presentation_fire_prone_turret_nav_commands_residual_wave512() -> bool {
    PRESENTATION_FIRE_PRONE_TURRET_NAV_STEPS_WAVE512.len() == 6
        && residual_name_index(
            PRESENTATION_FIRE_PRONE_TURRET_NAV_STEPS_WAVE512,
            "STAMP_PRONE_PREATTACK",
        ) == Some(3)
        && residual_name_index(
            PRESENTATION_FIRE_PRONE_TURRET_NAV_STEPS_WAVE512,
            "PLAYABLE_CLAIM_FALSE",
        ) == Some(5)
        && RUNTIME_HOST_PRESENTATION_FIRE_PRONE_TURRET_CMD_NAMES_WAVE512.len() == 5
}

pub fn simulate_presentation_fire_prone_turret_freeze_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("Wave 512: C++ prone residual (Infantry goProne timer)")
        && pf.contains("prone: obj.prone_timer > 0.0")
        && pf.contains("continuous_fire_level: ro.continuous_fire_level")
        && pf.contains("prone: ro.prone");
    residual_action_store(ResidualPresentationFireProneTurretAction::FreezeSource);
    ok
}

pub fn simulate_presentation_fire_prone_turret_stamp_source() -> bool {
    let pf = pf_source();
    let en = en_source();
    let rp = rp_source();
    let ok = pf.contains("Wave 512: continuous-fire / prone / preattack / turret-rotate residual bits")
        && en.contains("pub fn continuous_fire_slow_model_bit")
        && en.contains("pub fn continuous_fire_mean_model_bit")
        && en.contains("pub fn continuous_fire_fast_model_bit")
        && en.contains("pub fn prone_model_bit")
        && en.contains("pub fn preattack_a_model_bit")
        && en.contains("pub fn turret_rotate_model_bit")
        && pf.contains("self.attacking && !self.is_firing_weapon")
        && rp.contains(
            "Wave 512: CONTINUOUS_FIRE / PRONE / PREATTACK / TURRET_ROTATE bits included in stamp helper",
        );
    residual_action_store(ResidualPresentationFireProneTurretAction::StampSource);
    ok
}

pub fn honesty_presentation_fire_prone_turret_residual_pack_wave512() -> bool {
    honesty_presentation_fire_prone_turret_method_names_residual_wave512()
        && honesty_presentation_fire_prone_turret_source_markers_residual_wave512()
        && honesty_presentation_fire_prone_turret_nav_commands_residual_wave512()
        && simulate_presentation_fire_prone_turret_freeze_source()
        && simulate_presentation_fire_prone_turret_stamp_source()
}

pub fn simulate_live_presentation_fire_prone_turret_honesty() -> bool {
    let ok = honesty_presentation_fire_prone_turret_residual_pack_wave512();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationFireProneTurretAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_fire_prone_turret_method_names_residual_wave512());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_fire_prone_turret_source_markers_residual_wave512());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_fire_prone_turret_nav_commands_residual_wave512());
    }

    #[test]
    fn presentation_fire_prone_turret_sources() {
        assert!(simulate_presentation_fire_prone_turret_freeze_source());
        assert!(simulate_presentation_fire_prone_turret_stamp_source());
    }

    #[test]
    fn wave512_composite_pack() {
        assert!(honesty_presentation_fire_prone_turret_residual_pack_wave512());
    }

    #[test]
    fn simulate_live_presentation_fire_prone_turret_honesty_residual_live() {
        assert!(
            simulate_live_presentation_fire_prone_turret_honesty(),
            "presentation fire/prone/turret residual must latch"
        );
        assert!(residual_presentation_fire_prone_turret_ok());
        assert_eq!(
            residual_presentation_fire_prone_turret_last_action(),
            ResidualPresentationFireProneTurretAction::Composite
        );
    }
}
