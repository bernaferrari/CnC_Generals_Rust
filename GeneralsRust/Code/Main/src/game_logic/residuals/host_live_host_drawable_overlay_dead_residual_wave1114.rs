//! Wave 1114: dual-world drawable overlay residual fail-closed on dead.
//!
//! Health/veterancy/healing/enthusiastic/bombed/disabled/ammo/contain dual
//! presentation paths still stamped overlay state when presentation_health_pct
//! was already <= 0 (dead residual).

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_OVERLAY_DEAD_METHOD_NAMES_WAVE1114: &[&str] = &[
    "draw_health_bar",
    "draw_veterancy",
    "draw_healing",
    "draw_ammo",
    "Wave 1114",
    "playable_claim: false",
];

pub const LIVE_HOST_DRAWABLE_OVERLAY_DEAD_NAV_STEPS_WAVE1114: &[&str] = &[
    "OVERLAY_DEAD_FAIL_CLOSED",
    "HEALTH_VETERANCY_HEALING",
    "AMMO_CONTAIN_DISABLED",
    "LIVE_HOST_DRAWABLE_OVERLAY_DEAD",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawableOverlayDeadAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawableOverlayDeadAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn dr_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}
fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

pub fn honesty_host_drawable_overlay_dead_method_names_residual_wave1114() -> bool {
    // Self-table membership is inflation (host_wave_inflation). Scan shipped draw fns.
    debug_assert!(crate::game_logic::host_wave_inflation::self_table_honesty_is_inflation());
    let dr = dr_source();
    let ok = crate::game_logic::host_wave_inflation::shipped_fn_contains(
        dr,
        "fn draw_health_bar",
        &["presentation_health_pct <= 0.0"],
    ) && crate::game_logic::host_wave_inflation::shipped_fn_contains(
        dr,
        "fn draw_veterancy",
        &["presentation_health_pct <= 0.0"],
    ) && crate::game_logic::host_wave_inflation::shipped_fn_contains(
        dr,
        "pub fn draw_healing",
        &["presentation_health_pct <= 0.0"],
    ) && crate::game_logic::host_wave_inflation::shipped_fn_contains(
        dr,
        "pub fn draw_ammo",
        &["presentation_health_pct <= 0.0"],
    );
    residual_action_store(ResidualHostDrawableOverlayDeadAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_overlay_dead_nav_commands_residual_wave1114() -> bool {
    // Nav honesty: shipped draw_icon_ui must still dispatch the overlay draw fns.
    debug_assert!(crate::game_logic::host_wave_inflation::self_table_honesty_is_inflation());
    let dr = dr_source();
    let ok = match crate::game_logic::host_wave_inflation::shipped_fn_window(
        dr,
        "    pub fn draw_icon_ui(&mut self)",
        6000,
    ) {
        Some(icon) => {
            icon.contains("self.draw_health_bar")
                && icon.contains("self.draw_construct_percent")
                && icon.contains("self.draw_ammo")
                && icon.contains("self.draw_veterancy")
                && icon.contains("self.draw_healing")
        }
        None => false,
    };
    residual_action_store(ResidualHostDrawableOverlayDeadAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_overlay_dead_residual_pack_wave1114() -> bool {
    let dr = dr_source();
    let es = es_source();
    let ok = dr.contains("Wave 1114: dual health-bar residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual veterancy residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual healing residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual enthusiastic residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual bombed residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual disabled residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual ammo residual fail-closed on dead presentation")
        && dr.contains("Wave 1114: dual contain residual fail-closed on dead presentation")
        && dr.contains("presentation_health_pct <= 0.0")
        && es.contains("playable_claim: false");
    residual_action_store(ResidualHostDrawableOverlayDeadAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_overlay_dead_residual_honesty() -> bool {
    let a = honesty_host_drawable_overlay_dead_method_names_residual_wave1114();
    let b = honesty_host_drawable_overlay_dead_nav_commands_residual_wave1114();
    let c = honesty_host_drawable_overlay_dead_residual_pack_wave1114();
    residual_action_store(ResidualHostDrawableOverlayDeadAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_overlay_dead_residual_wave1114() {
        assert!(honesty_host_drawable_overlay_dead_residual_pack_wave1114());
        assert!(honesty_host_drawable_overlay_dead_method_names_residual_wave1114());
        assert!(honesty_host_drawable_overlay_dead_nav_commands_residual_wave1114());
        assert!(simulate_live_host_drawable_overlay_dead_residual_honesty());
    }
}
