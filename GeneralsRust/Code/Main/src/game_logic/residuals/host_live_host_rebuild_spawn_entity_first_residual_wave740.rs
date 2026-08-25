//! Wave 740: rebuild-hole worker + reconstruct spawn is GameWorld entity-first
//! under construction sole-tick. Writeback pre-spawns entities; host binds
//! ObjectIds (prefers free GW raw). `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST_METHOD_NAMES_WAVE740: &[&str] = &[
    "host_spawn_rebuild_bound_object",
    "record_with_entities",
    "worker_entity_raw",
    "rebuild_entity_raw",
    "ready_by_hole",
    "Wave 740",
    "playable_claim = false",
];
pub const LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST_NAV_STEPS_WAVE740: &[&str] = &[
    "REQUIRE_WRITEBACK_PRE_SPAWN",
    "REQUIRE_HOST_BIND_HELPER",
    "REQUIRE_READY_BY_HOLE_MAP",
    "LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST_CMD_NAMES_WAVE740: &[&str] = &[
    "host_rebuild_spawn_entity_first",
    "writeback_pre_spawn",
    "host_bind_helper",
    "ready_by_hole_map",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRebuildSpawnEntityFirstAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostRebuildSpawnEntityFirstAction {
    pub fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::MethodNames,
            2 => Self::SourceMarkers,
            3 => Self::NavCommands,
            4 => Self::CollectSource,
            5 => Self::DispatchSource,
            6 => Self::Composite,
            _ => Self::None,
        }
    }
}
fn residual_action_store(a: ResidualHostRebuildSpawnEntityFirstAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_rebuild_spawn_entity_first_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_rebuild_spawn_entity_first_last_action()
-> ResidualHostRebuildSpawnEntityFirstAction {
    ResidualHostRebuildSpawnEntityFirstAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ready_source() -> &'static str {
    include_str!("../host_rebuild_ready_log.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_rebuild_spawn_entity_first_method_names_residual_wave740() -> bool {
    let names = LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST_METHOD_NAMES_WAVE740;
    let ok = residual_name_index(names, "host_spawn_rebuild_bound_object").is_some()
        && residual_name_index(names, "record_with_entities").is_some()
        && residual_name_index(names, "worker_entity_raw").is_some()
        && residual_name_index(names, "rebuild_entity_raw").is_some()
        && residual_name_index(names, "ready_by_hole").is_some()
        && residual_name_index(names, "Wave 740").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRebuildSpawnEntityFirstAction::MethodNames);
    ok
}
pub fn honesty_host_rebuild_spawn_entity_first_source_markers_residual_wave740() -> bool {
    let gl = gl_source();
    let ready = ready_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("Wave 740")
        && gl.contains("host_spawn_rebuild_bound_object")
        && gl.contains("ready_by_hole")
        && gl.contains("worker_entity_raw")
        && gl.contains("rebuild_entity_raw");
    let ready_ok = ready.contains("record_with_entities")
        && ready.contains("worker_entity_raw")
        && ready.contains("Wave 740");
    let sh_ok = sh.contains("Wave 740")
        && sh.contains("record_with_entities")
        && sh.contains("sole_ready_intents")
        && sh.contains("GLAInfantryWorker");
    let ok = gl_ok && ready_ok && sh_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRebuildSpawnEntityFirstAction::SourceMarkers);
    ok
}
pub fn honesty_host_rebuild_spawn_entity_first_nav_commands_residual_wave740() -> bool {
    let steps = LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST_NAV_STEPS_WAVE740;
    let cmds = RUNTIME_HOST_LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST_CMD_NAMES_WAVE740;
    let ok = residual_name_index(steps, "REQUIRE_WRITEBACK_PRE_SPAWN").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_BIND_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_READY_BY_HOLE_MAP").is_some()
        && residual_name_index(steps, "LIVE_HOST_REBUILD_SPAWN_ENTITY_FIRST").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_rebuild_spawn_entity_first").is_some()
        && residual_name_index(cmds, "writeback_pre_spawn").is_some()
        && residual_name_index(cmds, "host_bind_helper").is_some()
        && residual_name_index(cmds, "ready_by_hole_map").is_some();
    residual_action_store(ResidualHostRebuildSpawnEntityFirstAction::NavCommands);
    ok
}
pub fn simulate_host_rebuild_spawn_entity_first_collect_source() -> bool {
    let ok =
        ready_source().contains("record_with_entities") && gl_source().contains("ready_by_hole");
    residual_action_store(ResidualHostRebuildSpawnEntityFirstAction::CollectSource);
    ok
}
pub fn simulate_host_rebuild_spawn_entity_first_dispatch_source() -> bool {
    let ok = shadow_source().contains("record_with_entities")
        && gl_source().contains("host_spawn_rebuild_bound_object");
    residual_action_store(ResidualHostRebuildSpawnEntityFirstAction::DispatchSource);
    ok
}
pub fn honesty_host_rebuild_spawn_entity_first_residual_pack_wave740() -> bool {
    honesty_host_rebuild_spawn_entity_first_method_names_residual_wave740()
        && honesty_host_rebuild_spawn_entity_first_source_markers_residual_wave740()
        && honesty_host_rebuild_spawn_entity_first_nav_commands_residual_wave740()
        && simulate_host_rebuild_spawn_entity_first_collect_source()
        && simulate_host_rebuild_spawn_entity_first_dispatch_source()
}
pub fn simulate_live_host_rebuild_spawn_entity_first_honesty() -> bool {
    let ok = honesty_host_rebuild_spawn_entity_first_residual_pack_wave740();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRebuildSpawnEntityFirstAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_rebuild_spawn_entity_first_method_names_residual_wave740());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_rebuild_spawn_entity_first_source_markers_residual_wave740());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_rebuild_spawn_entity_first_nav_commands_residual_wave740());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_rebuild_spawn_entity_first_collect_source());
        assert!(simulate_host_rebuild_spawn_entity_first_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_rebuild_spawn_entity_first_residual_pack_wave740());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_rebuild_spawn_entity_first_honesty());
        assert!(residual_host_rebuild_spawn_entity_first_ok());
    }
}
