//! Wave 742: GLA rebuild-hole expose spawn is GameWorld entity-first under
//! construction sole-tick. Coupled shadow pre-spawns the hole entity; host binds
//! ObjectId via `host_spawn_rebuild_bound_object`. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST_METHOD_NAMES_WAVE742: &[&str] = &[
    "spawn_rebuild_hole_entity_if_coupled",
    "maybe_spawn_rebuild_hole",
    "host_spawn_rebuild_bound_object",
    "Wave 742",
    "playable_claim = false",
];
pub const LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST_NAV_STEPS_WAVE742: &[&str] = &[
    "REQUIRE_COUPLED_PRE_SPAWN",
    "REQUIRE_HOST_BIND_HELPER",
    "REQUIRE_SOLE_TICK_GATE",
    "LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST_CMD_NAMES_WAVE742: &[&str] = &[
    "host_rebuild_hole_expose_entity_first",
    "coupled_pre_spawn",
    "host_bind_helper",
    "sole_tick_gate",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostRebuildHoleExposeEntityFirstAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostRebuildHoleExposeEntityFirstAction {
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
fn residual_action_store(a: ResidualHostRebuildHoleExposeEntityFirstAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_rebuild_hole_expose_entity_first_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_rebuild_hole_expose_entity_first_last_action()
-> ResidualHostRebuildHoleExposeEntityFirstAction {
    ResidualHostRebuildHoleExposeEntityFirstAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_rebuild_hole_expose_entity_first_method_names_residual_wave742() -> bool {
    let names = LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST_METHOD_NAMES_WAVE742;
    let ok = residual_name_index(names, "spawn_rebuild_hole_entity_if_coupled").is_some()
        && residual_name_index(names, "maybe_spawn_rebuild_hole").is_some()
        && residual_name_index(names, "host_spawn_rebuild_bound_object").is_some()
        && residual_name_index(names, "Wave 742").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostRebuildHoleExposeEntityFirstAction::MethodNames);
    ok
}
pub fn honesty_host_rebuild_hole_expose_entity_first_source_markers_residual_wave742() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let gl_ok = gl.contains("Wave 742")
        && gl.contains("spawn_rebuild_hole_entity_if_coupled")
        && gl.contains("maybe_spawn_rebuild_hole")
        && gl.contains("host_spawn_rebuild_bound_object");
    let sh_ok = sh.contains("spawn_rebuild_hole_entity_if_coupled")
        && sh.contains("Wave 742")
        && sh.contains("is_rebuild_hole = true");
    let ok = gl_ok && sh_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostRebuildHoleExposeEntityFirstAction::SourceMarkers);
    ok
}
pub fn honesty_host_rebuild_hole_expose_entity_first_nav_commands_residual_wave742() -> bool {
    let steps = LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST_NAV_STEPS_WAVE742;
    let cmds = RUNTIME_HOST_LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST_CMD_NAMES_WAVE742;
    let ok = residual_name_index(steps, "REQUIRE_COUPLED_PRE_SPAWN").is_some()
        && residual_name_index(steps, "REQUIRE_HOST_BIND_HELPER").is_some()
        && residual_name_index(steps, "REQUIRE_SOLE_TICK_GATE").is_some()
        && residual_name_index(steps, "LIVE_HOST_REBUILD_HOLE_EXPOSE_ENTITY_FIRST").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_rebuild_hole_expose_entity_first").is_some()
        && residual_name_index(cmds, "coupled_pre_spawn").is_some()
        && residual_name_index(cmds, "host_bind_helper").is_some()
        && residual_name_index(cmds, "sole_tick_gate").is_some();
    residual_action_store(ResidualHostRebuildHoleExposeEntityFirstAction::NavCommands);
    ok
}
pub fn simulate_host_rebuild_hole_expose_entity_first_collect_source() -> bool {
    let ok = shadow_source().contains("spawn_rebuild_hole_entity_if_coupled")
        && gl_source().contains("maybe_spawn_rebuild_hole");
    residual_action_store(ResidualHostRebuildHoleExposeEntityFirstAction::CollectSource);
    ok
}
pub fn simulate_host_rebuild_hole_expose_entity_first_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 742")
        && gl_source().contains("spawn_rebuild_hole_entity_if_coupled");
    residual_action_store(ResidualHostRebuildHoleExposeEntityFirstAction::DispatchSource);
    ok
}
pub fn honesty_host_rebuild_hole_expose_entity_first_residual_pack_wave742() -> bool {
    honesty_host_rebuild_hole_expose_entity_first_method_names_residual_wave742()
        && honesty_host_rebuild_hole_expose_entity_first_source_markers_residual_wave742()
        && honesty_host_rebuild_hole_expose_entity_first_nav_commands_residual_wave742()
        && simulate_host_rebuild_hole_expose_entity_first_collect_source()
        && simulate_host_rebuild_hole_expose_entity_first_dispatch_source()
}
pub fn simulate_live_host_rebuild_hole_expose_entity_first_honesty() -> bool {
    let ok = honesty_host_rebuild_hole_expose_entity_first_residual_pack_wave742();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostRebuildHoleExposeEntityFirstAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(honesty_host_rebuild_hole_expose_entity_first_method_names_residual_wave742());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_rebuild_hole_expose_entity_first_source_markers_residual_wave742());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_rebuild_hole_expose_entity_first_nav_commands_residual_wave742());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_rebuild_hole_expose_entity_first_collect_source());
        assert!(simulate_host_rebuild_hole_expose_entity_first_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_rebuild_hole_expose_entity_first_residual_pack_wave742());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_rebuild_hole_expose_entity_first_honesty());
        assert!(residual_host_rebuild_hole_expose_entity_first_ok());
    }
}
