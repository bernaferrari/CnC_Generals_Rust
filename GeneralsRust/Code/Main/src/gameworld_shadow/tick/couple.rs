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

/// RAII ownership of one coupled host frame.
///
/// The engine normally keeps a coupled frame open across host simulation and
/// GameWorld writeback.  Keeping that lifetime explicit ensures a panic cannot
/// leave authority flags or the active-shadow handle live on the next frame.
pub struct CoupledTickGuard {
    active: bool,
}

impl CoupledTickGuard {
    #[inline]
    pub fn enter() -> Self {
        begin_shadow_coupled_tick();
        Self { active: true }
    }
}

impl Drop for CoupledTickGuard {
    fn drop(&mut self) {
        if self.active {
            end_shadow_coupled_tick();
            self.active = false;
        }
    }
}

/// Host freeze of sole-tick systems is valid only on a coupled engine frame.
#[inline]
pub fn shadow_coupled_tick_active() -> bool {
    SHADOW_COUPLED_TICK_DEPTH.with(|d| d.get() > 0)
}

/// Temporary synchronous coupling slot. The owner is exclusively borrowed by
/// `with_coupled_shadow` for the entire lexical scope. Generation distinguishes
/// nested scopes; it does not establish the pointer lifetime.
#[derive(Clone, Copy)]
struct CoupledShadowSlot {
    generation: u64,
    ptr: *mut GameWorldShadow,
    /// A scoped callback currently owns the sole mutable access.  A nested
    /// callback must fail closed instead of reborrowing the TLS raw pointer.
    borrowed: bool,
    /// Live `with_coupled_shadow` frames on this stack. Ambient access fails
    /// closed at 0, so a stale slot can never be dereferenced; restoring the
    /// previous slot on scope exit is what decrements it.
    scope_depth: u32,
}

thread_local! {
    static COUPLE_GENERATION: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
    static COUPLED_SHADOW: std::cell::Cell<CoupledShadowSlot> = const {
        std::cell::Cell::new(CoupledShadowSlot {
            generation: 0,
            ptr: std::ptr::null_mut(),
            borrowed: false,
            scope_depth: 0,
        })
    };
}

/// Clear a leftover couple slot.
///
/// Bails while a scope frame or ambient borrow is live; those restore
/// themselves. With no scope on the stack a non-null pointer is stale, and
/// nulling it is what keeps later ambient access from touching freed memory.
#[inline]
pub fn clear_active_shadow_for_coupled_tick() {
    COUPLED_SHADOW.with(|c| {
        let slot = c.get();
        if slot.borrowed || slot.scope_depth > 0 {
            return;
        }
        c.set(CoupledShadowSlot {
            generation: 0,
            ptr: std::ptr::null_mut(),
            borrowed: false,
            scope_depth: 0,
        });
    });
}

/// Clears the scoped exclusive-borrow marker even if the callback panics.
struct CoupledShadowBorrowGuard {
    generation: u64,
}

impl Drop for CoupledShadowBorrowGuard {
    fn drop(&mut self) {
        COUPLED_SHADOW.with(|c| {
            let mut slot = c.get();
            if slot.generation == self.generation {
                slot.borrowed = false;
                c.set(slot);
            }
        });
    }
}

fn with_coupled_shadow_slot<R>(f: impl FnOnce(&mut GameWorldShadow) -> R) -> Option<R> {
    let (ptr, _borrow_guard) = COUPLED_SHADOW.with(|c| {
        let slot = c.get();
        // No live scope frame means any published pointer is stale; fail
        // closed instead of dereferencing it.
        if slot.scope_depth == 0 || slot.generation == 0 || slot.ptr.is_null() || slot.borrowed {
            return None;
        }
        c.set(CoupledShadowSlot {
            borrowed: true,
            ..slot
        });
        // The scoped borrow marker rejects re-entry and clears on unwind.
        // Only with_coupled_shadow can publish a pointer, and its private guard
        // restores the slot before the exclusive owner borrow ends.
        Some((
            slot.ptr,
            CoupledShadowBorrowGuard {
                generation: slot.generation,
            },
        ))
    })?;
    // SAFETY: the lexical borrow guard above is the sole dynamic access to this
    // generation. The pointer was installed from an owner `&mut GameWorldShadow`
    // whose exclusive borrow outlives the slot in with_coupled_shadow. Its
    // private restore guard cannot be leaked by callers. The borrowed marker
    // prevents concurrent/reentrant access; no reference escapes this callback.
    let shadow = unsafe { &mut *ptr };
    Some(f(shadow))
}

/// Restores the exact pre-scope couple slot when a [`with_coupled_shadow`]
/// scope ends, including during unwinding.  Private and never handed to the
/// scope callback, so a caller cannot `std::mem::forget` it: forget-safety
/// comes from the value never escaping the scope frame.
struct CoupledShadowScopeGuard {
    prev: CoupledShadowSlot,
}

