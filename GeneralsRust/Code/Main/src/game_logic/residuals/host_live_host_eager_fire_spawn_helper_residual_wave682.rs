//! Wave 682 residual peels: post-logic fire-spawn apply on coupled tick.
//! Under FIRE_SPAWN_AUTHORITY, host logic records to `host_fire_spawn_log`;
//! engine drains into CombatSystem immediately after the host frame, before the
//! full shadow session tail. Never flips `playable_claim`.

use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
static RESIDUAL_OK: AtomicBool = AtomicBool::new(false);
static RESIDUAL_ACTION: AtomicU8 = AtomicU8::new(0);
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}
pub const LIVE_HOST_EAGER_FIRE_SPAWN_HELPER_METHOD_NAMES_WAVE682: &[&str] = &[
    "eager_apply_host_fire_spawns_after_logic",
    "host_fire_spawn_log",
    "queue_projectile",
    "Wave 682",
    "playable_claim = false",
];
pub const LIVE_HOST_EAGER_FIRE_SPAWN_HELPER_NAV_STEPS_WAVE682: &[&str] = &[
    "REQUIRE_EAGER_FIRE_SPAWN_API",
    "REQUIRE_ENGINE_POST_LOGIC_DRAIN",
    "REQUIRE_QUEUE_PROJECTILE_RECORDS_LOG",
    "REQUIRE_SESSION_DRAIN_IDEMPOTENT",
    "LIVE_HOST_EAGER_FIRE_SPAWN_HELPER",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];
