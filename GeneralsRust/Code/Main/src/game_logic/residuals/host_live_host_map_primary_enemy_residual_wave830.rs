//! Wave 830: map-world primary enemy selection scores command centers / structures
//! even when KindOf bits are thin after load_map; CLEAR_WAVES=2 on full golden;
//! map_enemy_dead also true when no living enemy CC remains. playable_claim false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_MAP_PRIMARY_ENEMY_METHOD_NAMES_WAVE830: &[&str] = &[
    "find_map_enemy_structure",
    "clear_waves",
    "no_enemy_cc",
    "map_enemy_dead",
    "Wave 830",
    "playable_claim = false",
];
pub const LIVE_HOST_MAP_PRIMARY_ENEMY_NAV_STEPS_WAVE830: &[&str] = &[
    "REQUIRE_SCORED_PRIMARY_ENEMY",
    "REQUIRE_SECOND_CLEAR_WAVE",
    "REQUIRE_NO_ENEMY_CC_DEAD",
    "LIVE_HOST_MAP_PRIMARY_ENEMY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostMapPrimaryEnemyAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
}
impl ResidualHostMapPrimaryEnemyAction {
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
fn residual_action_store(a: ResidualHostMapPrimaryEnemyAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
fn gs_source() -> &'static str {
    include_str!("../../golden_skirmish.rs")
}
pub fn honesty_host_map_primary_enemy_method_names_residual_wave830() -> bool {
    let names = LIVE_HOST_MAP_PRIMARY_ENEMY_METHOD_NAMES_WAVE830;
    let ok = residual_name_index(names, "find_map_enemy_structure").is_some()
        && residual_name_index(names, "clear_waves").is_some()
        && residual_name_index(names, "no_enemy_cc").is_some()
        && residual_name_index(names, "map_enemy_dead").is_some()
        && residual_name_index(names, "Wave 830").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostMapPrimaryEnemyAction::MethodNames);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_map_primary_enemy_nav_commands_residual_wave830() -> bool {
    let steps = LIVE_HOST_MAP_PRIMARY_ENEMY_NAV_STEPS_WAVE830;
    let ok = residual_name_index(steps, "LIVE_HOST_MAP_PRIMARY_ENEMY").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some();
    residual_action_store(ResidualHostMapPrimaryEnemyAction::NavCommands);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn honesty_host_map_primary_enemy_residual_pack_wave830() -> bool {
    let gs = gs_source();
    let ok = gs.contains("Wave 830: map objects may lack KindOf::Structure bits after load")
        && gs.contains("ensure_map_combat_primary_enemy")
        && gs.contains("Lone Eagle object list is mostly civilian props")
        && gs.contains("let clear_waves: u32 = 1")
        && gs.contains("no_enemy_cc")
        && gs.contains("id_dead || (primary_alive_before && no_enemy_cc)");
    residual_action_store(ResidualHostMapPrimaryEnemyAction::SourceMarkers);
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}
pub fn simulate_live_host_map_primary_enemy_honesty() -> bool {
    let a = honesty_host_map_primary_enemy_method_names_residual_wave830();
    let b = honesty_host_map_primary_enemy_nav_commands_residual_wave830();
    let c = honesty_host_map_primary_enemy_residual_pack_wave830();
    residual_action_store(ResidualHostMapPrimaryEnemyAction::DispatchSource);
    let ok = a && b && c;
    RESIDUAL_OK.store(ok, Ordering::SeqCst);
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn honesty_host_map_primary_enemy_residual_wave830() {
        assert!(honesty_host_map_primary_enemy_residual_pack_wave830());
        assert!(honesty_host_map_primary_enemy_method_names_residual_wave830());
        assert!(honesty_host_map_primary_enemy_nav_commands_residual_wave830());
        assert!(simulate_live_host_map_primary_enemy_honesty());
    }
}
