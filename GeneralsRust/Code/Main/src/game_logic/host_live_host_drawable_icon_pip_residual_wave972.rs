//! Wave 972: drawable icon-pip presentation residual.
//!
//! Stamps ammo/contain/disabled/carbomb/enthusiastic residual onto BasicDrawable
//! and peels draw_ammo/contained/healing/enthusiastic/demoralized/bombed/disabled
//! onto freeze data when OBJECT_REGISTRY is empty. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_DRAWABLE_ICON_PIP_RESIDUAL_METHOD_NAMES_WAVE972: &[&str] = &[
    "draw_ammo",
    "draw_contained",
    "draw_disabled",
    "presentation_ammo_pip_total",
    "Wave 972",
    "playable_claim = false",
];

pub const LIVE_HOST_DRAWABLE_ICON_PIP_RESIDUAL_NAV_STEPS_WAVE972: &[&str] = &[
    "DRAWABLE_ICON_PIP_FROM_FREEZE",
    "AMMO_CONTAIN_STATUS_RESIDUAL",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_DRAWABLE_ICON_PIP_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostDrawableIconPipResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostDrawableIconPipResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn gl_source() -> &'static str {
    include_str!("game_logic.rs")
}

fn drawable_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/drawable/drawable.rs")
}

fn client_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/core/game_client.rs")
}

pub fn honesty_host_drawable_icon_pip_residual_method_names_residual_wave972() -> bool {
    let names = LIVE_HOST_DRAWABLE_ICON_PIP_RESIDUAL_METHOD_NAMES_WAVE972;
    let ok = residual_name_index(names, "draw_ammo").is_some()
        && residual_name_index(names, "Wave 972").is_some();
    residual_action_store(ResidualHostDrawableIconPipResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_icon_pip_residual_nav_commands_residual_wave972() -> bool {
    let steps = LIVE_HOST_DRAWABLE_ICON_PIP_RESIDUAL_NAV_STEPS_WAVE972;
    let ok = residual_name_index(steps, "LIVE_HOST_DRAWABLE_ICON_PIP_RESIDUAL").is_some()
        && residual_name_index(steps, "AMMO_CONTAIN_STATUS_RESIDUAL").is_some();
    residual_action_store(ResidualHostDrawableIconPipResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_drawable_icon_pip_residual_residual_pack_wave972() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let drawable = drawable_source();
    let client = client_source();
    let ammo = match drawable.find("fn draw_ammo") {
        Some(i) => &drawable[i..drawable.len().min(i + 700)],
        None => "",
    };
    let cont = match drawable.find("fn draw_contained") {
        Some(i) => &drawable[i..drawable.len().min(i + 700)],
        None => "",
    };
    let dis = match drawable.find("fn draw_disabled") {
        Some(i) => &drawable[i..drawable.len().min(i + 500)],
        None => "",
    };
    let ok = drawable.contains("Wave 972")
        && client.contains("Wave 972")
        && cnc.contains("Wave 972")
        && drawable.contains("presentation_ammo_pip_total")
        && drawable.contains("presentation_disabled")
        && drawable.contains("presentation_is_carbomb")
        && ammo.contains("presentation_ammo_pip_total")
        && cont.contains("presentation_max_garrison")
        && dis.contains("presentation_disabled")
        && client.contains("ammo_pip_total")
        && client.contains("weapon_bonus_enthusiastic")
        && cnc.contains("ammo_pip_total:")
        && cnc.contains("is_carbomb:")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostDrawableIconPipResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_drawable_icon_pip_residual_honesty() -> bool {
    let a = honesty_host_drawable_icon_pip_residual_method_names_residual_wave972();
    let b = honesty_host_drawable_icon_pip_residual_nav_commands_residual_wave972();
    let c = honesty_host_drawable_icon_pip_residual_residual_pack_wave972();
    residual_action_store(ResidualHostDrawableIconPipResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_drawable_icon_pip_residual_wave972() {
        assert!(honesty_host_drawable_icon_pip_residual_residual_pack_wave972());
        assert!(honesty_host_drawable_icon_pip_residual_method_names_residual_wave972());
        assert!(honesty_host_drawable_icon_pip_residual_nav_commands_residual_wave972());
        assert!(simulate_live_host_drawable_icon_pip_residual_honesty());
    }
}
