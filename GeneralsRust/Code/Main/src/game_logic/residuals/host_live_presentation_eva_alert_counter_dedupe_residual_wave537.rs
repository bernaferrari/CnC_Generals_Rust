//! Wave 537 residual peels: presentation EvaAlert classic-four tokens advance
//! `last_eva_*` so host counter residual does not double-push chat/HUD lines.
//! EvaAlert runs before counter sync. Never flips shell `playable_claim`.
//!
//! Orthogonal to Wave 536 Eva client dispatch residual.
//! Host residual only — network deferred.
//!
//! Sources:
//! - `cnc_game_engine.rs` apply_presentation_eva_alerts last_eva_* absorb
//! - `sync_eva_messages_from_presentation` alert-first order
//!
//! Fail-closed:
//! - Not full C++ Eva queue/priority matrix
//! - Shell `playable_claim` stays false; network deferred

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE_METHOD_NAMES_WAVE537: &[&str] = &[
    "apply_presentation_eva_alerts",
    "last_eva_low_power_count",
    "last_eva_insufficient_funds_count",
    "last_eva_base_under_attack_count",
    "last_eva_ally_under_attack_count",
    "EvaAlert runs before counter sync",
    "playable_claim = false",
];

pub const LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE_NAV_STEPS_WAVE537: &[&str] = &[
    "REQUIRE_EVA_ALERT_FIRST",
    "REQUIRE_CLASSIC_FOUR_ABSORB",
    "LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

pub const RUNTIME_HOST_LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE_CMD_NAMES_WAVE537: &[&str] = &[
    "eva_alert_counter_dedupe",
    "last_eva_absorb",
    "alert_before_counters",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualPresentationEvaAlertCounterDedupeAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}

impl ResidualPresentationEvaAlertCounterDedupeAction {
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

fn residual_action_store(action: ResidualPresentationEvaAlertCounterDedupeAction) {
    RESIDUAL_ACTION.store(action as u8, Ordering::SeqCst);
}

pub fn residual_presentation_eva_alert_counter_dedupe_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}

pub fn residual_presentation_eva_alert_counter_dedupe_last_action()
-> ResidualPresentationEvaAlertCounterDedupeAction {
    ResidualPresentationEvaAlertCounterDedupeAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}

fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

pub fn honesty_presentation_eva_alert_counter_dedupe_method_names_residual_wave537() -> bool {
    let names = LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE_METHOD_NAMES_WAVE537;
    let ok = residual_name_index(names, "apply_presentation_eva_alerts").is_some()
        && residual_name_index(names, "last_eva_low_power_count").is_some()
        && residual_name_index(names, "last_eva_insufficient_funds_count").is_some()
        && residual_name_index(names, "last_eva_base_under_attack_count").is_some()
        && residual_name_index(names, "last_eva_ally_under_attack_count").is_some()
        && residual_name_index(names, "EvaAlert runs before counter sync").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualPresentationEvaAlertCounterDedupeAction::MethodNames);
    ok
}

pub fn honesty_presentation_eva_alert_counter_dedupe_source_markers_residual_wave537() -> bool {
    let eng = eng_source();
    let sync_i = eng.find("fn sync_eva_messages_from_presentation");
    let order_ok = match sync_i {
        Some(s) => {
            let window = &eng[s..eng.len().min(s + 2500)];
            let a = window.find("self.apply_presentation_eva_alerts(pres);");
            let c = window.find("self.sync_eva_messages_from_host_counts(");
            matches!((a, c), (Some(ai), Some(ci)) if ai < ci)
        }
        None => false,
    };
    let ok = eng.contains("Wave 537")
        && eng.contains("absorb classic counter deltas")
        && eng.contains("last_eva_low_power_count")
        && eng.contains("\"LOWPOWER\"")
        && eng.contains("\"INSUFFICIENTFUNDS\"")
        && eng.contains("\"BASEUNDERATTACK\"")
        && eng.contains("\"ALLYUNDERATTACK\"")
        && order_ok
        && !eng.contains("playable_claim = true");
    residual_action_store(ResidualPresentationEvaAlertCounterDedupeAction::SourceMarkers);
    ok
}