impl Drop for CoupledShadowScopeGuard {
    fn drop(&mut self) {
        // Restoring `prev` also restores the previous scope_depth; the depth
        // lives in the slot, so there is no second channel to forget.
        COUPLED_SHADOW.with(|c| c.set(self.prev));
    }
}

/// Publish `shadow` as the live coupled shadow for the duration of `f` only.
///
/// The slot pointer is installed from `&mut GameWorldShadow` at entry and the exact
/// previous slot is restored at exit (normal return or unwind), so the TLS
/// pointer is dereferenceable only while the owner's exclusive borrow is still
/// alive on this stack frame.  A forgotten guard cannot leave the owner
/// dangling-published because the restore guard is private and never escapes.
///
/// `f` receives no reference: inside the scope, reach the shadow through the
/// ambient accessors (`with_active_shadow`, `with_active_shadow_mut`,
/// `push_coupled_world_mutation`, the `coupled_*` readers), which derive their
/// sole `&mut` inside `with_coupled_shadow_slot` under the `borrowed` marker.
/// Direct exclusive work runs via `with_active_shadow_mut(|s| ...)` inside `f`.
///
/// Nesting policy: supported, with stack discipline.  A nested
/// `with_coupled_shadow` saves the outer slot, publishes its own shadow
/// (ambient access inside resolves to the innermost scope), and the outer slot
/// resumes when the inner scope ends.  Nested ambient *borrows* still fail
/// closed while one is live. The private restore guard cannot escape or be
/// forgotten. Mutable match state should ultimately be passed explicitly;
/// this scoped context is only a migration aid.
pub fn with_coupled_shadow<R>(shadow: &mut GameWorldShadow, f: impl FnOnce() -> R) -> R {
    let prev = COUPLED_SHADOW.with(|c| {
        let prev = c.get();
        let generation = COUPLE_GENERATION.with(|g| {
            let next = g.get().wrapping_add(1).max(1);
            g.set(next);
            next
        });
        // scope_depth lives in the slot: publishing depth+1 here and the
        // scope guard restoring `prev` verbatim is the whole depth protocol.
        c.set(CoupledShadowSlot {
            generation,
            ptr: shadow as *mut GameWorldShadow,
            borrowed: false,
            scope_depth: prev.scope_depth.saturating_add(1),
        });
        prev
    });
    let _scope_guard = CoupledShadowScopeGuard { prev };
    f()
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
    max_health: f32,
) -> Option<u32> {
    if !shadow_coupled_tick_active() || !gameworld_shadow_enabled() {
        return None;
    }
    if !gameworld_construction_authority_enabled() {
        return None;
    }
    with_coupled_shadow_slot(|shadow| {
        use gamelogic::world::WorldMutation;
        use gamelogic::world::entities::EntityId;
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
            e.max_health = max_health.max(1.0);
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
    // A candidate save/map world must not bind IDs into the active shadow or
    // consume its pending bind token.  Its spawn log is isolated and discarded
    // on rollback (or intentionally rebuilt from host after commit).
    if crate::game_logic::staged_world_effects::world_stage_effects_active() {
        return false;
    }
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
    use super::*;

    #[test]
    fn coupled_shadow_has_no_escaped_installation_api() {
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
        assert!(!impl_src.contains("pub fn install_"));
        assert!(!impl_src.contains("pub struct CoupledShadowGuard"));
    }

    #[test]
    fn coupled_shadow_slot_rejects_reentry_and_recovers_after_unwind() {
        let mut shadow = GameWorldShadow::new(4);
        with_coupled_shadow(&mut shadow, || {
            let nested_was_rejected =
                with_coupled_shadow_slot(|_| with_coupled_shadow_slot(|_| ()).is_none());
            assert_eq!(nested_was_rejected, Some(true));

            let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = with_coupled_shadow_slot(|_| panic!("intentional coupled callback panic"));
            }));
            assert!(panicked.is_err());
            assert!(
                with_coupled_shadow_slot(|_| ()).is_some(),
                "the scoped borrow marker must clear during unwinding"
            );
        });
    }

    #[test]
    fn coupled_tick_guard_restores_depth_after_unwind() {
        assert!(!shadow_coupled_tick_active());
        let panicked = std::panic::catch_unwind(|| {
            let _guard = CoupledTickGuard::enter();
            assert!(shadow_coupled_tick_active());
            panic!("intentional coupled frame panic");
        });
        assert!(panicked.is_err());
        assert!(!shadow_coupled_tick_active());
    }

    #[test]
    fn scoped_shadow_publishes_only_inside_scope() {
        clear_active_shadow_for_coupled_tick();
        let mut shadow = GameWorldShadow::new(4);
        let shadow_addr = &mut shadow as *mut GameWorldShadow as usize;
        assert!(with_coupled_shadow_slot(|_| ()).is_none());
        with_coupled_shadow(&mut shadow, || {
            assert_eq!(
                with_coupled_shadow_slot(|s| s as *mut GameWorldShadow as usize),
                Some(shadow_addr)
            );
        });
        assert!(
            with_coupled_shadow_slot(|_| ()).is_none(),
            "slot must be restored after the scope returns"
        );
    }

    #[test]
    fn scoped_shadow_ambient_reads_observe_scope_boundaries() {
        clear_active_shadow_for_coupled_tick();
        let mut shadow = GameWorldShadow::new(4);
        let shadow_addr = &mut shadow as *mut GameWorldShadow as usize;
        let _tick = CoupledTickGuard::enter();
        assert!(with_active_shadow(|_| ()).is_none());
        with_coupled_shadow(&mut shadow, || {
            assert_eq!(
                with_active_shadow(|s| s as *const GameWorldShadow as usize),
                Some(shadow_addr)
            );
            assert_eq!(
                with_active_shadow_mut(|s| s as *mut GameWorldShadow as usize),
                Some(shadow_addr)
            );
        });
        assert!(with_active_shadow(|_| ()).is_none());
        assert!(with_active_shadow_mut(|_| ()).is_none());
    }

    #[test]
    fn scoped_shadow_nesting_follows_stack_discipline() {
        clear_active_shadow_for_coupled_tick();
        let mut outer = GameWorldShadow::new(4);
        let mut inner = GameWorldShadow::new(2);
        let outer_addr = &mut outer as *mut GameWorldShadow as usize;
        let inner_addr = &mut inner as *mut GameWorldShadow as usize;
        with_coupled_shadow(&mut outer, || {
            assert_eq!(
                with_coupled_shadow_slot(|s| s as *mut GameWorldShadow as usize),
                Some(outer_addr)
            );
            with_coupled_shadow(&mut inner, || {
                assert_eq!(
                    with_coupled_shadow_slot(|s| s as *mut GameWorldShadow as usize),
                    Some(inner_addr),
                    "innermost scope must win ambient resolution"
                );
            });
            assert_eq!(
                with_coupled_shadow_slot(|s| s as *mut GameWorldShadow as usize),
                Some(outer_addr),
                "outer scope must resume after the inner scope ends"
            );
        });
        assert!(with_coupled_shadow_slot(|_| ()).is_none());
    }

    #[test]
    fn scoped_shadow_restores_previous_slot_after_unwind() {
        clear_active_shadow_for_coupled_tick();
        let mut shadow = GameWorldShadow::new(4);
        let shadow_addr = &mut shadow as *mut GameWorldShadow as usize;
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            with_coupled_shadow(&mut shadow, || {
                let inner = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _ =
                        with_coupled_shadow_slot(|_| panic!("intentional scoped ambient panic"));
                }));
                assert!(inner.is_err());
                assert_eq!(
                    with_coupled_shadow_slot(|s| s as *mut GameWorldShadow as usize),
                    Some(shadow_addr),
                    "ambient borrow marker must clear and the scope must stay live"
                );
                panic!("intentional scoped frame panic");
            });
        }));
        assert!(panicked.is_err());
        assert!(
            with_coupled_shadow_slot(|_| ()).is_none(),
            "unwinding through the scope must restore the previous slot"
        );
    }

    #[test]
    fn scoped_shadow_early_return_allows_owner_move_and_drop() {
        clear_active_shadow_for_coupled_tick();
        let mut shadow = Box::new(GameWorldShadow::new(4));
        let result = with_coupled_shadow(&mut shadow, || {
            assert!(with_coupled_shadow_slot(|_| ()).is_some());
            17
        });
        assert_eq!(result, 17);
        let moved = *shadow;
        drop(moved);
        assert!(with_coupled_shadow_slot(|_| ()).is_none());
    }

    #[test]
    fn nested_scope_restores_outer_exclusive_borrow_marker() {
        clear_active_shadow_for_coupled_tick();
        let mut outer = GameWorldShadow::new(4);
        let mut inner = GameWorldShadow::new(2);
        with_coupled_shadow(&mut outer, || {
            with_coupled_shadow_slot(|_| {
                with_coupled_shadow(&mut inner, || {
                    assert!(with_coupled_shadow_slot(|_| ()).is_some());
                });
                assert!(with_coupled_shadow_slot(|_| ()).is_none());
            })
            .unwrap();
            assert!(with_coupled_shadow_slot(|_| ()).is_some());
        });
        assert!(with_coupled_shadow_slot(|_| ()).is_none());
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
    /// Source slot associated with the authoritative primary GameWorld weapon
    /// channel.  Presentation uses this only to honor C++ `getRemainingAmmo`
    /// while that slot is `RELOADING_CLIP`.
    pub active_weapon_slot: u8,
    pub weapon_fire_status: u8,
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
            active_weapon_slot: e.active_weapon_slot,
            weapon_fire_status: e.weapon_fire_status,
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

pub fn eager_mark_host_destroy_if_coupled(host: ObjectId) -> bool {
    if !gameworld_deferred_destroy_live() {
        return false;
    }
    with_coupled_shadow_slot(|shadow| {
        if !shadow.queue_destroy_for_host(host) {
            return false;
        }
        let _ = shadow.world_mut().apply_pending_mutations();
        true
    })
    .unwrap_or(false)
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
