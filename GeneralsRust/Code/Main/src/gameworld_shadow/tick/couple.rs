//! Coupled-tick depth, live shadow pointer, and host spawn/destroy bind helpers.

use super::*;
use crate::game_logic::{GameLogic, ObjectId};
use crate::gameworld_shadow::GameWorldShadow;
use gamelogic::world::WorldMutation;

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

/// Generation-checked couple handle. Not a TLS `NonNull` stash: the engine
/// owns `&mut GameWorldShadow` on the stack; this slot is only valid while
/// `generation` matches the live couple. Access with a stale generation fails closed.
#[derive(Clone, Copy)]
struct CoupledShadowSlot {
    generation: u64,
    ptr: *mut GameWorldShadow,
}

thread_local! {
    static COUPLE_GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static COUPLED_SHADOW: std::cell::Cell<CoupledShadowSlot> = const {
        std::cell::Cell::new(CoupledShadowSlot {
            generation: 0,
            ptr: std::ptr::null_mut(),
        })
    };
}

/// Guard that clears the generation slot on drop (panic-safe).
pub struct CoupledShadowGuard {
    generation: u64,
}

impl Drop for CoupledShadowGuard {
    fn drop(&mut self) {
        COUPLED_SHADOW.with(|c| {
            let slot = c.get();
            if slot.generation == self.generation {
                c.set(CoupledShadowSlot {
                    generation: 0,
                    ptr: std::ptr::null_mut(),
                });
            }
        });
    }
}

/// Install the live `GameWorldShadow` for the whole coupled tick (including
/// post-writeback complete/spawn). Keep the slot until `clear` / guard drop.
#[inline]
pub fn install_active_shadow_for_coupled_tick(shadow: &mut GameWorldShadow) {
    let generation = COUPLE_GENERATION.with(|g| {
        let next = g.get().wrapping_add(1).max(1);
        g.set(next);
        next
    });
    COUPLED_SHADOW.with(|c| {
        c.set(CoupledShadowSlot {
            generation,
            ptr: shadow as *mut GameWorldShadow,
        });
    });
}

/// Same as `install_active_shadow_for_coupled_tick` but clears on Drop.
pub fn install_coupled_shadow_guard(shadow: &mut GameWorldShadow) -> CoupledShadowGuard {
    install_active_shadow_for_coupled_tick(shadow);
    let generation = COUPLED_SHADOW.with(|c| c.get().generation);
    CoupledShadowGuard { generation }
}

/// Clear the generation-checked couple slot (end of couple only).
#[inline]
pub fn clear_active_shadow_for_coupled_tick() {
    COUPLED_SHADOW.with(|c| {
        c.set(CoupledShadowSlot {
            generation: 0,
            ptr: std::ptr::null_mut(),
        });
    });
}

fn with_coupled_shadow_slot<R>(f: impl FnOnce(&mut GameWorldShadow) -> R) -> Option<R> {
    COUPLED_SHADOW.with(|c| {
        let slot = c.get();
        if slot.generation == 0 || slot.ptr.is_null() {
            return None;
        }
        // SAFETY: `install_active_shadow_for_coupled_tick` stores a pointer to the
        // engine's exclusive `&mut GameWorldShadow` for this generation only.
        // Stale generations are rejected above. Cleared on couple end / Drop.
        let shadow = unsafe { &mut *slot.ptr };
        Some(f(shadow))
    })
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
    with_coupled_shadow_slot(|shadow| {
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
    .flatten()
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
    with_coupled_shadow_slot(|shadow| {
        if let Some(raw) = take_next_host_spawn_bind_entity() {
            return bind_host_to_existing_entity(shadow, event.id.0, raw);
        }
        shadow.apply_host_spawn_events(std::slice::from_ref(event), logic) > 0
    })
    .unwrap_or_else(|| {
        let _ = take_next_host_spawn_bind_entity();
        false
    })
}

/// Wave 681: if a coupled shadow tick is live, queue GameWorld Destroy for this
/// host ObjectId now (unmap after apply_pending).
///
/// Idempotent with end-of-tick `host_destroy_log` drain. Fail-closed when no
/// active shadow pointer or host id is unmapped.
/// Borrow the live coupled shadow immutably. Fail-closed when shadow is off.
pub fn with_active_shadow<R>(f: impl FnOnce(&GameWorldShadow) -> R) -> Option<R> {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return None;
    }
    with_coupled_shadow_slot(|shadow| f(shadow))
}

/// Borrow the live coupled shadow mutably. Fail-closed when shadow is off.
pub fn with_active_shadow_mut<R>(f: impl FnOnce(&mut GameWorldShadow) -> R) -> Option<R> {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return None;
    }
    with_coupled_shadow_slot(f)
}

/// Queue + apply a GameWorld mutation while the coupled session is live.
/// This is the write path for HP/pose/cash/target (no silent host-only mutate).
pub fn push_coupled_world_mutation(m: WorldMutation) -> bool {
    with_active_shadow_mut(|shadow| {
        shadow.world.queue_mutation(m);
        shadow.world.apply_pending_mutations() > 0
    })
    .unwrap_or(false)
}

