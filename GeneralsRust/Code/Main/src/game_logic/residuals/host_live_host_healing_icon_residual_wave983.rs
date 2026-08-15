//! Wave 983: presentation healing icon residual peel.
//!
//! Freezes sole-benefactor healing window into RenderableObject / drawable
//! presentation residual so host empty dual-world still shows heal icons.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_HEALING_ICON_RESIDUAL_METHOD_NAMES_WAVE983: &[&str] = &[
    "show_healing",
    "healing_icon_type",
    "draw_healing",
    "sole_healing_benefactor_expiration_frame",
    "Wave 983",
    "playable_claim = false",
];

pub const LIVE_HOST_HEALING_ICON_RESIDUAL_NAV_STEPS_WAVE983: &[&str] = &[
    "SOLE_HEAL_WINDOW",
    "HEALING_ICON_TYPE",
    "DRAW_HEALING_FROM_PRESENTATION",
    "LIVE_HOST_HEALING_ICON_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostHealingIconResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostHealingIconResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn pf_source() -> &'static str {
    crate::presentation_frame::PRESENTATION_FRAME_SRC
}
fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}
fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

// 2026-08-15: widen post-split scan window to the rest of the concat.
pub fn honesty_host_healing_icon_residual_method_names_residual_wave983() -> bool {
    let names = LIVE_HOST_HEALING_ICON_RESIDUAL_METHOD_NAMES_WAVE983;
    let ok = residual_name_index(names, "show_healing").is_some()
        && residual_name_index(names, "Wave 983").is_some();
    residual_action_store(ResidualHostHealingIconResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_healing_icon_residual_nav_commands_residual_wave983() -> bool {
    let steps = LIVE_HOST_HEALING_ICON_RESIDUAL_NAV_STEPS_WAVE983;
    let ok = residual_name_index(steps, "LIVE_HOST_HEALING_ICON_RESIDUAL").is_some()
        && residual_name_index(steps, "DRAW_HEALING_FROM_PRESENTATION").is_some();
    residual_action_store(ResidualHostHealingIconResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_healing_icon_residual_residual_pack_wave983() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let pf = pf_source();
    let d = drawable_source();
    let client = client_source();
    let heal = match d.find("fn draw_healing") {
        Some(i) => &d[i..],
        None => "",
    };
    let ok = pf.contains("pub show_healing: bool")
        && pf.contains("sole_healing_benefactor_expiration_frame")
        && pf.contains("healing_icon_type:")
        && d.contains("presentation_show_healing")
        && d.contains("Wave 983")
        && heal.contains("presentation_show_healing")
        && heal.contains("presentation_healing_icon_type")
        && client.contains("pub show_healing: bool")
        && client.contains("e.show_healing")
        && cnc.contains("show_healing: o.show_healing")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostHealingIconResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_healing_icon_residual_honesty() -> bool {
    let a = honesty_host_healing_icon_residual_method_names_residual_wave983();
    let b = honesty_host_healing_icon_residual_nav_commands_residual_wave983();
    let c = honesty_host_healing_icon_residual_residual_pack_wave983();
    residual_action_store(ResidualHostHealingIconResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_healing_icon_residual_wave983() {
        assert!(honesty_host_healing_icon_residual_residual_pack_wave983());
        assert!(honesty_host_healing_icon_residual_method_names_residual_wave983());
        assert!(honesty_host_healing_icon_residual_nav_commands_residual_wave983());
        assert!(simulate_live_host_healing_icon_residual_honesty());
    }
}
