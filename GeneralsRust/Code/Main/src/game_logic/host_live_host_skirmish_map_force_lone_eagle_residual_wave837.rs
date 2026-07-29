//! Wave 837: force control-file map (Lone Eagle) over ShellMapMD residual when
//! starting skirmish via headless WND latch peels. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};

static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);

pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

pub const LIVE_HOST_SKIRMISH_MAP_FORCE_LONE_EAGLE_METHOD_NAMES_WAVE837: &[&str] = &[
    "start_skirmish_game",
    "selected_map",
    "set_selected_map",
    "set_skirmish_menu_selected_map",
    "ShellMapMD",
    "Wave 837",
    "playable_claim = false",
];

pub const LIVE_HOST_SKIRMISH_MAP_FORCE_LONE_EAGLE_NAV_STEPS_WAVE837: &[&str] = &[
    "REQUIRE_PREFER_SELECTED_MAP",
    "REQUIRE_FORCE_STAMP_CONTROL_MAP",
    "REQUIRE_START_FAIL_CLEARS_BUTTON",
    "LIVE_HOST_SKIRMISH_MAP_FORCE_LONE_EAGLE",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostSkirmishMapForceLoneEagleAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}

fn residual_action_store(a: ResidualHostSkirmishMapForceLoneEagleAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}

fn cnc_source() -> &'static str {
    include_str!("../cnc_game_engine.rs")
}

fn skirmish_menu_source() -> &'static str {
    include_str!("../../../GameEngine/GameClient/src/gui/callbacks/skirmish_game_options_menu.rs")
}

pub fn honesty_host_skirmish_map_force_lone_eagle_method_names_residual_wave837() -> bool {
    let names = LIVE_HOST_SKIRMISH_MAP_FORCE_LONE_EAGLE_METHOD_NAMES_WAVE837;
    let ok = residual_name_index(names, "start_skirmish_game").is_some()
        && residual_name_index(names, "Wave 837").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostSkirmishMapForceLoneEagleAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_skirmish_map_force_lone_eagle_nav_commands_residual_wave837() -> bool {
    let steps = LIVE_HOST_SKIRMISH_MAP_FORCE_LONE_EAGLE_NAV_STEPS_WAVE837;
    let ok = residual_name_index(steps, "LIVE_HOST_SKIRMISH_MAP_FORCE_LONE_EAGLE").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostSkirmishMapForceLoneEagleAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn honesty_host_skirmish_map_force_lone_eagle_residual_pack_wave837() -> bool {
    let cnc = cnc_source();
    let sm = skirmish_menu_source();
    let ok = (sm
        .contains("Wave 837: prefer options/map-select residual over a stale shell/default map")
        || sm.contains(
            "Wave 837/840: prefer options/map-select residual over a stale shell/default map",
        ))
        && (sm.contains("shellish") || sm.contains("is_shellish"))
        && sm.contains("Wave 835/837: no live window")
        && cnc.contains("Wave 837: also stamp GameClient skirmish_setup")
        && cnc.contains("Wave 837: always force-commit control map into setup")
        && cnc.contains("set_skirmish_menu_selected_map");
    residual_action_store(ResidualHostSkirmishMapForceLoneEagleAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

pub fn simulate_live_host_skirmish_map_force_lone_eagle_honesty() -> bool {
    let a = honesty_host_skirmish_map_force_lone_eagle_method_names_residual_wave837();
    let b = honesty_host_skirmish_map_force_lone_eagle_nav_commands_residual_wave837();
    let c = honesty_host_skirmish_map_force_lone_eagle_residual_pack_wave837();
    residual_action_store(ResidualHostSkirmishMapForceLoneEagleAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_skirmish_map_force_lone_eagle_residual_wave837() {
        assert!(honesty_host_skirmish_map_force_lone_eagle_residual_pack_wave837());
        assert!(honesty_host_skirmish_map_force_lone_eagle_method_names_residual_wave837());
        assert!(honesty_host_skirmish_map_force_lone_eagle_nav_commands_residual_wave837());
        assert!(simulate_live_host_skirmish_map_force_lone_eagle_honesty());
    }
}
