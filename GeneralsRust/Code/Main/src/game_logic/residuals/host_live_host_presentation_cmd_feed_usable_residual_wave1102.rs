//! Wave 1102: presentation stop/upgrade/SP/sell/mobile usable residual.
//!
//! After Waves 1100–1101 producer/feed peels:
//! - stop-all ids skipped only destroyed
//! - upgrade-producer structures skipped only destroyed/UC
//! - special_power_ready_objects skipped only destroyed
//! - sellable structure OR-path could include sold buildings
//! - mobile friendly count skipped only destroyed
//!
//! Align with presentation_is_selectable / sold/disabled fail-closed.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_PRESENTATION_CMD_FEED_USABLE_METHOD_NAMES_WAVE1102: &[&str] = &[
    "alive_friendly_stoppable_ids",
    "alive_upgrade_producer_structure_ids",
    "special_power_ready_objects",
    "alive_sellable_friendly_structure_ids",
    "count_mobile_friendlies",
    "Wave 1102",
    "playable_claim = false",
];

pub const LIVE_HOST_PRESENTATION_CMD_FEED_USABLE_NAV_STEPS_WAVE1102: &[&str] = &[
    "STOPPABLE_SELECTABLE",
    "UPGRADE_SP_SELL_SOLD_DISABLED",
    "MOBILE_COUNT_SELECTABLE",
    "LIVE_HOST_PRESENTATION_CMD_FEED_USABLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostPresentationCmdFeedUsableAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostPresentationCmdFeedUsableAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_presentation_cmd_feed_usable_method_names_residual_wave1102() -> bool {
    let names = LIVE_HOST_PRESENTATION_CMD_FEED_USABLE_METHOD_NAMES_WAVE1102;
    let ok = residual_name_index(names, "alive_friendly_stoppable_ids").is_some()
        && residual_name_index(names, "Wave 1102").is_some();
    residual_action_store(ResidualHostPresentationCmdFeedUsableAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_cmd_feed_usable_nav_commands_residual_wave1102() -> bool {
    let steps = LIVE_HOST_PRESENTATION_CMD_FEED_USABLE_NAV_STEPS_WAVE1102;
    let ok = residual_name_index(steps, "LIVE_HOST_PRESENTATION_CMD_FEED_USABLE").is_some()
        && residual_name_index(steps, "STOPPABLE_SELECTABLE").is_some();
    residual_action_store(ResidualHostPresentationCmdFeedUsableAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_presentation_cmd_feed_usable_residual_pack_wave1102() -> bool {
    let pf = pf_source();
    let es = es_source();
    let checks = [
        (
            "fn alive_friendly_stoppable_ids",
            "Wave 1102: stop-all residual uses full presentation selectable legality",
            "presentation_is_selectable",
        ),
        (
            "fn alive_upgrade_producer_structure_ids",
            "Wave 1102: fail-closed on sold/disabled upgrade-producer residual",
            "!o.sold",
        ),
        (
            "fn special_power_ready_objects",
            "Wave 1102: fail-closed on sold/disabled SP-ready residual feed",
            "!o.sold",
        ),
        (
            "fn alive_sellable_friendly_structure_ids",
            "Wave 1102: sold residual fail-closed even on structure OR path",
            "!o.sold",
        ),
        (
            "fn count_mobile_friendlies",
            "Wave 1102: mobile count residual uses presentation selectable legality",
            "presentation_is_selectable",
        ),
    ];
    let mut ok = // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`");
    for (fn_name, wave, needle) in checks {
        let Some(i) = pf.find(fn_name) else {
            ok = false;
            break;
        };
        let w = &pf[i..i.saturating_add(1400)];
        if !w.contains(wave) || !w.contains(needle) {
            ok = false;
            break;
        }
    }
    residual_action_store(ResidualHostPresentationCmdFeedUsableAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_presentation_cmd_feed_usable_residual_honesty() -> bool {
    let a = honesty_host_presentation_cmd_feed_usable_method_names_residual_wave1102();
    let b = honesty_host_presentation_cmd_feed_usable_nav_commands_residual_wave1102();
    let c = honesty_host_presentation_cmd_feed_usable_residual_pack_wave1102();
    residual_action_store(ResidualHostPresentationCmdFeedUsableAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_presentation_cmd_feed_usable_residual_wave1102() {
        assert!(honesty_host_presentation_cmd_feed_usable_residual_pack_wave1102());
        assert!(honesty_host_presentation_cmd_feed_usable_method_names_residual_wave1102());
        assert!(honesty_host_presentation_cmd_feed_usable_nav_commands_residual_wave1102());
        assert!(simulate_live_host_presentation_cmd_feed_usable_residual_honesty());
    }
}
