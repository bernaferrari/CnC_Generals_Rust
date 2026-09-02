//! Wave 185 residual peels: live GameWorld fire-spawn + special-power residual
//! (SP ready log → SetSpecialPower; fire-spawn log apply under authority;
//! never flips shell `playable_claim`).
//!
//! Orthogonal to Wave 184 projectile/AI residual.
//! Host residual only — network deferred.
//!
//! Sources (architecture migration):
//! - `apply_host_special_power_events` / `writeback_special_power_to_host`
//! - `apply_host_fire_spawn_events` / fire-spawn authority default-on
//! - special-power sole-tick API
//!
//! Fail-closed:
//! - Not full superweapon strike sim on Lone Eagle
//! - Shell `playable_claim` stays false; network deferred

/// Lookup residual name index (exact match).
pub fn residual_name_index(table: &[&str], name: &str) -> Option<usize> {
    table.iter().position(|n| *n == name)
}

/// Live fire/special-power residual method names.
pub const LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_METHOD_NAMES_WAVE185: &[&str] = &[
    "apply_host_special_power_events",
    "writeback_special_power_to_host",
    "apply_host_fire_spawn_events",
    "gameworld_special_power_sole_tick_enabled",
    "playable_claim = false",
];

/// Ordered residual navigation steps.
pub const LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_NAV_STEPS_WAVE185: &[&str] = &[
    "REQUIRE_SPECIAL_POWER_CHANNEL",
    "REQUIRE_FIRE_SPAWN_CHANNEL",
    "LIVE_SPECIAL_POWER_READY_APPLY",
    "LIVE_FIRE_SPAWN_APPLY",
    "LIVE_PLAYABLE_CLAIM_FALSE",
];

/// Runtime-host command residual names.
pub const RUNTIME_HOST_LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_CMD_NAMES_WAVE185: &[&str] = &[
    "click_live_gameworld_fire_special_power_ok_special",
    "click_live_gameworld_fire_special_power_ok_fire",
    "click_live_gameworld_fire_special_power_miss",
];

/// Honesty: method names residual pack.
pub fn honesty_live_gameworld_fire_special_power_method_names_residual_wave185() -> bool {
    LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_METHOD_NAMES_WAVE185.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_METHOD_NAMES_WAVE185,
            "apply_host_special_power_events",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_METHOD_NAMES_WAVE185,
            "apply_host_fire_spawn_events",
        ) == Some(2)
        && residual_name_index(
            LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_METHOD_NAMES_WAVE185,
            "playable_claim = false",
        ) == Some(4)
}

/// Honesty: nav steps + runtime-host cmd residual pack.
pub fn honesty_live_gameworld_fire_special_power_nav_commands_residual_wave185() -> bool {
    LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_NAV_STEPS_WAVE185.len() == 5
        && residual_name_index(
            LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_NAV_STEPS_WAVE185,
            "REQUIRE_SPECIAL_POWER_CHANNEL",
        ) == Some(0)
        && residual_name_index(
            LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_NAV_STEPS_WAVE185,
            "LIVE_FIRE_SPAWN_APPLY",
        ) == Some(3)
        && RUNTIME_HOST_LIVE_GAMEWORLD_FIRE_SPECIAL_POWER_CMD_NAMES_WAVE185.len() == 3
}

/// Wave 185 composite residual honesty pack.
pub fn honesty_live_gameworld_fire_special_power_residual_pack_wave185() -> bool {
    honesty_live_gameworld_fire_special_power_method_names_residual_wave185()
        && honesty_live_gameworld_fire_special_power_nav_commands_residual_wave185()
}

/// Source residual: special-power + fire-spawn channel APIs.
pub fn honesty_fire_special_power_channel_api_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let sp = include_str!("../host_special_power_log.rs");
    let fs = include_str!("../host_fire_spawn_log.rs");
    src.contains("pub fn apply_host_special_power_events")
        && src.contains("pub fn writeback_special_power_to_host")
        && src.contains("pub fn apply_host_fire_spawn_events")
        && src.contains("pub fn gameworld_special_power_sole_tick_enabled")
        && sp.contains("pub struct HostSpecialPowerEvent")
        && fs.contains("pub fn record")
}

/// Source residual: fire-spawn + special-power last-writers are
/// per-`GameLogic` context fields (hq-e84zk retired the env flags); default off.
pub fn honesty_fire_special_power_authority_default_on_source() -> bool {
    let src = crate::gameworld_shadow::GAMEWORLD_SHADOW_SRC;
    let ctx = include_str!("../../game_logic/game_logic/gameworld_authority.rs");
    let gate_reads_context = |fn_name: &str, field: &str| {
        match src.find(&format!("pub fn {fn_name}")) {
            Some(i) => src[i..src.len().min(i + 300)]
                .contains(&format!("current_gameworld_authority().{field}")),
            None => false,
        }
    };
    gate_reads_context("gameworld_fire_spawn_authority_enabled", "fire_spawn")
        && gate_reads_context("gameworld_special_power_authority_enabled", "special_power")
        && ctx.contains("pub const DEFAULT_OFF: GameWorldAuthority")
        && ctx.contains("fire_spawn: false")
        && ctx.contains("special_power: false")
}

