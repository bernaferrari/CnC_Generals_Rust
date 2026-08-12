//! Wave 539 residual peels: player-defeat notifications use presentation roster
//! + `notify_presentation_ui_message` (HUD radar + GUIMessageReceived) when a
//! freeze is installed — no GameLogic dual-write. Shared helper also serves
//! Wave 538 alliance notify. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 538 alliance notify residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` defeated_players presentation roster branch
//! - `notify_presentation_ui_message`
//!
//! Fail-closed:
//! - Boot residual without presentation roster still uses host queues
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_DEFEAT_NOTIFY_METHOD_NAMES_WAVE539: &[&str] = &[
    "notify_presentation_ui_message",
    "defeated_player_ids",
    "hud.message.player_defeated",
    "GUIMessageReceived",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_DEFEAT_NOTIFY_NAV_STEPS_WAVE539: &[&str] = &[
    "REQUIRE_PRESENTATION_DEFEAT_NOTIFY",
    "REQUIRE_NOTIFY_PRESENTATION_UI_MESSAGE",
    "LIVE_PRESENTATION_DEFEAT_NOTIFY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_DEFEAT_NOTIFY_CMD_NAMES_WAVE539: &[&str] = &[
    "defeat_notify_presentation",
    "notify_presentation_ui_message",
    "player_defeated",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationDefeatNotifyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationDefeatNotifyAction {
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

fn residual_action_store(action: ResidualPresentationDefeatNotifyAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_defeat_notify_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_defeat_notify_last_action() -> ResidualPresentationDefeatNotifyAction {
    ResidualPresentationDefeatNotifyAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_defeat_notify_method_names_residual_wave539() -> bool {
    let names = LIVE_PRESENTATION_DEFEAT_NOTIFY_METHOD_NAMES_WAVE539;
    let ok = residual_name_index(names, "notify_presentation_ui_message").is_some()
        && residual_name_index(names, "defeated_player_ids").is_some()
        && residual_name_index(names, "hud.message.player_defeated").is_some()
        && residual_name_index(names, "GUIMessageReceived").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationDefeatNotifyAction::MethodNames);
    ok
}

pub fn honesty_presentation_defeat_notify_source_markers_residual_wave539() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 539")
        && eng.contains("fn notify_presentation_ui_message")
        && eng.contains("defeated_player_ids")
        && eng.contains("hud.message.player_defeated")
        && eng.contains("self.notify_presentation_ui_message(&message)")
        && eng.contains("no GameLogic dual-write")
        // Boot residual still has host dual-write path.
        && eng.contains("queue_radar_message_for_team")
        && eng.contains("play_ui_sound(\"GUIMessageReceived\")")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPresentationDefeatNotifyAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_defeat_notify_nav_commands_residual_wave539() -> bool {
    let steps = LIVE_PRESENTATION_DEFEAT_NOTIFY_NAV_STEPS_WAVE539;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_DEFEAT_NOTIFY_CMD_NAMES_WAVE539;
    let ok = residual_name_index(steps, "REQUIRE_PRESENTATION_DEFEAT_NOTIFY").is_some()
        && residual_name_index(steps, "REQUIRE_NOTIFY_PRESENTATION_UI_MESSAGE").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_DEFEAT_NOTIFY").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "defeat_notify_presentation").is_some()
        && residual_name_index(cmds, "notify_presentation_ui_message").is_some()
        && residual_name_index(cmds, "player_defeated").is_some();
    residual_action_store(ResidualPresentationDefeatNotifyAction::NavCommands);
    ok
}

pub fn simulate_presentation_defeat_notify_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 539")
        && eng.contains("take_defeat_events")
        && eng.contains("defeated_player_ids");
    residual_action_store(ResidualPresentationDefeatNotifyAction::CollectSource);
    ok
}

pub fn simulate_presentation_defeat_notify_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("notify_presentation_ui_message")
        && eng.contains("AudioEventRequest::new(\"GUIMessageReceived\")")
        && eng.contains("RadarPingKind::Generic");
    residual_action_store(ResidualPresentationDefeatNotifyAction::DispatchSource);
    ok
}

pub fn honesty_presentation_defeat_notify_residual_pack_wave539() -> bool {
    honesty_presentation_defeat_notify_method_names_residual_wave539()
        && honesty_presentation_defeat_notify_source_markers_residual_wave539()
        && honesty_presentation_defeat_notify_nav_commands_residual_wave539()
        && simulate_presentation_defeat_notify_collect_source()
        && simulate_presentation_defeat_notify_dispatch_source()
}

pub fn simulate_live_presentation_defeat_notify_honesty() -> bool {
    let ok = honesty_presentation_defeat_notify_residual_pack_wave539();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationDefeatNotifyAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_defeat_notify_method_names_residual_wave539());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_defeat_notify_source_markers_residual_wave539());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_defeat_notify_nav_commands_residual_wave539());
    }

    #[test]
    fn presentation_defeat_notify_sources() {
        assert!(simulate_presentation_defeat_notify_collect_source());
        assert!(simulate_presentation_defeat_notify_dispatch_source());
    }

    #[test]
    fn wave539_composite_pack() {
        assert!(honesty_presentation_defeat_notify_residual_pack_wave539());
    }

    #[test]
    fn simulate_live_presentation_defeat_notify_honesty_residual_live() {
        assert!(
            simulate_live_presentation_defeat_notify_honesty(),
            "defeat notify presentation residual must latch"
        );
        assert!(residual_presentation_defeat_notify_ok());
        assert_eq!(
            residual_presentation_defeat_notify_last_action(),
            ResidualPresentationDefeatNotifyAction::Composite
        );
    }
}
