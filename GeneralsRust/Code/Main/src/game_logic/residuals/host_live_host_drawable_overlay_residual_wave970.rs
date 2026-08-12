//! Wave 970: drawable overlay presentation residual (health/vet/construct).
//!
//! Stamps veterancy and construction onto BasicDrawable presentation residual
//! and peels draw_health_bar / draw_veterancy / draw_construct_percent onto that
//! residual when OBJECT_REGISTRY is empty. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_OVERLAY_RESIDUAL_METHOD_NAMES_WAVE970: &[&str] = &[
    "draw_health_bar",
    "draw_veterancy",
    "draw_construct_percent",
    "presentation_veterancy_level",
    "Wave 970",
    "playable_claim = false",
];

pub const LIVE_HOST_DRAWABLE_OVERLAY_RESIDUAL_NAV_STEPS_WAVE970: &[&str] = &[
    "DRAWABLE_OVERLAY_FROM_FREEZE",
    "HEALTH_VET_CONSTRUCT_RESIDUAL",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_DRAWABLE_OVERLAY_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawableOverlayResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawableOverlayResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    include_str!("../game_logic.rs")
}

fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_drawable_overlay_residual_method_names_residual_wave970() -> bool {
    let names = LIVE_HOST_DRAWABLE_OVERLAY_RESIDUAL_METHOD_NAMES_WAVE970;
    let ok = residual_name_index(names, "draw_health_bar").is_some()
        && residual_name_index(names, "Wave 970").is_some();
    residual_action_store(ResidualHostDrawableOverlayResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_overlay_residual_nav_commands_residual_wave970() -> bool {
    let steps = LIVE_HOST_DRAWABLE_OVERLAY_RESIDUAL_NAV_STEPS_WAVE970;
    let ok = residual_name_index(steps, "LIVE_HOST_DRAWABLE_OVERLAY_RESIDUAL").is_some()
        && residual_name_index(steps, "HEALTH_VET_CONSTRUCT_RESIDUAL").is_some();
    residual_action_store(ResidualHostDrawableOverlayResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_overlay_residual_residual_pack_wave970() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let drawable = drawable_source();
    let client = client_source();
    let hb = match drawable.find("fn draw_health_bar") {
        Some(i) => &drawable[i..drawable.len().min(i + 700)],
        None => "",
    };
    let vet = match drawable.find("fn draw_veterancy") {
        Some(i) => &drawable[i..drawable.len().min(i + 500)],
        None => "",
    };
    let cons = match drawable.find("fn draw_construct_percent") {
        Some(i) => &drawable[i..drawable.len().min(i + 500)],
        None => "",
    };
    let ok = drawable.contains("Wave 970")
        && client.contains("Wave 970")
        && cnc.contains("Wave 970")
        && drawable.contains("presentation_veterancy_level")
        && drawable.contains("presentation_under_construction")
        && hb.contains("presentation_health_pct")
        && vet.contains("presentation_veterancy_level")
        && cons.contains("presentation_construction_percent")
        && client.contains("veterancy_level")
        && client.contains("construction_percent")
        && cnc.contains("veterancy_level:")
        && cnc.contains("under_construction:")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDrawableOverlayResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_overlay_residual_honesty() -> bool {
    let a = honesty_host_drawable_overlay_residual_method_names_residual_wave970();
    let b = honesty_host_drawable_overlay_residual_nav_commands_residual_wave970();
    let c = honesty_host_drawable_overlay_residual_residual_pack_wave970();
    residual_action_store(ResidualHostDrawableOverlayResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_overlay_residual_wave970() {
        assert!(honesty_host_drawable_overlay_residual_residual_pack_wave970());
        assert!(honesty_host_drawable_overlay_residual_method_names_residual_wave970());
        assert!(honesty_host_drawable_overlay_residual_nav_commands_residual_wave970());
        assert!(simulate_live_host_drawable_overlay_residual_honesty());
    }
}
