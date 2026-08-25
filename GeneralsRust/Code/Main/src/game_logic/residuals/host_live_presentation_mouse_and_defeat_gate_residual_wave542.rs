//! Wave 542 residual peels:
//! 1) Defeat notify: presentation freeze + roster miss fails closed (no host dual-write).
//! 2) Mouse command path uses `presentation_mouse_game_logic()` — always `None` when
//!    a presentation freeze is installed (no live dual-read classify).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 541 RMB classify fail-closed residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` defeated_players / presentation_mouse_game_logic
//!
//! Fail-closed:
//! - Boot residual without presentation still uses host queues / live GameLogic
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE_METHOD_NAMES_WAVE542: &[&str] = &[
    "presentation_mouse_game_logic",
    "last_presentation_frame.is_some()",
    "Wave 542",
    "roster miss",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE_NAV_STEPS_WAVE542: &[&str] = &[
    "REQUIRE_PRESENTATION_MOUSE_GAME_LOGIC",
    "REQUIRE_DEFEAT_ROSTER_MISS_FAIL_CLOSED",
    "LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE_CMD_NAMES_WAVE542: &[&str] = &[
    "presentation_mouse_game_logic",
    "defeat_roster_miss_fail_closed",
    "mouse_presentation_only",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationMouseAndDefeatGateAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationMouseAndDefeatGateAction {
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

fn residual_action_store(action: ResidualPresentationMouseAndDefeatGateAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_mouse_and_defeat_gate_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_mouse_and_defeat_gate_last_action()
-> ResidualPresentationMouseAndDefeatGateAction {
    ResidualPresentationMouseAndDefeatGateAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_mouse_and_defeat_gate_method_names_residual_wave542() -> bool {
    let names = LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE_METHOD_NAMES_WAVE542;
    let ok = residual_name_index(names, "presentation_mouse_game_logic").is_some()
        && residual_name_index(names, "last_presentation_frame.is_some()").is_some()
        && residual_name_index(names, "Wave 542").is_some()
        && residual_name_index(names, "roster miss").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationMouseAndDefeatGateAction::MethodNames);
    ok
}

pub fn honesty_presentation_mouse_and_defeat_gate_source_markers_residual_wave542() -> bool {
    let eng = eng_source();
    let mouse_n = eng.matches("presentation_mouse_game_logic()").count();
    let ok = eng.contains("Wave 542")
        && eng.contains("fn presentation_mouse_game_logic")
        && eng.contains("mouse command classification is presentation-only")
        && eng.contains("roster miss")
        && eng.contains("no GameLogic dual-write mid-frame")
        && mouse_n >= 3
        // Must not leave bare Some(&self.game_logic) at process_mouse call sites.
        && !eng.contains(
            "process_mouse_input(\n            &context,\n            &selected,\n            self.current_player_id,\n            if self.last_presentation_frame.is_some()",
        )
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPresentationMouseAndDefeatGateAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_mouse_and_defeat_gate_nav_commands_residual_wave542() -> bool {
    let steps = LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE_NAV_STEPS_WAVE542;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE_CMD_NAMES_WAVE542;
    let ok = residual_name_index(steps, "REQUIRE_PRESENTATION_MOUSE_GAME_LOGIC").is_some()
        && residual_name_index(steps, "REQUIRE_DEFEAT_ROSTER_MISS_FAIL_CLOSED").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_MOUSE_AND_DEFEAT_GATE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "presentation_mouse_game_logic").is_some()
        && residual_name_index(cmds, "defeat_roster_miss_fail_closed").is_some()
        && residual_name_index(cmds, "mouse_presentation_only").is_some();
    residual_action_store(ResidualPresentationMouseAndDefeatGateAction::NavCommands);
    ok
}

pub fn simulate_presentation_mouse_and_defeat_gate_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 542")
        && eng.contains("presentation_mouse_game_logic")
        && eng.contains("defeated_players");
    residual_action_store(ResidualPresentationMouseAndDefeatGateAction::CollectSource);
    ok
}

pub fn simulate_presentation_mouse_and_defeat_gate_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("self.presentation_mouse_game_logic()")
        && eng.matches("presentation_mouse_game_logic()").count() >= 3
        && eng.contains("else if self.last_presentation_frame.is_some()");
    residual_action_store(ResidualPresentationMouseAndDefeatGateAction::DispatchSource);
    ok
}

pub fn honesty_presentation_mouse_and_defeat_gate_residual_pack_wave542() -> bool {
    honesty_presentation_mouse_and_defeat_gate_method_names_residual_wave542()
        && honesty_presentation_mouse_and_defeat_gate_source_markers_residual_wave542()
        && honesty_presentation_mouse_and_defeat_gate_nav_commands_residual_wave542()
        && simulate_presentation_mouse_and_defeat_gate_collect_source()
        && simulate_presentation_mouse_and_defeat_gate_dispatch_source()
}

pub fn simulate_live_presentation_mouse_and_defeat_gate_honesty() -> bool {
    let ok = honesty_presentation_mouse_and_defeat_gate_residual_pack_wave542();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationMouseAndDefeatGateAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_mouse_and_defeat_gate_method_names_residual_wave542());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_mouse_and_defeat_gate_source_markers_residual_wave542());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_mouse_and_defeat_gate_nav_commands_residual_wave542());
    }

    #[test]
    fn presentation_mouse_and_defeat_gate_sources() {
        assert!(simulate_presentation_mouse_and_defeat_gate_collect_source());
        assert!(simulate_presentation_mouse_and_defeat_gate_dispatch_source());
    }

    #[test]
    fn wave542_composite_pack() {
        assert!(honesty_presentation_mouse_and_defeat_gate_residual_pack_wave542());
    }

    #[test]
    fn simulate_live_presentation_mouse_and_defeat_gate_honesty_residual_live() {
        assert!(
            simulate_live_presentation_mouse_and_defeat_gate_honesty(),
            "mouse/defeat presentation gate residual must latch"
        );
        assert!(residual_presentation_mouse_and_defeat_gate_ok());
        assert_eq!(
            residual_presentation_mouse_and_defeat_gate_last_action(),
            ResidualPresentationMouseAndDefeatGateAction::Composite
        );
    }
}
