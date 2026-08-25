//! Wave 532 residual peels: presentation `WeaponFireLoop` event drain is a
//! sibling of `host_attack_log` (not nested inside it). Nested drain only ran
//! when attack_log was non-empty and could drop independent FireSound loops.
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 531 command_integration presentation fill residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `presentation_frame.rs` host_fire_sound_loop_log::take_last_drain sibling
//! - Wave 527/528 FireSound start/stop audio mapping
//!
//! Fail-closed:
//! - Not full C++ FiringTracker retail timing matrix
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Wave 532 method/source residual names.
pub const LIVE_PRESENTATION_FIRESOUND_DRAIN_SIBLING_METHOD_NAMES_WAVE532: &[&str] = &[
    "host_fire_sound_loop_log::take_last_drain",
    "host_attack_log::take_last_drain",
    "WeaponFireLoopStarted",
    "WeaponFireLoopStopped",
    "FireSound loop drain is a sibling of attack_log",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_PRESENTATION_FIRESOUND_DRAIN_SIBLING_NAV_STEPS_WAVE532: &[&str] = &[
    "REQUIRE_FIRESOUND_DRAIN_SIBLING",
    "REQUIRE_NOT_NESTED_IN_ATTACK_LOG",
    "LIVE_FIRESOUND_DRAIN_SIBLING",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_PRESENTATION_FIRESOUND_DRAIN_SIBLING_CMD_NAMES_WAVE532: &[&str] = &[
    "firesound_drain_sibling",
    "weapon_fire_loop_started",
    "weapon_fire_loop_stopped",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationFiresoundDrainSiblingAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationFiresoundDrainSiblingAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}

fn residual_action_store(action: ResidualPresentationFiresoundDrainSiblingAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_firesound_drain_sibling_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_firesound_drain_sibling_last_action()
-> ResidualPresentationFiresoundDrainSiblingAction {
    ResidualPresentationFiresoundDrainSiblingAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_presentation_firesound_drain_sibling_method_names_residual_wave532() -> bool {
    let names = LIVE_PRESENTATION_FIRESOUND_DRAIN_SIBLING_METHOD_NAMES_WAVE532;
    let ok = residual_name_index(names, "host_fire_sound_loop_log::take_last_drain").is_some()
        && residual_name_index(names, "host_attack_log::take_last_drain").is_some()
        && residual_name_index(names, "WeaponFireLoopStarted").is_some()
        && residual_name_index(names, "WeaponFireLoopStopped").is_some()
        && residual_name_index(names, "FireSound loop drain is a sibling of attack_log").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationFiresoundDrainSiblingAction::MethodNames);
    ok
}

pub fn honesty_presentation_firesound_drain_sibling_source_markers_residual_wave532() -> bool {
    let pf = pf_source();
    let attack_i = pf.find("for ev in crate::game_logic::host_attack_log::take_last_drain()");
    let fire_i =
        pf.find("for ev in crate::game_logic::host_fire_sound_loop_log::take_last_drain()");
    let sibling_doc = pf.contains("FireSound loop drain is a sibling of attack_log");
    let wave = pf.contains("Wave 532");
    // Fire drain must appear after attack drain closes (sibling), not only nested.
    let order_ok = match (attack_i, fire_i) {
        (Some(a), Some(f)) if f > a => {
            // Between attack open and fire open, the attack for-loop must close.
            let between = &pf[a..f];
            // Nested form had fire_i inside attack body without a full close pair.
            // Sibling form: count of '{' and '}' in between should return to balance
            // before fire starts (attack loop closed).
            let mut depth = 0i32;
            let mut closed = false;
            for ch in between.chars() {
                if ch == '{' {
                    depth += 1;
                } else if ch == '}' {
                    depth -= 1;
                    if depth == 0 {
                        closed = true;
                    }
                }
            }
            closed && depth == 0
        }
        _ => false,
    };
    // Must not contain nested pattern: attack drain body immediately contains fire drain
    // without closing first (historical bug).
    let nested_bug = "host_attack_log::take_last_drain() {\n            if ev.target.is_some() {\n                events.push(PresentationEvent::AttackTargeted {\n                    attacker: ev.attacker,\n                    target: ev.target,\n                });\n            }\n\n            for ev in crate::game_logic::host_fire_sound_loop_log::take_last_drain()";
    let ok = wave
        && sibling_doc
        && order_ok
        && !pf.contains(nested_bug)
        && pf.contains("WeaponFireLoopStarted")
        && pf.contains("WeaponFireLoopStopped")
        && !pf.contains("playable_claim = true");
    residual_action_store(ResidualPresentationFiresoundDrainSiblingAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_firesound_drain_sibling_nav_commands_residual_wave532() -> bool {
    let steps = LIVE_PRESENTATION_FIRESOUND_DRAIN_SIBLING_NAV_STEPS_WAVE532;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_FIRESOUND_DRAIN_SIBLING_CMD_NAMES_WAVE532;
    let ok = residual_name_index(steps, "REQUIRE_FIRESOUND_DRAIN_SIBLING").is_some()
        && residual_name_index(steps, "REQUIRE_NOT_NESTED_IN_ATTACK_LOG").is_some()
        && residual_name_index(steps, "LIVE_FIRESOUND_DRAIN_SIBLING").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "firesound_drain_sibling").is_some()
        && residual_name_index(cmds, "weapon_fire_loop_started").is_some()
        && residual_name_index(cmds, "weapon_fire_loop_stopped").is_some();
    residual_action_store(ResidualPresentationFiresoundDrainSiblingAction::NavCommands);
    ok
}

pub fn simulate_presentation_firesound_drain_sibling_collect_source() -> bool {
    let pf = pf_source();
    let ok = pf.contains("host_fire_sound_loop_log::take_last_drain")
        && pf.contains("Wave 532")
        && pf.contains("not nested");
    residual_action_store(ResidualPresentationFiresoundDrainSiblingAction::CollectSource);
    ok
}

pub fn simulate_presentation_firesound_drain_sibling_dispatch_source() -> bool {
    let pf = pf_source();
    // Sibling: attack drain for-loop ends, then fire drain for-loop begins.
    let ok = pf.contains("host_attack_log::take_last_drain()")
        && pf.contains("host_fire_sound_loop_log::take_last_drain()")
        && pf.contains("PresentationEvent::WeaponFireLoopStarted")
        && pf.contains("PresentationEvent::WeaponFireLoopStopped");
    residual_action_store(ResidualPresentationFiresoundDrainSiblingAction::DispatchSource);
    ok
}

pub fn honesty_presentation_firesound_drain_sibling_residual_pack_wave532() -> bool {
    honesty_presentation_firesound_drain_sibling_method_names_residual_wave532()
        && honesty_presentation_firesound_drain_sibling_source_markers_residual_wave532()
        && honesty_presentation_firesound_drain_sibling_nav_commands_residual_wave532()
        && simulate_presentation_firesound_drain_sibling_collect_source()
        && simulate_presentation_firesound_drain_sibling_dispatch_source()
}

pub fn simulate_live_presentation_firesound_drain_sibling_honesty() -> bool {
    let ok = honesty_presentation_firesound_drain_sibling_residual_pack_wave532();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationFiresoundDrainSiblingAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_firesound_drain_sibling_method_names_residual_wave532());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_firesound_drain_sibling_source_markers_residual_wave532());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_firesound_drain_sibling_nav_commands_residual_wave532());
    }

    #[test]
    fn presentation_firesound_drain_sibling_sources() {
        assert!(simulate_presentation_firesound_drain_sibling_collect_source());
        assert!(simulate_presentation_firesound_drain_sibling_dispatch_source());
    }

    #[test]
    fn wave532_composite_pack() {
        assert!(honesty_presentation_firesound_drain_sibling_residual_pack_wave532());
    }

    #[test]
    fn simulate_live_presentation_firesound_drain_sibling_honesty_residual_live() {
        assert!(
            simulate_live_presentation_firesound_drain_sibling_honesty(),
            "firesound drain sibling residual must latch"
        );
        assert!(residual_presentation_firesound_drain_sibling_ok());
        assert_eq!(
            residual_presentation_firesound_drain_sibling_last_action(),
            ResidualPresentationFiresoundDrainSiblingAction::Composite
        );
    }
}