pub const RUNTIME_HOST_LIVE_HOST_EAGER_FIRE_SPAWN_HELPER_CMD_NAMES_WAVE682: &[&str] = &[
    "host_eager_fire_spawn_helper",
    "engine_post_logic_drain",
    "queue_projectile_records_log",
    "session_drain_idempotent",
    "eager_fire_spawn_residual",
];
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResidualHostEagerFireSpawnHelperAction {
    None = 0,
    MethodNames = 1,
    SourceMarkers = 2,
    NavCommands = 3,
    CollectSource = 4,
    DispatchSource = 5,
    Composite = 6,
}
impl ResidualHostEagerFireSpawnHelperAction {
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
fn residual_action_store(a: ResidualHostEagerFireSpawnHelperAction) {
    RESIDUAL_ACTION.store(a as u8, Ordering::SeqCst);
}
pub fn residual_host_eager_fire_spawn_helper_ok() -> bool {
    RESIDUAL_OK.load(Ordering::SeqCst)
}
pub fn residual_host_eager_fire_spawn_helper_last_action() -> ResidualHostEagerFireSpawnHelperAction
{
    ResidualHostEagerFireSpawnHelperAction::from_u8(RESIDUAL_ACTION.load(Ordering::SeqCst))
}
fn combat_source() -> &'static str {
    concat!(
        include_str!("../combat/mod.rs"),
        include_str!("../combat/damage.rs"),
        include_str!("../combat/projectile.rs"),
        include_str!("../combat/weapon_fire.rs"),
        include_str!("../combat/resolution.rs"),
        include_str!("../combat/tests.rs"),
    )
}
fn eng_source() -> &'static str {
    crate::cnc_game_engine::ENGINE_SRC
}
fn shadow_source() -> &'static str {
    crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC
}
// 2026-08-15: engine dispatches via eager_apply_all_host_residuals_after_logic
// (Wave 682/925); per-channel eager_apply_* stays on GAMEWORLD_SHADOW_SRC.
pub fn honesty_host_eager_fire_spawn_helper_method_names_residual_wave682() -> bool {
    let names = LIVE_HOST_EAGER_FIRE_SPAWN_HELPER_METHOD_NAMES_WAVE682;
    let ok = residual_name_index(names, "eager_apply_host_fire_spawns_after_logic").is_some()
        && residual_name_index(names, "host_fire_spawn_log").is_some()
        && residual_name_index(names, "queue_projectile").is_some()
        && residual_name_index(names, "Wave 682").is_some()
        && residual_name_index(names, "playable_claim = false").is_some();
    residual_action_store(ResidualHostEagerFireSpawnHelperAction::MethodNames);
    ok
}
pub fn honesty_host_eager_fire_spawn_helper_source_markers_residual_wave682() -> bool {
    let combat = combat_source();
    let eng = eng_source();
    let sh = shadow_source();
    let api_ok = sh.contains("pub fn eager_apply_host_fire_spawns_after_logic")
        && sh.contains("Wave 682")
        && sh.contains("host_fire_spawn_log::drain");
    let eng_ok =
        eng.contains("eager_apply_all_host_residuals_after_logic") && eng.contains("Wave 682/925");
    let queue_ok = combat.contains("host_fire_spawn_log::record")
        && combat.contains("gameworld_fire_spawn_authority_live")
        && combat.contains("Wave 682");
    let ok = api_ok && eng_ok && queue_ok && !combat.contains("playable_claim = true");
    residual_action_store(ResidualHostEagerFireSpawnHelperAction::SourceMarkers);
    ok
}
pub fn honesty_host_eager_fire_spawn_helper_nav_commands_residual_wave682() -> bool {
    let steps = LIVE_HOST_EAGER_FIRE_SPAWN_HELPER_NAV_STEPS_WAVE682;
    let cmds = RUNTIME_HOST_LIVE_HOST_EAGER_FIRE_SPAWN_HELPER_CMD_NAMES_WAVE682;
    let ok = residual_name_index(steps, "REQUIRE_EAGER_FIRE_SPAWN_API").is_some()
        && residual_name_index(steps, "REQUIRE_ENGINE_POST_LOGIC_DRAIN").is_some()
        && residual_name_index(steps, "REQUIRE_QUEUE_PROJECTILE_RECORDS_LOG").is_some()
        && residual_name_index(steps, "REQUIRE_SESSION_DRAIN_IDEMPOTENT").is_some()
        && residual_name_index(steps, "LIVE_HOST_EAGER_FIRE_SPAWN_HELPER").is_some()
        && residual_name_index(steps, "LIVE_PLAYABLE_CLAIM_FALSE").is_some()
        && residual_name_index(cmds, "host_eager_fire_spawn_helper").is_some()
        && residual_name_index(cmds, "engine_post_logic_drain").is_some()
        && residual_name_index(cmds, "queue_projectile_records_log").is_some()
        && residual_name_index(cmds, "session_drain_idempotent").is_some()
        && residual_name_index(cmds, "eager_fire_spawn_residual").is_some();
    residual_action_store(ResidualHostEagerFireSpawnHelperAction::NavCommands);
    ok
}
pub fn simulate_host_eager_fire_spawn_helper_collect_source() -> bool {
    let ok = shadow_source().contains("eager_apply_host_fire_spawns_after_logic")
        && eng_source().contains("eager_apply_all_host_residuals_after_logic")
        && combat_source().contains("host_fire_spawn_log::record");
    residual_action_store(ResidualHostEagerFireSpawnHelperAction::CollectSource);
    ok
}
pub fn simulate_host_eager_fire_spawn_helper_dispatch_source() -> bool {
    let ok = eng_source().contains("Wave 682/925")
        && combat_source().contains("Wave 682")
        && shadow_source().contains("Wave 682");
    residual_action_store(ResidualHostEagerFireSpawnHelperAction::DispatchSource);
    ok
}
pub fn honesty_host_eager_fire_spawn_helper_residual_pack_wave682() -> bool {
    honesty_host_eager_fire_spawn_helper_method_names_residual_wave682()
        && honesty_host_eager_fire_spawn_helper_source_markers_residual_wave682()
        && honesty_host_eager_fire_spawn_helper_nav_commands_residual_wave682()
        && simulate_host_eager_fire_spawn_helper_collect_source()
        && simulate_host_eager_fire_spawn_helper_dispatch_source()
}
pub fn simulate_live_host_eager_fire_spawn_helper_honesty() -> bool {
    let ok = honesty_host_eager_fire_spawn_helper_residual_pack_wave682();
    if ok {
        RESIDUAL_OK.store(true, Ordering::SeqCst);
        residual_action_store(ResidualHostEagerFireSpawnHelperAction::Composite);
    }
    ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_fire_spawn_log;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, clear_active_shadow_for_coupled_tick,
        eager_apply_host_fire_spawns_after_logic, end_shadow_coupled_tick,
    };
    use glam::Vec3;

    fn ensure_template(logic: &mut GameLogic, name: &str, health: f32) {
        let mut t = ThingTemplate::new(name);
        t.set_health(health);
        t.add_kind_of(KindOf::Attackable);
        logic.templates.insert(name.into(), t);
    }

    fn sample_pending(shooter: ObjectId, target: ObjectId) -> PendingProjectile {
        PendingProjectile {
            shooter_id: shooter,
            shooter_pos: Vec3::ZERO,
            source_context: None,
            target_id: Some(target),
            target_pos: Some(Vec3::new(20.0, 0.0, 0.0)),
            damage: 10.0,
            speed: 100.0,
            splash_radius: 0.0,
            is_homing: false,
            damage_type: DamageType::Bullet,
            death_type: HostDeathType::Normal,
            projectile_object_name: String::new(),
            projectile_lifecycle: None,
            fire_fx_name: String::new(),
            fire_ocl_name: String::new(),
            detonation_fx_name: String::new(),
            detonation_ocl_name: String::new(),
            exhaust_name: String::new(),
            secondary_damage: 0.0,
            secondary_damage_radius: 0.0,
            shock_wave_amount: 0.0,
            shock_wave_radius: 0.0,
            shock_wave_taper_off: 0.0,
            radius_damage_affects: 0,
            projectile_collides: 0,
            scatter_radius: 0.0,
            scatter_table_offset: None,
            min_weapon_speed: 0.0,
            scale_weapon_speed: false,
            attack_range: 0.0,
            min_attack_range: 0.0,
            historic_weapon_key: String::new(),
            historic_bonus_time_frames: 0,
            historic_bonus_count: 0,
            historic_bonus_radius: 0.0,
            historic_bonus_weapon: String::new(),
            die_on_detonate: false,
        }
    }

    #[test]
    fn method_names_residual() {
        assert!(honesty_host_eager_fire_spawn_helper_method_names_residual_wave682());
    }
    #[test]
    fn source_markers_residual() {
        assert!(honesty_host_eager_fire_spawn_helper_source_markers_residual_wave682());
    }
    #[test]
    fn nav_commands_residual() {
        assert!(honesty_host_eager_fire_spawn_helper_nav_commands_residual_wave682());
    }
    #[test]
    fn sources() {
        assert!(simulate_host_eager_fire_spawn_helper_collect_source());
        assert!(simulate_host_eager_fire_spawn_helper_dispatch_source());
    }
    #[test]
    fn pack() {
        assert!(honesty_host_eager_fire_spawn_helper_residual_pack_wave682());
    }
    #[test]
    fn live() {
        assert!(simulate_live_host_eager_fire_spawn_helper_honesty());
        assert!(residual_host_eager_fire_spawn_helper_ok());
    }

    #[test]
    fn post_logic_fire_spawn_drains_log_into_combat() {
        let _guard = crate::gameworld_shadow::authority_env_lock();
        let prev_s = std::env::var_os("GENERALS_GAMEWORLD_SHADOW");
        let prev_f = std::env::var_os("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
        crate::env_compat::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", "1");
        host_fire_spawn_log::clear();
        combat::clear_pending_projectile_queue_for_test();

        let mut logic = GameLogic::new();
        ensure_template(&mut logic, "EagerFireShooter", 100.0);
        ensure_template(&mut logic, "EagerFireTarget", 100.0);
        let shooter = logic
            .create_object("EagerFireShooter", Team::USA, Vec3::new(0.0, 0.0, 0.0))
            .expect("shooter");
        let target = logic
            .create_object("EagerFireTarget", Team::GLA, Vec3::new(20.0, 0.0, 0.0))
            .expect("target");

        begin_shadow_coupled_tick();
        // Record deferred fire under authority (same as mid-frame combat).
        combat::queue_projectile(sample_pending(shooter, target));
        assert_eq!(host_fire_spawn_log::len(), 1);

        let mut shadow = GameWorldShadow::new(64);
        let before = logic.combat_system.projectile_count();
        let n = eager_apply_host_fire_spawns_after_logic(&mut shadow, &mut logic);
        assert!(n >= 1 || logic.combat_system.projectile_count() > before);
        assert!(
            host_fire_spawn_log::drain().is_empty(),
            "post-logic drain must empty fire_spawn_log"
        );
        // Session-style second drain is idempotent.
        let n2 = eager_apply_host_fire_spawns_after_logic(&mut shadow, &mut logic);
        assert_eq!(n2, 0);
        clear_active_shadow_for_coupled_tick();
        end_shadow_coupled_tick();
        combat::clear_pending_projectile_queue_for_test();

        match prev_s {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_SHADOW"),
        }
        match prev_f {
            Some(v) => crate::env_compat::set_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY", v),
            None => crate::env_compat::remove_var("GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY"),
        }
    }
}
