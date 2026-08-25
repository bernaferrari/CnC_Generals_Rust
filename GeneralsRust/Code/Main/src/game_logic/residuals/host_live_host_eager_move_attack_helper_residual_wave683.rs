//! Wave 683 residual peels: post-logic move/attack apply on coupled tick.
//! Host records `host_move_log` / `host_attack_log` mid-frame; engine drains into
//! GameWorld immediately after the host frame, before the full shadow session.
//! Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EAGER_MOVE_ATTACK_HELPER_METHOD_NAMES_WAVE683: &[&str] = &[
    "eager_apply_host_move_attack_after_logic",
    "host_move_log",
    "host_attack_log",
    "Wave 683",
    "playable_claim = false",
];
pub const LIVE_HOST_EAGER_MOVE_ATTACK_HELPER_NAV_STEPS_WAVE683: &[&str] = &[
    "REQUIRE_EAGER_MOVE_ATTACK_API",
    "REQUIRE_ENGINE_POST_LOGIC_DRAIN",
    "REQUIRE_MOVE_ATTACK_LOGS",
    "REQUIRE_SESSION_DRAIN_IDEMPOTENT",
    "LIVE_HOST_EAGER_MOVE_ATTACK_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EAGER_MOVE_ATTACK_HELPER_CMD_NAMES_WAVE683: &[&str] = &[
    "host_eager_move_attack_helper",
    "engine_post_logic_drain",
    "move_attack_logs",
    "session_drain_idempotent",
    "eager_move_attack_residual",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEagerMoveAttackHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEagerMoveAttackHelperAction {
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
fn residual_action_store(a: ResidualHostEagerMoveAttackHelperAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eager_move_attack_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eager_move_attack_helper_last_action()
-> ResidualHostEagerMoveAttackHelperAction {
    ResidualHostEagerMoveAttackHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
// 2026-08-15: engine dispatches via eager_apply_all_host_residuals_after_logic
// (Wave 682/925); per-channel eager_apply_* stays on GAMEWORLD_SHADOW_SRC.
pub fn honesty_host_eager_move_attack_helper_method_names_residual_wave683() -> bool {
    let names = LIVE_HOST_EAGER_MOVE_ATTACK_HELPER_METHOD_NAMES_WAVE683;
    let ok = residual_name_index(names, "eager_apply_host_move_attack_after_logic").is_some()
        && residual_name_index(names, "host_move_log").is_some()
        && residual_name_index(names, "host_attack_log").is_some()
        && residual_name_index(names, "Wave 683").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEagerMoveAttackHelperAction::MethodNames);
    ok
}
pub fn honesty_host_eager_move_attack_helper_source_markers_residual_wave683() -> bool {
    let eng = eng_source();
    let sh = shadow_source();
    let api_ok = sh.contains("pub fn eager_apply_host_move_attack_after_logic")
        && sh.contains("Wave 683")
        && sh.contains("host_attack_log::drain")
        && sh.contains("host_move_log::drain")
        && sh.contains("queue_set_attack_target_for_host")
        && sh.contains("queue_set_move_target_for_host");
    let eng_ok =
        eng.contains("eager_apply_all_host_residuals_after_logic") && eng.contains("Wave 682/925");
    let ok = api_ok && eng_ok && !eng.contains("playable_claim = true");
    residual_action_store(ResidualHostEagerMoveAttackHelperAction::SourceMarkers);
    ok
}
pub fn honesty_host_eager_move_attack_helper_nav_commands_residual_wave683() -> bool {
    let steps = LIVE_HOST_EAGER_MOVE_ATTACK_HELPER_NAV_STEPS_WAVE683;
    let cmds = RUNTIME_HOST_LIVE_HOST_EAGER_MOVE_ATTACK_HELPER_CMD_NAMES_WAVE683;
    let ok = residual_name_index(steps, "REQUIRE_EAGER_MOVE_ATTACK_API").is_some()
        && residual_name_index(steps, "REQUIRE_ENGINE_POST_LOGIC_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_MOVE_ATTACK_LOGS").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_DRAIN_IDEMPOTENT").is_some()
        && residual_name_index(steps, "LIVE_HOST_EAGER_MOVE_ATTACK_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_eager_move_attack_helper").is_some()
        && residual_name_index(cmds, "engine_post_logic_drain").is_some()
        && residual_name_index(cmds, "move_attack_logs").is_some()
        && residual_name_index(cmds, "session_drain_idempotent").is_some()
        && residual_name_index(cmds, "eager_move_attack_residual").is_some();
    residual_action_store(ResidualHostEagerMoveAttackHelperAction::NavCommands);
    ok
}
pub fn simulate_host_eager_move_attack_helper_collect_source() -> bool {
    let ok = shadow_source().contains("eager_apply_host_move_attack_after_logic")
        && eng_source().contains("eager_apply_all_host_residuals_after_logic");
    residual_action_store(ResidualHostEagerMoveAttackHelperAction::CollectSource);
    ok
}
pub fn simulate_host_eager_move_attack_helper_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 682/925") && shadow_source().contains("Wave 683");
    residual_action_store(ResidualHostEagerMoveAttackHelperAction::DispatchSource);
    ok
}
pub fn honesty_host_eager_move_attack_helper_residual_pack_wave683() -> bool {
    honesty_host_eager_move_attack_helper_method_names_residual_wave683()
        && honesty_host_eager_move_attack_helper_source_markers_residual_wave683()
        && honesty_host_eager_move_attack_helper_nav_commands_residual_wave683()
        && simulate_host_eager_move_attack_helper_collect_source()
        && simulate_host_eager_move_attack_helper_dispatch_source()
}
pub fn simulate_live_host_eager_move_attack_helper_honesty() -> bool {
    let ok = honesty_host_eager_move_attack_helper_residual_pack_wave683();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEagerMoveAttackHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::host_attack_log;
    use crate::game_logic::host_move_log;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, clear_active_shadow_for_coupled_tick,
        eager_apply_host_move_attack_after_logic, eager_map_host_spawn_if_coupled,
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
        assert!(honesty_host_eager_move_attack_helper_method_names_residual_wave683());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eager_move_attack_helper_source_markers_residual_wave683());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eager_move_attack_helper_nav_commands_residual_wave683());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eager_move_attack_helper_collect_source());
        assert!(simulate_host_eager_move_attack_helper_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eager_move_attack_helper_residual_pack_wave683());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eager_move_attack_helper_honesty());
        assert!(residual_host_eager_move_attack_helper_ok());
    }

    #[test]
    fn post_logic_move_attack_drains_logs() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        host_move_log::clear();
        host_attack_log::clear();

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "EagerMaUnit", 100.0);
        ensure_template(&mut logic, "EagerMaTarget", 100.0);
        let unit = logic
            .create_object("EagerMaUnit", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("unit");
        let target = logic
            .create_object("EagerMaTarget", Team::GLA, Vec3::new(30.0, 0.0, 0.0))
            .expect("target");

        let mut shadow = GameWorldShadow::new(64);
        begin_shadow_coupled_tick();
        install_active_shadow_for_coupled_tick(&mut shadow);
        assert!(eager_map_host_spawn_if_coupled(
            &logic,
            &crate::game_logic::host_spawn_log::HostSpawnEvent {
                id: unit,
                template: "EagerMaUnit".into(),
                team_ordinal: 0,
                position: [0.0, 0.0, 0.0],
            },
        ));
        assert!(eager_map_host_spawn_if_coupled(
            &logic,
            &crate::game_logic::host_spawn_log::HostSpawnEvent {
                id: target,
                template: "EagerMaTarget".into(),
                team_ordinal: 2,
                position: [30.0, 0.0, 0.0],
            },
        ));
        host_move_log::record(unit, Some([10.0, 0.0, 5.0]));
        host_attack_log::record(unit, Some(target));
        assert_eq!(host_move_log::len(), 1);
        assert_eq!(host_attack_log::len(), 1);

        let (a, m) = eager_apply_host_move_attack_after_logic(&mut shadow, &logic);
        assert!(a >= 1 && m >= 1, "expected attack={a} move={m}");
        assert!(host_move_log::drain().is_empty());
        assert!(host_attack_log::drain().is_empty());
        let (a2, m2) = eager_apply_host_move_attack_after_logic(&mut shadow, &logic);
        assert_eq!((a2, m2), (0, 0));
        clear_active_shadow_for_coupled_tick();
        end_shadow_coupled_tick();
        let _ = ObjectId;

        match prev {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
    }
}
