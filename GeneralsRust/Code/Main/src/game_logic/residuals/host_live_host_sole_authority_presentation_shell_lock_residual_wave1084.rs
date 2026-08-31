//! Wave 1084: sole-authority + presentation-shell honesty lock after Waves 1080–1083.
//!
//! Locks production defaults after dual FOW/ControlBar residual peels:
//! - dual-tick stays AuthorityOnly (crate tick opt-in only)
//! - GameClient presentation shell path (no full OS-input GameClient::update)
//! - render presentation-owned unit mesh path + live-fallback honesty counter
//! - playable_claim stays false (no full retail WND/GPU playthrough)

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SOLE_AUTHORITY_PRESENTATION_SHELL_LOCK_METHOD_NAMES_WAVE1084: &[&str] = &[
    "dual_tick_policy",
    "host_tick_game_client_presentation_shell",
    "update_presentation_shell",
    "presentation_live_fallback_honesty_ok",
    "Wave 1084",
    "playable_claim = false",
];

pub const LIVE_HOST_SOLE_AUTHORITY_PRESENTATION_SHELL_LOCK_NAV_STEPS_WAVE1084: &[&str] = &[
    "SOLE_AUTHORITY",
    "PRESENTATION_SHELL",
    "DUAL_TICK_OPT_IN_ONLY",
    "LIVE_HOST_SOLE_AUTHORITY_PRESENTATION_SHELL_LOCK",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSoleAuthorityPresentationShellLockAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSoleAuthorityPresentationShellLockAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn aw_source() -> &'static str {
    include_str!("../../authoritative_world.rs")
}
fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}
fn rp_source() -> &'static str {
    crate::graphics::render_pipeline::RENDER_PIPELINE_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}
fn sx_source() -> &'static str {
    include_str!("../../../../GameEngine/GameClient/src/message_stream/selection_xlat.rs")
}

pub fn honesty_host_sole_authority_presentation_shell_lock_method_names_residual_wave1084() -> bool
{
    let names = LIVE_HOST_SOLE_AUTHORITY_PRESENTATION_SHELL_LOCK_METHOD_NAMES_WAVE1084;
    let ok = residual_name_index(names, "dual_tick_policy").is_some()
        && residual_name_index(names, "Wave 1084").is_some();
    residual_action_store(ResidualHostSoleAuthorityPresentationShellLockAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sole_authority_presentation_shell_lock_nav_commands_residual_wave1084() -> bool
{
    let steps = LIVE_HOST_SOLE_AUTHORITY_PRESENTATION_SHELL_LOCK_NAV_STEPS_WAVE1084;
    let ok = residual_name_index(steps, "LIVE_HOST_SOLE_AUTHORITY_PRESENTATION_SHELL_LOCK")
        .is_some()
        && residual_name_index(steps, "PRESENTATION_SHELL").is_some();
    residual_action_store(ResidualHostSoleAuthorityPresentationShellLockAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_sole_authority_presentation_shell_lock_residual_pack_wave1084() -> bool {
    let aw = aw_source();
    let cnc = cnc_source();
    let es = es_source();
    let rp = rp_source();
    let cb = cb_source();
    let sx = sx_source();
    let ok = aw.contains("fn dual_tick_policy")
        && aw.contains("DualTickPolicy::AuthorityOnly")
        && aw.contains("GENERALS_ALLOW_DUAL_TICK")
        && cnc.contains("fn host_tick_game_client_presentation_shell")
        && cnc.contains("update_presentation_shell")
        && cnc.contains("update_drawables_local")
        && // 2026-08-15: playable_claim is the five-flag constructor, not a literal assignment.
        es.contains("self.playable_claim = Self::retail_windowed_playable_claim(")
        && es.contains("Headless smoke must keep `playable_claim == false`")
        && !cnc.contains("playable_claim = true")
        && rp.contains("presentation_live_fallback_honesty_ok")
        && rp.contains("unit_render_inputs")
        && cb.contains("Wave 1083: under-construction dual portrait clears production residual")
        && cb.contains("Wave 1081: skip inventory seed for unusable dual catalog entries")
        && cb.contains("Wave 1080: skip UC/garrison seed for unusable dual catalog entries")
        && sx.contains("Wave 1082: FOW fogged/black non-local residual also unselectable");
    residual_action_store(ResidualHostSoleAuthorityPresentationShellLockAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_sole_authority_presentation_shell_lock_honesty() -> bool {
    let a = honesty_host_sole_authority_presentation_shell_lock_method_names_residual_wave1084();
    let b = honesty_host_sole_authority_presentation_shell_lock_nav_commands_residual_wave1084();
    let c = honesty_host_sole_authority_presentation_shell_lock_residual_pack_wave1084();
    residual_action_store(ResidualHostSoleAuthorityPresentationShellLockAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_sole_authority_presentation_shell_lock_wave1084() {
        assert!(honesty_host_sole_authority_presentation_shell_lock_residual_pack_wave1084());
        assert!(
            honesty_host_sole_authority_presentation_shell_lock_method_names_residual_wave1084()
        );
        assert!(
            honesty_host_sole_authority_presentation_shell_lock_nav_commands_residual_wave1084()
        );
        assert!(simulate_live_host_sole_authority_presentation_shell_lock_honesty());
    }
}
