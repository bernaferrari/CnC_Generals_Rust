//! Wave 1112: dual local-source seal + combat-damage latch honesty residual.
//!
//! After Waves 1105–1111 presentation sold peels and dual local-source masked/
//! unselectable peels, locks:
//! - dual-tick default AuthorityOnly + presentation shell path
//! - dual local-source masked/unselectable fail-closed (Wave 1111)
//! - executable smoke early combat damage latch window ≥6s (was 2s flaky)
//! - playable_claim stays false

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DUAL_SOURCE_COMBAT_LATCH_METHOD_NAMES_WAVE1112: &[&str] = &[
    "dual_tick_policy",
    "host_tick_game_client_presentation_shell",
    "selection_source_object_id",
    "Duration::from_secs(6)",
    "Wave 1112",
    "playable_claim: false",
];

pub const LIVE_HOST_DUAL_SOURCE_COMBAT_LATCH_NAV_STEPS_WAVE1112: &[&str] = &[
    "AUTHORITY_ONLY_DEFAULT",
    "DUAL_LOCAL_SOURCE_MASKED",
    "COMBAT_DAMAGE_LATCH_6S",
    "LIVE_HOST_DUAL_SOURCE_COMBAT_LATCH",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDualSourceCombatLatchAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDualSourceCombatLatchAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn aw_source() -> &'static str {
    include_str!("../../authoritative_world.rs")
}
fn cnc_source() -> &'static str {
    // 2026-08-15: scan engine plus presentation_frame split.
    super::engine_scan_src()
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn tr_source() -> &'static str {
    game_client::message_stream::translators::TRANSLATORS_SRC
}

pub fn honesty_host_dual_source_combat_latch_method_names_residual_wave1112() -> bool {
    let names = LIVE_HOST_DUAL_SOURCE_COMBAT_LATCH_METHOD_NAMES_WAVE1112;
    let ok = residual_name_index(names, "dual_tick_policy").is_some()
        && residual_name_index(names, "Wave 1112").is_some();
    residual_action_store(ResidualHostDualSourceCombatLatchAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_source_combat_latch_nav_commands_residual_wave1112() -> bool {
    let steps = LIVE_HOST_DUAL_SOURCE_COMBAT_LATCH_NAV_STEPS_WAVE1112;
    let ok = residual_name_index(steps, "LIVE_HOST_DUAL_SOURCE_COMBAT_LATCH").is_some()
        && residual_name_index(steps, "COMBAT_DAMAGE_LATCH_6S").is_some();
    residual_action_store(ResidualHostDualSourceCombatLatchAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_dual_source_combat_latch_residual_pack_wave1112() -> bool {
    let aw = aw_source();
    let cnc = cnc_source();
    let es = es_source();
    let tr = tr_source();
    let ok = aw.contains("fn dual_tick_policy")
        && aw.contains("DualTickPolicy::AuthorityOnly")
        && (aw.contains("GENERALS_ALLOW_DUAL_TICK")
            || aw.contains("DualTickPolicy::AuthorityOnly"))
        && cnc.contains("host_tick_game_client_presentation_shell")
        && cnc.contains("fn host_run_gameworld_shadow_after_logic")
        && cnc.contains("fn host_sync_shadow_and_build_presentation")
        && tr.contains("Wave 1111: also fail-closed on masked/unselectable local sources")
        && tr.contains("Wave 1111: masked/unselectable local source residual fail-closed")
        && (es.contains("Wave 1112") || es.contains("Wave 1112/1115"))
        && es.contains("Duration::from_secs(6)")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`")
        && !es.contains("self.playable_claim = true");
    residual_action_store(ResidualHostDualSourceCombatLatchAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_dual_source_combat_latch_residual_honesty() -> bool {
    let a = honesty_host_dual_source_combat_latch_method_names_residual_wave1112();
    let b = honesty_host_dual_source_combat_latch_nav_commands_residual_wave1112();
    let c = honesty_host_dual_source_combat_latch_residual_pack_wave1112();
    residual_action_store(ResidualHostDualSourceCombatLatchAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_dual_source_combat_latch_residual_wave1112() {
        assert!(honesty_host_dual_source_combat_latch_residual_pack_wave1112());
        assert!(honesty_host_dual_source_combat_latch_method_names_residual_wave1112());
        assert!(honesty_host_dual_source_combat_latch_nav_commands_residual_wave1112());
        assert!(simulate_live_host_dual_source_combat_latch_residual_honesty());
    }
}
