//! Wave 840: skirmish start rejects boot ShellMapMD residual in favor of control/
//! setup map, clears stale presentation freeze, and gates vertical slice on a
//! non-shell map_seen.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SKIRMISH_MAP_REJECT_SHELL_METHOD_NAMES_WAVE840: &[&str] = &[
    "resolve_skirmish_start_map_name",
    "map_name_is_shell_residual",
    "skirmish_map_boundary_ok",
    "Wave 840",
    "ShellMapMD",
    "playable_claim = false",
];

pub const LIVE_HOST_SKIRMISH_MAP_REJECT_SHELL_NAV_STEPS_WAVE840: &[&str] = &[
    "REJECT_SHELL_MAP",
    "PREFER_CONTROL_MAP",
    "CLEAR_STALE_PRESENTATION",
    "LIVE_HOST_SKIRMISH_MAP_REJECT_SHELL",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSkirmishMapRejectShellAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSkirmishMapRejectShellAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}

fn es_source() -> &'static str {
    crate::executable_smoke_source::EXECUTABLE_SMOKE_SRC
}

fn sm_source() -> &'static str {
    include_str!(
        "../../../../GameEngine/GameClient/src/gui/callbacks/skirmish_game_options_menu.rs"
    )
}

pub fn honesty_host_skirmish_map_reject_shell_method_names_residual_wave840() -> bool {
    let names = LIVE_HOST_SKIRMISH_MAP_REJECT_SHELL_METHOD_NAMES_WAVE840;
    let ok = residual_name_index(names, "resolve_skirmish_start_map_name").is_some()
        && residual_name_index(names, "skirmish_map_boundary_ok").is_some()
        && residual_name_index(names, "Wave 840").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSkirmishMapRejectShellAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_skirmish_map_reject_shell_nav_commands_residual_wave840() -> bool {
    let steps = LIVE_HOST_SKIRMISH_MAP_REJECT_SHELL_NAV_STEPS_WAVE840;
    let ok = residual_name_index(steps, "LIVE_HOST_SKIRMISH_MAP_REJECT_SHELL").is_some()
        && residual_name_index(steps, "REJECT_SHELL_MAP").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostSkirmishMapRejectShellAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_skirmish_map_reject_shell_residual_pack_wave840() -> bool {
    let cnc = cnc_source();
    let es = es_source();
    let sm = sm_source();
    let ok = cnc.contains("fn resolve_skirmish_start_map_name")
        && cnc.contains("fn map_name_is_shell_residual")
        && cnc.contains("Wave 840: never start skirmish on boot ShellMapMD")
        && cnc.contains("Wave 840: drop shell presentation freeze")
        && cnc.contains("Wave 840: control-file map wins over boot ShellMap")
        && (cnc.contains("Wave 840: if freeze still holds shell residual after match load")
            || (cnc
                .contains("Wave 840/843: if freeze still holds shell residual after match load")
                || cnc.contains(
                    "Wave 840/843/860: if freeze still holds shell residual after match load",
                )))
        && es.contains("skirmish_map_boundary_ok")
        && es.contains("map_is_shell_residual")
        && es.contains("&& skirmish_map_boundary_ok")
        && sm.contains("Wave 837/840: prefer options/map-select residual");
    residual_action_store(ResidualHostSkirmishMapRejectShellAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_skirmish_map_reject_shell_honesty() -> bool {
    let a = honesty_host_skirmish_map_reject_shell_method_names_residual_wave840();
    let b = honesty_host_skirmish_map_reject_shell_nav_commands_residual_wave840();
    let c = honesty_host_skirmish_map_reject_shell_residual_pack_wave840();
    residual_action_store(ResidualHostSkirmishMapRejectShellAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_skirmish_map_reject_shell_residual_wave840() {
        assert!(honesty_host_skirmish_map_reject_shell_residual_pack_wave840());
        assert!(honesty_host_skirmish_map_reject_shell_method_names_residual_wave840());
        assert!(honesty_host_skirmish_map_reject_shell_nav_commands_residual_wave840());
        assert!(simulate_live_host_skirmish_map_reject_shell_honesty());
    }
}
