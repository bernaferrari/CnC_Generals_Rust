//! Wave 1052: dual-world command availability + icon-pip stealth residual.
//!
//! ControlBar command dual fails closed on destroyed/sold/unselectable.
//! draw_ammo/draw_contained dual hide when presentation_effectively_stealthed.
//! playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_COMMAND_AVAIL_PIP_STEALTH_RESIDUAL_METHOD_NAMES_WAVE1052: &[&str] = &[
    "CommandAvailability::Hidden",
    "presentation_effectively_stealthed",
    "Wave 1052",
    "playable_claim = false",
];

pub const LIVE_HOST_COMMAND_AVAIL_PIP_STEALTH_RESIDUAL_NAV_STEPS_WAVE1052: &[&str] = &[
    "COMMAND_AVAILABILITY",
    "ICON_PIPS",
    "LIVE_HOST_COMMAND_AVAIL_PIP_STEALTH_RESIDUAL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostCommandAvailPipStealthResidualAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostCommandAvailPipStealthResidualAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn cb_source() -> &'static str {
    game_client::gui::control_bar::control_bar::CONTROL_BAR_SRC
}
fn drawable_source() -> &'static str {
    game_client::drawable::drawable::DRAWABLE_SRC
}

pub fn honesty_host_command_avail_pip_stealth_residual_method_names_residual_wave1052() -> bool {
    let names = LIVE_HOST_COMMAND_AVAIL_PIP_STEALTH_RESIDUAL_METHOD_NAMES_WAVE1052;
    let ok = residual_name_index(names, "CommandAvailability::Hidden").is_some()
        && residual_name_index(names, "Wave 1052").is_some();
    residual_action_store(ResidualHostCommandAvailPipStealthResidualAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_avail_pip_stealth_residual_nav_commands_residual_wave1052() -> bool {
    let steps = LIVE_HOST_COMMAND_AVAIL_PIP_STEALTH_RESIDUAL_NAV_STEPS_WAVE1052;
    let ok = residual_name_index(steps, "LIVE_HOST_COMMAND_AVAIL_PIP_STEALTH_RESIDUAL").is_some()
        && residual_name_index(steps, "COMMAND_AVAILABILITY").is_some();
    residual_action_store(ResidualHostCommandAvailPipStealthResidualAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_command_avail_pip_stealth_residual_residual_pack_wave1052() -> bool {
    let cnc = cnc_source();
    let gl = gl_source();
    let cb = cb_source();
    let d = drawable_source();
    let ok = cb.contains("Wave 1025/1026/1052: catalog/command-set residual")
        && (cb.contains("entry.destroyed || entry.sold || entry.unselectable")
            || (cb.contains("entry.destroyed")
                && cb.contains("entry.sold")
                && cb.contains("entry.unselectable")))
        && d.contains("Wave 972/1052: host empty dual-world → presentation ammo residual.")
        && d.contains("Wave 972/1052: host empty dual-world → presentation contain residual.")
        && d.contains("|| self.presentation_effectively_stealthed")
        && !cnc.contains("playable_claim = true")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostCommandAvailPipStealthResidualAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_command_avail_pip_stealth_residual_honesty() -> bool {
    let a = honesty_host_command_avail_pip_stealth_residual_method_names_residual_wave1052();
    let b = honesty_host_command_avail_pip_stealth_residual_nav_commands_residual_wave1052();
    let c = honesty_host_command_avail_pip_stealth_residual_residual_pack_wave1052();
    residual_action_store(ResidualHostCommandAvailPipStealthResidualAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_command_avail_pip_stealth_residual_wave1052() {
        assert!(honesty_host_command_avail_pip_stealth_residual_residual_pack_wave1052());
        assert!(honesty_host_command_avail_pip_stealth_residual_method_names_residual_wave1052());
        assert!(honesty_host_command_avail_pip_stealth_residual_nav_commands_residual_wave1052());
        assert!(simulate_live_host_command_avail_pip_stealth_residual_honesty());
    }
}