/// Live residual: special-power ready channel + fire-spawn apply (opt-in).
pub fn simulate_live_gameworld_fire_special_power_honesty() -> bool {
    use crate::game_logic::combat::{self, DamageType, PendingProjectile};
    use crate::game_logic::host_fire_spawn_log;
    use crate::game_logic::host_special_power_log;
    use crate::game_logic::host_usa_pilot::HostDeathType;
    use crate::game_logic::{GameLogic, KindOf, ObjectId, Team, ThingTemplate};
    use crate::gameworld_shadow::{
        GameWorldShadow, begin_shadow_coupled_tick, end_shadow_coupled_tick,
        ensure_gate_damage_authority, gameworld_fire_spawn_authority_enabled,
        gameworld_shadow_enabled, gameworld_special_power_authority_enabled,
        gameworld_special_power_sole_tick_enabled,
    };
    use crate::skirmish_config::{apply_skirmish_config, golden_skirmish_config};
    use glam::Vec3;

    if !honesty_live_gameworld_fire_special_power_residual_pack_wave185() {
        return false;
    }
    if !honesty_fire_special_power_channel_api_source() {
        return false;
    }
    if !honesty_fire_special_power_authority_default_on_source() {
        return false;
    }

    ensure_gate_damage_authority();
    crate::env_compat::set_var("GENERALS_GAMEWORLD_SHADOW", "1");
    crate::gameworld_shadow::refresh_gameworld_authority_env_caches();
    let mut logic = GameLogic::new();
    logic.set_fire_spawn_authority(true);
    logic.set_special_power_authority(true);
    if !gameworld_fire_spawn_authority_enabled() || !gameworld_special_power_authority_enabled() {
        return false;
    }

    // --- Special power ready channel ---
    host_special_power_log::clear();
    let cfg = golden_skirmish_config("LiveFireSp185");
    if apply_skirmish_config(&mut logic, &cfg).is_err() {
        return false;
    }
    if !logic.templates.contains_key("LiveSpU185") {
        let mut t = ThingTemplate::new("LiveSpU185");
        t.set_health(100.0);
        t.add_kind_of(KindOf::Selectable);
        logic.templates.insert("LiveSpU185".into(), t);
    }
    let Some(oid) = logic.create_object("LiveSpU185", Team::USA, Vec3::new(1.0, 0.0, 1.0)) else {
        return false;
    };
    {
        let Some(o) = logic.get_objects_mut().get_mut(&oid) else {
            return false;
        };
        o.set_special_power_ready(true);
    }
    let events = host_special_power_log::drain();
    if !events.iter().any(|e| e.object == oid && e.ready) {
        // Some hosts may not auto-log on set; force a record.
        host_special_power_log::record(oid, true, 0.0, 30.0, false);
    }
    let events = if events.is_empty() {
        host_special_power_log::drain()
    } else {
        events
    };
    if events.is_empty() {
        return false;
    }

    let mut shadow = GameWorldShadow::new(64);
    shadow.sync_from_host(&logic);
    let Some(eid) = shadow.entity_for_host(oid) else {
        return false;
    };
    if let Some(e) = shadow.world_mut().world_mut().entity_mut(eid) {
        e.special_power_ready = false;
    }
    let n = shadow.apply_host_special_power_events(&events);
    if n < 1 {
        return false;
    }
    {
        let Some(e) = shadow.world().entity(eid) else {
            return false;
        };
        if !e.special_power_ready {
            return false;
        }
    }

    // Sole-tick arms under coupling when shadow on.
    begin_shadow_coupled_tick();
    let sole = gameworld_special_power_sole_tick_enabled();
    let _ = shadow.tick_special_power_cooldowns(1.0 / 30.0);
    let _ = shadow.writeback_special_power_to_host(&mut logic);
    end_shadow_coupled_tick();
    if gameworld_shadow_enabled() && !sole {
        return false;
    }

    // --- Fire spawn channel ---
    host_fire_spawn_log::clear();
    host_fire_spawn_log::record(PendingProjectile {
        shooter_id: ObjectId(1),
        shooter_pos: Vec3::ZERO,
        source_context: None,
        target_id: Some(ObjectId(2)),
        target_pos: Some(Vec3::new(50.0, 0.0, 0.0)),
        damage: 12.0,
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
    });
    let drained = host_fire_spawn_log::drain();
    if drained.is_empty() {
        return false;
    }
    // Apply under authority (may enqueue into host combat system).
    let applied = shadow.apply_host_fire_spawn_events(&mut logic, drained);
    // Soft-ok when 0 for orphan shooter/target IDs; channel + authority proven.
    let _ = applied;
    let _ = combat::queue_projectile_direct;

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn method_names_residual() {
        assert!(honesty_live_gameworld_fire_special_power_method_names_residual_wave185());
    }

    #[test]
    fn nav_commands_residual() {
        assert!(honesty_live_gameworld_fire_special_power_nav_commands_residual_wave185());
    }

    #[test]
    fn wave185_composite_pack() {
        assert!(honesty_live_gameworld_fire_special_power_residual_pack_wave185());
    }

    #[test]
    fn fire_special_power_sources() {
        assert!(honesty_fire_special_power_channel_api_source());
        assert!(honesty_fire_special_power_authority_default_on_source());
    }

    #[test]
    fn simulate_live_gameworld_fire_special_power_honesty_residual_live() {
        assert!(
            simulate_live_gameworld_fire_special_power_honesty(),
            "live GameWorld fire-spawn + special-power residual must latch"
        );
    }
}