/// GameWorld HP for a mapped host id (coupled session only).
/// True when this host id is mapped in the live coupled GameWorld.
/// Unmapped host objects must keep host construction/production advancing
/// (fail-open) or they stay under_construction forever.
pub fn coupled_host_mapped(host: ObjectId) -> bool {
    with_active_shadow(|shadow| shadow.entity_for_host(host).is_some()).unwrap_or(false)
}

pub fn coupled_entity_health(host: ObjectId) -> Option<f32> {
    with_active_shadow(|shadow| {
        let eid = shadow.entity_for_host(host)?;
        shadow.world.entity(eid).map(|e| e.health)
    })
    .flatten()
}

/// GameWorld construction percent + UC bit for a mapped host id.
/// Mid-frame HashMap `obj.construction_percent` / `obj.status.under_construction`
/// can lag writeback; this is truth while coupled.
#[cfg(test)]
mod couple_handle_tests {
    #[test]
    fn active_shadow_ptr_token_is_gone() {
        let impl_src = include_str!("couple.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap_or("");
        assert!(
            !impl_src.contains("NonNull<GameWorldShadow>"),
            "couple must not stash NonNull in TLS"
        );
        assert!(impl_src.contains("CoupledShadowSlot"));
        assert!(impl_src.contains("with_coupled_shadow_slot"));
    }
}

pub fn coupled_entity_construction(host: ObjectId) -> Option<(f32, bool)> {
    with_active_shadow(|shadow| {
        let eid = shadow.entity_for_host(host)?;
        shadow
            .world
            .entity(eid)
            .map(|e| (e.construction_percent, e.under_construction))
    })
    .flatten()
}

/// GameWorld pose for a mapped host id (coupled session only).
pub fn coupled_entity_pose(host: ObjectId) -> Option<[f32; 3]> {
    with_active_shadow(|shadow| {
        let eid = shadow.entity_for_host(host)?;
        shadow.world.entity(eid).map(|e| {
            let p = e.transform.position;
            [p.x, p.y, p.z]
        })
    })
    .flatten()
}

/// GameWorld cash for a mapped host player (coupled session only).
pub fn coupled_player_cash(host_player: u32) -> Option<u32> {
    with_active_shadow(|shadow| {
        let gw = shadow.host_player_to_gw.get(&host_player).copied()?;
        shadow.world.player(gw).map(|p| p.supplies)
    })
    .flatten()
}

/// GameWorld attack target host id for a mapped unit (coupled session only).
pub fn coupled_entity_target_host(host: ObjectId) -> Option<ObjectId> {
    with_active_shadow(|shadow| {
        let eid = shadow.entity_for_host(host)?;
        let tid = shadow.world.entity(eid)?.attack_target?;
        shadow.host_for_entity(tid)
    })
    .flatten()
}

/// Coupled fat-field snapshot so the host HashMap is a view, not a second store.
#[derive(Clone, Debug)]
pub struct CoupledFatView {
    pub weapon_ammo: u32,
    pub weapon_clip_size: u32,
    pub attack_substate_ordinal: u8,
    pub ai_state_ordinal: u8,
    pub occupant_count: u16,
    pub contained_by_host: u32,
    pub garrisoned_host_ids: Vec<u32>,
    pub move_target: Option<[f32; 3]>,
    pub path_waypoints: Vec<[f32; 3]>,
    pub path_index: u16,
}

/// GameWorld fat fields for a mapped host id (coupled session only).
pub fn coupled_entity_fat_view(host: ObjectId) -> Option<CoupledFatView> {
    with_active_shadow(|shadow| {
        let eid = shadow.entity_for_host(host)?;
        let e = shadow.world.entity(eid)?;
        Some(CoupledFatView {
            weapon_ammo: e.weapon_ammo,
            weapon_clip_size: e.weapon_clip_size,
            attack_substate_ordinal: e.attack_substate_ordinal,
            ai_state_ordinal: e.ai_state_ordinal,
            occupant_count: e.occupant_count,
            contained_by_host: e.contained_by_host,
            garrisoned_host_ids: e.garrisoned_host_ids.clone(),
            move_target: e.move_target,
            path_waypoints: e.path_waypoints.clone(),
            path_index: e.path_index,
        })
    })
    .flatten()
}

pub fn coupled_entity_weapon_ammo(host: ObjectId) -> Option<u32> {
    coupled_entity_fat_view(host).map(|v| v.weapon_ammo)
}

pub fn coupled_entity_attack_substate(host: ObjectId) -> Option<u8> {
    coupled_entity_fat_view(host).map(|v| v.attack_substate_ordinal)
}

pub fn coupled_entity_occupant_count(host: ObjectId) -> Option<u16> {
    coupled_entity_fat_view(host).map(|v| v.occupant_count)
}

/// Mapped-entity dest. Returns `None` when unmapped **or** dest is cleared.
/// Prefer [`coupled_entity_fat_view`] when you must distinguish those cases.
pub fn coupled_entity_move_dest(host: ObjectId) -> Option<[f32; 3]> {
    coupled_entity_fat_view(host).and_then(|v| v.move_target)
}

pub fn eager_unmap_host_destroy_if_coupled(host: ObjectId) -> bool {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return false;
    }
    with_coupled_shadow_slot(|shadow| {
        let (queued, applied) = shadow.apply_host_destroy_events(&[
            crate::game_logic::host_destroy_log::HostDestroyEvent { id: host },
        ]);
        queued > 0 || applied > 0
    })
    .unwrap_or(false)
}
