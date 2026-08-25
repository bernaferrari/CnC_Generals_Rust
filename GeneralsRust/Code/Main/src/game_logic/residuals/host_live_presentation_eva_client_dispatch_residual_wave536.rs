//! Wave 536 residual peels: presentation `EvaAlert` pulses drive HUD/chat EVA
//! lines and GameClient `EvaMessage::set_should_play` via C++ table tokens
//! (`EVA_LOWPOWER` → `LOWPOWER`). Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 535 particle spawn audio residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_presentation_eva_alerts
//! - `host_eva_log.rs` eva_event_table_token
//! - `presentation_frame.rs` EvaAlert events
//!
//! Fail-closed:
//! - Not full C++ Eva priority / side / Miles speech matrix
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_EVA_CLIENT_DISPATCH_METHOD_NAMES_WAVE536: &[&str] = &[
    "apply_presentation_eva_alerts",
    "eva_alert_client_token",
    "eva_event_table_token",
    "simulate_eva_set_should_play_by_name",
    "PresentationEvent::EvaAlert",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_EVA_CLIENT_DISPATCH_NAV_STEPS_WAVE536: &[&str] = &[
    "REQUIRE_EVA_ALERT_CLIENT_DISPATCH",
    "REQUIRE_TABLE_TOKEN_MAP",
    "LIVE_PRESENTATION_EVA_CLIENT_DISPATCH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_EVA_CLIENT_DISPATCH_CMD_NAMES_WAVE536: &[&str] = &[
    "eva_client_dispatch",
    "apply_presentation_eva_alerts",
    "eva_table_token",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationEvaClientDispatchAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationEvaClientDispatchAction {
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

fn residual_action_store(action: ResidualPresentationEvaClientDispatchAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_eva_client_dispatch_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_eva_client_dispatch_last_action()
-> ResidualPresentationEvaClientDispatchAction {
    ResidualPresentationEvaClientDispatchAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn log_source() -> &'static str {
    include_str!("../host_eva_log.rs")
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}

pub fn honesty_presentation_eva_client_dispatch_method_names_residual_wave536() -> bool {
    let names = LIVE_PRESENTATION_EVA_CLIENT_DISPATCH_METHOD_NAMES_WAVE536;
    let ok = residual_name_index(names, "apply_presentation_eva_alerts").is_some()
        && residual_name_index(names, "eva_alert_client_token").is_some()
        && residual_name_index(names, "eva_event_table_token").is_some()
        && residual_name_index(names, "simulate_eva_set_should_play_by_name").is_some()
        && residual_name_index(names, "PresentationEvent::EvaAlert").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationEvaClientDispatchAction::MethodNames);
    ok
}

pub fn honesty_presentation_eva_client_dispatch_source_markers_residual_wave536() -> bool {
    let eng = eng_source();
    let log = log_source();
    let pf = pf_source();
    let ok = eng.contains("Wave 536")
        && eng.contains("fn apply_presentation_eva_alerts")
        && eng.contains("fn eva_alert_client_token")
        && eng.contains("simulate_eva_set_should_play_by_name")
        && eng.contains("chat_panel.add_eva_message")
        && log.contains("eva_event_table_token")
        && log.contains("\"LOWPOWER\"")
        && log.contains("\"BEACONDETECTED\"")
        && pf.contains("PresentationEvent::EvaAlert")
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPresentationEvaClientDispatchAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_eva_client_dispatch_nav_commands_residual_wave536() -> bool {
    let steps = LIVE_PRESENTATION_EVA_CLIENT_DISPATCH_NAV_STEPS_WAVE536;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_EVA_CLIENT_DISPATCH_CMD_NAMES_WAVE536;
    let ok = residual_name_index(steps, "REQUIRE_EVA_ALERT_CLIENT_DISPATCH").is_some()
        && residual_name_index(steps, "REQUIRE_TABLE_TOKEN_MAP").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_EVA_CLIENT_DISPATCH").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "eva_client_dispatch").is_some()
        && residual_name_index(cmds, "apply_presentation_eva_alerts").is_some()
        && residual_name_index(cmds, "eva_table_token").is_some();
    residual_action_store(ResidualPresentationEvaClientDispatchAction::NavCommands);
    ok
}

pub fn simulate_presentation_eva_client_dispatch_collect_source() -> bool {
    let eng = eng_source();
    let log = log_source();
    let ok = eng.contains("apply_presentation_eva_alerts")
        && log.contains("eva_event_table_token")
        && log.contains("SUPERWEAPONLAUNCHED_OWN_SNEAK_ATTACK");
    residual_action_store(ResidualPresentationEvaClientDispatchAction::CollectSource);
    ok
}

pub fn simulate_presentation_eva_client_dispatch_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("simulate_eva_set_should_play_by_name(&token)")
        && eng.contains("eva_alert_human_message")
        && eng.contains("add_eva_message");
    residual_action_store(ResidualPresentationEvaClientDispatchAction::DispatchSource);
    ok
}

pub fn honesty_presentation_eva_client_dispatch_residual_pack_wave536() -> bool {
    honesty_presentation_eva_client_dispatch_method_names_residual_wave536()
        && honesty_presentation_eva_client_dispatch_source_markers_residual_wave536()
        && honesty_presentation_eva_client_dispatch_nav_commands_residual_wave536()
        && simulate_presentation_eva_client_dispatch_collect_source()
        && simulate_presentation_eva_client_dispatch_dispatch_source()
}

pub fn simulate_live_presentation_eva_client_dispatch_honesty() -> bool {
    let ok = honesty_presentation_eva_client_dispatch_residual_pack_wave536();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEvaClientDispatchAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_eva_client_dispatch_method_names_residual_wave536());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_eva_client_dispatch_source_markers_residual_wave536());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_eva_client_dispatch_nav_commands_residual_wave536());
    }

    #[test]
    fn presentation_eva_client_dispatch_sources() {
        assert!(simulate_presentation_eva_client_dispatch_collect_source());
        assert!(simulate_presentation_eva_client_dispatch_dispatch_source());
    }

    #[test]
    fn wave536_composite_pack() {
        assert!(honesty_presentation_eva_client_dispatch_residual_pack_wave536());
    }

    #[test]
    fn simulate_live_presentation_eva_client_dispatch_honesty_residual_live() {
        assert!(
            simulate_live_presentation_eva_client_dispatch_honesty(),
            "eva client dispatch residual must latch"
        );
        assert!(residual_presentation_eva_client_dispatch_ok());
        assert_eq!(
            residual_presentation_eva_client_dispatch_last_action(),
            ResidualPresentationEvaClientDispatchAction::Composite
        );
    }
}
