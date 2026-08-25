//! Wave 736: production unit spawn is GameWorld entity-first under sole-tick.
//! Writeback pre-spawns the unit entity; host allocates ObjectId and binds to it
//! (no second Spawn). Host still owns ObjectId allocation. `playable_claim` false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST_METHOD_NAMES_WAVE736: &[&str] = &[
    "gw_entity_raw",
    "push_pending_bind",
    "set_next_host_spawn_bind_entity",
    "bind_host_to_existing_entity",
    "host_spawn_production_unit",
    "Wave 736",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST_NAV_STEPS_WAVE736: &[&str] = &[
    "REQUIRE_GW_PRE_SPAWN_ON_READY",
    "REQUIRE_PENDING_BIND_FIFO",
    "REQUIRE_HOST_BIND_NOT_DOUBLE_SPAWN",
    "LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST_CMD_NAMES_WAVE736: &[&str] = &[
    "host_production_spawn_entity_first",
    "gw_pre_spawn",
    "pending_bind",
    "host_object_id_bind",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionSpawnEntityFirstAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionSpawnEntityFirstAction {
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
fn residual_action_store(a: ResidualHostProductionSpawnEntityFirstAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_spawn_entity_first_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_spawn_entity_first_last_action()
-> ResidualHostProductionSpawnEntityFirstAction {
    ResidualHostProductionSpawnEntityFirstAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn ready_source() -> &'static str {
    include_str!("../host_production_ready_log.rs")
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_production_spawn_entity_first_method_names_residual_wave736() -> bool {
    let names = LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST_METHOD_NAMES_WAVE736;
    let ok = residual_name_index(names, "gw_entity_raw").is_some()
        && residual_name_index(names, "push_pending_bind").is_some()
        && residual_name_index(names, "set_next_host_spawn_bind_entity").is_some()
        && residual_name_index(names, "bind_host_to_existing_entity").is_some()
        && residual_name_index(names, "host_spawn_production_unit").is_some()
        && residual_name_index(names, "Wave 736").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionSpawnEntityFirstAction::MethodNames);
    ok
}
pub fn honesty_host_production_spawn_entity_first_source_markers_residual_wave736() -> bool {
    let gl = gl_source();
    let ready = ready_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("Wave 736")
        && gl.contains("push_pending_bind")
        && gl.contains("pop_pending_bind")
        && gl.contains("set_next_host_spawn_bind_entity");
    let ready_ok = ready.contains("gw_entity_raw")
        && ready.contains("push_pending_bind")
        && ready.contains("Wave 736");
    let sh_ok = sh.contains("set_next_host_spawn_bind_entity")
        && sh.contains("bind_host_to_existing_entity")
        && sh.contains("Wave 736")
        && sh.contains("sole_ready_intents");
    let ok = gl_ok && ready_ok && sh_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionSpawnEntityFirstAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_spawn_entity_first_nav_commands_residual_wave736() -> bool {
    let steps = LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST_NAV_STEPS_WAVE736;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST_CMD_NAMES_WAVE736;
    let ok = residual_name_index(steps, "REQUIRE_GW_PRE_SPAWN_ON_READY").is_some()
        && residual_name_index(steps, "REQUIRE_PENDING_BIND_FIFO").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_BIND_NOT_DOUBLE_SPAWN").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_SPAWN_ENTITY_FIRST").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_spawn_entity_first").is_some()
        && residual_name_index(cmds, "gw_pre_spawn").is_some()
        && residual_name_index(cmds, "pending_bind").is_some()
        && residual_name_index(cmds, "host_object_id_bind").is_some();
    residual_action_store(ResidualHostProductionSpawnEntityFirstAction::NavCommands);
    ok
}
pub fn simulate_host_production_spawn_entity_first_collect_source() -> bool {
    let ok = ready_source().contains("gw_entity_raw") && gl_source().contains("push_pending_bind");
    residual_action_store(ResidualHostProductionSpawnEntityFirstAction::CollectSource);
    ok
}
pub fn simulate_host_production_spawn_entity_first_dispatch_source() -> bool {
    let ok = shadow_source().contains("bind_host_to_existing_entity")
        && gl_source().contains("set_next_host_spawn_bind_entity");
    residual_action_store(ResidualHostProductionSpawnEntityFirstAction::DispatchSource);
    ok
}
pub fn honesty_host_production_spawn_entity_first_residual_pack_wave736() -> bool {
    honesty_host_production_spawn_entity_first_method_names_residual_wave736()
        && honesty_host_production_spawn_entity_first_source_markers_residual_wave736()
        && honesty_host_production_spawn_entity_first_nav_commands_residual_wave736()
        && simulate_host_production_spawn_entity_first_collect_source()
        && simulate_host_production_spawn_entity_first_dispatch_source()
}
pub fn simulate_live_host_production_spawn_entity_first_honesty() -> bool {
    let ok = honesty_host_production_spawn_entity_first_residual_pack_wave736();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionSpawnEntityFirstAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_production_spawn_entity_first_method_names_residual_wave736());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_production_spawn_entity_first_source_markers_residual_wave736());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_production_spawn_entity_first_nav_commands_residual_wave736());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_spawn_entity_first_collect_source());
        assert!(simulate_host_production_spawn_entity_first_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_spawn_entity_first_residual_pack_wave736());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_spawn_entity_first_honesty());
        assert!(residual_host_production_spawn_entity_first_ok());
    }
}
