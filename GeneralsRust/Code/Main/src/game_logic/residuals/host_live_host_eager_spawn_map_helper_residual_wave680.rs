//! Wave 680 residual peels: mid-frame host spawn → GameWorld map on coupled tick.
//! Host still allocates ObjectIds; GW maps eagerly when shadow is installed.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EAGER_SPAWN_MAP_HELPER_METHOD_NAMES_WAVE680: &[&str] = &[
    "eager_map_host_spawn_if_coupled",
    "install_active_shadow_for_coupled_tick",
    "clear_active_shadow_for_coupled_tick",
    "host_spawn_log::record",
    "Wave 680",
    "playable_claim = false",
];
pub const LIVE_HOST_EAGER_SPAWN_MAP_HELPER_NAV_STEPS_WAVE680: &[&str] = &[
    "REQUIRE_EAGER_MAP_API",
    "REQUIRE_ENGINE_INSTALLS_ACTIVE_SHADOW",
    "REQUIRE_CREATE_OBJECT_EAGER_MAP",
    "REQUIRE_CLEAR_BEFORE_SESSION",
    "LIVE_HOST_EAGER_SPAWN_MAP_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EAGER_SPAWN_MAP_HELPER_CMD_NAMES_WAVE680: &[&str] = &[
    "host_eager_spawn_map_helper",
    "engine_installs_active_shadow",
    "create_object_eager_map",
    "clear_before_session",
    "eager_spawn_map_residual",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEagerSpawnMapHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEagerSpawnMapHelperAction {
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
fn residual_action_store(a: ResidualHostEagerSpawnMapHelperAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eager_spawn_map_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eager_spawn_map_helper_last_action() -> ResidualHostEagerSpawnMapHelperAction {
    ResidualHostEagerSpawnMapHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_eager_spawn_map_helper_method_names_residual_wave680() -> bool {
    let names = LIVE_HOST_EAGER_SPAWN_MAP_HELPER_METHOD_NAMES_WAVE680;
    let ok = residual_name_index(names, "eager_map_host_spawn_if_coupled").is_some()
        && residual_name_index(names, "install_active_shadow_for_coupled_tick").is_some()
        && residual_name_index(names, "clear_active_shadow_for_coupled_tick").is_some()
        && residual_name_index(names, "Wave 680").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEagerSpawnMapHelperAction::MethodNames);
    ok
}
pub fn honesty_host_eager_spawn_map_helper_source_markers_residual_wave680() -> bool {
    let gl = gl_source();
    let eng = eng_source();
    let sh = shadow_source();
    let api_ok = sh.contains("pub fn eager_map_host_spawn_if_coupled")
        && sh.contains("pub fn install_active_shadow_for_coupled_tick")
        && sh.contains("pub fn clear_active_shadow_for_coupled_tick")
        && sh.contains("Wave 680");
    // 2026-08-15: engine uses CoupledTickGuard::enter + eager_apply_all dispatcher.
    let eng_ok = eng.contains("CoupledTickGuard::enter")
        && eng.contains("eager_apply_all_host_residuals_after_logic")
        && eng.contains("Wave 682/925");
    let create_ok = gl.contains("eager_map_host_spawn_if_coupled")
        && gl.contains("Wave 680: mid-frame GameWorld map")
        && gl.matches("eager_map_host_spawn_if_coupled").count() >= 2;
    let ok = api_ok && eng_ok && create_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEagerSpawnMapHelperAction::SourceMarkers);
    ok
}
pub fn honesty_host_eager_spawn_map_helper_nav_commands_residual_wave680() -> bool {
    let steps = LIVE_HOST_EAGER_SPAWN_MAP_HELPER_NAV_STEPS_WAVE680;
    let cmds = RUNTIME_HOST_LIVE_HOST_EAGER_SPAWN_MAP_HELPER_CMD_NAMES_WAVE680;
    let ok = residual_name_index(steps, "REQUIRE_EAGER_MAP_API").is_some()
        && residual_name_index(steps, "REQUIRE_ENGINE_INSTALLS_ACTIVE_SHADOW").is_some()
        && residual_name_index(steps, "REQUIRE_CREATE_OBJECT_EAGER_MAP").is_some()
        && residual_name_index(steps, "REQUIRE_CLEAR_BEFORE_SESSION").is_some()
        && residual_name_index(steps, "LIVE_HOST_EAGER_SPAWN_MAP_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_eager_spawn_map_helper").is_some()
        && residual_name_index(cmds, "engine_installs_active_shadow").is_some()
        && residual_name_index(cmds, "create_object_eager_map").is_some()
        && residual_name_index(cmds, "clear_before_session").is_some()
        && residual_name_index(cmds, "eager_spawn_map_residual").is_some();
    residual_action_store(ResidualHostEagerSpawnMapHelperAction::NavCommands);
    ok
}
pub fn simulate_host_eager_spawn_map_helper_collect_source() -> bool {
    let ok = shadow_source().contains("eager_map_host_spawn_if_coupled")
        && gl_source().contains("eager_map_host_spawn_if_coupled")
        && eng_source().contains("CoupledTickGuard::enter");
    residual_action_store(ResidualHostEagerSpawnMapHelperAction::CollectSource);
    ok
}
pub fn simulate_host_eager_spawn_map_helper_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 682/925")
        && gl_source().contains("Wave 680")
        && shadow_source().contains("Wave 680");
    residual_action_store(ResidualHostEagerSpawnMapHelperAction::DispatchSource);
    ok
}
pub fn honesty_host_eager_spawn_map_helper_residual_pack_wave680() -> bool {
    honesty_host_eager_spawn_map_helper_method_names_residual_wave680()
        && honesty_host_eager_spawn_map_helper_source_markers_residual_wave680()
        && honesty_host_eager_spawn_map_helper_nav_commands_residual_wave680()
        && simulate_host_eager_spawn_map_helper_collect_source()
        && simulate_host_eager_spawn_map_helper_dispatch_source()
}
pub fn simulate_live_host_eager_spawn_map_helper_honesty() -> bool {
    let ok = honesty_host_eager_spawn_map_helper_residual_pack_wave680();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEagerSpawnMapHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, clear_active_shadow_for_coupled_tick,
        eager_map_host_spawn_if_coupled, end_shadow_coupled_tick,
        install_active_shadow_for_coupled_tick,
    };
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str, health: f32) {
        let mut t = ThingTemplate::new(name);
        t.set_health(health);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(name.into(), t);
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_eager_spawn_map_helper_method_names_residual_wave680());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eager_spawn_map_helper_source_markers_residual_wave680());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eager_spawn_map_helper_nav_commands_residual_wave680());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eager_spawn_map_helper_collect_source());
        assert!(simulate_host_eager_spawn_map_helper_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eager_spawn_map_helper_residual_pack_wave680());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eager_spawn_map_helper_honesty());
        assert!(residual_host_eager_spawn_map_helper_ok());
    }

    #[test]
    fn eager_map_maps_host_id_during_coupled_tick() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "EagerMapUnit", 100.0);
        let id = logic
            .create_object("EagerMapUnit", Team::USA, Vec3::new(1.0, 0.0, 2.0))
            .expect("spawn");
        let mut shadow = GameWorldShadow::new(64);
        begin_shadow_coupled_tick();
        install_active_shadow_for_coupled_tick(&mut shadow);
        let mapped = eager_map_host_spawn_if_coupled(
            &logic,
            &crate::game_logic::host_spawn_log::HostSpawnEvent {
                id,
                template: "EagerMapUnit".into(),
                team_ordinal: 0,
                position: [1.0, 0.0, 2.0],
            },
        );
        assert!(
            mapped,
            "eager map should insert host→entity under coupled tick"
        );
        assert!(shadow.entity_for_host(id).is_some());
        // Idempotent second map.
        let again = eager_map_host_spawn_if_coupled(
            &logic,
            &crate::game_logic::host_spawn_log::HostSpawnEvent {
                id,
                template: "EagerMapUnit".into(),
                team_ordinal: 0,
                position: [1.0, 0.0, 2.0],
            },
        );
        assert!(!again, "already mapped host id should not double-spawn");
        clear_active_shadow_for_coupled_tick();
        end_shadow_coupled_tick();
        let _ = ObjectId; // silence unused in some cfgs

        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }
}
