//! Coupled-tick depth, live shadow pointer, and host spawn/destroy bind helpers.

use crate::game_logic::{GameLogic, ObjectId};
use crate::gameworld_shadow::GameWorldShadow;
use super::*;

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
            super::eager_combat::clear_early_combat_batches();
            super::eager_ai::clear_early_ai_batches();
            super::eager_weapon::clear_early_weapon_batches();
            super::eager_orders::clear_early_orders_batches();
            super::eager_stealth::clear_early_stealth_batches();
            super::eager_contain::clear_early_contain_batches();
            super::eager_identity::clear_early_identity_batches();
            super::eager_misc::clear_early_misc_batches();
            super::eager_economy::clear_early_economy_batches();
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
