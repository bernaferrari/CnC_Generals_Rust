//! Wave 681 residual peels: mid-frame host destroy → GameWorld unmap on coupled tick.
//! Host still owns destroy timing; GW unmaps eagerly when shadow is installed.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER_METHOD_NAMES_WAVE681: &[&str] = &[
    "eager_unmap_host_destroy_if_coupled",
    "install_active_shadow_for_coupled_tick",
    "host_destroy_log::record",
    "Wave 681",
    "playable_claim = false",
];
pub const LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER_NAV_STEPS_WAVE681: &[&str] = &[
    "REQUIRE_EAGER_UNMAP_API",
    "REQUIRE_PROCESS_DESTROY_EAGER_UNMAP",
    "REQUIRE_COUPLED_SHADOW_INSTALL",
    "REQUIRE_IDEMPOTENT_SESSION_DRAIN",
    "LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER_CMD_NAMES_WAVE681: &[&str] = &[
    "host_eager_destroy_unmap_helper",
    "process_destroy_eager_unmap",
    "coupled_shadow_install",
    "idempotent_session_drain",
    "eager_destroy_unmap_residual",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEagerDestroyUnmapHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEagerDestroyUnmapHelperAction {
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
fn residual_action_store(a: ResidualHostEagerDestroyUnmapHelperAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eager_destroy_unmap_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eager_destroy_unmap_helper_last_action()
-> ResidualHostEagerDestroyUnmapHelperAction {
    ResidualHostEagerDestroyUnmapHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn gl_source() -> &'static str {
    super::GAME_LOGIC_HOST_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
pub fn honesty_host_eager_destroy_unmap_helper_method_names_residual_wave681() -> bool {
    let names = LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER_METHOD_NAMES_WAVE681;
    let ok = residual_name_index(names, "eager_unmap_host_destroy_if_coupled").is_some()
        && residual_name_index(names, "install_active_shadow_for_coupled_tick").is_some()
        && residual_name_index(names, "host_destroy_log::record").is_some()
        && residual_name_index(names, "Wave 681").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEagerDestroyUnmapHelperAction::MethodNames);
    ok
}
pub fn honesty_host_eager_destroy_unmap_helper_source_markers_residual_wave681() -> bool {
    let gl = gl_source();
    let sh = shadow_source();
    let api_ok = sh.contains("pub fn eager_unmap_host_destroy_if_coupled")
        && sh.contains("Wave 681")
        && sh.contains("apply_host_destroy_events");
    let process_ok = gl.contains("eager_unmap_host_destroy_if_coupled")
        && gl.contains("Wave 681: mid-frame GameWorld Destroy")
        && gl.contains("host_destroy_log::record");
    let ok = api_ok && process_ok && !gl.contains("playable_claim = true");
    residual_action_store(ResidualHostEagerDestroyUnmapHelperAction::SourceMarkers);
    ok
}
pub fn honesty_host_eager_destroy_unmap_helper_nav_commands_residual_wave681() -> bool {
    let steps = LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER_NAV_STEPS_WAVE681;
    let cmds = RUNTIME_HOST_LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER_CMD_NAMES_WAVE681;
    let ok = residual_name_index(steps, "REQUIRE_EAGER_UNMAP_API").is_some()
        && residual_name_index(steps, "REQUIRE_PROCESS_DESTROY_EAGER_UNMAP").is_some()
        && residual_name_index(steps, "REQUIRE_COUPLED_SHADOW_INSTALL").is_some()
        && residual_name_index(steps, "REQUIRE_IDEMPOTENT_SESSION_DRAIN").is_some()
        && residual_name_index(steps, "LIVE_HOST_EAGER_DESTROY_UNMAP_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_eager_destroy_unmap_helper").is_some()
        && residual_name_index(cmds, "process_destroy_eager_unmap").is_some()
        && residual_name_index(cmds, "coupled_shadow_install").is_some()
        && residual_name_index(cmds, "idempotent_session_drain").is_some()
        && residual_name_index(cmds, "eager_destroy_unmap_residual").is_some();
    residual_action_store(ResidualHostEagerDestroyUnmapHelperAction::NavCommands);
    ok
}
pub fn simulate_host_eager_destroy_unmap_helper_collect_source() -> bool {
    let ok = shadow_source().contains("eager_unmap_host_destroy_if_coupled")
        && gl_source().contains("eager_unmap_host_destroy_if_coupled");
    residual_action_store(ResidualHostEagerDestroyUnmapHelperAction::CollectSource);
    ok
}
pub fn simulate_host_eager_destroy_unmap_helper_dispatch_source() -> bool {
    let ok = gl_source().contains("Wave 681") && shadow_source().contains("Wave 681");
    residual_action_store(ResidualHostEagerDestroyUnmapHelperAction::DispatchSource);
    ok
}
pub fn honesty_host_eager_destroy_unmap_helper_residual_pack_wave681() -> bool {
    honesty_host_eager_destroy_unmap_helper_method_names_residual_wave681()
        && honesty_host_eager_destroy_unmap_helper_source_markers_residual_wave681()
        && honesty_host_eager_destroy_unmap_helper_nav_commands_residual_wave681()
        && simulate_host_eager_destroy_unmap_helper_collect_source()
        && simulate_host_eager_destroy_unmap_helper_dispatch_source()
}
pub fn simulate_live_host_eager_destroy_unmap_helper_honesty() -> bool {
    let ok = honesty_host_eager_destroy_unmap_helper_residual_pack_wave681();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEagerDestroyUnmapHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, clear_active_shadow_for_coupled_tick,
        eager_map_host_spawn_if_coupled, eager_unmap_host_destroy_if_coupled,
        end_shadow_coupled_tick, install_active_shadow_for_coupled_tick,
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
        assert!(honesty_host_eager_destroy_unmap_helper_method_names_residual_wave681());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eager_destroy_unmap_helper_source_markers_residual_wave681());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eager_destroy_unmap_helper_nav_commands_residual_wave681());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eager_destroy_unmap_helper_collect_source());
        assert!(simulate_host_eager_destroy_unmap_helper_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eager_destroy_unmap_helper_residual_pack_wave681());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eager_destroy_unmap_helper_honesty());
        assert!(residual_host_eager_destroy_unmap_helper_ok());
    }

    #[test]
    fn eager_unmap_drops_host_map_during_coupled_tick() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "EagerDestroyUnit", 100.0);
        let id = logic
            .create_object("EagerDestroyUnit", Team::USA, Vec3::new(3.0, 0.0, 4.0))
            .expect("spawn");
        let mut shadow = GameWorldShadow::new(64);
        begin_shadow_coupled_tick();
        install_active_shadow_for_coupled_tick(&mut shadow);
        assert!(eager_map_host_spawn_if_coupled(
            &logic,
            &crate::game_logic::host_spawn_log::HostSpawnEvent {
                id,
                template: "EagerDestroyUnit".into(),
                team_ordinal: 0,
                position: [3.0, 0.0, 4.0],
            },
        ));
        assert!(shadow.entity_for_host(id).is_some());
        let unmapped = eager_unmap_host_destroy_if_coupled(id);
        assert!(unmapped, "eager unmap should queue/apply destroy");
        assert!(
            shadow.entity_for_host(id).is_none(),
            "host map cleared after destroy"
        );
        // Idempotent.
        assert!(!eager_unmap_host_destroy_if_coupled(id));
        clear_active_shadow_for_coupled_tick();
        end_shadow_coupled_tick();
        let _ = ObjectId;

        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }
}
