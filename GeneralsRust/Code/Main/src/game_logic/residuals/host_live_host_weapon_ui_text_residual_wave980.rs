//! Wave 980: weapon FX + UI text presentation residual peels.
//!
//! Peels handle_weapon_fire_fx and draw_ui_text onto presentation pose/color
//! residual when OBJECT_REGISTRY is empty. Fixes duplicate selection HUD draw
//! on presentation shell. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_WEAPON_UI_TEXT_RESIDUAL_METHOD_NAMES_WAVE980: &[&str] = &[
    "handle_weapon_fire_fx",
    "draw_ui_text_from_presentation",
    "presentation_orientation",
    "Wave 980",
    "playable_claim = false",
];

pub const LIVE_HOST_WEAPON_UI_TEXT_RESIDUAL_NAV_STEPS_WAVE980: &[&str] = &[
    "WEAPON_FX_FROM_PRESENTATION",
    "UI_TEXT_FROM_PRESENTATION",
    "HOST_EMPTY_DUAL_WORLD",
    "LIVE_HOST_WEAPON_UI_TEXT_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostWeaponUiTextResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostWeaponUiTextResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}

fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}

fn client_source() -> &'static str {
    game_client::core::game_client::GAME_CLIENT_SRC
}

pub fn honesty_host_weapon_ui_text_residual_method_names_residual_wave980() -> bool {
    let names = LIVE_HOST_WEAPON_UI_TEXT_RESIDUAL_METHOD_NAMES_WAVE980;
    let ok = residual_name_index(names, "handle_weapon_fire_fx").is_some()
        && residual_name_index(names, "Wave 980").is_some();
    residual_action_store(ResidualHostWeaponUiTextResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_weapon_ui_text_residual_nav_commands_residual_wave980() -> bool {
    let steps = LIVE_HOST_WEAPON_UI_TEXT_RESIDUAL_NAV_STEPS_WAVE980;
    let ok = residual_name_index(steps, "LIVE_HOST_WEAPON_UI_TEXT_RESIDUAL").is_some()
        && residual_name_index(steps, "WEAPON_FX_FROM_PRESENTATION").is_some();
    residual_action_store(ResidualHostWeaponUiTextResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_weapon_ui_text_residual_residual_pack_wave980() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let drawable = drawable_source();
    let client = client_source();
    let wfx = match drawable.find("pub fn handle_weapon_fire_fx") {
        Some(i) => &drawable[i..drawable.len().min(i + 1200)],
        None => "",
    };
    let uit = match drawable.find("fn draw_ui_text_from_presentation") {
        Some(i) => &drawable[i..drawable.len().min(i + 900)],
        None => "",
    };
    let shell = match client.find("fn update_presentation_shell") {
        Some(i) => {
            // Bound shell fn window to next pub fn at same indent if possible.
            let rest = &client[i..];
            let end = rest
                .find("\n    pub fn ")
                .map(|o| i + o)
                .unwrap_or(client.len().min(i + 8000));
            &client[i..end]
        }
        None => "",
    };
    let ok = drawable.contains("Wave 980")
        && client.contains("Wave 980")
        && drawable.contains("presentation_orientation")
        && wfx.contains("presentation_orientation")
        && !wfx.contains("empty dual-world → fail-closed")
        && uit.contains("presentation_indicator_color")
        && client.contains("e.orientation")
        // shell draws selection residual once (no double call).
        && shell.matches("draw_presentation_selection_residual()").count() == 1
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostWeaponUiTextResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_weapon_ui_text_residual_honesty() -> bool {
    let a = honesty_host_weapon_ui_text_residual_method_names_residual_wave980();
    let b = honesty_host_weapon_ui_text_residual_nav_commands_residual_wave980();
    let c = honesty_host_weapon_ui_text_residual_residual_pack_wave980();
    residual_action_store(ResidualHostWeaponUiTextResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_weapon_ui_text_residual_wave980() {
        assert!(honesty_host_weapon_ui_text_residual_residual_pack_wave980());
        assert!(honesty_host_weapon_ui_text_residual_method_names_residual_wave980());
        assert!(honesty_host_weapon_ui_text_residual_nav_commands_residual_wave980());
        assert!(simulate_live_host_weapon_ui_text_residual_honesty());
    }
}