pub fn honesty_presentation_eva_alert_counter_dedupe_nav_commands_residual_wave537() -> bool {
    let steps = LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE_NAV_STEPS_WAVE537;
    let cmds = RUNTIME_HOST_LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE_CMD_NAMES_WAVE537;
    let ok = residual_name_index(steps, "REQUIRE_EVA_ALERT_FIRST").is_some()
        && residual_name_index(steps, "REQUIRE_CLASSIC_FOUR_ABSORB").is_some()
        && residual_name_index(steps, "LIVE_PRESENTATION_EVA_ALERT_COUNTER_DEDUPE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "eva_alert_counter_dedupe").is_some()
        && residual_name_index(cmds, "last_eva_absorb").is_some()
        && residual_name_index(cmds, "alert_before_counters").is_some();
    residual_action_store(ResidualPresentationEvaAlertCounterDedupeAction::NavCommands);
    ok
}

pub fn simulate_presentation_eva_alert_counter_dedupe_collect_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("Wave 537")
        && eng.contains("apply_presentation_eva_alerts")
        && eng.contains("sync_eva_messages_from_host_counts");
    residual_action_store(ResidualPresentationEvaAlertCounterDedupeAction::CollectSource);
    ok
}

pub fn simulate_presentation_eva_alert_counter_dedupe_dispatch_source() -> bool {
    let eng = eng_source();
    let ok = eng.contains("last_eva_low_power_count.max(pres.eva_low_power_count)")
        && eng.contains("pres.eva_insufficient_funds_count")
        && eng.contains("pres.eva_base_under_attack_count")
        && eng.contains("pres.eva_ally_under_attack_count")
        && eng.contains("absorb classic counter deltas");
    residual_action_store(ResidualPresentationEvaAlertCounterDedupeAction::DispatchSource);
    ok
}

pub fn honesty_presentation_eva_alert_counter_dedupe_residual_pack_wave537() -> bool {
    honesty_presentation_eva_alert_counter_dedupe_method_names_residual_wave537()
        && honesty_presentation_eva_alert_counter_dedupe_source_markers_residual_wave537()
        && honesty_presentation_eva_alert_counter_dedupe_nav_commands_residual_wave537()
        && simulate_presentation_eva_alert_counter_dedupe_collect_source()
        && simulate_presentation_eva_alert_counter_dedupe_dispatch_source()
}

pub fn simulate_live_presentation_eva_alert_counter_dedupe_honesty() -> bool {
    let ok = honesty_presentation_eva_alert_counter_dedupe_residual_pack_wave537();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualPresentationEvaAlertCounterDedupeAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_presentation_eva_alert_counter_dedupe_method_names_residual_wave537());
    }

    #[test]
    fn source_markers_residual() {
        assert!(honesty_presentation_eva_alert_counter_dedupe_source_markers_residual_wave537());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_presentation_eva_alert_counter_dedupe_nav_commands_residual_wave537());
    }

    #[test]
    fn presentation_eva_alert_counter_dedupe_sources() {
        assert!(simulate_presentation_eva_alert_counter_dedupe_collect_source());
        assert!(simulate_presentation_eva_alert_counter_dedupe_dispatch_source());
    }

    #[test]
    fn wave537_composite_pack() {
        assert!(honesty_presentation_eva_alert_counter_dedupe_residual_pack_wave537());
    }

    #[test]
    fn simulate_live_presentation_eva_alert_counter_dedupe_honesty_residual_live() {
        assert!(
            simulate_live_presentation_eva_alert_counter_dedupe_honesty(),
            "eva alert counter dedupe residual must latch"
        );
        assert!(residual_presentation_eva_alert_counter_dedupe_ok());
        assert_eq!(
            residual_presentation_eva_alert_counter_dedupe_last_action(),
            ResidualPresentationEvaAlertCounterDedupeAction::Composite
        );
    }
}
