//! Coupled-tick, authority env caches, and eager host residual apply.

use super::GameWorldShadow;
use crate::game_logic::{GameLogic, ObjectId};

/// Whether the optional engine shadow path is enabled.
/// True when Main create_object may attach gamelogic OBJECT_REGISTRY ids (opt-in only).
/// Host Object pose/HP/alive never dual-read the registry — stamp is metadata only.
/// Cached flag (env is process-stable; per-call getenv was a Lone Eagle hotspot).
/// 0 = unset, 1 = off, 2 = on.
static ENGINE_OBJECT_BRIDGE_CACHE: std::sync::atomic::AtomicU8 =
    std::sync::atomic::AtomicU8::new(0);

/// Invalidate bridge cache after test env mutation (production never needs this).
pub fn refresh_engine_object_bridge_cache() {
    ENGINE_OBJECT_BRIDGE_CACHE.store(0, std::sync::atomic::Ordering::Relaxed);
}

#[inline]
pub fn engine_object_bridge_enabled() -> bool {
    #[cfg(test)]
    {
        return std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some()
            || std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_some();
    }
    #[cfg(not(test))]
    {
        use std::sync::atomic::Ordering::Relaxed;
        match ENGINE_OBJECT_BRIDGE_CACHE.load(Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let on = std::env::var_os("GENERALS_ALLOW_DUAL_TICK").is_some()
                    || std::env::var_os("GENERALS_BRIDGE_ENGINE_OBJECTS").is_some();
                ENGINE_OBJECT_BRIDGE_CACHE.store(if on { 2 } else { 1 }, Relaxed);
                on
            }
        }
    }
}

/// Process-stable env bool cache: 0=unset, 1=false, 2=true.
#[inline]
pub fn env_flag_raw(name: &str, default_on: bool) -> bool {
    match std::env::var(name) {
        Ok(v) => {
            let v = v.trim();
            !(v == "0"
                || v.eq_ignore_ascii_case("false")
                || v.eq_ignore_ascii_case("off")
                || v.eq_ignore_ascii_case("no"))
        }
        Err(_) => default_on,
    }
}

#[inline]
pub fn env_flag_cached(cache: &std::sync::atomic::AtomicU8, name: &str, default_on: bool) -> bool {
    // Unit tests mutate GENERALS_* mid-process; always re-read under cfg(test).
    #[cfg(test)]
    {
        let _ = cache;
        return env_flag_raw(name, default_on);
    }
    #[cfg(not(test))]
    {
        use std::sync::atomic::Ordering::Relaxed;
        match cache.load(Relaxed) {
            1 => false,
            2 => true,
            _ => {
                let on = env_flag_raw(name, default_on);
                cache.store(if on { 2 } else { 1 }, Relaxed);
                on
            }
        }
    }
}

/// Invalidate authority env caches after test env mutation.
pub fn refresh_gameworld_authority_env_caches() {
    refresh_engine_object_bridge_cache();
    for c in [
        &SHADOW_ENABLED_CACHE,
        &DAMAGE_AUTH_CACHE,
        &ECONOMY_AUTH_CACHE,
        &MOVEMENT_AUTH_CACHE,
        &AI_ATTACK_AUTH_CACHE,
        &PROJECTILE_AUTH_CACHE,
        &AI_DECISION_AUTH_CACHE,
        &FIRE_SPAWN_AUTH_CACHE,
        &CONSTRUCTION_AUTH_CACHE,
        &SPECIAL_POWER_AUTH_CACHE,
        &PRODUCTION_AUTH_CACHE,
    ] {
        c.store(0, std::sync::atomic::Ordering::Relaxed);
    }
}

static SHADOW_ENABLED_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static DAMAGE_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static ECONOMY_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static MOVEMENT_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static AI_ATTACK_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static PROJECTILE_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static AI_DECISION_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static FIRE_SPAWN_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static CONSTRUCTION_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static SPECIAL_POWER_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);
static PRODUCTION_AUTH_CACHE: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

/// True only while the production engine couples host update → shadow_session.
/// Host-only gates (golden/shell) never set this, so construction/production
/// percent still advance without a dual-world writeback.
// Thread-local depth so parallel unit tests cannot clear each other's couple mark.
// Engine remains single-threaded; nested begin/end pairs are supported.
std::thread_local! {
    static SHADOW_COUPLED_TICK_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// Mark the current host frame as shadow-coupled (engine path / tests).
#[inline]
pub fn begin_shadow_coupled_tick() {
    SHADOW_COUPLED_TICK_DEPTH.with(|d| d.set(d.get().saturating_add(1)));
}

/// Clear one shadow-coupled host-frame mark (nested-safe).
#[inline]
pub fn end_shadow_coupled_tick() {
    SHADOW_COUPLED_TICK_DEPTH.with(|d| {
        let next = d.get().saturating_sub(1);
        d.set(next);
        // Wave 684: drop unused post-logic damage handoff when outermost couple ends.
        if next == 0 {
            EARLY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_HEAL_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_MAX_HEALTH_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_EXPERIENCE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_AI_STATE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_FIRE_INTENT_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_OWNER_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_MOVEMENT_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_STATUS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_VETERANCY_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_WEAPON_BONUS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_WEAPON_SLOT_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_WEAPON_SET_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_ENTITY_POWER_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_TURRET_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_GUARD_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_RALLY_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_TARGET_LOCATION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_DETECTOR_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_CONTINUOUS_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_AI_ATTITUDE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_OVERCHARGE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_STEALTH_FLAGS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_CONTAIN_CAPACITY_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_HIVE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_OVERLORD_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_COMMAND_SET_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_DISGUISE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_VISION_CAMO_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_WEAPON_STATS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_SELECTION_RADIUS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_MODEL_CONDITION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_DEMO_MINE_CHEER_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_FORMATION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_CRUSH_VISION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_BUILDING_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_IDENTITY_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_GROUND_HEIGHT_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_MODEL_MESH_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_FOW_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_KIND_OF_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_FAERIE_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_REPULSOR_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_DISABLE_TIMERS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_BODY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_DEATH_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PHYSICS_MOTIVE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_LOCOMOTOR_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_BOUNCE_LAND_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_AI_MOOD_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_AI_REQUEST_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_SHOCK_STUN_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_STEALTH_DELAY_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_SOLE_HEALING_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_RADAR_EXTEND_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_HIJACKER_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_REBUILD_PRODUCER_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_STORED_SUPPLIES_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_SPECIAL_POWER_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_RADAR_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PLAYER_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PLAYER_META_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PLAYER_COOLDOWN_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PRODUCTION_DOOR_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PRODUCTION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_CONSTRUCTION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_COMBAT_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_PROJECTILE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_DESTROY_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_CONTAIN_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_AI_DECISION_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_MOVE_BATCH.with(|c| *c.borrow_mut() = None);
            EARLY_FIRE_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
        }
    });
}

/// Host freeze of sole-tick systems is valid only on a coupled engine frame.
#[inline]
pub fn shadow_coupled_tick_active() -> bool {
    SHADOW_COUPLED_TICK_DEPTH.with(|d| d.get() > 0)
}

// Wave 680: mid-frame host spawn → GameWorld map while a coupled shadow tick is live.
// Engine installs the live shadow pointer around host logic; cleared before session.
thread_local! {
    static ACTIVE_SHADOW_PTR: std::cell::Cell<Option<std::ptr::NonNull<GameWorldShadow>>> =
        std::cell::Cell::new(None);
}

/// Install the live `GameWorldShadow` for Wave 680 eager spawn mapping.
///
/// Caller must `clear_active_shadow_for_coupled_tick` before dropping/moving the
/// shadow or re-entering `shadow_session_after_host_tick`.
#[inline]
pub fn install_active_shadow_for_coupled_tick(shadow: &mut GameWorldShadow) {
    ACTIVE_SHADOW_PTR.with(|c| {
        // SAFETY: pointer is only used while `shadow` is exclusively borrowed by the
        // engine for the coupled host tick; cleared before other mut uses.
        c.set(Some(unsafe {
            std::ptr::NonNull::new_unchecked(shadow as *mut GameWorldShadow)
        }));
    });
}

/// Clear Wave 680 active shadow pointer (nested-safe single slot).
#[inline]
pub fn clear_active_shadow_for_coupled_tick() {
    ACTIVE_SHADOW_PTR.with(|c| c.set(None));
    // Note: EARLY_DAMAGE_BATCH is consumed by shadow_session_after_host_tick
    // (Wave 684). Do not clear here — engine clears the active shadow pointer
    // before the session exclusive borrow.
}

/// Wave 680: if a coupled shadow tick is live, map this host spawn into GameWorld now.
///
/// Idempotent with end-of-tick `host_spawn_log` drain (`apply_host_spawn_events`
/// skips already-mapped host IDs). Fail-closed when no active shadow pointer.
thread_local! {
    /// Wave 736: when set, the next host spawn event binds to this pre-spawned
    /// GameWorld entity raw id instead of queueing a second Spawn mutation.
    static NEXT_HOST_SPAWN_BIND_ENTITY: std::cell::Cell<Option<u32>> =
        std::cell::Cell::new(None);
}

/// Wave 736: bind the next host `create_object` / spawn-log map to a pre-spawned GW entity.
pub fn set_next_host_spawn_bind_entity(gw_entity_raw: u32) {
    NEXT_HOST_SPAWN_BIND_ENTITY.with(|c| c.set(Some(gw_entity_raw)));
}

pub fn take_next_host_spawn_bind_entity() -> Option<u32> {
    NEXT_HOST_SPAWN_BIND_ENTITY.with(|c| c.take())
}

/// Wave 736: bind host ObjectId → existing GameWorld entity (production entity-first).
pub fn bind_host_to_existing_entity(
    shadow: &mut GameWorldShadow,
    host_id: u32,
    gw_entity_raw: u32,
) -> bool {
    use gamelogic::world::entities::EntityId;
    let eid = EntityId::from_raw(gw_entity_raw);
    if shadow.world.entity(eid).is_none() {
        return false;
    }
    if shadow.host_to_entity.contains_key(&host_id) {
        return true;
    }
    shadow.host_to_entity.insert(host_id, eid);
    shadow.entity_to_host.insert(eid.get(), host_id);
    true
}

/// Wave 742: if a coupled shadow tick is live under construction authority, pre-spawn
/// a rebuild-hole entity and return its raw id for host ObjectId bind (entity-first).
/// Fail-closed: returns None when shadow is not coupled / disabled.
pub fn spawn_rebuild_hole_entity_if_coupled(
    template: &str,
    position: [f32; 3],
    orientation: f32,
    health: f32,
) -> Option<u32> {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return None;
    }
    if !gameworld_construction_authority_enabled() {
        return None;
    }
    ACTIVE_SHADOW_PTR.with(|c| {
        let Some(ptr) = c.get() else {
            return None;
        };
        // SAFETY: engine installs live shadow for coupled tick and clears before drop.
        let shadow = unsafe { &mut *ptr.as_ptr() };
        use gamelogic::world::entities::EntityId;
        use gamelogic::world::WorldMutation;
        shadow.world.queue_mutation(WorldMutation::Spawn {
            template: template.to_string(),
            owner: None,
            position,
            health: health.max(1.0),
        });
        let _ = shadow.world.apply_pending_mutations();
        let raw = shadow
            .world
            .take_last_spawned_entity()
            .map(|eid| eid.get())?;
        if let Some(e) = shadow.world.world_mut().entity_mut(EntityId::from_raw(raw)) {
            e.transform.orientation = orientation;
            e.is_rebuild_hole = true;
            e.construction_percent = 1.0;
        }
        Some(raw)
    })
}

pub fn eager_map_host_spawn_if_coupled(
    logic: &GameLogic,
    event: &crate::game_logic::host_spawn_log::HostSpawnEvent,
) -> bool {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        // Still consume bind token so sole-tick without live shadow does not leak.
        let _ = take_next_host_spawn_bind_entity();
        return false;
    }
    ACTIVE_SHADOW_PTR.with(|c| {
        let Some(ptr) = c.get() else {
            let _ = take_next_host_spawn_bind_entity();
            return false;
        };
        // SAFETY: engine installs live shadow for coupled tick and clears before
        // drop/session. `create_object` does not re-enter the engine shadow field.
        let shadow = unsafe { &mut *ptr.as_ptr() };
        // Wave 736: production entity-first bind (no second Spawn).
        if let Some(raw) = take_next_host_spawn_bind_entity() {
            return bind_host_to_existing_entity(shadow, event.id.0, raw);
        }
        shadow.apply_host_spawn_events(std::slice::from_ref(event), logic) > 0
    })
}
/// Wave 681: if a coupled shadow tick is live, queue GameWorld Destroy for this
/// host ObjectId now (unmap after apply_pending).
///
/// Idempotent with end-of-tick `host_destroy_log` drain. Fail-closed when no
/// active shadow pointer or host id is unmapped.
pub fn eager_unmap_host_destroy_if_coupled(host: ObjectId) -> bool {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return false;
    }
    ACTIVE_SHADOW_PTR.with(|c| {
        let Some(ptr) = c.get() else {
            return false;
        };
        // SAFETY: same coupled-tick install contract as eager_map_host_spawn_if_coupled.
        let shadow = unsafe { &mut *ptr.as_ptr() };
        let (queued, applied) = shadow.apply_host_destroy_events(&[
            crate::game_logic::host_destroy_log::HostDestroyEvent { id: host },
        ]);
        queued > 0 || applied > 0
    })
}

/// Wave 682: immediately after the host logic frame on a coupled tick, drain
/// `host_fire_spawn_log` into host CombatSystem + GameWorld projectile map.
///
/// Runs before `shadow_session_after_host_tick` so deferred weapon discharges
/// materialize the same frame without waiting for the full session tail.
/// Session fire-spawn drain stays idempotent (log already empty).
///
/// Safe exclusive borrows: caller passes `&mut GameWorldShadow` + `&mut GameLogic`.
/// Wave 925: single post-logic host→GameWorld residual batch (coupled tick).
/// Replaces N separate eager_apply_* dual-borrows in the engine frame path.
#[inline]
pub fn eager_apply_all_host_residuals_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &mut crate::game_logic::GameLogic,
) {
    let _ = eager_apply_host_fire_spawns_after_logic(shadow, logic);
    let _ = eager_apply_host_move_attack_after_logic(shadow, logic);
    let _ = eager_apply_host_damage_after_logic(shadow, logic);
    let _ = eager_apply_host_heal_after_logic(shadow, logic);
    let _ = eager_apply_host_max_health_after_logic(shadow, logic);
    let _ = eager_apply_host_experience_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_state_after_logic(shadow, logic);
    let _ = eager_apply_host_fire_intent_after_logic(shadow, logic);
    let _ = eager_apply_host_owner_after_logic(shadow, logic);
    let _ = eager_apply_host_movement_after_logic(shadow, logic);
    let _ = eager_apply_host_status_after_logic(shadow, logic);
    let _ = eager_apply_host_veterancy_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_bonus_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_slot_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_set_after_logic(shadow, logic);
    let _ = eager_apply_host_entity_power_after_logic(shadow, logic);
    let _ = eager_apply_host_turret_after_logic(shadow, logic);
    let _ = eager_apply_host_guard_after_logic(shadow, logic);
    let _ = eager_apply_host_rally_after_logic(shadow, logic);
    let _ = eager_apply_host_target_location_after_logic(shadow, logic);
    let _ = eager_apply_host_detector_after_logic(shadow, logic);
    let _ = eager_apply_host_continuous_fire_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_attitude_after_logic(shadow, logic);
    let _ = eager_apply_host_overcharge_after_logic(shadow, logic);
    let _ = eager_apply_host_stealth_flags_after_logic(shadow, logic);
    let _ = eager_apply_host_contain_capacity_after_logic(shadow, logic);
    let _ = eager_apply_host_hive_after_logic(shadow, logic);
    let _ = eager_apply_host_overlord_after_logic(shadow, logic);
    let _ = eager_apply_host_command_set_after_logic(shadow, logic);
    let _ = eager_apply_host_disguise_after_logic(shadow, logic);
    let _ = eager_apply_host_vision_camo_after_logic(shadow, logic);
    let _ = eager_apply_host_weapon_stats_after_logic(shadow, logic);
    let _ = eager_apply_host_selection_radius_after_logic(shadow, logic);
    let _ = eager_apply_host_model_condition_after_logic(shadow, logic);
    let _ = eager_apply_host_demo_mine_cheer_after_logic(shadow, logic);
    let _ = eager_apply_host_formation_after_logic(shadow, logic);
    let _ = eager_apply_host_crush_vision_after_logic(shadow, logic);
    let _ = eager_apply_host_building_type_after_logic(shadow, logic);
    let _ = eager_apply_host_identity_after_logic(shadow, logic);
    let _ = eager_apply_host_ground_height_after_logic(shadow, logic);
    let _ = eager_apply_host_model_mesh_after_logic(shadow, logic);
    let _ = eager_apply_host_fow_after_logic(shadow, logic);
    let _ = eager_apply_host_kind_of_after_logic(shadow, logic);
    let _ = eager_apply_host_faerie_fire_after_logic(shadow, logic);
    let _ = eager_apply_host_repulsor_after_logic(shadow, logic);
    let _ = eager_apply_host_disable_timers_after_logic(shadow, logic);
    let _ = eager_apply_host_body_damage_after_logic(shadow, logic);
    let _ = eager_apply_host_death_type_after_logic(shadow, logic);
    let _ = eager_apply_host_physics_motive_after_logic(shadow, logic);
    let _ = eager_apply_host_locomotor_after_logic(shadow, logic);
    let _ = eager_apply_host_bounce_land_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_mood_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_request_after_logic(shadow, logic);
    let _ = eager_apply_host_shock_stun_after_logic(shadow, logic);
    let _ = eager_apply_host_stealth_delay_after_logic(shadow, logic);
    let _ = eager_apply_host_sole_healing_after_logic(shadow, logic);
    let _ = eager_apply_host_radar_extend_after_logic(shadow, logic);
    let _ = eager_apply_host_hijacker_after_logic(shadow, logic);
    let _ = eager_apply_host_rebuild_producer_after_logic(shadow, logic);
    let _ = eager_apply_host_stored_supplies_after_logic(shadow, logic);
    let _ = eager_apply_host_special_power_after_logic(shadow, logic);
    let _ = eager_apply_host_radar_after_logic(shadow, logic);
    let _ = eager_apply_host_player_progress_after_logic(shadow, logic);
    let _ = eager_apply_host_player_meta_after_logic(shadow, logic);
    let _ = eager_apply_host_player_cooldown_after_logic(shadow, logic);
    let _ = eager_apply_host_production_door_after_logic(shadow, logic);
    let _ = eager_apply_host_production_after_logic(shadow, logic);
    let _ = eager_apply_host_production_progress_after_logic(shadow, logic);
    let _ = eager_apply_host_construction_after_logic(shadow, logic);
    let _ = eager_apply_host_construction_progress_after_logic(shadow, logic);
    let _ = eager_apply_host_combat_attack_after_logic(shadow, logic);
    let _ = eager_apply_host_projectile_after_logic(shadow, logic);
    let _ = eager_apply_host_destroy_after_logic(shadow, logic);
    let _ = eager_apply_host_contain_after_logic(shadow, logic);
    let _ = eager_apply_host_ai_decision_after_logic(shadow, logic);
    let _ = eager_apply_host_spawn_after_logic(shadow, logic);
}

