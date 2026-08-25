//! Wave 538 residual peels: alliance victory/defeat notifications route
//! radar + UI SFX through HUD/audio subsystem when a presentation freeze is
//! installed (no GameLogic `queue_radar_message` / `play_ui_sound` dual-write).
//! Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 537 EvaAlert counter dedupe residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` alliance_events presentation branch
//! - `AudioEventRequest` GUIMessageReceived
//! - `GameHUD::add_radar_message`
//!
//! Fail-closed:
//! - Boot/menu without presentation still uses host GameLogic queues
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_ALLIANCE_NOTIFY_METHOD_NAMES_WAVE538: &[&str] = &[
    "last_presentation_frame.is_some()",
    "add_radar_message",
    "GUIMessageReceived",
    "queue_radar_message_for_team",
    "play_ui_sound",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_ALLIANCE_NOTIFY_NAV_STEPS_WAVE538: &[&str] = &[
    "REQUIRE_PRESENTATION_ALLIANCE_NOTIFY",
    "REQUIRE_NO_LOGIC_RADAR_DUAL_WRITE",
    "LIVE_PRESENTATION_ALLIANCE_NOTIFY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_ALLIANCE_NOTIFY_CMD_NAMES_WAVE538: &[&str] = &[
    "alliance_notify_presentation",
    "gui_message_received",
    "radar_hud_only",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationAllianceNotifyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationAllianceNotifyAction {
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

fn residual_action_store(action: ResidualPresentationAllianceNotifyAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_alliance_notify_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_alliance_notify_last_action()
-> ResidualPresentationAllianceNotifyAction {
    ResidualPresentationAllianceNotifyAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}

pub fn honesty_presentation_alliance_notify_method_names_residual_wave538() -> bool {
    let names = LIVE_PRESENTATION_ALLIANCE_NOTIFY_METHOD_NAMES_WAVE538;
    let ok = residual_name_index(names, "last_presentation_frame.is_some()").is_some()
        && residual_name_index(names, "add_radar_message").is_some()
        && residual_name_index(names, "GUIMessageReceived").is_some()
        && residual_name_index(names, "queue_radar_message_for_team").is_some()
        && residual_name_index(names, "play_ui_sound").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationAllianceNotifyAction::MethodNames);
    ok
}

pub fn honesty_presentation_alliance_notify_source_markers_residual_wave538() -> bool {
    let eng = eng_source();
    // Presentation branch must own radar/SFX when frame installed.
    let ok = eng.contains("Wave 538")
        && eng.contains("no GameLogic dual-write mid-frame")
        && eng.contains("if self.last_presentation_frame.is_some()")
        && eng.contains("notify_presentation_ui_message")
        && eng.contains("GUIMessageReceived")
        && eng.contains("AudioEventRequest::new(\"GUIMessageReceived\")")
        && eng.contains("notify_boot_ui_message")
        && !eng.contains("self.game_logic.queue_radar_message_for_team")
        && !eng.contains("self.game_logic.play_ui_sound(\"GUIMessageReceived\")")
        && eng.contains("} else {")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPresentationAllianceNotifyAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_alliance_notify_nav_commands_residual_wave538() -> bool {
    let steps = LIVE_PRESENTATION_ALLIANCE_NOTIFY_NAV_STEPS_WAVE538;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_ALLIANCE_NOTIFY_CMD_NAMES_WAVE538;
    let ok = residual_name_index(steps, "REQUIRE_PRESENTATION_ALLIANCE_NOTIFY").is_some()
        && residual_name_index(steps, "REQUIRE_NO_LOGIC_RADAR_DUAL_WRITE").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_ALLIANCE_NOTIFY").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "alliance_notify_presentation").is_some()
        && residual_name_index(cmds, "gui_message_received").is_some()
        && residual_name_index(cmds, "radar_hud_only").is_some();
    residual_action_store(ResidualPresentationAllianceNotifyAction::NavCommands);
    ok
}

pub fn simulate_presentation_alliance_notify_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 538")
        && eng.contains("alliance_events")
        && eng.contains("AlliedVictory");
    residual_action_store(ResidualPresentationAllianceNotifyAction::CollectSource);
    ok
}

pub fn simulate_presentation_alliance_notify_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("notify_presentation_ui_message")
        && eng.contains("AudioManagerSubsystem")
        && eng.contains("audio.queue_event(req)")
        && eng.contains("RadarPingKind::Generic");
    residual_action_store(ResidualPresentationAllianceNotifyAction::DispatchSource);
    ok
}

pub fn honesty_presentation_alliance_notify_residual_pack_wave538() -> bool {
    honesty_presentation_alliance_notify_method_names_residual_wave538()
        && honesty_presentation_alliance_notify_source_markers_residual_wave538()
        && honesty_presentation_alliance_notify_nav_commands_residual_wave538()
        && simulate_presentation_alliance_notify_collect_source()
        && simulate_presentation_alliance_notify_dispatch_source()
}

pub fn simulate_live_presentation_alliance_notify_honesty() -> bool {
    let ok = honesty_presentation_alliance_notify_residual_pack_wave538();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationAllianceNotifyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_alliance_notify_method_names_residual_wave538());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_alliance_notify_source_markers_residual_wave538());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_alliance_notify_nav_commands_residual_wave538());
    }

    #[test]
    fn presentation_alliance_notify_sources() {
        assert!(simulate_presentation_alliance_notify_collect_source());
        assert!(simulate_presentation_alliance_notify_dispatch_source());
    }

    #[test]
    fn wave538_composite_pack() {
        assert!(honesty_presentation_alliance_notify_residual_pack_wave538());
    }

    #[test]
    fn simulate_live_presentation_alliance_notify_honesty_residual_live() {
        assert!(
            simulate_live_presentation_alliance_notify_honesty(),
            "alliance notify presentation residual must latch"
        );
        assert!(residual_presentation_alliance_notify_ok());
        assert_eq!(
            residual_presentation_alliance_notify_last_action(),
            ResidualPresentationAllianceNotifyAction::Composite
        );
    }
}
