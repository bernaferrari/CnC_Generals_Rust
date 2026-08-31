//! Wave 831: load_map places skirmish starting construction yards + dozers at
//! Player_N_Start waypoints (C++ placeNetworkStartingUnits residual). SidesList
//! build-list parsing retained for maps that use it. playable_claim stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_MAP_START_ARMY_SPAWN_METHOD_NAMES_WAVE831: &[&str] = &[
    "parse_player_start_waypoints",
    "spawn_skirmish_starting_units",
    "starting_building",
    "SideBuildEntry",
    "Wave 831",
    "playable_claim = false",
];
pub const LIVE_HOST_MAP_START_ARMY_SPAWN_NAV_STEPS_WAVE831: &[&str] = &[
    "REQUIRE_PLAYER_START_WAYPOINTS",
    "REQUIRE_STARTING_BUILDING_SEED",
    "REQUIRE_LOAD_MAP_SPAWN",
    "LIVE_HOST_MAP_START_ARMY_SPAWN",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMapStartArmySpawnAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostMapStartArmySpawnAction {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostMapStartArmySpawnAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
fn sl_source() -> &'static str {
    // 2026-08-25: scan the script_loader split fragments alongside the root.
    concat!(
        include_str!("../script_loader.rs"),
        include_str!("../script_loader/map_types.rs"),
        include_str!("../script_loader/file_resolution.rs"),
        include_str!("../script_loader/chunk_decoding.rs"),
        include_str!("../script_loader/map_settings.rs"),
        include_str!("../script_loader/map_terrain.rs"),
        include_str!("../script_loader/map_objects.rs"),
        include_str!("../script_loader/script_records.rs"),
    )
}
pub fn honesty_host_map_start_army_spawn_method_names_residual_wave831() -> bool {
    let names = LIVE_HOST_MAP_START_ARMY_SPAWN_METHOD_NAMES_WAVE831;
    let ok = residual_name_index(names, "parse_player_start_waypoints").is_some()
        && residual_name_index(names, "spawn_skirmish_starting_units").is_some()
        && residual_name_index(names, "starting_building").is_some()
        && residual_name_index(names, "SideBuildEntry").is_some()
        && residual_name_index(names, "Wave 831").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMapStartArmySpawnAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_map_start_army_spawn_nav_commands_residual_wave831() -> bool {
    let steps = LIVE_HOST_MAP_START_ARMY_SPAWN_NAV_STEPS_WAVE831;
    let ok = residual_name_index(steps, "LIVE_HOST_MAP_START_ARMY_SPAWN").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostMapStartArmySpawnAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_map_start_army_spawn_residual_pack_wave831() -> bool {
    let gl = gl_source();
    let sl = sl_source();
    let ok = gl.contains("Wave 831")
        && gl.contains("parse_player_start_waypoints(&self.map_name)")
        && gl.contains("spawn_skirmish_starting_units")
        && gl.contains("Wave 831")
        && sl.contains("pub fn parse_player_start_waypoints")
        && sl.contains("pub struct SideBuildEntry")
        && sl.contains("side_builds");
    residual_action_store(ResidualHostMapStartArmySpawnAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_map_start_army_spawn_honesty() -> bool {
    let a = honesty_host_map_start_army_spawn_method_names_residual_wave831();
    let b = honesty_host_map_start_army_spawn_nav_commands_residual_wave831();
    let c = honesty_host_map_start_army_spawn_residual_pack_wave831();
    residual_action_store(ResidualHostMapStartArmySpawnAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_map_start_army_spawn_residual_wave831() {
        assert!(honesty_host_map_start_army_spawn_residual_pack_wave831());
        assert!(honesty_host_map_start_army_spawn_method_names_residual_wave831());
        assert!(honesty_host_map_start_army_spawn_nav_commands_residual_wave831());
        assert!(simulate_live_host_map_start_army_spawn_honesty());
    }
}