pub fn eager_apply_host_fire_spawns_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &mut GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_fire_spawn_authority_enabled()
    {
        return 0;
    }
    // Wave 682/712: post-logic fire-spawn materialize (exclusive shadow+logic borrows).
    let spawns = crate::game_logic::host_fire_spawn_log::drain();
    if spawns.is_empty() {
        EARLY_FIRE_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_fire_spawn_events(logic, spawns.clone());
    EARLY_FIRE_SPAWN_BATCH.with(|c| *c.borrow_mut() = Some((spawns, true)));
    n
}
/// Wave 683: immediately after the host logic frame on a coupled tick, drain
/// `host_attack_log` / `host_move_log` into GameWorld attack/move targets.
///
/// Runs before `shadow_session_after_host_tick` so AI/path residuals see current
/// orders the same frame. Session drains stay idempotent (logs already empty).
///
/// Safe exclusive borrows: caller passes `&mut GameWorldShadow` + `&GameLogic`.
pub fn eager_apply_host_move_attack_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> (usize, usize) {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return (0, 0);
    }
    // Wave 683/712: post-logic move/attack materialize (exclusive shadow borrow).
    let attack_events = crate::game_logic::host_attack_log::drain();
    let move_events = crate::game_logic::host_move_log::drain();
    let mut attacks = 0usize;
    let mut moves = 0usize;
    for ev in &attack_events {
        if shadow.queue_set_attack_target_for_host(ev.attacker, ev.target) {
            attacks = attacks.saturating_add(1);
        }
    }
    for ev in &move_events {
        if shadow.queue_set_move_target_for_host(ev.unit, ev.destination) {
            moves = moves.saturating_add(1);
        }
    }
    // Host movement.target residual (path follow destinations not always logged).
    if gameworld_movement_authority_enabled() {
        let _ = shadow.apply_host_move_targets(logic);
    }
    if attacks > 0 || moves > 0 {
        let _ = shadow.apply_pending();
    }
    // Wave 712: handoff batches so session does not re-queue.
    if attack_events.is_empty() {
        EARLY_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
    } else {
        EARLY_ATTACK_BATCH.with(|c| *c.borrow_mut() = Some((attack_events, true)));
    }
    if move_events.is_empty() {
        EARLY_MOVE_BATCH.with(|c| *c.borrow_mut() = None);
    } else {
        EARLY_MOVE_BATCH.with(|c| *c.borrow_mut() = Some((move_events, true)));
    }
    (attacks, moves)
}
// Wave 684: post-logic damage batch handoff to shadow session (avoid double-apply).
thread_local! {
    static EARLY_DAMAGE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_damage_log::HostDamageEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

/// Take post-logic damage batch if Wave 684 already drained+applied it.
/// Returns `(events, already_applied_to_shadow)`.
pub fn take_early_damage_batch() -> Option<(
    Vec<crate::game_logic::host_damage_log::HostDamageEvent>,
    bool,
)> {
    EARLY_DAMAGE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 684: immediately after the host logic frame on a coupled tick, drain
/// `host_damage_log` into GameWorld Damage/Destroy mutations.
///
/// Stashes the batch for `shadow_session_after_host_tick` so `write_health` still
/// sees non-empty events and does not double-apply mutations.
pub fn eager_apply_host_damage_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 684: post-logic damage materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_damage_log::drain();
    if events.is_empty() {
        EARLY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let (queued, applied) = shadow.apply_host_damage_events(&events);
    EARLY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    queued.saturating_add(applied)
}
// Wave 685: post-logic heal batch handoff to shadow session (avoid double-apply).
thread_local! {
    static EARLY_HEAL_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_heal_log::HostHealEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_heal_batch() -> Option<(Vec<crate::game_logic::host_heal_log::HostHealEvent>, bool)> {
    EARLY_HEAL_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 685: immediately after the host logic frame on a coupled tick, drain
/// `host_heal_log` into GameWorld SetHealth mutations.
///
/// Stashes the batch for `shadow_session_after_host_tick` so `write_health` still
/// sees non-empty heals and does not double-apply mutations.
pub fn eager_apply_host_heal_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 685: post-logic heal materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_heal_log::drain();
    if events.is_empty() {
        EARLY_HEAL_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_heal_events(&events);
    EARLY_HEAL_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 686: post-logic max-health / experience batch handoff (avoid double-apply).
thread_local! {
    static EARLY_MAX_HEALTH_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_max_health_log::HostMaxHealthEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_EXPERIENCE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_experience_log::HostExperienceEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_max_health_batch() -> Option<(
    Vec<crate::game_logic::host_max_health_log::HostMaxHealthEvent>,
    bool,
)> {
    EARLY_MAX_HEALTH_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_experience_batch() -> Option<(
    Vec<crate::game_logic::host_experience_log::HostExperienceEvent>,
    bool,
)> {
    EARLY_EXPERIENCE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 686: post-logic drain `host_max_health_log` into GameWorld SetMaxHealth.
pub fn eager_apply_host_max_health_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 686: post-logic max-health materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_max_health_log::drain();
    if events.is_empty() {
        EARLY_MAX_HEALTH_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_max_health_events(&events);
    EARLY_MAX_HEALTH_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 686: post-logic drain `host_experience_log` into GameWorld SetExperience.
pub fn eager_apply_host_experience_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_damage_authority_enabled()
    {
        return 0;
    }
    // Wave 686: post-logic experience materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_experience_log::drain();
    if events.is_empty() {
        EARLY_EXPERIENCE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_experience_events(&events);
    EARLY_EXPERIENCE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 687: post-logic AI-state / fire-intent batch handoff (avoid double-apply).
thread_local! {
    static EARLY_AI_STATE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_state_log::HostAiStateEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_FIRE_INTENT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_fire_intent_log::HostFireIntentEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_state_batch() -> Option<(
    Vec<crate::game_logic::host_ai_state_log::HostAiStateEvent>,
    bool,
)> {
    EARLY_AI_STATE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_fire_intent_batch() -> Option<(
    Vec<crate::game_logic::host_fire_intent_log::HostFireIntentEvent>,
    bool,
)> {
    EARLY_FIRE_INTENT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 687: post-logic drain `host_ai_state_log` into GameWorld SetAiState.
pub fn eager_apply_host_ai_state_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 687: post-logic AI-state materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_state_log::drain();
    if events.is_empty() {
        EARLY_AI_STATE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_state_events(&events);
    EARLY_AI_STATE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 687: post-logic drain `host_fire_intent_log` into GameWorld SetFireIntent.
pub fn eager_apply_host_fire_intent_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 687: post-logic fire-intent materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_fire_intent_log::drain();
    if events.is_empty() {
        EARLY_FIRE_INTENT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_fire_intent_events(&events);
    EARLY_FIRE_INTENT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 688: post-logic owner / movement batch handoff (avoid double-apply).
thread_local! {
    static EARLY_OWNER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_owner_log::HostOwnerEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_MOVEMENT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_movement_log::HostMovementEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_owner_batch() -> Option<(Vec<crate::game_logic::host_owner_log::HostOwnerEvent>, bool)>
{
    EARLY_OWNER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_movement_batch() -> Option<(
    Vec<crate::game_logic::host_movement_log::HostMovementEvent>,
    bool,
)> {
    EARLY_MOVEMENT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 688: post-logic drain `host_owner_log` into GameWorld TransferOwner.
pub fn eager_apply_host_owner_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 688: post-logic owner materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_owner_log::drain();
    if events.is_empty() {
        EARLY_OWNER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_owner_events(logic, &events);
    EARLY_OWNER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 688: post-logic drain `host_movement_log` into GameWorld SetMovement.
pub fn eager_apply_host_movement_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 688: post-logic movement residual materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_movement_log::drain();
    if events.is_empty() {
        EARLY_MOVEMENT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_movement_events(&events);
    EARLY_MOVEMENT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 689: post-logic combat-status / veterancy batch handoff (avoid double-apply).
thread_local! {
    static EARLY_STATUS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_status_log::HostStatusEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_VETERANCY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_veterancy_log::HostVeterancyEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_status_batch() -> Option<(
    Vec<crate::game_logic::host_status_log::HostStatusEvent>,
    bool,
)> {
    EARLY_STATUS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_veterancy_batch() -> Option<(
    Vec<crate::game_logic::host_veterancy_log::HostVeterancyEvent>,
    bool,
)> {
    EARLY_VETERANCY_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 689: post-logic drain `host_status_log` into GameWorld SetCombatStatus.
pub fn eager_apply_host_status_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 689: post-logic combat-status materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_status_log::drain();
    if events.is_empty() {
        EARLY_STATUS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let mut n = 0usize;
    for ev in &events {
        if shadow.queue_set_combat_status_for_host(*ev) {
            n = n.saturating_add(1);
        }
    }
    if n > 0 {
        let _ = shadow.apply_pending();
    }
    EARLY_STATUS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 689: post-logic drain `host_veterancy_log` into GameWorld SetVeterancy.
pub fn eager_apply_host_veterancy_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 689: post-logic veterancy materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_veterancy_log::drain();
    if events.is_empty() {
        EARLY_VETERANCY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let mut n = 0usize;
    for ev in &events {
        if shadow.queue_set_veterancy_for_host(ev.object, ev.ordinal) {
            n = n.saturating_add(1);
        }
    }
    if n > 0 {
        let _ = shadow.apply_pending();
    }
    EARLY_VETERANCY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 690: post-logic weapon-bonus / weapon-slot batch handoff (avoid double-apply).
thread_local! {
    static EARLY_WEAPON_BONUS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_WEAPON_SLOT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_slot_log::HostWeaponSlotEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_weapon_bonus_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_bonus_log::HostWeaponBonusEvent>,
    bool,
)> {
    EARLY_WEAPON_BONUS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_weapon_slot_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_slot_log::HostWeaponSlotEvent>,
    bool,
)> {
    EARLY_WEAPON_SLOT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 690: post-logic drain `host_weapon_bonus_log` into GameWorld SetWeaponBonus.
pub fn eager_apply_host_weapon_bonus_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 690: post-logic weapon-bonus materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_bonus_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_BONUS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_bonus_events(&events);
    EARLY_WEAPON_BONUS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 690: post-logic drain `host_weapon_slot_log` into GameWorld SetActiveWeaponSlot.
pub fn eager_apply_host_weapon_slot_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 690: post-logic weapon-slot materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_slot_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_SLOT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_slot_events(&events);
    EARLY_WEAPON_SLOT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 691: post-logic weapon-set / entity-power batch handoff (avoid double-apply).
thread_local! {
    static EARLY_WEAPON_SET_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_set_log::HostWeaponSetEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_ENTITY_POWER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_entity_power_log::HostEntityPowerEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_weapon_set_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_set_log::HostWeaponSetEvent>,
    bool,
)> {
    EARLY_WEAPON_SET_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_entity_power_batch() -> Option<(
    Vec<crate::game_logic::host_entity_power_log::HostEntityPowerEvent>,
    bool,
)> {
    EARLY_ENTITY_POWER_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 691: post-logic drain `host_weapon_set_log` into GameWorld SetWeaponSetFlags.
pub fn eager_apply_host_weapon_set_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 691: post-logic weapon-set materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_set_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_SET_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_set_events(&events);
    EARLY_WEAPON_SET_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 691: post-logic drain `host_entity_power_log` into GameWorld SetEntityPower.
pub fn eager_apply_host_entity_power_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 691: post-logic entity-power materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_entity_power_log::drain();
    if events.is_empty() {
        EARLY_ENTITY_POWER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_entity_power_events(&events);
    EARLY_ENTITY_POWER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 692: post-logic turret / guard / rally batch handoff (avoid double-apply).
thread_local! {
    static EARLY_TURRET_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_turret_log::HostTurretEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_GUARD_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_guard_log::HostGuardEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_RALLY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_rally_log::HostRallyEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_turret_batch() -> Option<(
    Vec<crate::game_logic::host_turret_log::HostTurretEvent>,
    bool,
)> {
    EARLY_TURRET_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_guard_batch() -> Option<(Vec<crate::game_logic::host_guard_log::HostGuardEvent>, bool)>
{
    EARLY_GUARD_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_rally_batch() -> Option<(Vec<crate::game_logic::host_rally_log::HostRallyEvent>, bool)>
{
    EARLY_RALLY_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 692: post-logic drain `host_turret_log` into GameWorld SetTurret.
pub fn eager_apply_host_turret_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 692: post-logic turret materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_turret_log::drain();
    if events.is_empty() {
        EARLY_TURRET_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_turret_events(&events);
    EARLY_TURRET_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 692: post-logic drain `host_guard_log` into GameWorld SetGuard.
pub fn eager_apply_host_guard_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 692: post-logic guard materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_guard_log::drain();
    if events.is_empty() {
        EARLY_GUARD_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_guard_events(&events);
    EARLY_GUARD_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 692: post-logic drain `host_rally_log` into GameWorld SetRallyPoint.
pub fn eager_apply_host_rally_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 692: post-logic rally materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_rally_log::drain();
    if events.is_empty() {
        EARLY_RALLY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_rally_events(&events);
    EARLY_RALLY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 693: post-logic target-location / detector / continuous-fire batch handoff.
thread_local! {
    static EARLY_TARGET_LOCATION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_target_location_log::HostTargetLocationEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_DETECTOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_detector_log::HostDetectorEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_CONTINUOUS_FIRE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_continuous_fire_log::HostContinuousFireEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_target_location_batch() -> Option<(
    Vec<crate::game_logic::host_target_location_log::HostTargetLocationEvent>,
    bool,
)> {
    EARLY_TARGET_LOCATION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_detector_batch() -> Option<(
    Vec<crate::game_logic::host_detector_log::HostDetectorEvent>,
    bool,
)> {
    EARLY_DETECTOR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_continuous_fire_batch() -> Option<(
    Vec<crate::game_logic::host_continuous_fire_log::HostContinuousFireEvent>,
    bool,
)> {
    EARLY_CONTINUOUS_FIRE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 693: post-logic drain `host_target_location_log` into GameWorld SetTargetLocation.
pub fn eager_apply_host_target_location_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 693: post-logic target-location materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_target_location_log::drain();
    if events.is_empty() {
        EARLY_TARGET_LOCATION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_target_location_events(&events);
    EARLY_TARGET_LOCATION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 693: post-logic drain `host_detector_log` into GameWorld SetDetector.
pub fn eager_apply_host_detector_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 693: post-logic detector materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_detector_log::drain();
    if events.is_empty() {
        EARLY_DETECTOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_detector_events(&events);
    EARLY_DETECTOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 693: post-logic drain `host_continuous_fire_log` into GameWorld SetContinuousFire.
pub fn eager_apply_host_continuous_fire_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 693: post-logic continuous-fire materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_continuous_fire_log::drain();
    if events.is_empty() {
        EARLY_CONTINUOUS_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_continuous_fire_events(&events);
    EARLY_CONTINUOUS_FIRE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 694: post-logic ai-attitude / overcharge / stealth-flags batch handoff.
thread_local! {
    static EARLY_AI_ATTITUDE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_attitude_log::HostAiAttitudeEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_OVERCHARGE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_overcharge_log::HostOverchargeEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_STEALTH_FLAGS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_attitude_batch() -> Option<(
    Vec<crate::game_logic::host_ai_attitude_log::HostAiAttitudeEvent>,
    bool,
)> {
    EARLY_AI_ATTITUDE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_overcharge_batch() -> Option<(
    Vec<crate::game_logic::host_overcharge_log::HostOverchargeEvent>,
    bool,
)> {
    EARLY_OVERCHARGE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_stealth_flags_batch() -> Option<(
    Vec<crate::game_logic::host_stealth_flags_log::HostStealthFlagsEvent>,
    bool,
)> {
    EARLY_STEALTH_FLAGS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 694: post-logic drain `host_ai_attitude_log` into GameWorld SetAiAttitude.
pub fn eager_apply_host_ai_attitude_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 694: post-logic AI-attitude materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_attitude_log::drain();
    if events.is_empty() {
        EARLY_AI_ATTITUDE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_attitude_events(&events);
    EARLY_AI_ATTITUDE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 694: post-logic drain `host_overcharge_log` into GameWorld SetOvercharge.
pub fn eager_apply_host_overcharge_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 694: post-logic overcharge materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_overcharge_log::drain();
    if events.is_empty() {
        EARLY_OVERCHARGE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_overcharge_events(&events);
    EARLY_OVERCHARGE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 694: post-logic drain `host_stealth_flags_log` into GameWorld SetStealthFlags.
pub fn eager_apply_host_stealth_flags_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 694: post-logic stealth-flags materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_stealth_flags_log::drain();
    if events.is_empty() {
        EARLY_STEALTH_FLAGS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_stealth_flags_events(&events);
    EARLY_STEALTH_FLAGS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 695: post-logic contain-capacity / hive / overlord batch handoff.
thread_local! {
    static EARLY_CONTAIN_CAPACITY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_contain_capacity_log::HostContainCapacityEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_HIVE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_hive_log::HostHiveEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_OVERLORD_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_overlord_log::HostOverlordEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_contain_capacity_batch() -> Option<(
    Vec<crate::game_logic::host_contain_capacity_log::HostContainCapacityEvent>,
    bool,
)> {
    EARLY_CONTAIN_CAPACITY_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_hive_batch() -> Option<(Vec<crate::game_logic::host_hive_log::HostHiveEvent>, bool)> {
    EARLY_HIVE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_overlord_batch() -> Option<(
    Vec<crate::game_logic::host_overlord_log::HostOverlordEvent>,
    bool,
)> {
    EARLY_OVERLORD_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 695: post-logic drain `host_contain_capacity_log` into GameWorld SetContainCapacity.
pub fn eager_apply_host_contain_capacity_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 695: post-logic contain-capacity materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_contain_capacity_log::drain();
    if events.is_empty() {
        EARLY_CONTAIN_CAPACITY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_contain_capacity_events(&events);
    EARLY_CONTAIN_CAPACITY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 695: post-logic drain `host_hive_log` into GameWorld SetHiveSlaves.
pub fn eager_apply_host_hive_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 695: post-logic hive materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_hive_log::drain();
    if events.is_empty() {
        EARLY_HIVE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_hive_events(&events);
    EARLY_HIVE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 695: post-logic drain `host_overlord_log` into GameWorld SetOverlordAddon.
pub fn eager_apply_host_overlord_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 695: post-logic overlord materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_overlord_log::drain();
    if events.is_empty() {
        EARLY_OVERLORD_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_overlord_events(&events);
    EARLY_OVERLORD_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 696: post-logic command-set / disguise / vision-camo batch handoff.
thread_local! {
    static EARLY_COMMAND_SET_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_command_set_log::HostCommandSetEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_DISGUISE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_disguise_log::HostDisguiseEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_VISION_CAMO_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_vision_camo_log::HostVisionCamoEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_command_set_batch() -> Option<(
    Vec<crate::game_logic::host_command_set_log::HostCommandSetEvent>,
    bool,
)> {
    EARLY_COMMAND_SET_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_disguise_batch() -> Option<(
    Vec<crate::game_logic::host_disguise_log::HostDisguiseEvent>,
    bool,
)> {
    EARLY_DISGUISE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_vision_camo_batch() -> Option<(
    Vec<crate::game_logic::host_vision_camo_log::HostVisionCamoEvent>,
    bool,
)> {
    EARLY_VISION_CAMO_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 696: post-logic drain `host_command_set_log` into GameWorld SetCommandSet.
pub fn eager_apply_host_command_set_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 696: post-logic command-set materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_command_set_log::drain();
    if events.is_empty() {
        EARLY_COMMAND_SET_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_command_set_events(&events);
    EARLY_COMMAND_SET_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 696: post-logic drain `host_disguise_log` into GameWorld SetDisguise.
pub fn eager_apply_host_disguise_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 696: post-logic disguise materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_disguise_log::drain();
    if events.is_empty() {
        EARLY_DISGUISE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_disguise_events(&events);
    EARLY_DISGUISE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 696: post-logic drain `host_vision_camo_log` into GameWorld SetVisionCamo.
pub fn eager_apply_host_vision_camo_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 696: post-logic vision-camo materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_vision_camo_log::drain();
    if events.is_empty() {
        EARLY_VISION_CAMO_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_vision_camo_events(&events);
    EARLY_VISION_CAMO_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 697: post-logic weapon-stats / selection-radius / model-condition batch handoff.
thread_local! {
    static EARLY_WEAPON_STATS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_SELECTION_RADIUS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_selection_radius_log::HostSelectionRadiusEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_MODEL_CONDITION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_model_condition_log::HostModelConditionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_weapon_stats_batch() -> Option<(
    Vec<crate::game_logic::host_weapon_stats_log::HostWeaponStatsEvent>,
    bool,
)> {
    EARLY_WEAPON_STATS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_selection_radius_batch() -> Option<(
    Vec<crate::game_logic::host_selection_radius_log::HostSelectionRadiusEvent>,
    bool,
)> {
    EARLY_SELECTION_RADIUS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_model_condition_batch() -> Option<(
    Vec<crate::game_logic::host_model_condition_log::HostModelConditionEvent>,
    bool,
)> {
    EARLY_MODEL_CONDITION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 697: post-logic drain `host_weapon_stats_log` into GameWorld SetWeaponStats.
pub fn eager_apply_host_weapon_stats_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 697: post-logic weapon-stats materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_weapon_stats_log::drain();
    if events.is_empty() {
        EARLY_WEAPON_STATS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_weapon_stats_events(&events);
    EARLY_WEAPON_STATS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 697: post-logic drain `host_selection_radius_log` into GameWorld SetSelectionRadius.
pub fn eager_apply_host_selection_radius_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 697: post-logic selection-radius materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_selection_radius_log::drain();
    if events.is_empty() {
        EARLY_SELECTION_RADIUS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_selection_radius_events(&events);
    EARLY_SELECTION_RADIUS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 697: post-logic drain `host_model_condition_log` into GameWorld SetModelCondition.
pub fn eager_apply_host_model_condition_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 697: post-logic model-condition materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_model_condition_log::drain();
    if events.is_empty() {
        EARLY_MODEL_CONDITION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_model_condition_events(&events);
    EARLY_MODEL_CONDITION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 698: post-logic demo-mine-cheer / formation / crush-vision batch handoff.
thread_local! {
    static EARLY_DEMO_MINE_CHEER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_demo_mine_cheer_log::HostDemoMineCheerEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_FORMATION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_formation_log::HostFormationEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_CRUSH_VISION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_crush_vision_log::HostCrushVisionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_demo_mine_cheer_batch() -> Option<(
    Vec<crate::game_logic::host_demo_mine_cheer_log::HostDemoMineCheerEvent>,
    bool,
)> {
    EARLY_DEMO_MINE_CHEER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_formation_batch() -> Option<(
    Vec<crate::game_logic::host_formation_log::HostFormationEvent>,
    bool,
)> {
    EARLY_FORMATION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_crush_vision_batch() -> Option<(
    Vec<crate::game_logic::host_crush_vision_log::HostCrushVisionEvent>,
    bool,
)> {
    EARLY_CRUSH_VISION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 698: post-logic drain `host_demo_mine_cheer_log` into GameWorld SetDemoMineCheer.
pub fn eager_apply_host_demo_mine_cheer_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 698: post-logic demo-mine-cheer materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_demo_mine_cheer_log::drain();
    if events.is_empty() {
        EARLY_DEMO_MINE_CHEER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_demo_mine_cheer_events(&events);
    EARLY_DEMO_MINE_CHEER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 698: post-logic drain `host_formation_log` into GameWorld SetFormation.
pub fn eager_apply_host_formation_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 698: post-logic formation materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_formation_log::drain();
    if events.is_empty() {
        EARLY_FORMATION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_formation_events(&events);
    EARLY_FORMATION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 698: post-logic drain `host_crush_vision_log` into GameWorld SetCrushVision.
pub fn eager_apply_host_crush_vision_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 698: post-logic crush-vision materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_crush_vision_log::drain();
    if events.is_empty() {
        EARLY_CRUSH_VISION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_crush_vision_events(&events);
    EARLY_CRUSH_VISION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 699: post-logic building-type / identity / ground-height batch handoff.
thread_local! {
    static EARLY_BUILDING_TYPE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_building_type_log::HostBuildingTypeEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_IDENTITY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_identity_log::HostIdentityEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_GROUND_HEIGHT_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ground_height_log::HostGroundHeightEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_building_type_batch() -> Option<(
    Vec<crate::game_logic::host_building_type_log::HostBuildingTypeEvent>,
    bool,
)> {
    EARLY_BUILDING_TYPE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_identity_batch() -> Option<(
    Vec<crate::game_logic::host_identity_log::HostIdentityEvent>,
    bool,
)> {
    EARLY_IDENTITY_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_ground_height_batch() -> Option<(
    Vec<crate::game_logic::host_ground_height_log::HostGroundHeightEvent>,
    bool,
)> {
    EARLY_GROUND_HEIGHT_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 699: post-logic drain `host_building_type_log` into GameWorld SetBuildingType.
pub fn eager_apply_host_building_type_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 699: post-logic building-type materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_building_type_log::drain();
    if events.is_empty() {
        EARLY_BUILDING_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_building_type_events(&events);
    EARLY_BUILDING_TYPE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 699: post-logic drain `host_identity_log` into GameWorld SetIdentity.
pub fn eager_apply_host_identity_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 699: post-logic identity materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_identity_log::drain();
    if events.is_empty() {
        EARLY_IDENTITY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_identity_events(&events);
    EARLY_IDENTITY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 699: post-logic drain `host_ground_height_log` into GameWorld SetGroundHeight.
pub fn eager_apply_host_ground_height_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 699: post-logic ground-height materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ground_height_log::drain();
    if events.is_empty() {
        EARLY_GROUND_HEIGHT_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ground_height_events(&events);
    EARLY_GROUND_HEIGHT_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 700: post-logic model-mesh / fow / kind-of batch handoff.
thread_local! {
    static EARLY_MODEL_MESH_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_model_mesh_log::HostModelMeshEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_FOW_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_fow_log::HostFowEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_KIND_OF_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_kind_of_log::HostKindOfEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_model_mesh_batch() -> Option<(
    Vec<crate::game_logic::host_model_mesh_log::HostModelMeshEvent>,
    bool,
)> {
    EARLY_MODEL_MESH_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_fow_batch() -> Option<(Vec<crate::game_logic::host_fow_log::HostFowEvent>, bool)> {
    EARLY_FOW_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_kind_of_batch() -> Option<(
    Vec<crate::game_logic::host_kind_of_log::HostKindOfEvent>,
    bool,
)> {
    EARLY_KIND_OF_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 700: post-logic drain `host_model_mesh_log` into GameWorld SetModelMesh.
pub fn eager_apply_host_model_mesh_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 700: post-logic model-mesh materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_model_mesh_log::drain();
    if events.is_empty() {
        EARLY_MODEL_MESH_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_model_mesh_events(&events);
    EARLY_MODEL_MESH_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 700: post-logic drain `host_fow_log` into GameWorld SetFow.
pub fn eager_apply_host_fow_after_logic(shadow: &mut GameWorldShadow, _logic: &GameLogic) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 700: post-logic FOW materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_fow_log::drain();
    if events.is_empty() {
        EARLY_FOW_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_fow_events(&events);
    EARLY_FOW_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 700: post-logic drain `host_kind_of_log` into GameWorld SetKindOfBits.
pub fn eager_apply_host_kind_of_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 700: post-logic kind-of materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_kind_of_log::drain();
    if events.is_empty() {
        EARLY_KIND_OF_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_kind_of_events(&events);
    EARLY_KIND_OF_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 701: post-logic faerie-fire / repulsor / disable-timers batch handoff.
thread_local! {
    static EARLY_FAERIE_FIRE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_faerie_fire_log::HostFaerieFireEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_REPULSOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_repulsor_log::HostRepulsorEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_DISABLE_TIMERS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_disable_timers_log::HostDisableTimersEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_faerie_fire_batch() -> Option<(
    Vec<crate::game_logic::host_faerie_fire_log::HostFaerieFireEvent>,
    bool,
)> {
    EARLY_FAERIE_FIRE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_repulsor_batch() -> Option<(
    Vec<crate::game_logic::host_repulsor_log::HostRepulsorEvent>,
    bool,
)> {
    EARLY_REPULSOR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_disable_timers_batch() -> Option<(
    Vec<crate::game_logic::host_disable_timers_log::HostDisableTimersEvent>,
    bool,
)> {
    EARLY_DISABLE_TIMERS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 701: post-logic drain `host_faerie_fire_log` into GameWorld SetFaerieFire.
pub fn eager_apply_host_faerie_fire_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 701: post-logic faerie-fire materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_faerie_fire_log::drain();
    if events.is_empty() {
        EARLY_FAERIE_FIRE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_faerie_fire_events(&events);
    EARLY_FAERIE_FIRE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 701: post-logic drain `host_repulsor_log` into GameWorld SetRepulsor.
pub fn eager_apply_host_repulsor_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 701: post-logic repulsor materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_repulsor_log::drain();
    if events.is_empty() {
        EARLY_REPULSOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_repulsor_events(&events);
    EARLY_REPULSOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 701: post-logic drain `host_disable_timers_log` into GameWorld SetDisableTimers.
pub fn eager_apply_host_disable_timers_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 701: post-logic disable-timers materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_disable_timers_log::drain();
    if events.is_empty() {
        EARLY_DISABLE_TIMERS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_disable_timers_events(&events);
    EARLY_DISABLE_TIMERS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 702: post-logic body-damage / death-type / physics-motive batch handoff.
thread_local! {
    static EARLY_BODY_DAMAGE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_body_damage_log::HostBodyDamageEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_DEATH_TYPE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_death_type_log::HostDeathTypeEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_PHYSICS_MOTIVE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_physics_motive_log::HostPhysicsMotiveEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_body_damage_batch() -> Option<(
    Vec<crate::game_logic::host_body_damage_log::HostBodyDamageEvent>,
    bool,
)> {
    EARLY_BODY_DAMAGE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_death_type_batch() -> Option<(
    Vec<crate::game_logic::host_death_type_log::HostDeathTypeEvent>,
    bool,
)> {
    EARLY_DEATH_TYPE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_physics_motive_batch() -> Option<(
    Vec<crate::game_logic::host_physics_motive_log::HostPhysicsMotiveEvent>,
    bool,
)> {
    EARLY_PHYSICS_MOTIVE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 702: post-logic drain `host_body_damage_log` into GameWorld SetBodyDamage.
pub fn eager_apply_host_body_damage_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 702: post-logic body-damage materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_body_damage_log::drain();
    if events.is_empty() {
        EARLY_BODY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_body_damage_events(&events);
    EARLY_BODY_DAMAGE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 702: post-logic drain `host_death_type_log` into GameWorld SetDeathType.
pub fn eager_apply_host_death_type_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 702: post-logic death-type materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_death_type_log::drain();
    if events.is_empty() {
        EARLY_DEATH_TYPE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_death_type_events(&events);
    EARLY_DEATH_TYPE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 702: post-logic drain `host_physics_motive_log` into GameWorld SetPhysicsMotive.
pub fn eager_apply_host_physics_motive_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 702: post-logic physics-motive materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_physics_motive_log::drain();
    if events.is_empty() {
        EARLY_PHYSICS_MOTIVE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_physics_motive_events(&events);
    EARLY_PHYSICS_MOTIVE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 703: post-logic locomotor / bounce-land batch handoff.
thread_local! {
    static EARLY_LOCOMOTOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_locomotor_log::HostLocomotorEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_BOUNCE_LAND_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_bounce_land_log::HostBounceLandEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_locomotor_batch() -> Option<(
    Vec<crate::game_logic::host_locomotor_log::HostLocomotorEvent>,
    bool,
)> {
    EARLY_LOCOMOTOR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_bounce_land_batch() -> Option<(
    Vec<crate::game_logic::host_bounce_land_log::HostBounceLandEvent>,
    bool,
)> {
    EARLY_BOUNCE_LAND_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 703: post-logic drain `host_locomotor_log` into GameWorld SetLocomotor.
pub fn eager_apply_host_locomotor_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 703: post-logic locomotor materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_locomotor_log::drain();
    if events.is_empty() {
        EARLY_LOCOMOTOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_locomotor_events(&events);
    EARLY_LOCOMOTOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 703: post-logic drain `host_bounce_land_log` into GameWorld SetBounceLand.
pub fn eager_apply_host_bounce_land_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 703: post-logic bounce-land materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_bounce_land_log::drain();
    if events.is_empty() {
        EARLY_BOUNCE_LAND_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_bounce_land_events(&events);
    EARLY_BOUNCE_LAND_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 704: post-logic AI-mood / AI-request / shock-stun batch handoff.
thread_local! {
    static EARLY_AI_MOOD_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_mood_log::HostAiMoodEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_AI_REQUEST_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_request_log::HostAiRequestEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_SHOCK_STUN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_shock_stun_log::HostShockStunEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_ai_mood_batch() -> Option<(
    Vec<crate::game_logic::host_ai_mood_log::HostAiMoodEvent>,
    bool,
)> {
    EARLY_AI_MOOD_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_ai_request_batch() -> Option<(
    Vec<crate::game_logic::host_ai_request_log::HostAiRequestEvent>,
    bool,
)> {
    EARLY_AI_REQUEST_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_shock_stun_batch() -> Option<(
    Vec<crate::game_logic::host_shock_stun_log::HostShockStunEvent>,
    bool,
)> {
    EARLY_SHOCK_STUN_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 704: post-logic drain `host_ai_mood_log` into GameWorld SetAiMood.
pub fn eager_apply_host_ai_mood_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 704: post-logic AI-mood materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_mood_log::drain();
    if events.is_empty() {
        EARLY_AI_MOOD_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_mood_events(&events);
    EARLY_AI_MOOD_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 704: post-logic drain `host_ai_request_log` into GameWorld SetAiRequest.
pub fn eager_apply_host_ai_request_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 704: post-logic AI-request materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_request_log::drain();
    if events.is_empty() {
        EARLY_AI_REQUEST_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_ai_request_events(&events);
    EARLY_AI_REQUEST_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 704: post-logic drain `host_shock_stun_log` into GameWorld SetShockStun.
pub fn eager_apply_host_shock_stun_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 704: post-logic shock-stun materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_shock_stun_log::drain();
    if events.is_empty() {
        EARLY_SHOCK_STUN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_shock_stun_events(&events);
    EARLY_SHOCK_STUN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 705: post-logic stealth-delay / sole-healing / radar-extend batch handoff.
thread_local! {
    static EARLY_STEALTH_DELAY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_stealth_delay_log::HostStealthDelayEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_SOLE_HEALING_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_sole_healing_log::HostSoleHealingEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_RADAR_EXTEND_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_radar_extend_log::HostRadarExtendEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_stealth_delay_batch() -> Option<(
    Vec<crate::game_logic::host_stealth_delay_log::HostStealthDelayEvent>,
    bool,
)> {
    EARLY_STEALTH_DELAY_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_sole_healing_batch() -> Option<(
    Vec<crate::game_logic::host_sole_healing_log::HostSoleHealingEvent>,
    bool,
)> {
    EARLY_SOLE_HEALING_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_radar_extend_batch() -> Option<(
    Vec<crate::game_logic::host_radar_extend_log::HostRadarExtendEvent>,
    bool,
)> {
    EARLY_RADAR_EXTEND_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 705: post-logic drain `host_stealth_delay_log` into GameWorld SetStealthDelay.
pub fn eager_apply_host_stealth_delay_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 705: post-logic stealth-delay materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_stealth_delay_log::drain();
    if events.is_empty() {
        EARLY_STEALTH_DELAY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_stealth_delay_events(&events);
    EARLY_STEALTH_DELAY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 705: post-logic drain `host_sole_healing_log` into GameWorld SetSoleHealing.
pub fn eager_apply_host_sole_healing_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 705: post-logic sole-healing materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_sole_healing_log::drain();
    if events.is_empty() {
        EARLY_SOLE_HEALING_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_sole_healing_events(&events);
    EARLY_SOLE_HEALING_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 705: post-logic drain `host_radar_extend_log` into GameWorld SetRadarExtend.
pub fn eager_apply_host_radar_extend_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 705: post-logic radar-extend materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_radar_extend_log::drain();
    if events.is_empty() {
        EARLY_RADAR_EXTEND_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_radar_extend_events(&events);
    EARLY_RADAR_EXTEND_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 706: post-logic hijacker / rebuild-producer / stored-supplies batch handoff.
thread_local! {
    static EARLY_HIJACKER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_hijacker_log::HostHijackerEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_REBUILD_PRODUCER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_rebuild_producer_log::HostRebuildProducerEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_STORED_SUPPLIES_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_stored_supplies_log::HostStoredSuppliesEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_hijacker_batch() -> Option<(
    Vec<crate::game_logic::host_hijacker_log::HostHijackerEvent>,
    bool,
)> {
    EARLY_HIJACKER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_rebuild_producer_batch() -> Option<(
    Vec<crate::game_logic::host_rebuild_producer_log::HostRebuildProducerEvent>,
    bool,
)> {
    EARLY_REBUILD_PRODUCER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_stored_supplies_batch() -> Option<(
    Vec<crate::game_logic::host_stored_supplies_log::HostStoredSuppliesEvent>,
    bool,
)> {
    EARLY_STORED_SUPPLIES_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 706: post-logic drain `host_hijacker_log` into GameWorld SetHijacker.
pub fn eager_apply_host_hijacker_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 706: post-logic hijacker materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_hijacker_log::drain();
    if events.is_empty() {
        EARLY_HIJACKER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_hijacker_events(&events);
    EARLY_HIJACKER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 706: post-logic drain `host_rebuild_producer_log` into GameWorld SetRebuildProducer.
pub fn eager_apply_host_rebuild_producer_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 706: post-logic rebuild-producer materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_rebuild_producer_log::drain();
    if events.is_empty() {
        EARLY_REBUILD_PRODUCER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_rebuild_producer_events(&events);
    EARLY_REBUILD_PRODUCER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 706: post-logic drain `host_stored_supplies_log` into GameWorld SetStoredSupplies.
pub fn eager_apply_host_stored_supplies_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 706: post-logic stored-supplies materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_stored_supplies_log::drain();
    if events.is_empty() {
        EARLY_STORED_SUPPLIES_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_stored_supplies_events(&events);
    EARLY_STORED_SUPPLIES_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 707: post-logic special-power / radar / player-progress batch handoff.
thread_local! {
    static EARLY_SPECIAL_POWER_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_special_power_log::HostSpecialPowerEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_RADAR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_radar_log::HostRadarEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_PLAYER_PROGRESS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_player_progress_log::HostPlayerProgressEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_special_power_batch() -> Option<(
    Vec<crate::game_logic::host_special_power_log::HostSpecialPowerEvent>,
    bool,
)> {
    EARLY_SPECIAL_POWER_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_radar_batch() -> Option<(Vec<crate::game_logic::host_radar_log::HostRadarEvent>, bool)>
{
    EARLY_RADAR_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_player_progress_batch() -> Option<(
    Vec<crate::game_logic::host_player_progress_log::HostPlayerProgressEvent>,
    bool,
)> {
    EARLY_PLAYER_PROGRESS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 707: post-logic drain `host_special_power_log` into GameWorld SetSpecialPower.
pub fn eager_apply_host_special_power_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 707: post-logic special-power materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_special_power_log::drain();
    if events.is_empty() {
        EARLY_SPECIAL_POWER_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_special_power_events(&events);
    EARLY_SPECIAL_POWER_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 707: post-logic drain `host_radar_log` into GameWorld SetRadar.
pub fn eager_apply_host_radar_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 707: post-logic radar materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_radar_log::drain();
    if events.is_empty() {
        EARLY_RADAR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_radar_events(&events);
    EARLY_RADAR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 707: post-logic drain `host_player_progress_log` into GameWorld SetPlayerProgress.
pub fn eager_apply_host_player_progress_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 707: post-logic player-progress materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_player_progress_log::drain();
    if events.is_empty() {
        EARLY_PLAYER_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_player_progress_events(&events);
    EARLY_PLAYER_PROGRESS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 708: post-logic player-meta / player-cooldown / production-door batch handoff.
thread_local! {
    static EARLY_PLAYER_META_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_player_meta_log::HostPlayerMetaEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_PLAYER_COOLDOWN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_player_cooldown_log::HostPlayerCooldownEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_PRODUCTION_DOOR_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_production_door_log::HostProductionDoorEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_player_meta_batch() -> Option<(
    Vec<crate::game_logic::host_player_meta_log::HostPlayerMetaEvent>,
    bool,
)> {
    EARLY_PLAYER_META_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_player_cooldown_batch() -> Option<(
    Vec<crate::game_logic::host_player_cooldown_log::HostPlayerCooldownEvent>,
    bool,
)> {
    EARLY_PLAYER_COOLDOWN_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_production_door_batch() -> Option<(
    Vec<crate::game_logic::host_production_door_log::HostProductionDoorEvent>,
    bool,
)> {
    EARLY_PRODUCTION_DOOR_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 708: post-logic drain `host_player_meta_log` into GameWorld player meta.
pub fn eager_apply_host_player_meta_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 708: post-logic player-meta materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_player_meta_log::drain();
    if events.is_empty() {
        EARLY_PLAYER_META_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_player_meta_events(&events);
    EARLY_PLAYER_META_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 708: post-logic drain `host_player_cooldown_log` into GameWorld player cooldowns.
pub fn eager_apply_host_player_cooldown_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 708: post-logic player-cooldown materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_player_cooldown_log::drain();
    if events.is_empty() {
        EARLY_PLAYER_COOLDOWN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_player_cooldown_events(&events);
    EARLY_PLAYER_COOLDOWN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 708: post-logic drain `host_production_door_log` into GameWorld SetProductionDoor.
pub fn eager_apply_host_production_door_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 708: post-logic production-door materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_production_door_log::drain();
    if events.is_empty() {
        EARLY_PRODUCTION_DOOR_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_production_door_events(&events);
    EARLY_PRODUCTION_DOOR_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 709: post-logic production / production-progress / construction batch handoff.
thread_local! {
    static EARLY_PRODUCTION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_production_log::HostProductionEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_PRODUCTION_PROGRESS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_production_progress_log::HostProductionProgressEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_CONSTRUCTION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_construction_log::HostConstructionEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_CONSTRUCTION_PROGRESS_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_construction_progress_log::HostConstructionProgressEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_production_batch() -> Option<(
    Vec<crate::game_logic::host_production_log::HostProductionEvent>,
    bool,
)> {
    EARLY_PRODUCTION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_production_progress_batch() -> Option<(
    Vec<crate::game_logic::host_production_progress_log::HostProductionProgressEvent>,
    bool,
)> {
    EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_construction_batch() -> Option<(
    Vec<crate::game_logic::host_construction_log::HostConstructionEvent>,
    bool,
)> {
    EARLY_CONSTRUCTION_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_construction_progress_batch() -> Option<(
    Vec<crate::game_logic::host_construction_progress_log::HostConstructionProgressEvent>,
    bool,
)> {
    EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 709: post-logic drain `host_production_log` into GameWorld production mutations.
pub fn eager_apply_host_production_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic production materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_production_log::drain();
    if events.is_empty() {
        EARLY_PRODUCTION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_production_events(&events, logic);
    EARLY_PRODUCTION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 709: post-logic drain `host_production_progress_log` into GameWorld production progress.
pub fn eager_apply_host_production_progress_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic production-progress materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_production_progress_log::drain();
    if events.is_empty() {
        EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_production_progress_events(&events);
    EARLY_PRODUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 709: post-logic drain `host_construction_log` into GameWorld construction.
pub fn eager_apply_host_construction_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic construction materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_construction_log::drain();
    if events.is_empty() {
        EARLY_CONSTRUCTION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_construction_events(&events, logic);
    EARLY_CONSTRUCTION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 709: post-logic drain `host_construction_progress_log` into GameWorld construction progress.
pub fn eager_apply_host_construction_progress_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 709: post-logic construction-progress materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_construction_progress_log::drain();
    if events.is_empty() {
        EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_construction_progress_events(&events);
    EARLY_CONSTRUCTION_PROGRESS_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 710: post-logic combat-attack / projectile batch handoff.
thread_local! {
    static EARLY_COMBAT_ATTACK_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_combat_attack_log::HostCombatAttackEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_PROJECTILE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_projectile_log::HostProjectileEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_combat_attack_batch() -> Option<(
    Vec<crate::game_logic::host_combat_attack_log::HostCombatAttackEvent>,
    bool,
)> {
    EARLY_COMBAT_ATTACK_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_projectile_batch() -> Option<(
    Vec<crate::game_logic::host_projectile_log::HostProjectileEvent>,
    bool,
)> {
    EARLY_PROJECTILE_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 710: post-logic drain `host_combat_attack_log` into GameWorld combat-attack state.
pub fn eager_apply_host_combat_attack_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 710: post-logic combat-attack materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_combat_attack_log::drain();
    if events.is_empty() {
        EARLY_COMBAT_ATTACK_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_combat_attack_events(&events);
    EARLY_COMBAT_ATTACK_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 710: post-logic drain `host_projectile_log` into GameWorld projectile flight.
pub fn eager_apply_host_projectile_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 710: post-logic projectile materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_projectile_log::drain();
    if events.is_empty() {
        EARLY_PROJECTILE_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_projectile_events(&events);
    EARLY_PROJECTILE_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 711: post-logic destroy / contain / AI-decision batch handoff.
thread_local! {
    static EARLY_DESTROY_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_destroy_log::HostDestroyEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_CONTAIN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_contain_log::HostContainEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_AI_DECISION_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_ai_decision_log::HostAiDecisionEvent>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_destroy_batch() -> Option<(
    Vec<crate::game_logic::host_destroy_log::HostDestroyEvent>,
    bool,
)> {
    EARLY_DESTROY_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_contain_batch() -> Option<(
    Vec<crate::game_logic::host_contain_log::HostContainEvent>,
    bool,
)> {
    EARLY_CONTAIN_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_ai_decision_batch() -> Option<(
    Vec<crate::game_logic::host_ai_decision_log::HostAiDecisionEvent>,
    bool,
)> {
    EARLY_AI_DECISION_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 711: post-logic drain `host_destroy_log` into GameWorld destroy queue.
pub fn eager_apply_host_destroy_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 711: post-logic destroy materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_destroy_log::drain();
    if events.is_empty() {
        EARLY_DESTROY_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let (queued, _) = shadow.apply_host_destroy_events(&events);
    EARLY_DESTROY_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    queued
}

/// Wave 711: post-logic drain `host_contain_log` into GameWorld contain/garrison.
pub fn eager_apply_host_contain_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 711: post-logic contain materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_contain_log::drain();
    if events.is_empty() {
        EARLY_CONTAIN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_contain_events(&events);
    EARLY_CONTAIN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Wave 711: post-logic drain `host_ai_decision_log` into GameWorld AI decisions.
pub fn eager_apply_host_ai_decision_after_logic(
    shadow: &mut GameWorldShadow,
    _logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active()
        || !gameworld_shadow_enabled()
        || !gameworld_ai_decision_authority_enabled()
    {
        return 0;
    }
    // Wave 711: post-logic AI-decision materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_ai_decision_log::drain();
    if events.is_empty() {
        EARLY_AI_DECISION_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_ai_decisions_as_world_mutations(&events);
    EARLY_AI_DECISION_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}
// Wave 712: post-logic spawn batch handoff (complements mid-frame eager_map).
thread_local! {
    static EARLY_SPAWN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_spawn_log::HostSpawnEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_ATTACK_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_attack_log::HostAttackEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_MOVE_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::host_move_log::HostMoveEvent>, bool)>> =
        std::cell::RefCell::new(None);
    static EARLY_FIRE_SPAWN_BATCH: std::cell::RefCell<Option<(Vec<crate::game_logic::combat::PendingProjectile>, bool)>> =
        std::cell::RefCell::new(None);
}

pub fn take_early_spawn_batch() -> Option<(Vec<crate::game_logic::host_spawn_log::HostSpawnEvent>, bool)>
{
    EARLY_SPAWN_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_attack_batch() -> Option<(
    Vec<crate::game_logic::host_attack_log::HostAttackEvent>,
    bool,
)> {
    EARLY_ATTACK_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_move_batch() -> Option<(Vec<crate::game_logic::host_move_log::HostMoveEvent>, bool)> {
    EARLY_MOVE_BATCH.with(|c| c.borrow_mut().take())
}

pub fn take_early_fire_spawn_batch() -> Option<(Vec<crate::game_logic::combat::PendingProjectile>, bool)>
{
    EARLY_FIRE_SPAWN_BATCH.with(|c| c.borrow_mut().take())
}

/// Wave 712: post-logic drain remaining `host_spawn_log` into GameWorld (idempotent with mid-frame map).
pub fn eager_apply_host_spawn_after_logic(
    shadow: &mut GameWorldShadow,
    logic: &GameLogic,
) -> usize {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return 0;
    }
    // Wave 712: post-logic spawn materialize (exclusive shadow borrow).
    let events = crate::game_logic::host_spawn_log::drain();
    if events.is_empty() {
        EARLY_SPAWN_BATCH.with(|c| *c.borrow_mut() = None);
        return 0;
    }
    let n = shadow.apply_host_spawn_events(&events, logic);
    EARLY_SPAWN_BATCH.with(|c| *c.borrow_mut() = Some((events, true)));
    n
}

/// Serializes tests (and residual harnesses) that mutate GENERALS_GAMEWORLD_* env.
#[cfg(test)]
pub(crate) fn authority_env_lock() -> std::sync::MutexGuard<'static, ()> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|e| e.into_inner())
}

pub fn gameworld_shadow_enabled() -> bool {
    env_flag_cached(&SHADOW_ENABLED_CACHE, "GENERALS_GAMEWORLD_SHADOW", true)
}

/// When enabled, GameWorld shadow mutations are the **last writer** for HP each tick.
/// Host combat still runs mid-frame; end-of-tick reapplies drained damage events
/// on the shadow and writebacks health/destroyed onto host objects.
/// Implies a shadow session (separate GENERALS_GAMEWORLD_SHADOW not required).
///
/// Env: `GENERALS_GAMEWORLD_DAMAGE_AUTHORITY=0|false` off; unset/`1` = **on** (production default).
pub fn gameworld_damage_authority_enabled() -> bool {
    env_flag_cached(
        &DAMAGE_AUTH_CACHE,
        "GENERALS_GAMEWORLD_DAMAGE_AUTHORITY",
        true,
    )
}

/// Damage HP defer only while shadow can writeback (alias of enabled&&shadow).
#[inline]
pub fn gameworld_damage_authority_live() -> bool {
    // Fail-open to host HP when no coupled engine shadow session is active
    // (unit tests, host-only gates). Matches construction/production sole-tick.
    gameworld_damage_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Economy last-writer (player supplies/power). Unset = **on**; `0|false` off.
pub fn gameworld_economy_authority_enabled() -> bool {
    env_flag_cached(
        &ECONOMY_AUTH_CACHE,
        "GENERALS_GAMEWORLD_ECONOMY_AUTHORITY",
        true,
    )
}

/// Economy last-writer is only meaningful while a shadow session can write back cash.
/// Host-only matches must mutate supplies immediately (same coupling as damage/fire-spawn).
#[inline]
pub fn gameworld_economy_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_economy_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled, GameWorld integrates path/move targets after the host tick and
/// writebacks pose/movement as last-writer. Host `update_movement` skips integrate.
///
/// Env: `GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_movement_authority_enabled() -> bool {
    env_flag_cached(
        &MOVEMENT_AUTH_CACHE,
        "GENERALS_GAMEWORLD_MOVEMENT_AUTHORITY",
        true,
    )
}

/// Movement last-writer only while shadow can step/writeback poses.
#[inline]
pub fn gameworld_movement_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_movement_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled (default), GameWorld SetAttackTarget + SetFireIntent writeback is the
/// last-writer for host attack target / fire-intent residual after each shadow session.
/// Host still *decides* and discharges weapons; opt out with `=0|false`.
/// Env: `GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_ai_attack_authority_enabled() -> bool {
    env_flag_cached(
        &AI_ATTACK_AUTH_CACHE,
        "GENERALS_GAMEWORLD_AI_ATTACK_AUTHORITY",
        true,
    )
}

/// AI attack/fire-intent channel only while shadow can writeback.
#[inline]
pub fn gameworld_ai_attack_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_ai_attack_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled (default), GameWorld steps projectile flight residual and
/// last-writes pose/lifetime into host CombatSystem before hit resolution.
/// Host still owns spawn/fire and hit/damage application.
/// Env: `GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_projectile_authority_enabled() -> bool {
    env_flag_cached(
        &PROJECTILE_AUTH_CACHE,
        "GENERALS_GAMEWORLD_PROJECTILE_AUTHORITY",
        true,
    )
}

/// Projectile integrate defer only while shadow session steps flight.
#[inline]
pub fn gameworld_projectile_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_projectile_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled (default), host `update_ai` only *records* AICommand decisions;
/// GameWorld applies attack/move/state mutations and writeback is last-writer.
/// Combat runs before AI in the host tick, so deferred apply is next-frame parity.
/// Env: `GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_ai_decision_authority_enabled() -> bool {
    env_flag_cached(
        &AI_DECISION_AUTH_CACHE,
        "GENERALS_GAMEWORLD_AI_DECISION_AUTHORITY",
        true,
    )
}

/// AI decision last-writer only while shadow can apply/writeback decisions.
#[inline]
pub fn gameworld_ai_decision_authority_live() -> bool {
    // Fail-open to host AI state when no coupled engine shadow writeback frame.
    gameworld_ai_decision_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled (default), `queue_projectile` only logs fire-spawns; shadow
/// applies them into host CombatSystem before projectile integrate authority.
/// Env: `GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_fire_spawn_authority_enabled() -> bool {
    env_flag_cached(
        &FIRE_SPAWN_AUTH_CACHE,
        "GENERALS_GAMEWORLD_FIRE_SPAWN_AUTHORITY",
        true,
    )
}

/// Fire-spawn defer only while shadow can drain spawn log.
#[inline]
pub fn gameworld_fire_spawn_authority_live() -> bool {
    // Fail-open to host when no coupled engine shadow writeback frame.
    gameworld_fire_spawn_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled, host construction percent is last-written from GameWorld after
/// progress logs (host still computes projected percent for completion side effects).
/// Env: `GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_construction_authority_enabled() -> bool {
    env_flag_cached(
        &CONSTRUCTION_AUTH_CACHE,
        "GENERALS_GAMEWORLD_CONSTRUCTION_AUTHORITY",
        true,
    )
}

/// Construction progress last-writer only while shadow can sole-tick percent.
#[inline]
pub fn gameworld_construction_authority_live() -> bool {
    gameworld_construction_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Host skips construction percent advance only when authority AND shadow session run.
pub fn gameworld_construction_sole_tick_enabled() -> bool {
    gameworld_construction_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Env: `GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_special_power_authority_enabled() -> bool {
    env_flag_cached(
        &SPECIAL_POWER_AUTH_CACHE,
        "GENERALS_GAMEWORLD_SPECIAL_POWER_AUTHORITY",
        true,
    )
}

/// Host skips SP countdown advance only when authority AND shadow session run.
pub fn gameworld_special_power_sole_tick_enabled() -> bool {
    gameworld_special_power_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// When enabled (default), GameWorld shadow is last-writer for production queue
/// identity (items/progress/rally/exit delay) via host progress logs + writeback.
/// Host still *completes/spawns* production (completion residual); shadow sole-ticks
/// queue progress + exit delay under PRODUCTION_AUTHORITY (Wave 464).
///
/// Env: `GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY=0|false` off; unset/`1` = **on**.
pub fn gameworld_production_authority_enabled() -> bool {
    env_flag_cached(
        &PRODUCTION_AUTH_CACHE,
        "GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY",
        true,
    )
}

/// Production queue last-writer only while shadow can sole-tick progress.
#[inline]
pub fn gameworld_production_authority_live() -> bool {
    gameworld_production_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Host skips progress advance only when production authority AND shadow session run.
pub fn gameworld_production_sole_tick_enabled() -> bool {
    gameworld_production_authority_enabled()
        && gameworld_shadow_enabled()
        && shadow_coupled_tick_active()
}

/// Gates/smoke: no-op when production defaults are already on.
/// Still forces `1` if env was never set (explicit documentation for gate binaries).
pub fn ensure_gate_damage_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_DAMAGE_AUTHORITY", "1");
        }
    }
    ensure_gate_economy_authority();
    ensure_gate_production_authority();
    // Caches may have been primed before gate env force-on.
    refresh_gameworld_authority_env_caches();
}

/// Gates/smoke: force economy authority env to `1` when unset.
pub fn ensure_gate_economy_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_ECONOMY_AUTHORITY", "1");
        }
    }
}

/// Gates/smoke: force production authority env to `1` when unset.
pub fn ensure_gate_production_authority() {
    if std::env::var_os("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY").is_none() {
        unsafe {
            std::env::set_var("GENERALS_GAMEWORLD_PRODUCTION_AUTHORITY", "1");
        }
    }
}


impl super::GameWorldShadow {
    /// Wave 761: expire faerie/repulsor/disable/frenzy/continuous-fire/selection flash.
    pub fn tick_status_timer_expirations(&mut self, frame: u32) -> usize {
        // Wave 761: under coupled dual-tick, GameWorld expires status timers
        // (faerie/repulsor/disable/frenzy/continuous-fire coast). Host peels the
        // matching mid-frame ticks so writeback is last-writer without dual expire.
        use gamelogic::world::entities::EntityId;
        let eids: Vec<EntityId> = self.host_to_entity.values().copied().collect();
        let mut n = 0usize;
        // Wave 813: snapshot living infantry + China horde units (borrow-safe).
        const INFANTRY_BIT: u32 = 1u32 << 1; // KindOf::Infantry in presentation ORDER
        let mut infantry_snapshot: Vec<(u32, u8, f32, f32, bool)> = Vec::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            let alive = e.health > 0.0 && !e.destroyed;
            if !alive {
                continue;
            }
            let Some(&hid) = self.entity_to_host.get(&eid.get()) else {
                continue;
            };
            if (e.kind_of_bits & INFANTRY_BIT) != 0 {
                infantry_snapshot.push((
                    hid,
                    e.team_ordinal,
                    e.transform.position.x,
                    e.transform.position.z,
                    alive,
                ));
            }
        }

        // Wave 812: snapshot living Battlemasters for horde residual (borrow-safe).
        let mut battlemaster_snapshot: Vec<(u32, u8, f32, f32, bool)> = Vec::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            let alive = e.health > 0.0 && !e.destroyed;
            if !alive {
                continue;
            }
            if !crate::game_logic::host_battlemaster::is_battlemaster_template(e.template_name()) {
                continue;
            }
            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                battlemaster_snapshot.push((
                    hid,
                    e.team_ordinal,
                    e.transform.position.x,
                    e.transform.position.z,
                    alive,
                ));
            }
        }

        // Wave 811: underpowered teams from shadow player economy (borrow-safe).
        let mut underpowered_team_ords: std::collections::HashSet<u8> =
            std::collections::HashSet::new();
        for (_pid, pd) in self.world.world().active_players() {
            if pd.power_available < 0 {
                if let Some(t) = pd.team {
                    underpowered_team_ords.insert(t);
                }
            }
        }

        // Wave 807: snapshot sticky/booby attach target positions (borrow-safe).
        let mut sticky_booby_targets: std::collections::HashMap<u32, (glam::Vec3, bool, bool)> =
            std::collections::HashMap::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            let tid = if e.sticky_bomb_attached {
                Some(e.sticky_bomb_attached_to)
            } else if e.booby_trap_special && e.booby_trap_has_attached {
                Some(e.booby_trap_attached_to)
            } else {
                None
            };
            let Some(tid) = tid else { continue };
            if sticky_booby_targets.contains_key(&tid) {
                continue;
            }
            if let Some(teid) = self.host_to_entity.get(&tid).copied() {
                if let Some(t) = self.world.entity(teid) {
                    let tp = t.transform.position;
                    let alive = t.health > 0.0 && !t.destroyed;
                    let immobile = (t.kind_of_bits & 1) != 0; // Structure bit 0
                    sticky_booby_targets
                        .insert(tid, (glam::Vec3::new(tp.x, tp.y, tp.z), alive, immobile));
                }
            }
        }

        // Wave 805: snapshot live scorpion missile intended positions (borrow-safe).
        let mut scorpion_retarget: std::collections::HashMap<u32, glam::Vec3> =
            std::collections::HashMap::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            if e.scorpion_missile_projectile && e.scorpion_missile_has_intended {
                let tid = e.scorpion_missile_intended;
                if let Some(teid) = self.host_to_entity.get(&tid).copied() {
                    if let Some(t) = self.world.entity(teid) {
                        if t.health > 0.0 && !t.destroyed {
                            let tp = t.transform.position;
                            scorpion_retarget.insert(eid.get(), glam::Vec3::new(tp.x, tp.y, tp.z));
                        }
                    }
                }
            }
        }

        // Wave 820: snapshot fire-spread candidates for ignition (borrow-safe).
        let mut fire_spread_candidates: Vec<(u32, f32, f32, bool)> = Vec::new();
        for eid in &eids {
            let Some(e) = self.world.entity(*eid) else {
                continue;
            };
            if !e.fire_spread_active {
                continue;
            }
            let Some(&hid) = self.entity_to_host.get(&eid.get()) else {
                continue;
            };
            let would = e.fire_spread_state == 0; // Normal can ignite
            fire_spread_candidates.push((
                hid,
                e.transform.position.x,
                e.transform.position.z,
                would,
            ));
        }

        for eid in eids {
            let Some(e) = self.world.world_mut().entity_mut(eid) else {
                continue;
            };
            let mut changed = false;
            if e.faerie_fire && e.faerie_fire_until_frame > 0 && frame >= e.faerie_fire_until_frame
            {
                e.faerie_fire = false;
                e.faerie_fire_until_frame = 0;
                changed = true;
            }
            if e.repulsor && e.repulsor_until_frame > 0 {
                e.repulsor_until_frame = e.repulsor_until_frame.saturating_sub(1);
                if e.repulsor_until_frame == 0 {
                    e.repulsor = false;
                }
                changed = true;
            }
            if e.disabled_emp
                && e.disabled_emp_until_frame > 0
                && frame >= e.disabled_emp_until_frame
            {
                e.disabled_emp = false;
                e.disabled_emp_until_frame = 0;
                changed = true;
            }
            if e.disabled_hacked
                && e.disabled_hacked_until_frame > 0
                && frame >= e.disabled_hacked_until_frame
            {
                e.disabled_hacked = false;
                e.disabled_hacked_until_frame = 0;
                changed = true;
            }
            if e.disabled_paralyzed
                && e.disabled_paralyzed_until_frame > 0
                && frame >= e.disabled_paralyzed_until_frame
            {
                e.disabled_paralyzed = false;
                e.disabled_paralyzed_until_frame = 0;
                changed = true;
            }
            if e.weapon_bonus_frenzy
                && e.weapon_bonus_frenzy_until_frame > 0
                && frame >= e.weapon_bonus_frenzy_until_frame
            {
                e.weapon_bonus_frenzy = false;
                e.weapon_bonus_frenzy_until_frame = 0;
                e.weapon_bonus_frenzy_level = 0;
                changed = true;
            }
            if e.continuous_fire_level > 0 {
                let until = e.continuous_fire_coast_until_frame;
                if until > 0 && frame >= until {
                    e.continuous_fire_level = 0;
                    e.continuous_fire_consecutive = 0;
                    e.continuous_fire_coast_until_frame = 0;
                    changed = true;
                }
            }
            if e.selection_flash_remaining > 0 {
                e.selection_flash_remaining = e.selection_flash_remaining.saturating_sub(1);
                changed = true;
            }
            // Wave 762: eject-invulnerable until_frame residual (parity with host tick).
            if e.eject_invulnerable
                && e.eject_invulnerable_until_frame > 0
                && frame >= e.eject_invulnerable_until_frame
            {
                e.eject_invulnerable = false;
                e.eject_invulnerable_until_frame = 0;
                changed = true;
            }
            // Wave 763: force-reload-when-idle residual (C++ FiringTracker).
            if e.frame_to_force_reload > 0 && frame >= e.frame_to_force_reload {
                let needs = e.weapon_clip_size > 0 && e.weapon_ammo < e.weapon_clip_size;
                if needs {
                    e.weapon_ammo = e.weapon_clip_size;
                }
                e.frame_to_force_reload = 0;
                changed = true;
            }
            // Wave 764: shock-stun frame countdown residual (physics/rates stay host).
            if e.shock_stun_frames > 0 {
                e.shock_stun_frames = e.shock_stun_frames.saturating_sub(1);
                changed = true;
            }
            // Wave 765: subdual damage heal residual (C++ SubdualDamageHeal*).
            if e.subdual_damage > 0.0
                && e.subdual_heal_rate_frames > 0
                && e.subdual_heal_amount > 0.0
            {
                if e.subdual_heal_countdown > 0 {
                    e.subdual_heal_countdown -= 1;
                    changed = true;
                } else {
                    let was = e.max_health > 0.0 && e.subdual_damage + 1e-3 >= e.max_health;
                    e.subdual_damage = (e.subdual_damage - e.subdual_heal_amount).max(0.0);
                    e.subdual_heal_countdown = e.subdual_heal_rate_frames;
                    let now = e.max_health > 0.0 && e.subdual_damage + 1e-3 >= e.max_health;
                    if was && !now {
                        e.disabled_subdued = false;
                    }
                    changed = true;
                }
            }
            // Wave 766: ObjectDefectionHelper timer residual (flash/audio via writeback).
            e.defection_flash_this_frame = false;
            e.defection_final_white_flash = false;
            if e.defection_undetected {
                let dead = e.destroyed || e.health <= 0.0;
                if dead || e.is_firing_weapon {
                    e.defection_undetected = false;
                    e.defection_do_fx = false;
                    e.defection_detection_end = 0;
                    e.defection_flash_phase = 0.0;
                    changed = true;
                } else if e.defection_detection_end > 0 && frame >= e.defection_detection_end {
                    e.defection_undetected = false;
                    if e.defection_do_fx {
                        e.defection_final_white_flash = true;
                    }
                    e.defection_do_fx = false;
                    e.defection_detection_end = 0;
                    e.defection_flash_phase = 0.0;
                    changed = true;
                } else if e.defection_do_fx && e.defection_detection_end > 0 {
                    let last_phase = (e.defection_flash_phase as i32) & 1;
                    let time_left = e.defection_detection_end.saturating_sub(frame) as f32;
                    let max_t = 300f32;
                    e.defection_flash_phase += 0.5 * (1.0 - (time_left / max_t));
                    let this_phase = (e.defection_flash_phase as i32) & 1;
                    if last_phase != 0 && this_phase == 0 {
                        e.defection_flash_this_frame = true;
                    }
                    changed = true;
                }
            }
            // Wave 767: FireSoundLoopTime residual (FiringTracker audio loop stop).
            if e.fire_sound_loop_until_frame > 0 && frame >= e.fire_sound_loop_until_frame {
                let sound = std::mem::take(&mut e.fire_sound_loop_name);
                e.fire_sound_loop_until_frame = 0;
                if !sound.is_empty() {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_fire_sound_loop_log::record(
                            crate::game_logic::ObjectId(hid),
                            sound,
                            false,
                        );
                    }
                }
                changed = true;
            }
            // Wave 768: LifetimeUpdate residual (auto-die after min/max frames).
            if e.lifetime_active
                && e.lifetime_expire_at_frame > 0
                && frame >= e.lifetime_expire_at_frame
            {
                e.lifetime_active = false;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_lifetime_expire_log::record(
                        crate::game_logic::ObjectId(hid),
                    );
                }
                changed = true;
            }
            // Wave 769: PoisonedBehavior DoT residual (UNRESISTABLE retake).
            if e.poison_overall_stop_frame != 0 {
                if e.poison_damage_frame != 0 && frame >= e.poison_damage_frame {
                    let amount = e.poison_damage_amount;
                    if amount > 0.0 {
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_poison_dot_log::record(
                                crate::game_logic::ObjectId(hid),
                                amount,
                                crate::game_logic::host_usa_pilot::HostDeathType::Poisoned,
                            );
                        }
                    }
                    let interval =
                        crate::game_logic::host_poisoned_behavior::poison_interval_frames();
                    e.poison_damage_frame = frame.saturating_add(interval);
                    changed = true;
                }
                if frame >= e.poison_overall_stop_frame {
                    e.poison_damage_frame = 0;
                    e.poison_overall_stop_frame = 0;
                    e.poison_damage_amount = 0.0;
                    e.poison_tint = false;
                    changed = true;
                }
            }
            // Wave 770: ToppleUpdate fall residual (trees / crushable props).
            if e.topple_active && e.topple_state == 1 {
                use crate::game_logic::host_topple::{
                    TOPPLE_ANGULAR_LIMIT, TOPPLE_BOUNCE_VELOCITY_PERCENT, TOPPLE_OPTIONS_NO_BOUNCE,
                    TOPPLE_VELOCITY_BOUNCE_LIMIT,
                };
                let mut cur_vel = e.topple_angular_velocity;
                if e.topple_angular_accumulation + cur_vel > TOPPLE_ANGULAR_LIMIT {
                    cur_vel = TOPPLE_ANGULAR_LIMIT - e.topple_angular_accumulation;
                }
                e.topple_lean_radians += cur_vel;
                e.topple_angular_accumulation += cur_vel;
                if e.topple_angular_accumulation >= TOPPLE_ANGULAR_LIMIT - 1e-6
                    && e.topple_angular_velocity > 0.0
                {
                    e.topple_angular_velocity *= -TOPPLE_BOUNCE_VELOCITY_PERCENT;
                    let no_bounce = (e.topple_options & TOPPLE_OPTIONS_NO_BOUNCE) != 0;
                    if no_bounce || e.topple_angular_velocity.abs() < TOPPLE_VELOCITY_BOUNCE_LIMIT {
                        e.topple_angular_velocity = 0.0;
                        e.topple_state = 2; // Down
                        if e.topple_kill_when_toppled {
                            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                                crate::game_logic::host_topple_kill_log::record(
                                    crate::game_logic::ObjectId(hid),
                                );
                            }
                        }
                    }
                } else {
                    e.topple_angular_velocity += e.topple_angular_acceleration;
                }
                changed = true;
            }
            // Wave 771: HeightDieUpdate residual (die when altitude reaches target).
            if e.height_die_active && !e.height_die_has_died {
                let hat = e.transform.position.y - e.ground_height;
                let contained = e.contained_by_host != 0;
                if contained {
                    e.height_die_last_height = hat;
                    changed = true;
                } else if frame < e.height_die_earliest_frame {
                    e.height_die_last_height = hat;
                    changed = true;
                } else {
                    let mut direction_ok = true;
                    if e.height_die_only_when_descending && hat >= e.height_die_last_height {
                        direction_ok = false;
                    }
                    e.height_die_last_height = hat;
                    if direction_ok && hat <= e.height_die_target_hat {
                        e.height_die_has_died = true;
                        e.height_die_active = false;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_height_die_kill_log::record(
                                crate::game_logic::ObjectId(hid),
                            );
                            // Wave 800: SCUD warhead detonation on HeightDie residual.
                            if e.scud_launcher_missile_projectile {
                                e.scud_launcher_missile_projectile = false;
                                let team = Self::entity_team_from_ordinal(e.team_ordinal);
                                let source = e.producer_id.map(crate::game_logic::ObjectId);
                                let pos = e.transform.position;
                                crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                                    crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                        id: crate::game_logic::ObjectId(hid),
                                        source,
                                        team,
                                        pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                        kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Scud {
                                            toxin: e.scud_launcher_missile_toxin,
                                        },
                                    },
                                );
                            }
                        }
                    }
                    changed = true;
                }
            }
            // Wave 772: JetSlowDeathBehavior residual (fixed-wing crash death).
            if e.jet_slow_death_active && !e.jet_slow_death_done {
                use crate::game_logic::host_jet_slow_death::{
                    JET_FINAL_BLOWUP_DELAY_FRAMES, JET_GRAVITY,
                };
                let hat = (e.transform.position.y - e.ground_height).max(0.0);
                let mut dy = 0.0_f32;
                let mut d_roll = 0.0_f32;
                let mut done = false;
                if e.jet_slow_death_started_on_ground {
                    if e.jet_slow_death_hit_ground_frame == 0 {
                        e.jet_slow_death_hit_ground_frame = frame;
                    }
                    if frame.saturating_sub(e.jet_slow_death_hit_ground_frame) >= 5 {
                        e.jet_slow_death_done = true;
                        e.jet_slow_death_active = false;
                        done = true;
                    }
                } else if !e.jet_slow_death_hit_ground {
                    d_roll = e.jet_slow_death_roll_rate;
                    e.jet_slow_death_roll_accum += d_roll;
                    e.jet_slow_death_roll_rate *= e.jet_slow_death_roll_rate_delta;
                    e.jet_slow_death_vertical_velocity +=
                        JET_GRAVITY * e.jet_slow_death_fall_how_fast;
                    dy = e.jet_slow_death_vertical_velocity;
                    if hat + dy <= 0.5 {
                        e.jet_slow_death_hit_ground = true;
                        e.jet_slow_death_hit_ground_frame = frame;
                        e.jet_slow_death_vertical_velocity = 0.0;
                        dy = -hat;
                    }
                } else if frame.saturating_sub(e.jet_slow_death_hit_ground_frame)
                    >= JET_FINAL_BLOWUP_DELAY_FRAMES
                {
                    e.jet_slow_death_done = true;
                    e.jet_slow_death_active = false;
                    done = true;
                }
                if dy.abs() > 0.0 || d_roll.abs() > 0.0 {
                    e.transform.position.y = (e.transform.position.y + dy).max(e.ground_height);
                    e.transform.orientation += d_roll;
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_jet_slow_death_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 773: HelicopterSlowDeathBehavior residual (spiral crash death).
            if e.heli_slow_death_active && !e.heli_slow_death_done {
                use crate::game_logic::host_helicopter_slow_death::{
                    HELI_BLADE_FLY_OFF_FRAMES, HELI_CRASH_GRAVITY, HELI_GROUND_SETTLE_FRAMES,
                    HELI_MAX_SELF_SPIN, HELI_MIN_SELF_SPIN, HELI_SELF_SPIN_UPDATE_AMOUNT,
                    HELI_SELF_SPIN_UPDATE_DELAY_FRAMES, HELI_SPIRAL_FORWARD_SPEED_DAMPING,
                    HELI_SPIRAL_TURN_RATE,
                };
                let hat = (e.transform.position.y - e.ground_height).max(0.0);
                if !e.heli_slow_death_blade_flew_off
                    && frame.saturating_sub(e.heli_slow_death_activate_frame)
                        >= HELI_BLADE_FLY_OFF_FRAMES
                {
                    e.heli_slow_death_blade_flew_off = true;
                }
                e.heli_slow_death_frames_since_spin_update =
                    e.heli_slow_death_frames_since_spin_update.saturating_add(1);
                if e.heli_slow_death_frames_since_spin_update >= HELI_SELF_SPIN_UPDATE_DELAY_FRAMES
                {
                    e.heli_slow_death_frames_since_spin_update = 0;
                    e.heli_slow_death_self_spin +=
                        e.heli_slow_death_self_spin_dir * HELI_SELF_SPIN_UPDATE_AMOUNT;
                    if e.heli_slow_death_self_spin >= HELI_MAX_SELF_SPIN {
                        e.heli_slow_death_self_spin = HELI_MAX_SELF_SPIN;
                        e.heli_slow_death_self_spin_dir = -1.0;
                    } else if e.heli_slow_death_self_spin <= HELI_MIN_SELF_SPIN {
                        e.heli_slow_death_self_spin = HELI_MIN_SELF_SPIN;
                        e.heli_slow_death_self_spin_dir = 1.0;
                    }
                }
                let mut dx = 0.0_f32;
                let mut dy = 0.0_f32;
                let mut dz = 0.0_f32;
                let mut d_orient = 0.0_f32;
                let mut done = false;
                if !e.heli_slow_death_hit_ground {
                    e.heli_slow_death_orbit_angle += HELI_SPIRAL_TURN_RATE;
                    d_orient = e.heli_slow_death_self_spin + HELI_SPIRAL_TURN_RATE;
                    e.heli_slow_death_orientation_delta += d_orient;
                    dx = e.heli_slow_death_orbit_angle.cos() * e.heli_slow_death_forward_speed;
                    dz = e.heli_slow_death_orbit_angle.sin() * e.heli_slow_death_forward_speed;
                    e.heli_slow_death_forward_speed *= HELI_SPIRAL_FORWARD_SPEED_DAMPING;
                    e.heli_slow_death_vertical_velocity += HELI_CRASH_GRAVITY;
                    dy = e.heli_slow_death_vertical_velocity;
                    if hat + dy <= 0.5 {
                        e.heli_slow_death_hit_ground = true;
                        e.heli_slow_death_hit_ground_frame = frame;
                        e.heli_slow_death_vertical_velocity = 0.0;
                        dy = -hat;
                    }
                } else if frame.saturating_sub(e.heli_slow_death_hit_ground_frame)
                    >= HELI_GROUND_SETTLE_FRAMES
                {
                    e.heli_slow_death_done = true;
                    e.heli_slow_death_active = false;
                    done = true;
                }
                if dx.abs() > 0.0 || dy.abs() > 0.0 || dz.abs() > 0.0 || d_orient.abs() > 0.0 {
                    e.transform.position.x += dx;
                    e.transform.position.y = (e.transform.position.y + dy).max(e.ground_height);
                    e.transform.position.z += dz;
                    e.transform.orientation += d_orient;
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_heli_slow_death_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 774: SlowDeathBehavior residual (sink delay + destroy).
            // phase: 0 Inactive, 1 WaitingToSink, 2 Sinking, 3 WaitingToDestroy, 4 Done
            if e.slow_death_phase != 0 && e.slow_death_phase != 4 {
                let mut done = false;
                match e.slow_death_phase {
                    1 => {
                        // WaitingToSink
                        if frame >= e.slow_death_sink_at_frame {
                            e.slow_death_phase = 2;
                        }
                        if frame >= e.slow_death_destroy_at_frame {
                            e.slow_death_phase = 4;
                            done = true;
                        }
                    }
                    2 => {
                        // Sinking
                        if e.slow_death_sink_rate_per_frame > 0.0 {
                            e.slow_death_sink_offset -= e.slow_death_sink_rate_per_frame;
                            if e.slow_death_sink_offset < e.slow_death_destruction_altitude {
                                e.slow_death_sink_offset = e.slow_death_destruction_altitude;
                            }
                        }
                        if frame >= e.slow_death_destroy_at_frame {
                            e.slow_death_phase = 4;
                            done = true;
                        }
                    }
                    3 => {
                        // WaitingToDestroy
                        if frame >= e.slow_death_destroy_at_frame {
                            e.slow_death_phase = 4;
                            done = true;
                        }
                    }
                    _ => {}
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_slow_death_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 775: StructureCollapseUpdate residual (building sink collapse).
            // state: 0 Standing, 1 WaitingForStart, 2 Collapsing, 3 Done
            if e.structure_collapse_state != 0 && e.structure_collapse_state != 3 {
                use crate::game_logic::host_structure_collapse::STRUCTURE_COLLAPSE_GRAVITY;
                let mut done = false;
                match e.structure_collapse_state {
                    1 => {
                        // WaitingForStart — shudder residual
                        if e.structure_collapse_max_shudder > 0.0 {
                            let t = frame as f32 * 0.37;
                            e.structure_collapse_shudder_x =
                                t.sin() * e.structure_collapse_max_shudder;
                            e.structure_collapse_shudder_z =
                                (t * 1.3).cos() * e.structure_collapse_max_shudder;
                        } else {
                            e.structure_collapse_shudder_x = 0.0;
                            e.structure_collapse_shudder_z = 0.0;
                        }
                        if frame >= e.structure_collapse_start_frame {
                            e.structure_collapse_state = 2;
                            e.structure_collapse_velocity = 0.0;
                        }
                    }
                    2 => {
                        // Collapsing
                        e.structure_collapse_current_height -= e.structure_collapse_velocity;
                        e.structure_collapse_velocity -=
                            STRUCTURE_COLLAPSE_GRAVITY * (1.0 - e.structure_collapse_damping);
                        if e.structure_collapse_max_shudder > 0.0 {
                            let t = frame as f32 * 0.37;
                            e.structure_collapse_shudder_x =
                                t.sin() * e.structure_collapse_max_shudder;
                            e.structure_collapse_shudder_z =
                                (t * 1.3).cos() * e.structure_collapse_max_shudder;
                        } else {
                            e.structure_collapse_shudder_x = 0.0;
                            e.structure_collapse_shudder_z = 0.0;
                        }
                        if e.structure_collapse_current_height
                            + e.structure_collapse_building_height
                            <= 0.0
                        {
                            e.structure_collapse_current_height =
                                -e.structure_collapse_building_height;
                            e.structure_collapse_shudder_x = 0.0;
                            e.structure_collapse_shudder_z = 0.0;
                            e.structure_collapse_state = 3;
                            done = true;
                        }
                    }
                    _ => {}
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_structure_collapse_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 776: StructureToppleUpdate residual (building fall death).
            // state: 0 Standing, 1 WaitingForStart, 2 Toppling, 3 WaitingForDone, 4 Done
            if e.structure_topple_state != 0 && e.structure_topple_state != 4 {
                use crate::game_logic::host_structure_topple::{
                    STRUCTURE_TOPPLE_ACCEL_FACTOR, STRUCTURE_TOPPLE_DONE_DELAY_FRAMES,
                    STRUCTURE_TOPPLE_INTEGRITY_DEFAULT,
                };
                let mut done = false;
                match e.structure_topple_state {
                    1 => {
                        if frame >= e.structure_topple_start_frame {
                            e.structure_topple_state = 2;
                            e.structure_topple_structural_integrity =
                                STRUCTURE_TOPPLE_INTEGRITY_DEFAULT;
                        }
                    }
                    2 => {
                        let integrity_term =
                            (1.0 - e.structure_topple_structural_integrity).max(0.0);
                        let topple_acceleration = STRUCTURE_TOPPLE_ACCEL_FACTOR
                            * e.structure_topple_accumulated_angle.sin()
                            * integrity_term;
                        let accel = if e.structure_topple_velocity <= 1e-6
                            && e.structure_topple_accumulated_angle <= 1e-6
                        {
                            STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.05
                        } else {
                            topple_acceleration.max(STRUCTURE_TOPPLE_ACCEL_FACTOR * 0.01)
                        };
                        e.structure_topple_velocity += accel;
                        if e.structure_topple_structural_integrity > 0.0 {
                            e.structure_topple_structural_integrity *=
                                e.structure_topple_structural_decay;
                            if e.structure_topple_structural_integrity < 0.0 {
                                e.structure_topple_structural_integrity = 0.0;
                            }
                        }
                        e.structure_topple_accumulated_angle += e.structure_topple_velocity;
                        e.structure_topple_lean_radians = e.structure_topple_accumulated_angle;
                        if e.structure_topple_accumulated_angle >= std::f32::consts::FRAC_PI_2 {
                            e.structure_topple_accumulated_angle = std::f32::consts::FRAC_PI_2;
                            e.structure_topple_lean_radians = e.structure_topple_accumulated_angle;
                            e.structure_topple_state = 3;
                            e.structure_topple_done_frame =
                                frame.saturating_add(STRUCTURE_TOPPLE_DONE_DELAY_FRAMES);
                        }
                    }
                    3 => {
                        if frame >= e.structure_topple_done_frame {
                            e.structure_topple_state = 4;
                            done = true;
                        }
                    }
                    _ => {}
                }
                // Wave 777: StructureTopple applyCrushingDamage residual samples.
                if matches!(e.structure_topple_state, 2 | 3 | 4) {
                    use crate::game_logic::host_structure_topple::{
                        HostStructureToppleData, HostStructureToppleState,
                    };
                    let mut st = HostStructureToppleData {
                        state: match e.structure_topple_state {
                            1 => HostStructureToppleState::WaitingForStart,
                            2 => HostStructureToppleState::Toppling,
                            3 => HostStructureToppleState::WaitingForDone,
                            4 => HostStructureToppleState::Done,
                            _ => HostStructureToppleState::Standing,
                        },
                        topple_start_frame: e.structure_topple_start_frame,
                        dir_x: e.structure_topple_dir_x,
                        dir_y: e.structure_topple_dir_y,
                        topple_velocity: e.structure_topple_velocity,
                        accumulated_angle: e.structure_topple_accumulated_angle,
                        structural_integrity: e.structure_topple_structural_integrity,
                        structural_decay: e.structure_topple_structural_decay,
                        done_frame: e.structure_topple_done_frame,
                        lean_radians: e.structure_topple_lean_radians,
                        last_crushed_location: e.structure_topple_last_crushed_location,
                        building_height: e.structure_topple_building_height,
                        facing_width: e.structure_topple_facing_width,
                    };
                    let samples =
                        st.take_crush_sweep_samples(e.transform.position.x, e.transform.position.z);
                    e.structure_topple_last_crushed_location = st.last_crushed_location;
                    if !samples.is_empty() {
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_structure_topple_crush_log::record(
                                crate::game_logic::ObjectId(hid),
                                samples,
                            );
                        }
                    }
                }
                if done {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_structure_topple_kill_log::record(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 778: FireWeaponWhenDamagedBehavior continuous residual.
            if e.fwwd_active {
                use crate::game_logic::host_enum_table_residual::{
                    host_calc_body_damage_state, HostBodyDamageType,
                };
                let reload = e.fwwd_continuous_reload_frames.max(1);
                let ready = e.fwwd_last_continuous_frame == 0
                    || frame.saturating_sub(e.fwwd_last_continuous_frame) >= reload;
                if ready {
                    let max_h = e.max_health.max(e.health).max(1.0);
                    let state = host_calc_body_damage_state(e.health, max_h);
                    let name = match state {
                        HostBodyDamageType::Pristine => e.fwwd_continuous_pristine.as_str(),
                        HostBodyDamageType::Damaged => e.fwwd_continuous_damaged.as_str(),
                        HostBodyDamageType::ReallyDamaged => {
                            e.fwwd_continuous_really_damaged.as_str()
                        }
                        HostBodyDamageType::Rubble => e.fwwd_continuous_rubble.as_str(),
                    };
                    // Continuous weapons typically only for damaged+ unless pristine set.
                    let skip_pristine = matches!(state, HostBodyDamageType::Pristine)
                        && e.fwwd_continuous_pristine.is_empty();
                    if !skip_pristine && !name.is_empty() {
                        e.fwwd_last_continuous_frame = frame;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_fwwd_continuous_log::record(
                                crate::game_logic::ObjectId(hid),
                                name.to_string(),
                            );
                        }
                        changed = true;
                    }
                }
            }
            // Wave 780: BaseRegenerateUpdate residual (structure auto-heal).
            if e.base_regen_active && !e.base_regen_done_sold {
                use crate::game_logic::host_base_regenerate::{
                    base_regen_heal_amount, BASE_REGEN_HEAL_RATE_FRAMES,
                };
                if e.base_regen_pending_damage {
                    // C++ onDamage non-healing: delay wake.
                    use crate::game_logic::host_base_regenerate::BASE_REGEN_DELAY_FRAMES;
                    e.base_regen_wake_frame = frame.saturating_add(BASE_REGEN_DELAY_FRAMES);
                    e.base_regen_pending_damage = false;
                    changed = true;
                }
                if e.sold {
                    e.base_regen_done_sold = true;
                    changed = true;
                } else if !e.under_construction {
                    let max_h = e.max_health.max(e.health).max(1.0);
                    if e.health + f32::EPSILON < max_h && frame >= e.base_regen_wake_frame {
                        let elapsed = frame.saturating_sub(e.base_regen_wake_frame);
                        if elapsed % BASE_REGEN_HEAL_RATE_FRAMES == 0 {
                            let amount = base_regen_heal_amount(max_h);
                            if amount > 0.0 {
                                e.health = (e.health + amount).min(max_h);
                                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                                    crate::game_logic::host_heal_log::record(
                                        crate::game_logic::ObjectId(hid),
                                        e.health,
                                    );
                                }
                                changed = true;
                            }
                        }
                    }
                }
            }
            // Wave 781: EnemyNearUpdate residual (MODELCONDITION_ENEMYNEAR).
            if e.enemy_near_active {
                let was = e.enemy_near;
                if e.enemy_near_scan_delay == 0 {
                    let delay_time = e.enemy_near_scan_delay_time.max(1);
                    let sx = e.transform.position.x;
                    let sz = e.transform.position.z;
                    let vision = e.enemy_near_vision_range.max(e.vision_range);
                    let my_team = e.team_ordinal;
                    // Drop entity borrow before scan (scan needs world()).
                    {
                        #[allow(dropping_references)]
                        drop(e);
                    }
                    let present = self.scan_enemy_near_present(eid, sx, sz, vision, my_team);
                    if let Some(e2) = self.world.world_mut().entity_mut(eid) {
                        e2.enemy_near_scan_delay = delay_time;
                        e2.enemy_near = present;
                        if present && !was {
                            e2.enemy_near_model = true;
                        } else if !present && was {
                            e2.enemy_near_model = false;
                        }
                    }
                    n += 1;
                    continue;
                } else {
                    e.enemy_near_scan_delay = e.enemy_near_scan_delay.saturating_sub(1);
                    changed = true;
                }
            }
            // Wave 782: ProneUpdate residual (infantry cower countdown).
            if e.prone_active && e.prone_frames > 0 {
                e.prone_frames -= 1;
                if e.prone_frames == 0 {
                    e.prone_model = false;
                    e.prone_no_attack = false;
                    if let Some(bit) =
                        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                            "PRONE",
                        )
                    {
                        e.model_condition_bits &= !(1u128 << bit);
                    }
                } else {
                    e.prone_model = true;
                    e.prone_no_attack = true;
                    if let Some(bit) =
                        crate::game_logic::host_enum_table_residual::model_condition_bit_name_index(
                            "PRONE",
                        )
                    {
                        e.model_condition_bits |= 1u128 << bit;
                    }
                }
                changed = true;
            }
            // Wave 783: FloatUpdate residual (boat sway / optional water snap).
            if e.float_update_active {
                use crate::game_logic::host_float_update::{
                    FLOAT_PITCH_PHASE, FLOAT_SWAY_AMP, FLOAT_YAW_PHASE,
                };
                let angle = frame as f32;
                e.float_yaw = (angle * FLOAT_YAW_PHASE).sin() * FLOAT_SWAY_AMP;
                e.float_pitch = (angle * FLOAT_PITCH_PHASE).sin() * FLOAT_SWAY_AMP;
                // Residual water snap: when enabled, stick Y to terrain ground height.
                if e.float_update_enabled {
                    e.transform.position.y = e.ground_height;
                }
                changed = true;
            }
            // Wave 784: AnimationSteeringUpdate residual (turn anim conditions).
            if e.anim_steer_active {
                use crate::game_logic::host_animation_steering::HostAnimSteerTurnAnim;
                use crate::game_logic::object::PhysicsTurningType;
                if frame >= e.anim_steer_next_transition_frame {
                    let turning = match e.physics_turning_ordinal {
                        -1 => PhysicsTurningType::TurnNegative,
                        1 => PhysicsTurningType::TurnPositive,
                        _ => PhysicsTurningType::TurnNone,
                    };
                    let cur = match e.anim_steer_turn {
                        1 => HostAnimSteerTurnAnim::CenterToRight,
                        2 => HostAnimSteerTurnAnim::CenterToLeft,
                        3 => HostAnimSteerTurnAnim::LeftToCenter,
                        4 => HostAnimSteerTurnAnim::RightToCenter,
                        _ => HostAnimSteerTurnAnim::Invalid,
                    };
                    let mut next = cur;
                    let mut changed_anim: Option<&'static str> = None;
                    let tf = e.anim_steer_transition_frames.max(1);
                    match cur {
                        HostAnimSteerTurnAnim::Invalid => {
                            if turning == PhysicsTurningType::TurnNegative {
                                next = HostAnimSteerTurnAnim::CenterToRight;
                                e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                                changed_anim = Some("CENTER_TO_RIGHT");
                            } else if turning == PhysicsTurningType::TurnPositive {
                                next = HostAnimSteerTurnAnim::CenterToLeft;
                                e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                                changed_anim = Some("CENTER_TO_LEFT");
                            }
                        }
                        HostAnimSteerTurnAnim::CenterToRight => {
                            if turning != PhysicsTurningType::TurnNegative {
                                next = HostAnimSteerTurnAnim::RightToCenter;
                                e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                                changed_anim = Some("RIGHT_TO_CENTER");
                            }
                        }
                        HostAnimSteerTurnAnim::CenterToLeft => {
                            if turning != PhysicsTurningType::TurnPositive {
                                next = HostAnimSteerTurnAnim::LeftToCenter;
                                e.anim_steer_next_transition_frame = frame.saturating_add(tf);
                                changed_anim = Some("LEFT_TO_CENTER");
                            }
                        }
                        HostAnimSteerTurnAnim::LeftToCenter
                        | HostAnimSteerTurnAnim::RightToCenter => {
                            if turning == PhysicsTurningType::TurnNone {
                                next = HostAnimSteerTurnAnim::Invalid;
                                e.anim_steer_next_transition_frame = frame;
                                e.anim_steer_has_condition = false;
                            }
                        }
                    }
                    e.anim_steer_turn = next as u8;
                    if let Some(_) = changed_anim {
                        e.anim_steer_has_condition = true;
                    }
                    changed = true;
                }
            }
            // Wave 785: RadiusDecalUpdate residual (SW delivery decal throb/kill).
            if e.radius_decal_awake {
                if e.radius_decal_kill_when_idle && !e.attacking {
                    e.radius_decal_empty = true;
                    e.radius_decal_awake = false;
                    e.radius_decal_kill_when_idle = false;
                    e.radius_decal_opacity = 0.0;
                    changed = true;
                } else if !e.radius_decal_empty {
                    let period = e.radius_decal_throb_frames.max(1);
                    let phase = frame.saturating_sub(e.radius_decal_birth_frame) % (period * 2);
                    let t = if phase <= period {
                        phase as f32 / period as f32
                    } else {
                        2.0 - (phase as f32 / period as f32)
                    };
                    e.radius_decal_opacity = e.radius_decal_opacity_min
                        + (e.radius_decal_opacity_max - e.radius_decal_opacity_min) * t;
                    changed = true;
                }
            }
            // Wave 786: CheckpointUpdate residual (gate open/close + path radius).
            if e.checkpoint_active {
                use crate::game_logic::host_checkpoint_update::CHECKPOINT_RADIUS_STEP;
                if e.checkpoint_scan_delay == 0 {
                    let delay_time = e.checkpoint_scan_delay_time.max(1);
                    let sx = e.transform.position.x;
                    let sz = e.transform.position.z;
                    let vision = e.checkpoint_vision_range.max(e.vision_range);
                    let my_team = e.team_ordinal;
                    // Drop entity borrow before scan (scan needs world()).
                    {
                        #[allow(dropping_references)]
                        drop(e);
                    }
                    let (enemy, ally) = self.scan_checkpoint_near(eid, sx, sz, vision, my_team);
                    if let Some(e2) = self.world.world_mut().entity_mut(eid) {
                        e2.checkpoint_scan_delay = delay_time;
                        let change =
                            e2.checkpoint_enemy_near != enemy || e2.checkpoint_ally_near != ally;
                        e2.checkpoint_enemy_near = enemy;
                        e2.checkpoint_ally_near = ally;
                        let open = ally && !enemy;
                        if change {
                            e2.checkpoint_open = open;
                            e2.checkpoint_door_anim = if open { 1 } else { 2 };
                        }
                        if e2.checkpoint_open {
                            if e2.checkpoint_path_radius > 0.0 {
                                e2.checkpoint_path_radius =
                                    (e2.checkpoint_path_radius - CHECKPOINT_RADIUS_STEP).max(0.0);
                            }
                        } else if e2.checkpoint_path_radius < e2.checkpoint_max_minor_radius {
                            e2.checkpoint_path_radius = (e2.checkpoint_path_radius
                                + CHECKPOINT_RADIUS_STEP)
                                .min(e2.checkpoint_max_minor_radius);
                        }
                    }
                    n += 1;
                    continue;
                } else {
                    e.checkpoint_scan_delay = e.checkpoint_scan_delay.saturating_sub(1);
                    if e.checkpoint_open {
                        if e.checkpoint_path_radius > 0.0 {
                            e.checkpoint_path_radius =
                                (e.checkpoint_path_radius - CHECKPOINT_RADIUS_STEP).max(0.0);
                        }
                    } else if e.checkpoint_path_radius < e.checkpoint_max_minor_radius {
                        e.checkpoint_path_radius = (e.checkpoint_path_radius
                            + CHECKPOINT_RADIUS_STEP)
                            .min(e.checkpoint_max_minor_radius);
                    }
                    changed = true;
                }
            }
            // Wave 787: SmartBombTargetHomingUpdate residual (course fudge).
            if e.smart_bomb_homing_active && e.smart_bomb_target_received {
                use crate::game_logic::host_smart_bomb_target_homing::SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN;
                let hat = (e.transform.position.y - e.ground_height).max(0.0);
                if hat >= SMART_BOMB_SIGNIFICANTLY_ABOVE_TERRAIN {
                    let status = e.smart_bomb_course_scalar.clamp(0.0, 1.0);
                    let target_c = 1.0 - status;
                    e.transform.position.x =
                        e.smart_bomb_target_x * target_c + e.transform.position.x * status;
                    e.transform.position.z =
                        e.smart_bomb_target_z * target_c + e.transform.position.z * status;
                    // Y altitude unchanged.
                    changed = true;
                }
            }
            // Wave 788: DaisyCutter/MOAB DeliverPayload flight residual.
            if e.daisy_transport_active {
                use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
                let tier = if e.daisy_transport_tier == 1 {
                    DaisyFlightPayloadTier::Moab
                } else {
                    DaisyFlightPayloadTier::DaisyCutter
                };
                let dest_x = e.daisy_transport_target_x;
                let dest_z = e.daisy_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 20.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(150.0);
                let mut vel = glam::Vec3::ZERO;
                let over = if dist < 5.0 {
                    true
                } else {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                    dist <= tier.delivery_distance() * 0.5
                };
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                // stash velocity residual on bomb vel field unused for transport
                if over {
                    e.daisy_transport_active = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e
                            .producer_id
                            .map(crate::game_logic::ObjectId)
                            .unwrap_or(crate::game_logic::ObjectId(hid));
                        crate::game_logic::host_daisy_cutter_drop_log::record_drop(
                            crate::game_logic::host_daisy_cutter_drop_log::DaisyDropEvent {
                                team,
                                target: glam::Vec3::new(
                                    e.daisy_transport_target_x,
                                    e.daisy_transport_target_y,
                                    e.daisy_transport_target_z,
                                ),
                                producer,
                                tier,
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.daisy_cutter_bomb {
                use crate::game_logic::host_daisy_cutter_flight::DaisyFlightPayloadTier;
                if e.daisy_bomb_vel_y == 0.0 {
                    e.daisy_bomb_vel_y = -16.0;
                }
                e.transform.position.y += e.daisy_bomb_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.daisy_cutter_bomb = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        let tier = if e.daisy_bomb_is_moab {
                            DaisyFlightPayloadTier::Moab
                        } else {
                            DaisyFlightPayloadTier::DaisyCutter
                        };
                        crate::game_logic::host_daisy_cutter_drop_log::record_detonate(
                            crate::game_logic::host_daisy_cutter_drop_log::DaisyDetonateEvent {
                                bomb: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                                tier,
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 789: AnthraxBomb DeliverPayload flight residual.
            if e.anthrax_transport_active {
                use crate::game_logic::host_anthrax_bomb_flight::{
                    AnthraxBombPayloadTier, ANTHRAX_DELIVERY_DISTANCE,
                };
                let tier = if e.anthrax_transport_tier == 1 {
                    AnthraxBombPayloadTier::Gamma
                } else {
                    AnthraxBombPayloadTier::Base
                };
                let dest_x = e.anthrax_transport_target_x;
                let dest_z = e.anthrax_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 18.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(150.0);
                let mut vel = glam::Vec3::ZERO;
                let over = if dist < 5.0 {
                    true
                } else {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                    dist <= ANTHRAX_DELIVERY_DISTANCE * 0.5
                };
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                if over {
                    e.anthrax_transport_active = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e
                            .producer_id
                            .map(crate::game_logic::ObjectId)
                            .unwrap_or(crate::game_logic::ObjectId(hid));
                        crate::game_logic::host_anthrax_bomb_drop_log::record_drop(
                            crate::game_logic::host_anthrax_bomb_drop_log::AnthraxDropEvent {
                                team,
                                target: glam::Vec3::new(
                                    e.anthrax_transport_target_x,
                                    e.anthrax_transport_target_y,
                                    e.anthrax_transport_target_z,
                                ),
                                producer,
                                tier,
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.anthrax_bomb_payload {
                if e.anthrax_bomb_vel_y == 0.0 {
                    e.anthrax_bomb_vel_y = -14.0;
                }
                e.transform.position.y += e.anthrax_bomb_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.anthrax_bomb_payload = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_anthrax_bomb_drop_log::record_detonate(
                            crate::game_logic::host_anthrax_bomb_drop_log::AnthraxDetonateEvent {
                                bomb: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 790: ClusterMines DeliverPayload flight residual.
            if e.cluster_mines_transport_active {
                use crate::game_logic::host_mines::CLUSTER_MINES_DELIVERY_DISTANCE;
                let dest_x = e.cluster_mines_transport_target_x;
                let dest_z = e.cluster_mines_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 18.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(150.0);
                let mut vel = glam::Vec3::ZERO;
                let over = if dist < 5.0 {
                    true
                } else {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                    dist <= CLUSTER_MINES_DELIVERY_DISTANCE * 0.5
                };
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                if over {
                    e.cluster_mines_transport_active = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e
                            .producer_id
                            .map(crate::game_logic::ObjectId)
                            .unwrap_or(crate::game_logic::ObjectId(hid));
                        crate::game_logic::host_cluster_mines_drop_log::record_drop(
                            crate::game_logic::host_cluster_mines_drop_log::ClusterMinesDropEvent {
                                team,
                                target: glam::Vec3::new(
                                    e.cluster_mines_transport_target_x,
                                    e.cluster_mines_transport_target_y,
                                    e.cluster_mines_transport_target_z,
                                ),
                                producer,
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.cluster_mines_bomb {
                if e.cluster_mines_bomb_vel_y == 0.0 {
                    e.cluster_mines_bomb_vel_y = -14.0;
                }
                e.transform.position.y += e.cluster_mines_bomb_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.cluster_mines_bomb = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_cluster_mines_drop_log::record_detonate(
                            crate::game_logic::host_cluster_mines_drop_log::ClusterMinesDetonateEvent {
                                bomb: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 791: EMP Pulse DeliverPayload + spheroid residual.
            if e.emp_pulse_transport_active {
                use crate::game_logic::host_emp_pulse::EMP_PULSE_DELIVERY_DISTANCE;
                let dest_x = e.emp_pulse_transport_target_x;
                let dest_z = e.emp_pulse_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 18.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(150.0);
                let mut vel = glam::Vec3::ZERO;
                let over = if dist < 5.0 {
                    true
                } else {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                    dist <= EMP_PULSE_DELIVERY_DISTANCE * 0.5
                };
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                if over {
                    e.emp_pulse_transport_active = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e
                            .producer_id
                            .map(crate::game_logic::ObjectId)
                            .unwrap_or(crate::game_logic::ObjectId(hid));
                        crate::game_logic::host_emp_pulse_drop_log::record_drop(
                            crate::game_logic::host_emp_pulse_drop_log::EmpPulseDropEvent {
                                team,
                                target: glam::Vec3::new(
                                    e.emp_pulse_transport_target_x,
                                    e.emp_pulse_transport_target_y,
                                    e.emp_pulse_transport_target_z,
                                ),
                                producer,
                                player_id: e.emp_pulse_transport_player_id,
                                caster_id: e.emp_pulse_transport_caster_id,
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.emp_pulse_bomb {
                if e.emp_pulse_bomb_vel_y == 0.0 {
                    e.emp_pulse_bomb_vel_y = -14.0;
                }
                e.transform.position.y += e.emp_pulse_bomb_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.emp_pulse_bomb = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_emp_pulse_drop_log::record_detonate(
                            crate::game_logic::host_emp_pulse_drop_log::EmpPulseDetonateEvent {
                                bomb: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.emp_pulse_spheroid
                && e.emp_pulse_spheroid_expires_frame > 0
                && frame >= e.emp_pulse_spheroid_expires_frame
            {
                e.emp_pulse_spheroid = false;
                e.emp_pulse_spheroid_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_emp_pulse_drop_log::record_spheroid_expire(
                        crate::game_logic::host_emp_pulse_drop_log::EmpPulseSpheroidExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                        },
                    );
                }
                changed = true;
            }
            // Wave 792: A10 Thunderbolt transport residual.
            if e.a10_strike_transport_active {
                let dest_x = e.a10_strike_transport_target_x;
                let dest_z = e.a10_strike_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 22.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(140.0);
                let mut vel = glam::Vec3::ZERO;
                if dist >= 5.0 {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                }
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                changed = true;
            }
            if e.a10_strike_missile {
                if e.a10_strike_missile_vel_y == 0.0 {
                    e.a10_strike_missile_vel_y = -20.0;
                }
                e.transform.position.y += e.a10_strike_missile_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.a10_strike_missile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_a10_strike_drop_log::record_detonate(
                            crate::game_logic::host_a10_strike_drop_log::A10DetonateEvent {
                                missile: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 793: ArtilleryBarrage transport residual.
            if e.artillery_barrage_transport_active {
                use crate::game_logic::special_power_strikes::ARTILLERY_BARRAGE_PREFERRED_HEIGHT;
                let dest_x = e.artillery_barrage_transport_target_x;
                let dest_z = e.artillery_barrage_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 14.0_f32;
                let mut new_pos = pos;
                new_pos.y = ARTILLERY_BARRAGE_PREFERRED_HEIGHT.max(120.0);
                let mut vel = glam::Vec3::ZERO;
                if dist >= 5.0 {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                }
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                changed = true;
            }
            if e.artillery_barrage_shell {
                if e.artillery_barrage_shell_vel_y == 0.0 {
                    e.artillery_barrage_shell_vel_y = -18.0;
                }
                e.transform.position.y += e.artillery_barrage_shell_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.artillery_barrage_shell = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_artillery_barrage_drop_log::record_detonate(
                            crate::game_logic::host_artillery_barrage_drop_log::ArtilleryDetonateEvent {
                                shell: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 794: CarpetBomb transport residual.
            if e.carpet_bomb_transport_active {
                let dest_x = e.carpet_bomb_transport_target_x;
                let dest_z = e.carpet_bomb_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 18.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(120.0);
                let mut vel = glam::Vec3::ZERO;
                if dist >= 5.0 {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    vel = glam::Vec3::new(new_pos.x - pos.x, new_pos.y - pos.y, new_pos.z - pos.z);
                }
                e.transform.position = new_pos;
                if vel.length_squared() > 1e-6 {
                    e.transform.orientation = vel.z.atan2(vel.x);
                }
                changed = true;
            }
            if e.carpet_bomb_payload {
                if e.carpet_bomb_payload_vel_y == 0.0 {
                    e.carpet_bomb_payload_vel_y = -15.0;
                }
                e.transform.position.y += e.carpet_bomb_payload_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.carpet_bomb_payload = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_carpet_bomb_drop_log::record_detonate(
                            crate::game_logic::host_carpet_bomb_drop_log::CarpetBombDetonateEvent {
                                bomb: crate::game_logic::ObjectId(hid),
                                producer,
                                team,
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    0.0,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 795: Leaflet B52 DeliverPayload residual.
            if e.leaflet_transport_active {
                use crate::game_logic::host_leaflet_drop::LEAFLET_DELIVERY_DISTANCE;
                let dest_x = e.leaflet_transport_target_x;
                let dest_z = e.leaflet_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 20.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(140.0);
                if dist > 1.0 {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    e.transform.position = new_pos;
                    e.transform.orientation = dz.atan2(dx);
                }
                if dist <= LEAFLET_DELIVERY_DISTANCE * 0.5 {
                    e.leaflet_transport_active = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e
                            .producer_id
                            .map(crate::game_logic::ObjectId)
                            .unwrap_or(crate::game_logic::ObjectId(hid));
                        crate::game_logic::host_leaflet_b52_drop_log::record_drop(
                            crate::game_logic::host_leaflet_b52_drop_log::LeafletB52DropEvent {
                                team,
                                target: glam::Vec3::new(
                                    e.leaflet_transport_target_x,
                                    e.leaflet_transport_target_y,
                                    e.leaflet_transport_target_z,
                                ),
                                producer,
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.leaflet_container {
                if e.leaflet_container_vel_y == 0.0 {
                    e.leaflet_container_vel_y = -12.0;
                }
                e.transform.position.y += e.leaflet_container_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.leaflet_container = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_leaflet_b52_drop_log::record_ground(
                            crate::game_logic::host_leaflet_b52_drop_log::LeafletContainerGroundEvent {
                                id: crate::game_logic::ObjectId(hid),
                                pos: glam::Vec3::new(
                                    e.transform.position.x,
                                    e.transform.position.y,
                                    e.transform.position.z,
                                ),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 796: Paradrop cargo-plane residual.
            if e.paradrop_transport_active {
                use crate::game_logic::host_paradrop::PARADROP_DELIVERY_DISTANCE;
                let dest_x = e.paradrop_transport_target_x;
                let dest_z = e.paradrop_transport_target_z;
                let pos = e.transform.position;
                let dx = dest_x - pos.x;
                let dz = dest_z - pos.z;
                let dist = (dx * dx + dz * dz).sqrt();
                let speed = 18.0_f32;
                let mut new_pos = pos;
                new_pos.y = new_pos.y.max(150.0);
                if dist > 1.0 {
                    let step = speed.min(dist);
                    new_pos.x += dx / dist * step;
                    new_pos.z += dz / dist * step;
                    e.transform.position = new_pos;
                    e.transform.orientation = dz.atan2(dx);
                }
                if dist <= PARADROP_DELIVERY_DISTANCE {
                    e.paradrop_transport_active = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let producer = e
                            .producer_id
                            .map(crate::game_logic::ObjectId)
                            .unwrap_or(crate::game_logic::ObjectId(hid));
                        crate::game_logic::host_paradrop_cargo_drop_log::record_drop(
                            crate::game_logic::host_paradrop_cargo_drop_log::ParadropCargoDropEvent {
                                team,
                                target: glam::Vec3::new(
                                    e.paradrop_transport_target_x,
                                    e.paradrop_transport_target_y,
                                    e.paradrop_transport_target_z,
                                ),
                                producer,
                            },
                        );
                    }
                }
                changed = true;
            }
            if e.paradrop_parachute {
                if e.paradrop_parachute_vel_y == 0.0 {
                    e.paradrop_parachute_vel_y = -8.0;
                }
                if e.paradrop_parachute_vel_y < -2.0 {
                    e.paradrop_parachute_vel_y = -2.5;
                }
                e.transform.position.y += e.paradrop_parachute_vel_y;
                if e.transform.position.y <= 5.0 {
                    e.paradrop_parachute = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_paradrop_cargo_drop_log::record_ground(
                            crate::game_logic::host_paradrop_cargo_drop_log::ParadropParachuteGroundEvent {
                                id: crate::game_logic::ObjectId(hid),
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 797: AuroraBomb projectile dive residual.
            if e.aurora_bomb_projectile {
                use crate::game_logic::host_aurora_bomb::{
                    AURORA_BOMB_LOCO_MIN_SPEED, AURORA_BOMB_LOCO_SPEED,
                };
                if e.aurora_bomb_mission_id > 0 && !e.aurora_bomb_mission_live {
                    e.aurora_bomb_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_aurora_bomb_projectile_log::record_destroy(
                            crate::game_logic::host_aurora_bomb_projectile_log::AuroraBombProjectileDestroyEvent {
                                id: crate::game_logic::ObjectId(hid),
                                snap_aim: None,
                            },
                        );
                    }
                    changed = true;
                } else {
                    let speed = AURORA_BOMB_LOCO_SPEED / 30.0;
                    let min_speed = AURORA_BOMB_LOCO_MIN_SPEED / 30.0;
                    let pos = e.transform.position;
                    let (aim_x, aim_y, aim_z) = if e.aurora_bomb_has_aim {
                        (
                            e.aurora_bomb_aim_x,
                            e.aurora_bomb_aim_y,
                            e.aurora_bomb_aim_z,
                        )
                    } else {
                        (pos.x, pos.y, pos.z)
                    };
                    let dx = aim_x - pos.x;
                    let dy = aim_y - pos.y;
                    let dz = aim_z - pos.z;
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    let (mut vx, mut vy, mut vz) = if dist > 0.001 {
                        let s = speed.max(min_speed);
                        let mut vx = dx / dist * s;
                        let mut vy = dy / dist * s;
                        let mut vz = dz / dist * s;
                        if pos.y > aim_y + 10.0 {
                            vy = vy.min(-min_speed * 0.5);
                        }
                        (vx, vy, vz)
                    } else {
                        (0.0, -speed, 0.0)
                    };
                    let new_x = pos.x + vx;
                    let new_y = pos.y + vy;
                    let new_z = pos.z + vz;
                    e.transform.position.x = new_x;
                    e.transform.position.y = new_y;
                    e.transform.position.z = new_z;
                    if vx * vx + vz * vz > 1e-6 {
                        e.transform.orientation = vz.atan2(vx);
                    }
                    let near = (aim_x - new_x) * (aim_x - new_x)
                        + (aim_z - new_z) * (aim_z - new_z)
                        < 8.0 * 8.0
                        && new_y <= aim_y + 12.0;
                    if near {
                        e.aurora_bomb_projectile = false;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_aurora_bomb_projectile_log::record_destroy(
                                crate::game_logic::host_aurora_bomb_projectile_log::AuroraBombProjectileDestroyEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    snap_aim: if e.aurora_bomb_has_aim {
                                        Some([
                                            e.aurora_bomb_aim_x,
                                            e.aurora_bomb_aim_y,
                                            e.aurora_bomb_aim_z,
                                        ])
                                    } else {
                                        None
                                    },
                                },
                            );
                        }
                    }
                    changed = true;
                }
            }

            // Wave 798: ToxinStream projectile residual.
            if e.toxin_stream_projectile {
                use crate::game_logic::host_toxin_tractor::{
                    toxin_stream_missile_step_speed, TOXIN_STREAM_MISSILE_TURN_DISTANCE,
                };
                let pos = e.transform.position;
                let (aim_x, aim_y, aim_z) = if e.toxin_stream_has_aim {
                    (
                        e.toxin_stream_aim_x,
                        e.toxin_stream_aim_y,
                        e.toxin_stream_aim_z,
                    )
                } else {
                    (pos.x, pos.y, pos.z)
                };
                let fuel_done =
                    e.toxin_stream_has_fuel && e.toxin_stream_fuel_expires_frame <= frame;
                let ignited = if e.toxin_stream_has_ignition {
                    e.toxin_stream_ignition_frame <= frame
                } else {
                    true
                };
                let can_steer = e.toxin_stream_travelled >= TOXIN_STREAM_MISSILE_TURN_DISTANCE;
                let speed = toxin_stream_missile_step_speed(ignited && can_steer);
                let dx = aim_x - pos.x;
                let dy = aim_y - pos.y;
                let dz = aim_z - pos.z;
                let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                let step_speed = if dist > 0.001 { speed.min(dist) } else { speed };
                let (vx, vy, vz) = if dist > 0.001 {
                    (
                        dx / dist * step_speed,
                        dy / dist * step_speed,
                        dz / dist * step_speed,
                    )
                } else {
                    (0.0, -step_speed, 0.0)
                };
                let step = (vx * vx + vy * vy + vz * vz).sqrt().max(step_speed);
                let new_x = pos.x + vx;
                let new_y = pos.y + vy;
                let new_z = pos.z + vz;
                e.transform.position.x = new_x;
                e.transform.position.y = new_y;
                e.transform.position.z = new_z;
                e.toxin_stream_travelled += step;
                e.toxin_stream_has_aim = true;
                e.toxin_stream_aim_x = aim_x;
                e.toxin_stream_aim_y = aim_y;
                e.toxin_stream_aim_z = aim_z;
                if vx * vx + vz * vz > 1e-6 {
                    e.transform.orientation = vz.atan2(vx);
                }
                if e.toxin_stream_has_shooter {
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let intended = if e.toxin_stream_has_intended {
                            Some(crate::game_logic::ObjectId(e.toxin_stream_intended))
                        } else {
                            None
                        };
                        crate::game_logic::host_toxin_stream_projectile_log::record_stream(
                            crate::game_logic::host_toxin_stream_projectile_log::ToxinStreamPointEvent {
                                shooter: crate::game_logic::ObjectId(e.toxin_stream_shooter),
                                pos: glam::Vec3::new(new_x, new_y, new_z),
                                intended,
                                aim: glam::Vec3::new(aim_x, aim_y, aim_z),
                            },
                        );
                        let _ = hid;
                    }
                }
                let near = dist <= speed + 0.001
                    || (aim_x - new_x) * (aim_x - new_x)
                        + (aim_y - new_y) * (aim_y - new_y)
                        + (aim_z - new_z) * (aim_z - new_z)
                        < 6.0 * 6.0;
                if fuel_done || near {
                    e.toxin_stream_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        let intended = if e.toxin_stream_has_intended {
                            Some(crate::game_logic::ObjectId(e.toxin_stream_intended))
                        } else {
                            None
                        };
                        let impact_pos = if near {
                            glam::Vec3::new(aim_x, aim_y, aim_z)
                        } else {
                            glam::Vec3::new(new_x, new_y, new_z)
                        };
                        crate::game_logic::host_toxin_stream_projectile_log::record_impact(
                            crate::game_logic::host_toxin_stream_projectile_log::ToxinStreamImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: impact_pos,
                                team,
                            },
                        );
                    }
                }
                changed = true;
            }

            // Wave 799: AngryMob projectile residual.
            if e.angry_mob_projectile {
                use crate::game_logic::host_angry_mob::{
                    angry_mob_projectile_bezier_point, AngryMobProjectileKind,
                };
                let pos = e.transform.position;
                let from = if e.angry_mob_projectile_has_from {
                    glam::Vec3::new(
                        e.angry_mob_projectile_from_x,
                        e.angry_mob_projectile_from_y,
                        e.angry_mob_projectile_from_z,
                    )
                } else {
                    glam::Vec3::new(pos.x, pos.y, pos.z)
                };
                let aim = if e.angry_mob_projectile_has_aim {
                    glam::Vec3::new(
                        e.angry_mob_projectile_aim_x,
                        e.angry_mob_projectile_aim_y,
                        e.angry_mob_projectile_aim_z,
                    )
                } else {
                    from
                };
                let launch = e.angry_mob_projectile_launch_frame;
                let flight = e.angry_mob_projectile_flight_frames.max(1);
                let kind = AngryMobProjectileKind::from_u8(e.angry_mob_projectile_kind);
                let elapsed = frame.saturating_sub(launch);
                let t = (elapsed as f32 / flight as f32).clamp(0.0, 1.0);
                let new_pos = angry_mob_projectile_bezier_point(from, aim, t, kind);
                e.transform.position.x = new_pos.x;
                e.transform.position.y = new_pos.y;
                e.transform.position.z = new_pos.z;
                if elapsed >= flight {
                    e.angry_mob_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        let intended = if e.angry_mob_projectile_has_intended {
                            Some(crate::game_logic::ObjectId(e.angry_mob_projectile_intended))
                        } else {
                            None
                        };
                        crate::game_logic::host_angry_mob_projectile_log::record_impact(
                            crate::game_logic::host_angry_mob_projectile_log::AngryMobProjectileImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                                kind: e.angry_mob_projectile_kind,
                            },
                        );
                    }
                }
                changed = true;
            }

            // Wave 800: SCUD launcher missile residual.
            if e.scud_launcher_missile_projectile {
                use crate::game_logic::host_scud_launcher::{
                    SCUD_MISSILE_DIVE_DISTANCE, SCUD_MISSILE_INITIAL_VELOCITY,
                    SCUD_MISSILE_LOFT_HEIGHT, SCUD_MISSILE_TURN_DISTANCE,
                };
                let speed = SCUD_MISSILE_INITIAL_VELOCITY / 30.0;
                let pos = e.transform.position;
                let (aim_x, aim_y, aim_z) = if e.scud_launcher_missile_has_aim {
                    (
                        e.scud_launcher_missile_aim_x,
                        e.scud_launcher_missile_aim_y,
                        e.scud_launcher_missile_aim_z,
                    )
                } else {
                    (pos.x, pos.y, pos.z)
                };
                let fuel_done = e.scud_launcher_missile_has_fuel
                    && e.scud_launcher_missile_fuel_expires_frame <= frame;
                let travelled = e.scud_launcher_missile_travelled;
                let dx = aim_x - pos.x;
                let dy = aim_y - pos.y;
                let dz = aim_z - pos.z;
                let horiz = (dx * dx + dz * dz).sqrt();
                let (vx, vy, vz) = if travelled < SCUD_MISSILE_TURN_DISTANCE {
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    let (dxn, dyn_y, dzn) = if dist > 0.001 {
                        (dx / dist, dy / dist, dz / dist)
                    } else {
                        (0.0, 1.0, 0.0)
                    };
                    let mut vy = dyn_y * speed;
                    if pos.y < aim_y + SCUD_MISSILE_LOFT_HEIGHT {
                        vy = speed * 0.85;
                    }
                    (dxn * speed, vy, dzn * speed)
                } else if horiz > SCUD_MISSILE_DIVE_DISTANCE {
                    let loft_y = aim_y + SCUD_MISSILE_LOFT_HEIGHT * 0.5;
                    let lx = aim_x - pos.x;
                    let ly = loft_y - pos.y;
                    let lz = aim_z - pos.z;
                    let dist = (lx * lx + ly * ly + lz * lz).sqrt();
                    if dist > 0.001 {
                        (lx / dist * speed, ly / dist * speed, lz / dist * speed)
                    } else {
                        (0.0, -speed, 0.0)
                    }
                } else {
                    let dist = (dx * dx + dy * dy + dz * dz).sqrt();
                    if dist > 0.001 {
                        (dx / dist * speed, dy / dist * speed, dz / dist * speed)
                    } else {
                        (0.0, -speed, 0.0)
                    }
                };
                let step = (vx * vx + vy * vy + vz * vz).sqrt().max(speed);
                let new_x = pos.x + vx;
                let new_y = pos.y + vy;
                let new_z = pos.z + vz;
                e.transform.position.x = new_x;
                e.transform.position.y = new_y;
                e.transform.position.z = new_z;
                e.scud_launcher_missile_travelled += step;
                if vx * vx + vz * vz > 1e-6 {
                    e.transform.orientation = vz.atan2(vx);
                }
                let near = (aim_x - new_x) * (aim_x - new_x) + (aim_z - new_z) * (aim_z - new_z)
                    < 12.0 * 12.0
                    && new_y <= aim_y + 15.0;
                if fuel_done || near {
                    e.scud_launcher_missile_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                            crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                team,
                                pos: glam::Vec3::new(new_x, new_y, new_z),
                                kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Scud {
                                    toxin: e.scud_launcher_missile_toxin,
                                },
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 800: Neutron cannon shell residual.
            if e.neutron_cannon_shell_projectile {
                use crate::game_logic::host_neutron_shell::neutron_shell_bezier_point;
                let pos = e.transform.position;
                let from = if e.neutron_shell_has_from {
                    glam::Vec3::new(
                        e.neutron_shell_from_x,
                        e.neutron_shell_from_y,
                        e.neutron_shell_from_z,
                    )
                } else {
                    glam::Vec3::new(pos.x, pos.y, pos.z)
                };
                let aim = if e.neutron_shell_has_aim {
                    glam::Vec3::new(
                        e.neutron_shell_aim_x,
                        e.neutron_shell_aim_y,
                        e.neutron_shell_aim_z,
                    )
                } else {
                    from
                };
                let launch = e.neutron_shell_launch_frame;
                let total = e.neutron_shell_flight_frames.max(1);
                let elapsed = frame.saturating_sub(launch);
                let t = (elapsed as f32 / total as f32).clamp(0.0, 1.0);
                let new_pos = neutron_shell_bezier_point(from, aim, t);
                let dx = new_pos.x - pos.x;
                let dz = new_pos.z - pos.z;
                e.transform.position.x = new_pos.x;
                e.transform.position.y = new_pos.y;
                e.transform.position.z = new_pos.z;
                if dx * dx + dz * dz > 1e-6 {
                    e.transform.orientation = dz.atan2(dx);
                }
                if elapsed >= total || t >= 0.999 {
                    e.neutron_cannon_shell_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                            crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                team,
                                pos: aim,
                                kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Neutron,
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 800: Nuke cannon shell residual.
            if e.nuke_cannon_shell_projectile {
                use crate::game_logic::host_nuke_cannon::nuke_shell_bezier_point;
                let pos = e.transform.position;
                let from = if e.nuke_shell_has_from {
                    glam::Vec3::new(
                        e.nuke_shell_from_x,
                        e.nuke_shell_from_y,
                        e.nuke_shell_from_z,
                    )
                } else {
                    glam::Vec3::new(pos.x, pos.y, pos.z)
                };
                let aim = if e.nuke_shell_has_aim {
                    glam::Vec3::new(e.nuke_shell_aim_x, e.nuke_shell_aim_y, e.nuke_shell_aim_z)
                } else {
                    from
                };
                let launch = e.nuke_shell_launch_frame;
                let frames = e.nuke_shell_flight_frames.max(1);
                let elapsed = frame.saturating_sub(launch);
                let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
                let new_pos = nuke_shell_bezier_point(from, aim, t);
                let dx = new_pos.x - pos.x;
                let dz = new_pos.z - pos.z;
                e.transform.position.x = new_pos.x;
                e.transform.position.y = new_pos.y;
                e.transform.position.z = new_pos.z;
                if dx * dx + dz * dz > 1.0e-6 {
                    e.transform.orientation = dz.atan2(dx);
                }
                if elapsed >= frames {
                    e.nuke_cannon_shell_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        crate::game_logic::host_cannon_shell_projectile_log::record_impact(
                            crate::game_logic::host_cannon_shell_projectile_log::CannonShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                team,
                                pos: aim,
                                kind: crate::game_logic::host_cannon_shell_projectile_log::CannonShellKind::Nuke,
                            },
                        );
                    }
                }
                changed = true;
            }

            // Wave 802: Nuke / Anthrax / Inferno field-object lifetime residual.
            if e.nuke_radiation_field
                && e.nuke_radiation_field_expires_frame > 0
                && frame >= e.nuke_radiation_field_expires_frame
            {
                e.nuke_radiation_field = false;
                e.nuke_radiation_field_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::NukeRadiation,
                         producer: None, },
                    );
                }
                changed = true;
            }
            if e.anthrax_toxin_field
                && e.anthrax_toxin_field_expires_frame > 0
                && frame >= e.anthrax_toxin_field_expires_frame
            {
                e.anthrax_toxin_field = false;
                e.anthrax_toxin_field_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::AnthraxToxin,
                         producer: None, },
                    );
                }
                changed = true;
            }
            if e.inferno_fire_field
                && e.inferno_fire_field_expires_frame > 0
                && frame >= e.inferno_fire_field_expires_frame
            {
                e.inferno_fire_field = false;
                e.inferno_fire_field_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::InfernoFire,
                         producer: None, },
                    );
                }
                changed = true;
            }

            // Wave 803: Inferno shell projectile residual.
            if e.inferno_shell_projectile {
                use crate::game_logic::host_inferno_cannon::inferno_shell_bezier_point;
                let pos = e.transform.position;
                let from = if e.inferno_shell_has_from {
                    glam::Vec3::new(
                        e.inferno_shell_from_x,
                        e.inferno_shell_from_y,
                        e.inferno_shell_from_z,
                    )
                } else {
                    glam::Vec3::new(pos.x, pos.y, pos.z)
                };
                let aim = if e.inferno_shell_has_aim {
                    glam::Vec3::new(
                        e.inferno_shell_aim_x,
                        e.inferno_shell_aim_y,
                        e.inferno_shell_aim_z,
                    )
                } else {
                    from
                };
                let launch = e.inferno_shell_launch_frame;
                let frames = e.inferno_shell_flight_frames.max(1);
                let elapsed = frame.saturating_sub(launch);
                let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
                let new_pos = inferno_shell_bezier_point(from, aim, t);
                let dx = new_pos.x - pos.x;
                let dz = new_pos.z - pos.z;
                e.transform.position.x = new_pos.x;
                e.transform.position.y = new_pos.y;
                e.transform.position.z = new_pos.z;
                if dx * dx + dz * dz > 1.0e-6 {
                    e.transform.orientation = dz.atan2(dx);
                }
                if elapsed >= frames {
                    e.inferno_shell_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let team = Self::entity_team_from_ordinal(e.team_ordinal);
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        let intended = if e.inferno_shell_has_intended {
                            Some(crate::game_logic::ObjectId(e.inferno_shell_intended))
                        } else {
                            None
                        };
                        crate::game_logic::host_inferno_shell_projectile_log::record_impact(
                            crate::game_logic::host_inferno_shell_projectile_log::InfernoShellImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                                upgraded: e.inferno_shell_upgraded,
                                team,
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 803: SpySatellite ping lifetime residual.
            if e.spy_satellite_ping
                && e.spy_satellite_ping_expires_frame > 0
                && frame >= e.spy_satellite_ping_expires_frame
            {
                e.spy_satellite_ping = false;
                e.spy_satellite_ping_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_spy_satellite_ping_log::record_expire(
                        crate::game_logic::ObjectId(hid),
                    );
                }
                changed = true;
            }

            // Wave 804: Flashbang grenade residual.
            if e.flashbang_grenade_projectile {
                use crate::game_logic::host_ranger::flashbang_shell_bezier_point;
                let pos = e.transform.position;
                let from = if e.flashbang_grenade_has_from {
                    glam::Vec3::new(
                        e.flashbang_grenade_from_x,
                        e.flashbang_grenade_from_y,
                        e.flashbang_grenade_from_z,
                    )
                } else {
                    glam::Vec3::new(pos.x, pos.y, pos.z)
                };
                let aim = if e.flashbang_grenade_has_aim {
                    glam::Vec3::new(
                        e.flashbang_grenade_aim_x,
                        e.flashbang_grenade_aim_y,
                        e.flashbang_grenade_aim_z,
                    )
                } else {
                    from
                };
                let launch = e.flashbang_grenade_launch_frame;
                let frames = e.flashbang_grenade_flight_frames.max(1);
                let elapsed = frame.saturating_sub(launch);
                let t = (elapsed as f32 / frames as f32).clamp(0.0, 1.0);
                let new_pos = flashbang_shell_bezier_point(from, aim, t);
                let dx = new_pos.x - pos.x;
                let dz = new_pos.z - pos.z;
                e.transform.position.x = new_pos.x;
                e.transform.position.y = new_pos.y;
                e.transform.position.z = new_pos.z;
                if dx * dx + dz * dz > 1.0e-6 {
                    e.transform.orientation = dz.atan2(dx);
                }
                if elapsed >= frames {
                    e.flashbang_grenade_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        let intended = if e.flashbang_grenade_has_intended {
                            Some(crate::game_logic::ObjectId(e.flashbang_grenade_intended))
                        } else {
                            None
                        };
                        crate::game_logic::host_flashbang_comanche_helix_projectile_log::record_flashbang(
                            crate::game_logic::host_flashbang_comanche_helix_projectile_log::FlashbangImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 804: Comanche rocket-pod residual.
            if e.comanche_rocket_pod_projectile {
                let vx = e.velocity[0];
                let vy = e.velocity[1];
                let vz = e.velocity[2];
                e.transform.position.x += vx;
                e.transform.position.y += vy;
                e.transform.position.z += vz;
                if e.comanche_rocket_pod_projectile_expires_frame > 0
                    && frame >= e.comanche_rocket_pod_projectile_expires_frame
                {
                    e.comanche_rocket_pod_projectile = false;
                    e.comanche_rocket_pod_projectile_expires_frame = 0;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_flashbang_comanche_helix_projectile_log::record_comanche_expire(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                }
                changed = true;
            }
            // Wave 804: Helix napalm bomb residual.
            if e.helix_napalm_bomb_projectile {
                e.transform.position.x += e.velocity[0];
                e.transform.position.y += e.velocity[1];
                e.transform.position.z += e.velocity[2];
                changed = true;
            }

            // Wave 805: Scorpion tank missile residual.
            if e.scorpion_missile_projectile {
                use crate::game_logic::host_scorpion::{
                    SCORPION_MISSILE_INITIAL_VELOCITY, SCORPION_MISSILE_PROJECTILE_SPEED,
                    SCORPION_MISSILE_TURN_DISTANCE,
                };
                let launch = SCORPION_MISSILE_INITIAL_VELOCITY / 30.0;
                let cruise = SCORPION_MISSILE_PROJECTILE_SPEED / 30.0;
                let pos = glam::Vec3::new(
                    e.transform.position.x,
                    e.transform.position.y,
                    e.transform.position.z,
                );
                let mut aim = if e.scorpion_missile_has_aim {
                    glam::Vec3::new(
                        e.scorpion_missile_aim_x,
                        e.scorpion_missile_aim_y,
                        e.scorpion_missile_aim_z,
                    )
                } else {
                    pos
                };
                if let Some(rt) = scorpion_retarget.get(&eid.get()).copied() {
                    aim = rt;
                    e.scorpion_missile_has_aim = true;
                    e.scorpion_missile_aim_x = aim.x;
                    e.scorpion_missile_aim_y = aim.y;
                    e.scorpion_missile_aim_z = aim.z;
                }
                let speed = if e.scorpion_missile_travelled < SCORPION_MISSILE_TURN_DISTANCE {
                    launch
                } else {
                    cruise
                };
                let to_aim = aim - pos;
                let vel = if to_aim.length() > 0.001 {
                    to_aim.normalize() * speed
                } else {
                    glam::Vec3::new(0.0, -speed, 0.0)
                };
                let step = vel.length().max(speed);
                e.velocity = [vel.x, vel.y, vel.z];
                e.transform.position.x = pos.x + vel.x;
                e.transform.position.y = pos.y + vel.y;
                e.transform.position.z = pos.z + vel.z;
                e.scorpion_missile_travelled += step;
                e.transform.orientation = vel.z.atan2(vel.x);
                let new_pos = glam::Vec3::new(
                    e.transform.position.x,
                    e.transform.position.y,
                    e.transform.position.z,
                );
                let fuel_done = e.scorpion_missile_fuel_expires_frame > 0
                    && frame >= e.scorpion_missile_fuel_expires_frame;
                let near = (aim - new_pos).length() < 6.0;
                if fuel_done || near {
                    e.scorpion_missile_projectile = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        let source = e.producer_id.map(crate::game_logic::ObjectId);
                        let intended = if e.scorpion_missile_has_intended {
                            Some(crate::game_logic::ObjectId(e.scorpion_missile_intended))
                        } else {
                            None
                        };
                        crate::game_logic::host_scorpion_missile_projectile_log::record_impact(
                            crate::game_logic::host_scorpion_missile_projectile_log::ScorpionMissileImpactEvent {
                                id: crate::game_logic::ObjectId(hid),
                                source,
                                intended,
                                pos: aim,
                                slot: e.scorpion_missile_slot,
                            },
                        );
                    }
                }
                changed = true;
            }

            // Wave 806: Spectre howitzer shell / flare / laser-beam lifetimes.
            if e.spectre_howitzer_shell {
                let height_die = e.transform.position.y <= 1.0;
                let timed = e.spectre_howitzer_shell_expires_frame > 0
                    && frame >= e.spectre_howitzer_shell_expires_frame;
                if height_die || timed {
                    e.spectre_howitzer_shell = false;
                    e.spectre_howitzer_shell_expires_frame = 0;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_field_object_expire_log::record(
                            crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                                id: crate::game_logic::ObjectId(hid),
                                team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                                kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::SpectreHowitzerShell,
                                producer: None,
                            },
                        );
                    }
                    changed = true;
                }
            }
            if e.countermeasure_flare
                && e.countermeasure_flare_expires_frame > 0
                && frame >= e.countermeasure_flare_expires_frame
            {
                e.countermeasure_flare = false;
                e.countermeasure_flare_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::CountermeasureFlare,
                            producer: e.producer_id.map(crate::game_logic::ObjectId),
                        },
                    );
                }
                changed = true;
            }
            if e.point_defense_laser_beam
                && e.point_defense_laser_beam_expires_frame > 0
                && frame >= e.point_defense_laser_beam_expires_frame
            {
                e.point_defense_laser_beam = false;
                e.point_defense_laser_beam_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::PointDefenseLaserBeam,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }
            if e.weapon_laser_beam
                && e.weapon_laser_beam_expires_frame > 0
                && frame >= e.weapon_laser_beam_expires_frame
            {
                e.weapon_laser_beam = false;
                e.weapon_laser_beam_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::WeaponLaserBeam,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }

            // Wave 807: Sticky bomb / booby-trap attach follow residual.
            if e.sticky_bomb_attached {
                const STICKY_OFFSET_Z: f32 = 8.0;
                let tid = e.sticky_bomb_attached_to;
                match sticky_booby_targets.get(&tid).copied() {
                    Some((tpos, true, immobile)) => {
                        let new_pos = if immobile {
                            glam::Vec3::new(tpos.x, 0.0, tpos.z)
                        } else {
                            glam::Vec3::new(tpos.x, tpos.y + STICKY_OFFSET_Z, tpos.z)
                        };
                        e.transform.position.x = new_pos.x;
                        e.transform.position.y = new_pos.y;
                        e.transform.position.z = new_pos.z;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_sticky_booby_attach_log::record_sticky_follow(
                                crate::game_logic::host_sticky_booby_attach_log::StickyFollowEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    pos: new_pos,
                                },
                            );
                        }
                        changed = true;
                    }
                    _ => {
                        e.sticky_bomb_attached = false;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_sticky_booby_attach_log::record_sticky_destroy(
                                crate::game_logic::host_sticky_booby_attach_log::StickyDestroyEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                },
                            );
                        }
                        changed = true;
                    }
                }
            }
            if e.booby_trap_special && e.booby_trap_has_attached {
                const STICKY_OFFSET_Y: f32 = 8.0;
                let tid = e.booby_trap_attached_to;
                match sticky_booby_targets.get(&tid).copied() {
                    Some((tpos, true, _)) => {
                        let new_pos = glam::Vec3::new(tpos.x, tpos.y + STICKY_OFFSET_Y, tpos.z);
                        e.transform.position.x = new_pos.x;
                        e.transform.position.y = new_pos.y;
                        e.transform.position.z = new_pos.z;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_sticky_booby_attach_log::record_booby_follow(
                                crate::game_logic::host_sticky_booby_attach_log::BoobyFollowEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    pos: new_pos,
                                },
                            );
                        }
                        changed = true;
                    }
                    _ => {
                        e.booby_trap_special = false;
                        e.booby_trap_has_attached = false;
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_sticky_booby_attach_log::record_booby_destroy(
                                crate::game_logic::host_sticky_booby_attach_log::BoobyDestroyEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                },
                            );
                        }
                        changed = true;
                    }
                }
            }

            // Wave 808: Particle uplink trail/orbital/connector laser lifetimes.
            if e.particle_trail_remnant
                && e.particle_trail_remnant_expires_frame > 0
                && frame >= e.particle_trail_remnant_expires_frame
            {
                e.particle_trail_remnant = false;
                e.particle_trail_remnant_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::ParticleTrailRemnant,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }
            if e.particle_orbital_laser
                && e.particle_orbital_laser_expires_frame > 0
                && frame >= e.particle_orbital_laser_expires_frame
            {
                e.particle_orbital_laser = false;
                e.particle_orbital_laser_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::ParticleOrbitalLaser,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }
            if e.particle_connector_laser
                && e.particle_connector_laser_expires_frame > 0
                && frame >= e.particle_connector_laser_expires_frame
            {
                e.particle_connector_laser = false;
                e.particle_connector_laser_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::ParticleConnectorLaser,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }

            // Wave 809: Firewall segment crawl + lifetime residual.
            if e.firewall_segment {
                use crate::game_logic::host_firewall::FIREWALL_INCH_PER_FRAME;
                let (dx, dz) = if e.firewall_segment_has_dir {
                    (
                        e.firewall_segment_dir_x * FIREWALL_INCH_PER_FRAME,
                        e.firewall_segment_dir_z * FIREWALL_INCH_PER_FRAME,
                    )
                } else {
                    (FIREWALL_INCH_PER_FRAME, 0.0)
                };
                e.transform.position.x += dx;
                e.transform.position.z += dz;
                if e.firewall_segment_expires_frame > 0 && frame >= e.firewall_segment_expires_frame
                {
                    e.firewall_segment = false;
                    e.firewall_segment_expires_frame = 0;
                    e.firewall_segment_has_wall_id = false;
                    e.firewall_segment_has_dir = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_field_object_expire_log::record(
                            crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                                id: crate::game_logic::ObjectId(hid),
                                team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                                kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::FirewallSegment,
                                producer: None,
                            },
                        );
                    }
                }
                changed = true;
            }
            // Wave 809: Radar van ping lifetime residual.
            if e.radar_van_ping
                && e.radar_van_ping_expires_frame > 0
                && frame >= e.radar_van_ping_expires_frame
            {
                e.radar_van_ping = false;
                e.radar_van_ping_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::RadarVanPing,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }

            // Wave 810: Power plant rods extend completion residual.
            if e.power_plant_rods_done_frame > 0 && frame >= e.power_plant_rods_done_frame {
                use crate::game_logic::host_enum_table_residual::model_condition_bit_name_index;
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADING") {
                    e.model_condition_bits &= !(1u128 << bit);
                }
                if let Some(bit) = model_condition_bit_name_index("POWER_PLANT_UPGRADED") {
                    e.model_condition_bits |= 1u128 << bit;
                }
                e.power_plant_rods_done_frame = 0;
                e.power_plant_rods_extended = true;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_power_plant_rods_log::record_complete(
                        crate::game_logic::host_power_plant_rods_log::PowerPlantRodsCompleteEvent {
                            id: crate::game_logic::ObjectId(hid),
                            model_condition_bits: e.model_condition_bits,
                        },
                    );
                }
                changed = true;
            }

            // Wave 811: Powered buildings disabled_underpowered residual.
            {
                const POWERED_BIT: u32 = 1u32 << 28; // KindOf::Powered in presentation ORDER
                if (e.kind_of_bits & POWERED_BIT) != 0 {
                    let constructed =
                        !e.under_construction && e.construction_percent + 0.001 >= 1.0;
                    let alive = e.health > 0.0 && !e.destroyed;
                    let should =
                        underpowered_team_ords.contains(&e.team_ordinal) && alive && constructed;
                    if e.disabled_underpowered != should {
                        e.disabled_underpowered = should;
                        changed = true;
                    }
                } else if e.disabled_underpowered {
                    e.disabled_underpowered = false;
                    changed = true;
                }
            }

            // Wave 812: Battlemaster horde status residual.
            if crate::game_logic::host_battlemaster::is_battlemaster_template(e.template_name()) {
                use crate::game_logic::host_battlemaster::{
                    counts_toward_battlemaster_horde, distance_2d, is_in_horde,
                    BATTLE_MASTER_HORDE_RADIUS,
                };
                let alive = e.health > 0.0 && !e.destroyed;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let x = e.transform.position.x;
                    let z = e.transform.position.z;
                    let team = e.team_ordinal;
                    let mut nearby = 0u32;
                    for (oid, oteam, ox, oz, oalive) in &battlemaster_snapshot {
                        if *oid == hid {
                            continue;
                        }
                        let dist = distance_2d(x, z, *ox, *oz);
                        if counts_toward_battlemaster_horde(
                            alive,
                            *oalive,
                            team == *oteam,
                            true,
                            dist,
                            BATTLE_MASTER_HORDE_RADIUS,
                        ) {
                            nearby = nearby.saturating_add(1);
                        }
                    }
                    let now_horde = alive && is_in_horde(nearby);
                    let was = e.weapon_bonus_horde;
                    if e.weapon_bonus_horde != now_horde || now_horde {
                        e.weapon_bonus_horde = now_horde;
                        crate::game_logic::host_battlemaster_horde_log::record(
                            crate::game_logic::host_battlemaster_horde_log::BattlemasterHordeEvent {
                                id: crate::game_logic::ObjectId(hid),
                                now_horde,
                                was_horde: was,
                            },
                        );
                        changed = true;
                    }
                }
            }

            // Wave 813: China infantry horde status residual.
            if crate::game_logic::host_red_guard::is_china_infantry_horde_unit(e.template_name()) {
                use crate::game_logic::host_red_guard::{
                    counts_toward_infantry_horde, distance_2d, is_in_infantry_horde,
                    INFANTRY_HORDE_RADIUS,
                };
                let alive = e.health > 0.0 && !e.destroyed;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let x = e.transform.position.x;
                    let z = e.transform.position.z;
                    let team = e.team_ordinal;
                    let mut nearby = 0u32;
                    for (oid, oteam, ox, oz, oalive) in &infantry_snapshot {
                        if *oid == hid {
                            continue;
                        }
                        let dist = distance_2d(x, z, *ox, *oz);
                        if counts_toward_infantry_horde(
                            alive,
                            *oalive,
                            team == *oteam,
                            true,
                            dist,
                            INFANTRY_HORDE_RADIUS,
                        ) {
                            nearby = nearby.saturating_add(1);
                        }
                    }
                    let now_horde = alive && is_in_infantry_horde(nearby);
                    let was = e.weapon_bonus_horde;
                    if e.weapon_bonus_horde != now_horde || now_horde {
                        e.weapon_bonus_horde = now_horde;
                        let name = e.template_name();
                        let kind = if crate::game_logic::host_red_guard::is_red_guard_template(name)
                        {
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind::RedGuard
                        } else if crate::game_logic::host_tank_hunter::is_tank_hunter_template(name)
                        {
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind::TankHunter
                        } else {
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeKind::Minigunner
                        };
                        crate::game_logic::host_china_infantry_horde_log::record(
                            crate::game_logic::host_china_infantry_horde_log::ChinaInfantryHordeEvent {
                                id: crate::game_logic::ObjectId(hid),
                                kind,
                                now_horde,
                                was_horde: was,
                            },
                        );
                        changed = true;
                    }
                }
            }

            // Wave 814: Stinger hive slave respawn residual.
            if crate::game_logic::host_base_defense::is_stinger_site_structure(e.template_name()) {
                use crate::game_logic::host_base_defense::{
                    next_stinger_slave_respawn_frame, should_respawn_stinger_slave,
                    STINGER_SOLDIER_MAX_HEALTH, STINGER_SPAWN_NUMBER,
                };
                let alive = e.health > 0.0 && !e.destroyed;
                if alive {
                    // Align roster alive count to hive_slave_count mirror when diverged.
                    let roster_alive = e.hive_slaves_alive.iter().filter(|&&a| a).count() as u8;
                    if roster_alive != e.hive_slave_count {
                        let desired = e.hive_slave_count.min(3);
                        for i in 0..3 {
                            let should = (i as u8) < desired;
                            e.hive_slaves_alive[i] = should;
                            if should && e.hive_slaves_hp[i] <= 0.0 {
                                e.hive_slaves_hp[i] = if e.hive_slave_hp > 0.0 {
                                    e.hive_slave_hp
                                } else {
                                    STINGER_SOLDIER_MAX_HEALTH
                                };
                            }
                            if !should {
                                e.hive_slaves_hp[i] = 0.0;
                            }
                        }
                        changed = true;
                    }
                    if should_respawn_stinger_slave(
                        e.hive_slave_count,
                        frame,
                        e.hive_slave_respawn_frame,
                    ) {
                        // Respawn first dead slot.
                        let mut did = false;
                        for i in 0..3 {
                            if !e.hive_slaves_alive[i] {
                                e.hive_slaves_alive[i] = true;
                                e.hive_slaves_hp[i] = STINGER_SOLDIER_MAX_HEALTH;
                                did = true;
                                break;
                            }
                        }
                        if did {
                            e.hive_slave_count =
                                e.hive_slaves_alive.iter().filter(|&&a| a).count() as u8;
                            e.hive_slave_hp = e
                                .hive_slaves_alive
                                .iter()
                                .zip(e.hive_slaves_hp.iter())
                                .find(|(a, _)| **a)
                                .map(|(_, h)| *h)
                                .unwrap_or(0.0);
                        } else {
                            e.hive_slave_count = e
                                .hive_slave_count
                                .saturating_add(1)
                                .min(STINGER_SPAWN_NUMBER as u8);
                            if e.hive_slave_count == 1 {
                                e.hive_slave_hp = STINGER_SOLDIER_MAX_HEALTH;
                            }
                        }
                        if e.hive_slave_count < STINGER_SPAWN_NUMBER as u8 {
                            e.hive_slave_respawn_frame = next_stinger_slave_respawn_frame(frame, 0);
                        } else {
                            e.hive_slave_respawn_frame = 0;
                        }
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_stinger_hive_log::record(
                                crate::game_logic::host_stinger_hive_log::StingerHiveRespawnEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    hive_slave_count: e.hive_slave_count,
                                    hive_slave_hp: e.hive_slave_hp,
                                    hive_slave_respawn_frame: e.hive_slave_respawn_frame,
                                    slaves_alive: e.hive_slaves_alive,
                                    slaves_hp: e.hive_slaves_hp,
                                },
                            );
                        }
                        changed = true;
                    } else if e.hive_slave_count < STINGER_SPAWN_NUMBER as u8
                        && e.hive_slave_respawn_frame == 0
                    {
                        e.hive_slave_respawn_frame = next_stinger_slave_respawn_frame(frame, 0);
                        changed = true;
                    }
                }
            }

            // Wave 815: ACTIVELY_CONSTRUCTING model condition residual.
            {
                use crate::game_logic::host_enum_table_residual::actively_constructing_model_bit;
                let ac_bit = actively_constructing_model_bit();
                let ac_mask = 1u128 << ac_bit;
                const WORKER_BIT: u32 = 1u32 << 9; // KindOf::Worker
                let alive = e.health > 0.0 && !e.destroyed;
                let name = e.template_name();
                let is_worker = (e.kind_of_bits & WORKER_BIT) != 0
                    || name.contains("Dozer")
                    || name.contains("Worker")
                    || name.contains("dozer")
                    || name.contains("worker");
                const STRUCTURE_BIT: u32 = 1u32 << 0;
                let can_construct = is_worker && (e.kind_of_bits & STRUCTURE_BIT) == 0;
                let is_dozer_building = can_construct && matches!(e.ai_state_ordinal, 7 | 8); // Constructing | Repairing
                let is_producing =
                    e.production_queue_len > 0 || !e.production_queue_items.is_empty();
                let has_bit = (e.model_condition_bits & ac_mask) != 0;
                if alive && (can_construct || is_producing || has_bit) {
                    let want = is_dozer_building || is_producing;
                    let before = e.model_condition_bits;
                    if want {
                        e.model_condition_bits |= ac_mask;
                    } else {
                        e.model_condition_bits &= !ac_mask;
                    }
                    if e.model_condition_bits != before {
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            crate::game_logic::host_actively_constructing_log::record(
                                crate::game_logic::host_actively_constructing_log::ActivelyConstructingEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    model_condition_bits: e.model_condition_bits,
                                    want,
                                },
                            );
                        }
                        changed = true;
                    }
                }
            }

            // Wave 817: Money/salvage crate DeletionUpdate residual.
            if e.money_crate
                && e.money_crate_expires_frame > 0
                && frame >= e.money_crate_expires_frame
            {
                e.money_crate = false;
                e.money_crate_expires_frame = 0;
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    crate::game_logic::host_field_object_expire_log::record(
                        crate::game_logic::host_field_object_expire_log::FieldObjectExpireEvent {
                            id: crate::game_logic::ObjectId(hid),
                            team: Some(Self::entity_team_from_ordinal(e.team_ordinal)),
                            kind: crate::game_logic::host_field_object_expire_log::FieldObjectKind::MoneyCrate,
                            producer: None,
                        },
                    );
                }
                changed = true;
            }

            // Wave 819: Dozer/worker bored idle timer residual.
            {
                const WORKER_BIT: u32 = 1u32 << 9;
                const STRUCTURE_BIT: u32 = 1u32 << 0;
                let alive = e.health > 0.0 && !e.destroyed;
                let name = e.template_name();
                let is_worker = (e.kind_of_bits & WORKER_BIT) != 0
                    || name.contains("Dozer")
                    || name.contains("Worker")
                    || name.contains("dozer")
                    || name.contains("worker");
                let can_repair = is_worker && (e.kind_of_bits & STRUCTURE_BIT) == 0;
                if alive && can_repair {
                    if e.ai_state_ordinal == 0 {
                        // Idle
                        if e.idle_since_frame == 0 {
                            e.idle_since_frame = frame.max(1);
                            changed = true;
                        }
                        let bored = crate::game_logic::host_repair::DOZER_BORED_TIME_FRAMES;
                        if frame.saturating_sub(e.idle_since_frame) >= bored {
                            // Match host: reset stamp then attempt service acquire on host drain.
                            e.idle_since_frame = frame.max(1);
                            if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                                crate::game_logic::host_dozer_bored_log::record(
                                    crate::game_logic::host_dozer_bored_log::DozerBoredEvent {
                                        id: crate::game_logic::ObjectId(hid),
                                    },
                                );
                            }
                            changed = true;
                        }
                    } else if e.idle_since_frame != 0 {
                        e.idle_since_frame = 0;
                        changed = true;
                    }
                }
            }

            // Wave 820: FireSpread/Flammable residual.
            if e.fire_spread_active {
                use crate::game_logic::host_fire_spread::HostFireSpreadData;
                // Rebuild temp data, tick, write back fields.
                let mut fs = HostFireSpreadData::tree_default();
                fs.active = e.fire_spread_active;
                fs.state = match e.fire_spread_state {
                    1 => crate::game_logic::host_fire_spread::HostFlammableState::Aflame,
                    2 => crate::game_logic::host_fire_spread::HostFlammableState::Burned,
                    _ => crate::game_logic::host_fire_spread::HostFlammableState::Normal,
                };
                fs.aflame_end_frame = e.fire_spread_aflame_end_frame;
                fs.burned_end_frame = e.fire_spread_burned_end_frame;
                fs.next_spread_frame = e.fire_spread_next_spread_frame;
                fs.min_spread_delay = e.fire_spread_min_delay;
                fs.max_spread_delay = e.fire_spread_max_delay;
                fs.spread_try_range = e.fire_spread_try_range;
                fs.aflame_duration = e.fire_spread_aflame_duration;
                fs.burned_delay = e.fire_spread_burned_delay;
                fs.spread_enabled = e.fire_spread_enabled;
                fs.flame_damage_accum = e.fire_spread_flame_damage_accum;
                fs.flame_damage_limit = e.fire_spread_flame_damage_limit;
                let fr = fs.tick_flammable(frame);
                let sr = fs.tick_spread(frame);
                e.fire_spread_state = match fs.state {
                    crate::game_logic::host_fire_spread::HostFlammableState::Normal => 0,
                    crate::game_logic::host_fire_spread::HostFlammableState::Aflame => 1,
                    crate::game_logic::host_fire_spread::HostFlammableState::Burned => 2,
                };
                e.fire_spread_aflame_end_frame = fs.aflame_end_frame;
                e.fire_spread_burned_end_frame = fs.burned_end_frame;
                e.fire_spread_next_spread_frame = fs.next_spread_frame;
                e.fire_spread_flame_damage_accum = fs.flame_damage_accum;
                let mut ignite = None;
                if sr.try_spread {
                    let px = e.transform.position.x;
                    let pz = e.transform.position.z;
                    let range = e.fire_spread_try_range;
                    let mut best: Option<(u32, f32)> = None;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        for &(oid, ox, oz, would) in &fire_spread_candidates {
                            if oid == hid || !would {
                                continue;
                            }
                            let dx = ox - px;
                            let dz = oz - pz;
                            let dist = (dx * dx + dz * dz).sqrt();
                            if dist <= range && best.map(|(_, d)| dist < d).unwrap_or(true) {
                                best = Some((oid, dist));
                            }
                        }
                        ignite = best.map(|(oid, _)| crate::game_logic::ObjectId(oid));
                    }
                }
                if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                    let pos = e.transform.position;
                    crate::game_logic::host_fire_spread_log::record(
                        crate::game_logic::host_fire_spread_log::FireSpreadTickEvent {
                            id: crate::game_logic::ObjectId(hid),
                            state: e.fire_spread_state,
                            aflame_end_frame: e.fire_spread_aflame_end_frame,
                            burned_end_frame: e.fire_spread_burned_end_frame,
                            next_spread_frame: e.fire_spread_next_spread_frame,
                            became_burned: fr.became_burned,
                            aflame: fr.aflame,
                            try_spread: sr.try_spread,
                            spawn_embers: sr.spawn_embers,
                            ignite_target: ignite,
                            pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                            spread_try_range: e.fire_spread_try_range,
                            flame_damage_accum: e.fire_spread_flame_damage_accum,
                        },
                    );
                }
                changed = true;
            }

            // Wave 821: Black market / oil derrick AutoDeposit residual.
            if e.black_market_building {
                use crate::game_logic::{
                    is_legal_black_market_income_source, BLACK_MARKET_DEPOSIT_AMOUNT,
                    BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES,
                };
                let alive = e.health > 0.0 && !e.destroyed;
                let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
                let neutral = e.team_ordinal == 255;
                if is_legal_black_market_income_source(alive, constructed, neutral) {
                    if e.black_market_next_deposit_frame == 0 {
                        e.black_market_next_deposit_frame =
                            frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1));
                        changed = true;
                    } else if frame >= e.black_market_next_deposit_frame {
                        e.black_market_next_deposit_frame =
                            frame.saturating_add(BLACK_MARKET_DEPOSIT_INTERVAL_FRAMES.max(1));
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            let pos = e.transform.position;
                            crate::game_logic::host_auto_deposit_log::record(
                                crate::game_logic::host_auto_deposit_log::AutoDepositEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    kind: crate::game_logic::host_auto_deposit_log::AutoDepositKind::BlackMarket,
                                    team: Self::entity_team_from_ordinal(e.team_ordinal),
                                    pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                    amount: BLACK_MARKET_DEPOSIT_AMOUNT,
                                    next_deposit_frame: e.black_market_next_deposit_frame,
                                    stealthed: e.stealthed,
                                    detected: e.detected,
                                    supply_lines_boost: 0,
                                },
                            );
                        }
                        changed = true;
                    }
                }
            }
            if e.oil_derrick_building {
                use crate::game_logic::{
                    is_legal_oil_derrick_income_source, oil_derrick_deposit_amount,
                    OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES,
                };
                let alive = e.health > 0.0 && !e.destroyed;
                let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
                let neutral = e.team_ordinal == 255;
                if is_legal_oil_derrick_income_source(alive, constructed, neutral) {
                    if e.oil_derrick_next_deposit_frame == 0 {
                        e.oil_derrick_next_deposit_frame =
                            frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1));
                        changed = true;
                    } else if frame >= e.oil_derrick_next_deposit_frame {
                        e.oil_derrick_next_deposit_frame =
                            frame.saturating_add(OIL_DERRICK_DEPOSIT_INTERVAL_FRAMES.max(1));
                        // Supply lines boost resolved on host drain (player upgrades).
                        let (amount, boost) = oil_derrick_deposit_amount(false);
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            let pos = e.transform.position;
                            crate::game_logic::host_auto_deposit_log::record(
                                crate::game_logic::host_auto_deposit_log::AutoDepositEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    kind: crate::game_logic::host_auto_deposit_log::AutoDepositKind::OilDerrick,
                                    team: Self::entity_team_from_ordinal(e.team_ordinal),
                                    pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                    amount,
                                    next_deposit_frame: e.oil_derrick_next_deposit_frame,
                                    stealthed: e.stealthed,
                                    detected: e.detected,
                                    supply_lines_boost: boost,
                                },
                            );
                        }
                        changed = true;
                    }
                }
            }

            // Wave 822: China Hacker HackInternet residual cash.
            if e.hacker_unit && e.hacker_hacking {
                use crate::game_logic::{
                    cash_amount_for_level, cash_interval_frames, is_legal_hacker_income_source,
                    VeterancyLevel,
                };
                let alive = e.health > 0.0 && !e.destroyed;
                let neutral = e.team_ordinal == 255;
                let disabled_hacked = e.disabled_hacked;
                if is_legal_hacker_income_source(alive, neutral, disabled_hacked) {
                    let level = match e.veterancy_ordinal {
                        1 => VeterancyLevel::Veteran,
                        2 => VeterancyLevel::Elite,
                        3 => VeterancyLevel::Heroic,
                        _ => VeterancyLevel::Rookie,
                    };
                    let amount = cash_amount_for_level(level);
                    let interval = cash_interval_frames(e.hacker_in_internet_center);
                    if e.hacker_next_deposit_frame == 0 {
                        e.hacker_next_deposit_frame = frame.saturating_add(interval.max(1));
                        changed = true;
                    } else if frame >= e.hacker_next_deposit_frame {
                        e.hacker_next_deposit_frame = frame.saturating_add(interval.max(1));
                        if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                            let pos = e.transform.position;
                            crate::game_logic::host_hacker_income_log::record(
                                crate::game_logic::host_hacker_income_log::HackerIncomeEvent {
                                    id: crate::game_logic::ObjectId(hid),
                                    team: Self::entity_team_from_ordinal(e.team_ordinal),
                                    pos: glam::Vec3::new(pos.x, pos.y, pos.z),
                                    amount,
                                    next_deposit_frame: e.hacker_next_deposit_frame,
                                    in_internet_center: e.hacker_in_internet_center,
                                    stealthed: e.stealthed,
                                    detected: e.detected,
                                    veterancy_ordinal: e.veterancy_ordinal,
                                    container_radius: 0.0,
                                },
                            );
                        }
                        changed = true;
                    }
                }
            }

            if changed {
                n += 1;
            }
        }

        // Wave 801: AngryMob member follow residual (post-loop; needs nexus positions).
        {
            use gamelogic::world::entities::EntityId;
            use std::f32::consts::PI;
            // Snapshot host_id -> position for nexus lookup without nested borrows.
            let mut host_pos: std::collections::HashMap<u32, (f32, f32, f32, bool)> =
                std::collections::HashMap::new();
            for (&hid, &eid) in self.host_to_entity.iter() {
                if let Some(ent) = self.world.entity(eid) {
                    let p = ent.transform.position;
                    host_pos.insert(hid, (p.x, p.y, p.z, ent.destroyed || ent.health <= 0.0));
                }
            }
            let member_eids: Vec<EntityId> = self.host_to_entity.values().copied().collect();
            for eid in member_eids {
                let Some(e) = self.world.world_mut().entity_mut(eid) else {
                    continue;
                };
                if !e.angry_mob_member {
                    continue;
                }
                let mut destroy = false;
                if !e.angry_mob_has_nexus {
                    destroy = true;
                } else if let Some(&(x, y, z, dead)) = host_pos.get(&e.angry_mob_nexus_id) {
                    if dead {
                        destroy = true;
                    } else {
                        let hid = self.entity_to_host.get(&eid.get()).copied().unwrap_or(0);
                        let slot = hid % 8;
                        let angle = (slot as f32) * (2.0 * PI / 8.0);
                        let radius = 8.0 + (slot % 3) as f32 * 2.0;
                        e.transform.position.x = x + angle.cos() * radius;
                        e.transform.position.y = y;
                        e.transform.position.z = z + angle.sin() * radius;
                        n = n.saturating_add(1);
                    }
                } else {
                    destroy = true;
                }
                if destroy {
                    e.angry_mob_member = false;
                    if let Some(&hid) = self.entity_to_host.get(&eid.get()) {
                        crate::game_logic::host_angry_mob_member_follow_log::record_destroy(
                            crate::game_logic::ObjectId(hid),
                        );
                    }
                    n = n.saturating_add(1);
                }
            }
        }
        // Wave 792 pending A10 missile drops (registry sole-tick).
        {
            let mut due = Vec::new();
            let mut keep = Vec::new();
            for p in self.a10_pending_drops.drain(..) {
                if p.drop_frame <= frame {
                    due.push(p);
                } else {
                    keep.push(p);
                }
            }
            self.a10_pending_drops = keep;
            for p in due {
                let team = if let Some(&eid) = self.host_to_entity.get(&p.source_id) {
                    self.world
                        .entity(eid)
                        .map(|e| Self::entity_team_from_ordinal(e.team_ordinal))
                        .unwrap_or(crate::game_logic::Team::Neutral)
                } else {
                    crate::game_logic::Team::Neutral
                };
                crate::game_logic::host_a10_strike_drop_log::record_drop(
                    crate::game_logic::host_a10_strike_drop_log::A10DropEvent {
                        team,
                        target: p.target,
                        producer: crate::game_logic::ObjectId(p.source_id),
                    },
                );
                n = n.saturating_add(1);
            }
        }

        // Wave 793 pending ArtilleryBarrage shell drops (registry sole-tick).
        {
            let mut due = Vec::new();
            let mut keep = Vec::new();
            for p in self.artillery_pending_drops.drain(..) {
                if p.drop_frame <= frame {
                    due.push(p);
                } else {
                    keep.push(p);
                }
            }
            self.artillery_pending_drops = keep;
            for p in due {
                let team = if let Some(&eid) = self.host_to_entity.get(&p.source_id) {
                    self.world
                        .entity(eid)
                        .map(|e| Self::entity_team_from_ordinal(e.team_ordinal))
                        .unwrap_or(crate::game_logic::Team::Neutral)
                } else {
                    crate::game_logic::Team::Neutral
                };
                crate::game_logic::host_artillery_barrage_drop_log::record_drop(
                    crate::game_logic::host_artillery_barrage_drop_log::ArtilleryDropEvent {
                        team,
                        target: p.target,
                        producer: crate::game_logic::ObjectId(p.source_id),
                    },
                );
                n = n.saturating_add(1);
            }
        }

        // Wave 794 pending CarpetBomb drops (registry sole-tick).
        {
            let mut due = Vec::new();
            let mut keep = Vec::new();
            for p in self.carpet_pending_drops.drain(..) {
                if p.drop_frame <= frame {
                    due.push(p);
                } else {
                    keep.push(p);
                }
            }
            self.carpet_pending_drops = keep;
            for p in due {
                let team = if let Some(&eid) = self.host_to_entity.get(&p.source_id) {
                    self.world
                        .entity(eid)
                        .map(|e| Self::entity_team_from_ordinal(e.team_ordinal))
                        .unwrap_or(crate::game_logic::Team::Neutral)
                } else {
                    crate::game_logic::Team::Neutral
                };
                crate::game_logic::host_carpet_bomb_drop_log::record_drop(
                    crate::game_logic::host_carpet_bomb_drop_log::CarpetBombDropEvent {
                        team,
                        target: p.target,
                        producer: crate::game_logic::ObjectId(p.source_id),
                    },
                );
                n = n.saturating_add(1);
            }
        }

        // Wave 818: player radar provider count residual.
        {
            use crate::game_logic::host_radar::is_legal_radar_provider;
            const COMMAND_CENTER_BIT: u32 = 1u32 << 8;
            let mut providers_by_team: std::collections::HashMap<u8, u32> =
                std::collections::HashMap::new();
            let eids: Vec<_> = self.host_to_entity.values().copied().collect();
            for eid in &eids {
                let Some(e) = self.world.entity(*eid) else {
                    continue;
                };
                let alive = e.health > 0.0 && !e.destroyed;
                let constructed = !e.under_construction && e.construction_percent + 0.001 >= 1.0;
                let is_cc = (e.kind_of_bits & COMMAND_CENTER_BIT) != 0;
                let name = e.template_name();
                if !is_legal_radar_provider(alive, constructed, is_cc, name) {
                    continue;
                }
                if e.team_ordinal == 255 {
                    continue; // Neutral
                }
                *providers_by_team.entry(e.team_ordinal).or_insert(0) += 1;
            }
            let player_ids: Vec<_> = self
                .world
                .world()
                .active_players()
                .map(|(pid, _)| pid)
                .collect();
            for pid in player_ids {
                let Some(pd) = self.world.player_mut(pid) else {
                    continue;
                };
                let count = pd
                    .team
                    .and_then(|t| providers_by_team.get(&t).copied())
                    .unwrap_or(0) as i32;
                let had = pd.radar_count > 0 && !pd.radar_disabled;
                let prev = pd.radar_count;
                pd.radar_count = count;
                let has_now = pd.radar_count > 0 && !pd.radar_disabled;
                if prev != count || had != has_now {
                    let host_pid = self
                        .host_player_to_gw
                        .iter()
                        .find(|(_, gw)| **gw == pid)
                        .map(|(h, _)| *h)
                        .unwrap_or(u32::from(pid.get()));
                    crate::game_logic::host_player_radar_log::record(
                        crate::game_logic::host_player_radar_log::PlayerRadarEvent {
                            player_id: host_pid,
                            radar_count: pd.radar_count,
                            had_radar: had,
                            has_radar: has_now,
                        },
                    );
                    n = n.saturating_add(1);
                }
            }
        }
        // Wave 816: player is_alive residual from living team entities.
        {
            let mut living_teams: std::collections::HashSet<u8> = std::collections::HashSet::new();
            let alive_eids: Vec<_> = self.host_to_entity.values().copied().collect();
            for eid in &alive_eids {
                let Some(e) = self.world.entity(*eid) else {
                    continue;
                };
                if e.health > 0.0 && !e.destroyed {
                    living_teams.insert(e.team_ordinal);
                }
            }
            // Also Neutral-skip: host uses player.team match; team_ordinal 255 = Neutral
            let player_ids: Vec<_> = self
                .world
                .world()
                .active_players()
                .map(|(pid, _)| pid)
                .collect();
            for pid in player_ids {
                let Some(pd) = self.world.player_mut(pid) else {
                    continue;
                };
                let alive = pd.team.map(|t| living_teams.contains(&t)).unwrap_or(false);
                if pd.is_alive != alive {
                    pd.is_alive = alive;
                    n = n.saturating_add(1);
                } else {
                    pd.is_alive = alive;
                }
            }
        }

        n
    }
}
