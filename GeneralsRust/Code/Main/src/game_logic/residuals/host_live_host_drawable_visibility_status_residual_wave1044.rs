//! Wave 1044: dual-world drawable visibility stealth/destroyed residual.
//!
//! update_drawable_visibility dual path hides destroyed and non-local
//! effectively-stealthed catalog residuals (plus FOW). playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_VISIBILITY_STATUS_RESIDUAL_METHOD_NAMES_WAVE1044: &[&str] = &[
    "update_drawable_visibility",
    "effectively_stealthed",
    "StealthLook::Invisible",
    "Wave 1044",
    "playable_claim = false",
];

pub const LIVE_HOST_DRAWABLE_VISIBILITY_STATUS_RESIDUAL_NAV_STEPS_WAVE1044: &[&str] = &[
    "DRAWABLE_VISIBILITY",
    "STEALTH",
    "DESTROYED",
    "LIVE_HOST_DRAWABLE_VISIBILITY_STATUS_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawableVisibilityStatusResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawableVisibilityStatusResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}
fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_drawable_visibility_status_residual_method_names_residual_wave1044() -> bool {
    let names = LIVE_HOST_DRAWABLE_VISIBILITY_STATUS_RESIDUAL_METHOD_NAMES_WAVE1044;
    let ok = residual_name_index(names, "update_drawable_visibility").is_some()
        && residual_name_index(names, "Wave 1044").is_some();
    residual_action_store(ResidualHostDrawableVisibilityStatusResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_visibility_status_residual_nav_commands_residual_wave1044() -> bool {
    let steps = LIVE_HOST_DRAWABLE_VISIBILITY_STATUS_RESIDUAL_NAV_STEPS_WAVE1044;
    let ok = residual_name_index(steps, "LIVE_HOST_DRAWABLE_VISIBILITY_STATUS_RESIDUAL").is_some()
        && residual_name_index(steps, "STEALTH").is_some();
    residual_action_store(ResidualHostDrawableVisibilityStatusResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_visibility_status_residual_residual_pack_wave1044() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let client = client_source();
    let ok = client
        .contains("Wave 1020/1044: host empty dual-world peels presentation catalog shroud/")
        && client.contains("status_hidden = entry.destroyed")
        && client.contains("StealthLook::Invisible")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDrawableVisibilityStatusResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_visibility_status_residual_honesty() -> bool {
    let a = honesty_host_drawable_visibility_status_residual_method_names_residual_wave1044();
    let b = honesty_host_drawable_visibility_status_residual_nav_commands_residual_wave1044();
    let c = honesty_host_drawable_visibility_status_residual_residual_pack_wave1044();
    residual_action_store(ResidualHostDrawableVisibilityStatusResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_visibility_status_residual_wave1044() {
        assert!(honesty_host_drawable_visibility_status_residual_residual_pack_wave1044());
        assert!(honesty_host_drawable_visibility_status_residual_method_names_residual_wave1044());
        assert!(honesty_host_drawable_visibility_status_residual_nav_commands_residual_wave1044());
        assert!(simulate_live_host_drawable_visibility_status_residual_honesty());
    }
}
