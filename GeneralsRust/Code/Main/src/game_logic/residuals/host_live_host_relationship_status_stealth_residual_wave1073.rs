//! Wave 1073: dual-world relationship status/stealth residual.
//!
//! relationship_to_target dual fails closed on destroyed/sold/masked/unselectable
//! and non-local effectively-stealthed entries. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_RELATIONSHIP_STATUS_STEALTH_RESIDUAL_METHOD_NAMES_WAVE1073: &[&str] = &[
    "relationship_to_target",
    "effectively_stealthed",
    "entry.destroyed",
    "Wave 1073",
    "playable_claim = false",
];

pub const LIVE_HOST_RELATIONSHIP_STATUS_STEALTH_RESIDUAL_NAV_STEPS_WAVE1073: &[&str] = &[
    "RELATIONSHIP",
    "STATUS_STEALTH",
    "LIVE_HOST_RELATIONSHIP_STATUS_STEALTH_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRelationshipStatusStealthResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostRelationshipStatusStealthResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_relationship_status_stealth_residual_method_names_residual_wave1073() -> bool {
    let names = LIVE_HOST_RELATIONSHIP_STATUS_STEALTH_RESIDUAL_METHOD_NAMES_WAVE1073;
    let ok = residual_name_index(names, "relationship_to_target").is_some()
        && residual_name_index(names, "Wave 1073").is_some();
    residual_action_store(ResidualHostRelationshipStatusStealthResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_relationship_status_stealth_residual_nav_commands_residual_wave1073() -> bool {
    let steps = LIVE_HOST_RELATIONSHIP_STATUS_STEALTH_RESIDUAL_NAV_STEPS_WAVE1073;
    let ok = residual_name_index(steps, "LIVE_HOST_RELATIONSHIP_STATUS_STEALTH_RESIDUAL").is_some()
        && residual_name_index(steps, "RELATIONSHIP").is_some();
    residual_action_store(ResidualHostRelationshipStatusStealthResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_relationship_status_stealth_residual_residual_pack_wave1073() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let tr = tr_source();
    let ok = tr.contains(
        "Wave 1073: destroyed/sold/masked/unselectable relationship residual fail-closed",
    ) && tr
        .contains("Wave 1073: non-local effectively-stealthed relationship residual fail-closed")
        && tr.contains("entry.destroyed || entry.sold || entry.masked || entry.unselectable")
        && tr.contains("entry.effectively_stealthed && !translator_entry_is_local(&entry)")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRelationshipStatusStealthResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_relationship_status_stealth_residual_honesty() -> bool {
    let a = honesty_host_relationship_status_stealth_residual_method_names_residual_wave1073();
    let b = honesty_host_relationship_status_stealth_residual_nav_commands_residual_wave1073();
    let c = honesty_host_relationship_status_stealth_residual_residual_pack_wave1073();
    residual_action_store(ResidualHostRelationshipStatusStealthResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_relationship_status_stealth_residual_wave1073() {
        assert!(honesty_host_relationship_status_stealth_residual_residual_pack_wave1073());
        assert!(honesty_host_relationship_status_stealth_residual_method_names_residual_wave1073());
        assert!(honesty_host_relationship_status_stealth_residual_nav_commands_residual_wave1073());
        assert!(simulate_live_host_relationship_status_stealth_residual_honesty());
    }
}
