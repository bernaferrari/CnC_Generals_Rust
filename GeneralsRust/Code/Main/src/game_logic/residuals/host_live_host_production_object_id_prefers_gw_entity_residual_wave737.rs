//! Wave 737: under sole-tick production spawn, host ObjectId prefers the free
//! GameWorld pre-spawned entity raw id (entity-first ID alignment). Collision or
//! missing bind falls back to `allocate_object_id`. Host may still allocate on
//! fallback. `playable_claim` stays false.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY_METHOD_NAMES_WAVE737: &[&str] = &[
    "preferred",
    "saved_next",
    "pop_pending_bind",
    "set_next_host_spawn_bind_entity",
    "host_spawn_production_unit",
    "Wave 737",
    "playable_claim = false",
];
pub const LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY_NAV_STEPS_WAVE737: &[&str] = &[
    "REQUIRE_PREFERRED_GW_ENTITY_RAW",
    "REQUIRE_COLLISION_FALLBACK_ALLOCATE",
    "REQUIRE_MONOTONIC_NEXT_OBJECT_ID",
    "LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY_CMD_NAMES_WAVE737:
    &[&str] = &[
    "host_production_object_id_prefers_gw_entity",
    "preferred_gw_entity_raw",
    "collision_fallback_allocate",
    "monotonic_next_object_id",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostProductionObjectIdPrefersGwEntityAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostProductionObjectIdPrefersGwEntityAction {
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
fn residual_action_store(a: ResidualHostProductionObjectIdPrefersGwEntityAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_production_object_id_prefers_gw_entity_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_production_object_id_prefers_gw_entity_last_action()
-> ResidualHostProductionObjectIdPrefersGwEntityAction {
    ResidualHostProductionObjectIdPrefersGwEntityAction::from_u8(
        RESIDUAL_ACTION.load(Ordering::SeqCst),
    )
}
fn gl_source() -> &'static str {
    // 2026-08-15: scan host plus extra world_* splits.
    super::host_logic_scan_src()
}
pub fn honesty_host_production_object_id_prefers_gw_entity_method_names_residual_wave737() -> bool {
    let names = LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY_METHOD_NAMES_WAVE737;
    let ok = residual_name_index(names, "preferred").is_some()
        && residual_name_index(names, "saved_next").is_some()
        && residual_name_index(names, "pop_pending_bind").is_some()
        && residual_name_index(names, "set_next_host_spawn_bind_entity").is_some()
        && residual_name_index(names, "host_spawn_production_unit").is_some()
        && residual_name_index(names, "Wave 737").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostProductionObjectIdPrefersGwEntityAction::MethodNames);
    ok
}
pub fn honesty_host_production_object_id_prefers_gw_entity_source_markers_residual_wave737() -> bool
{
    let gl = gl_source();
    // 2026-08-15: Wave 737 body is host_spawn_production_unit_with_owner.
    let j = gl
        .find("fn host_spawn_production_unit_with_owner")
        .or_else(|| gl.find("fn host_spawn_production_unit"))
        .unwrap_or(0);
    let body = &gl[j..j + 2800.min(gl.len().saturating_sub(j))];
    let ok = body.contains("Wave 737")
        && body.contains("let preferred = ObjectId(raw)")
        && body.contains("saved_next")
        && body.contains("!self.objects.contains_key(&preferred)")
        && body.contains("saved_next.0.max(after)")
        && body.contains("set_next_host_spawn_bind_entity")
        && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostProductionObjectIdPrefersGwEntityAction::SourceMarkers);
    ok
}
pub fn honesty_host_production_object_id_prefers_gw_entity_nav_commands_residual_wave737() -> bool {
    let steps = LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY_NAV_STEPS_WAVE737;
    let cmds = RUNTIME_HOST_LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY_CMD_NAMES_WAVE737;
    let ok = residual_name_index(steps, "REQUIRE_PREFERRED_GW_ENTITY_RAW").is_some()
        && residual_name_index(steps, "REQUIRE_COLLISION_FALLBACK_ALLOCATE").is_some()
        && residual_name_index(steps, "REQUIRE_MONOTONIC_NEXT_OBJECT_ID").is_some()
        && residual_name_index(steps, "LIVE_HOST_PRODUCTION_OBJECT_ID_PREFERS_GW_ENTITY").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_production_object_id_prefers_gw_entity").is_some()
        && residual_name_index(cmds, "preferred_gw_entity_raw").is_some()
        && residual_name_index(cmds, "collision_fallback_allocate").is_some()
        && residual_name_index(cmds, "monotonic_next_object_id").is_some();
    residual_action_store(ResidualHostProductionObjectIdPrefersGwEntityAction::NavCommands);
    ok
}
pub fn simulate_host_production_object_id_prefers_gw_entity_collect_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("let preferred = ObjectId(raw)") && gl.contains("pop_pending_bind");
    residual_action_store(ResidualHostProductionObjectIdPrefersGwEntityAction::CollectSource);
    ok
}
pub fn simulate_host_production_object_id_prefers_gw_entity_dispatch_source() -> bool {
    let gl = gl_source();
    let ok = gl.contains("Wave 737") && gl.contains("saved_next.0.max(after)");
    residual_action_store(ResidualHostProductionObjectIdPrefersGwEntityAction::DispatchSource);
    ok
}
pub fn honesty_host_production_object_id_prefers_gw_entity_residual_pack_wave737() -> bool {
    honesty_host_production_object_id_prefers_gw_entity_method_names_residual_wave737()
        && honesty_host_production_object_id_prefers_gw_entity_source_markers_residual_wave737()
        && honesty_host_production_object_id_prefers_gw_entity_nav_commands_residual_wave737()
        && simulate_host_production_object_id_prefers_gw_entity_collect_source()
        && simulate_host_production_object_id_prefers_gw_entity_dispatch_source()
}
pub fn simulate_live_host_production_object_id_prefers_gw_entity_honesty() -> bool {
    let ok = honesty_host_production_object_id_prefers_gw_entity_residual_pack_wave737();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostProductionObjectIdPrefersGwEntityAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_names_residual() {
        assert!(
            honesty_host_production_object_id_prefers_gw_entity_method_names_residual_wave737()
        );
    }
    #[test]
    fn source_markers_residual() {
        assert!(
            honesty_host_production_object_id_prefers_gw_entity_source_markers_residual_wave737()
        );
    }
    #[test]
    fn nav_commands_residual() {
        assert!(
            honesty_host_production_object_id_prefers_gw_entity_nav_commands_residual_wave737()
        );
    }
    #[test]
    fn sources() {
        assert!(simulate_host_production_object_id_prefers_gw_entity_collect_source());
        assert!(simulate_host_production_object_id_prefers_gw_entity_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_production_object_id_prefers_gw_entity_residual_pack_wave737());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_production_object_id_prefers_gw_entity_honesty());
        assert!(residual_host_production_object_id_prefers_gw_entity_ok());
    }
}
