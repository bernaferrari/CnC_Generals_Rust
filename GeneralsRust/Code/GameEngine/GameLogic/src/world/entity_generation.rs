//! Per-id generation for `EntityId` resolution.
//!
//! C++ `ObjectID` is monotonic and never reused in a session
//! (`GameLogic::allocateObjectID`, `GameLogic.cpp:3816-3821`;
//! `findObjectByID` is a vector slot or NULL, `GameLogic.h:386-400`).
//! Rust keeps `EntityId` as that wire type. The store tracks a `u32`
//! generation bumped on remove so a mutation aimed at a removed-then-reused
//! id fail-closes instead of writing the new occupant. Each store also
//! carries a world epoch so a handle minted in one world cannot resolve as
//! an unrelated object in another world that happens to share id+generation
//! (ids are monotonic in production — `spawn_at` reuse is test/restore only
//! — so this is defense-in-depth, never serialized: the epoch is runtime
//! safety metadata and stays out of every snapshot/envelope).

use super::entities::{Entity, EntityId, EntityStore, TemplateRef, Transform};
use super::{PlayerId, WorldMutation};
use std::sync::atomic::{AtomicU32, Ordering};

/// Monotonic counter backing `next_world_epoch`.
static WORLD_EPOCH_COUNTER: AtomicU32 = AtomicU32::new(0);

/// Fresh epoch for a newly constructed `EntityStore`. First call returns 1;
/// 0 stays reserved as "legacy/unassigned" for `EntityHandle::new`.
pub(in crate::world) fn next_world_epoch() -> u32 {
    WORLD_EPOCH_COUNTER.fetch_add(1, Ordering::Relaxed) + 1
}

/// Resolution handle. Not the wire type — `EntityId` stays the saved id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityHandle {
    id: EntityId,
    generation: u32,
    /// World epoch of the store that minted this handle. 0 = legacy
    /// (minted via `new`, i.e. from wire data that never carried an epoch).
    epoch: u32,
}

impl EntityHandle {
    /// Legacy bridge: mints an epoch-0 handle that resolves in any store.
    /// Kept `const` and signature-stable for snapshot restore in Main
    /// (`lifecycle_tail.rs`), which rebuilds handles from wire data with no
    /// epoch to carry. Live code should mint via `EntityStore::handle_of`.
    pub const fn new(id: EntityId, generation: u32) -> Self {
        Self {
            id,
            generation,
            epoch: 0,
        }
    }

    /// Mint a handle bound to a specific store's world epoch.
    pub const fn for_world(id: EntityId, generation: u32, epoch: u32) -> Self {
        Self {
            id,
            generation,
            epoch,
        }
    }

    pub const fn id(self) -> EntityId {
        self.id
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    /// World epoch this handle was minted under (0 = legacy bridge).
    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    /// True when both handles were minted under the same world epoch.
    pub fn same_world(&self, other: &EntityHandle) -> bool {
        self.epoch == other.epoch
    }
}

/// Guard contract for a queued mutation, captured at enqueue time.
///
/// C++ `ObjectID`s are never reused (`GameLogic::allocateObjectID`,
/// `GameLogic.cpp:3816-3821`), so a mutation there always targets an
/// unambiguous object. The Rust store can reuse an id via the restore path
/// (`EntityStore::spawn_at`), so the guard must record which of three
/// situations held at enqueue instead of conflating them in an `Option`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MutationGuard {
    /// Mutation has no primary entity (spawn / player / projectile / AI
    /// channels). The apply loop validates nothing.
    Entityless,
    /// Primary entity was alive at enqueue. The apply loop resolves the
    /// handle and skips the mutation once that exact occupant is gone.
    Primary(EntityHandle),
    /// Mutation needs its primary but the id was not live at enqueue.
    /// `next_generation` is the generation a NEW occupant of the id would
    /// receive (`EntityStore::peek_next_generation`, no allocation): the
    /// apply loop proceeds only for an occupant created at/after it, so a
    /// pre-enqueue occupant can never be hit through id reuse.
    PrimaryAbsentAtEnqueue { id: EntityId, next_generation: u32 },
}

/// Queued mutation plus the guard captured at enqueue.
#[derive(Debug, Clone)]
pub(crate) struct GuardedMutation {
    pub mutation: WorldMutation,
    pub guard: MutationGuard,
}

impl EntityStore {
    /// Live generation if `id` is currently occupied.
    pub fn live_generation(&self, id: EntityId) -> Option<u32> {
        if !self.alive.contains_key(&id) {
            return None;
        }
        self.generations.get(&id.get()).copied()
    }

    pub fn handle_of(&self, id: EntityId) -> Option<EntityHandle> {
        self.live_generation(id)
            .map(|generation| EntityHandle::for_world(id, generation, self.world_epoch))
    }

    /// Epoch gate shared by `resolve`/`resolve_mut`: a handle resolves only
    /// in the store whose epoch minted it, EXCEPT epoch-0 legacy handles
    /// (the `EntityHandle::new` bridge above), which keep resolving in any
    /// store exactly as before the epoch existed.
    fn accepts_handle_epoch(&self, handle: EntityHandle) -> bool {
        handle.epoch == 0 || handle.epoch == self.world_epoch
    }

    pub fn resolve(&self, handle: EntityHandle) -> Option<&Entity> {
        if !self.accepts_handle_epoch(handle) {
            return None;
        }
        let live = self.live_generation(handle.id)?;
        if live != handle.generation {
            return None;
        }
        self.alive.get(&handle.id)
    }

    pub fn resolve_mut(&mut self, handle: EntityHandle) -> Option<&mut Entity> {
        if !self.accepts_handle_epoch(handle) {
            return None;
        }
        let live = self.live_generation(handle.id)?;
        if live != handle.generation {
            return None;
        }
        self.alive.get_mut(&handle.id)
    }

    /// Restore / reuse path. Fails closed if `id` is 0 or still live.
    pub fn spawn_at(
        &mut self,
        id: EntityId,
        template: TemplateRef,
        owner: Option<PlayerId>,
        transform: Transform,
        health: f32,
    ) -> Option<EntityId> {
        if id.get() < EntityId::FIRST.get() || self.alive.contains_key(&id) {
            return None;
        }
        if id.get() >= self.next_id {
            self.next_id = id.get().wrapping_add(1).max(EntityId::FIRST.get());
        }
        self.finish_spawn(id, template, owner, transform, health);
        Some(id)
    }

    /// Generation a NEW occupant of `id` would receive, without allocating
    /// it: the same value `allocate_live_generation` hands the next spawn
    /// (the stored counter, or 1 for an id the store has never seen).
    /// `bump_generation` on remove is what advances it between occupants.
    pub(crate) fn peek_next_generation(&self, id: EntityId) -> u32 {
        self.generations.get(&id.get()).copied().unwrap_or(1)
    }

    pub(crate) fn allocate_live_generation(&mut self, id: EntityId) -> u32 {
        let raw = id.get();
        let generation = self.generations.get(&raw).copied().unwrap_or(1);
        self.generations.insert(raw, generation);
        generation
    }

    pub(crate) fn bump_generation(&mut self, id: EntityId) {
        let raw = id.get();
        let next = self
            .generations
            .get(&raw)
            .copied()
            .unwrap_or(1)
            .saturating_add(1);
        self.generations.insert(raw, next);
    }
}

pub(crate) fn guard_for(store: &EntityStore, mutation: &WorldMutation) -> MutationGuard {
    match primary_entity_id(mutation) {
        None => MutationGuard::Entityless,
        Some(id) => match store.handle_of(id) {
            Some(handle) => MutationGuard::Primary(handle),
            None => MutationGuard::PrimaryAbsentAtEnqueue {
                id,
                next_generation: store.peek_next_generation(id),
            },
        },
    }
}

fn primary_entity_id(mutation: &WorldMutation) -> Option<EntityId> {
    match mutation {
        WorldMutation::Destroy(id) => Some(*id),
        WorldMutation::TransferOwner { object, .. } => Some(*object),
        WorldMutation::SetAttackTarget { attacker, .. } => Some(*attacker),
        WorldMutation::SetMoveTarget { unit, .. }
        | WorldMutation::SetTargetLocation { unit, .. }
        | WorldMutation::SetGuard { unit, .. }
        | WorldMutation::SetRallyPoint { unit, .. } => Some(*unit),
        WorldMutation::ContainEnter { container, .. }
        | WorldMutation::ContainExit { container, .. } => Some(*container),
        WorldMutation::Damage { target, .. }
        | WorldMutation::SetHealth { target, .. }
        | WorldMutation::SetMaxHealth { target, .. }
        | WorldMutation::SetBodyDamage { target, .. }
        | WorldMutation::SetDeathType { target, .. }
        | WorldMutation::SetRadarExtend { target, .. }
        | WorldMutation::SetShockStun { target, .. }
        | WorldMutation::SetPhysicsMotive { target, .. }
        | WorldMutation::SetLocomotor { target, .. }
        | WorldMutation::SetBounceLand { target, .. }
        | WorldMutation::SetTransform { target, .. }
        | WorldMutation::SetCombatStatus { target, .. }
        | WorldMutation::SetVeterancy { target, .. }
        | WorldMutation::SetExperience { target, .. }
        | WorldMutation::SetWeaponBonus { target, .. }
        | WorldMutation::SetActiveWeaponSlot { target, .. }
        | WorldMutation::SetEntityPower { target, .. }
        | WorldMutation::SetTurret { target, .. }
        | WorldMutation::SetDetector { target, .. }
        | WorldMutation::SetContinuousFire { target, .. }
        | WorldMutation::SetCombatAttack { target, .. }
        | WorldMutation::SetFaerieFire { target, .. }
        | WorldMutation::SetRepulsor { target, .. }
        | WorldMutation::SetDisableTimers { target, .. }
        | WorldMutation::SetAiAttitude { target, .. }
        | WorldMutation::SetAiMood { target, .. }
        | WorldMutation::SetAiRequest { target, .. }
        | WorldMutation::SetWeaponSetFlags { target, .. }
        | WorldMutation::SetOvercharge { target, .. }
        | WorldMutation::SetContainCapacity { target, .. }
        | WorldMutation::SetHiveSlaves { target, .. }
        | WorldMutation::SetHijacker { target, .. }
        | WorldMutation::SetStealthFlags { target, .. }
        | WorldMutation::SetStealthDelay { target, .. }
        | WorldMutation::SetOverlordAddon { target, .. }
        | WorldMutation::SetCommandSet { target, .. }
        | WorldMutation::SetDisguise { target, .. }
        | WorldMutation::SetVisionCamo { target, .. }
        | WorldMutation::SetWeaponSlot { target, .. }
        | WorldMutation::SetWeaponStats { target, .. }
        | WorldMutation::SetFireIntent { target, .. }
        | WorldMutation::SetMovement { target, .. }
        | WorldMutation::SetSelectionRadius { target, .. }
        | WorldMutation::SetModelCondition { target, .. }
        | WorldMutation::SetDemoMineCheer { target, .. }
        | WorldMutation::SetFormation { target, .. }
        | WorldMutation::SetCrushVision { target, .. }
        | WorldMutation::SetBuildingType { target, .. }
        | WorldMutation::SetIdentity { target, .. }
        | WorldMutation::SetGroundHeight { target, .. }
        | WorldMutation::SetModelMesh { target, .. }
        | WorldMutation::SetFow { target, .. }
        | WorldMutation::SetKindOfBits { target, .. }
        | WorldMutation::SetProductionQueue { target, .. }
        | WorldMutation::SetExitDelay { target, .. }
        | WorldMutation::SetProductionExitRuntime { target, .. }
        | WorldMutation::SetProductionDoor { target, .. }
        | WorldMutation::SetConstruction { target, .. }
        | WorldMutation::SetRebuildProducer { target, .. }
        | WorldMutation::SetSoleHealing { target, .. }
        | WorldMutation::SetSpecialPower { target, .. }
        | WorldMutation::SetStoredSupplies { target, .. }
        | WorldMutation::SetAiState { target, .. }
        | WorldMutation::SetContain { target, .. } => Some(*target),
        WorldMutation::Spawn { .. }
        | WorldMutation::SetSupplies { .. }
        | WorldMutation::SetPower { .. }
        | WorldMutation::CompleteUpgrade { .. }
        | WorldMutation::SetProjectileFlight { .. }
        | WorldMutation::PushAiDecision { .. }
        | WorldMutation::SetPlayerRadar { .. }
        | WorldMutation::SetPlayerProgress { .. }
        | WorldMutation::SetPlayerSciences { .. }
        | WorldMutation::SetPlayerAlive { .. }
        | WorldMutation::SetPlayerCooldowns { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::GameWorld;
    use crate::world::entities::Transform;

    fn spawn_store() -> (EntityStore, EntityId) {
        let mut store = EntityStore::new();
        let id = store.spawn(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        (store, id)
    }

    #[test]
    fn live_handle_resolves_and_remove_invalidates() {
        let (mut store, id) = spawn_store();
        let handle = store.handle_of(id).expect("live");
        assert_eq!(handle.generation(), 1);
        assert!(store.resolve(handle).is_some());
        assert!(store.remove(id).is_some());
        assert!(store.resolve(handle).is_none());
        assert!(store.live_generation(id).is_none());
    }

    #[test]
    fn reused_id_rejects_stale_generation() {
        let (mut store, id) = spawn_store();
        let stale = store.handle_of(id).expect("live");
        assert!(store.remove(id).is_some());
        let reused = store
            .spawn_at(
                id,
                TemplateRef::new("Tank"),
                None,
                Transform::default(),
                80.0,
            )
            .expect("reuse");
        assert_eq!(reused, id);
        let fresh = store.handle_of(id).expect("reused live");
        assert_ne!(fresh.generation(), stale.generation());
        assert!(store.resolve(stale).is_none());
        assert!(store.resolve(fresh).is_some());
        assert!(
            store
                .spawn_at(id, TemplateRef::new("Dup"), None, Transform::default(), 1.0)
                .is_none()
        );
    }

    #[test]
    fn mutation_against_reused_generation_fails_closed() {
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        world.queue_mutation(WorldMutation::SetHealth {
            target: id,
            health: 1.0,
        });
        assert!(world.world_mut().remove_entity(id));
        let reused = world
            .spawn_entity_at(
                id,
                TemplateRef::new("Tank"),
                None,
                Transform::default(),
                80.0,
            )
            .expect("reuse");
        assert_eq!(reused, id);
        assert_eq!(world.apply_pending_mutations(), 0);
        assert_eq!(world.entity(id).expect("reused").health, 80.0);
    }

    #[test]
    fn guard_for_classifies_entityless_primary_and_absent() {
        let (mut store, id) = spawn_store();
        let entityless = WorldMutation::Spawn {
            template: "Ranger".to_string(),
            owner: None,
            position: [0.0, 0.0, 0.0],
            health: 100.0,
        };
        assert!(matches!(
            guard_for(&store, &entityless),
            MutationGuard::Entityless
        ));
        let needs_entity = WorldMutation::SetHealth {
            target: id,
            health: 1.0,
        };
        assert!(matches!(
            guard_for(&store, &needs_entity),
            MutationGuard::Primary(_)
        ));
        assert!(store.remove(id).is_some());
        match guard_for(&store, &needs_entity) {
            MutationGuard::PrimaryAbsentAtEnqueue {
                id: absent_id,
                next_generation,
            } => {
                assert_eq!(absent_id, id);
                // Remove bumped the counter to 2; a new occupant gets 2.
                assert_eq!(next_generation, 2);
            }
            other => panic!("unexpected guard: {other:?}"),
        }
    }

    #[test]
    fn peek_next_generation_matches_what_reuse_allocates() {
        let (mut store, id) = spawn_store();
        assert_eq!(store.peek_next_generation(EntityId::from_raw(9999)), 1);
        assert!(store.remove(id).is_some());
        let peeked = store.peek_next_generation(id);
        assert_eq!(peeked, 2);
        assert!(store
            .spawn_at(
                id,
                TemplateRef::new("Tank"),
                None,
                Transform::default(),
                80.0
            )
            .is_some());
        assert_eq!(store.live_generation(id), Some(peeked));
    }

    #[test]
    fn absent_at_enqueue_mutation_applies_to_fresh_occupant_only() {
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        assert!(world.world_mut().remove_entity(id));
        // Queued while the id is dead: the removed occupant is gone, but a
        // NEW occupant created after enqueue is a legitimate target.
        world.queue_mutation(WorldMutation::SetHealth {
            target: id,
            health: 7.0,
        });
        let reused = world
            .spawn_entity_at(
                id,
                TemplateRef::new("Tank"),
                None,
                Transform::default(),
                80.0,
            )
            .expect("reuse");
        assert_eq!(reused, id);
        assert_eq!(world.apply_pending_mutations(), 1);
        assert_eq!(world.entity(id).expect("reused").health, 7.0);
    }

    #[test]
    fn absent_at_enqueue_mutation_skips_occupant_older_than_recorded_generation() {
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        // The live counter only grows, so the public flow cannot enqueue this
        // state today; a restore path rolling generations forward under a
        // still-live occupant must fail closed instead of writing it.
        world.pending.push(GuardedMutation {
            mutation: WorldMutation::SetHealth {
                target: id,
                health: 1.0,
            },
            guard: MutationGuard::PrimaryAbsentAtEnqueue {
                id,
                next_generation: 2,
            },
        });
        assert_eq!(world.apply_pending_mutations(), 0);
        assert_eq!(world.entity(id).expect("live").health, 100.0);
    }

    #[test]
    fn same_batch_spawn_then_mutate_applies() {
        let mut world = GameWorld::new(1);
        // Spawn is entityless; a needs-entity mutation queued for the id the
        // Spawn will create must apply to the fresh occupant (floor 1 on an
        // id the store has never seen).
        world.queue_mutation(WorldMutation::Spawn {
            template: "Ranger".to_string(),
            owner: None,
            position: [0.0, 0.0, 0.0],
            health: 100.0,
        });
        let target = EntityId::FIRST;
        world.queue_mutation(WorldMutation::SetHealth {
            target,
            health: 42.0,
        });
        assert_eq!(world.apply_pending_mutations(), 2);
        assert_eq!(world.entity(target).expect("spawned").health, 42.0);
        assert_eq!(world.take_last_spawned_entity(), Some(target));
    }

    #[test]
    fn entityless_mutation_applies_with_empty_store() {
        let mut world = GameWorld::new(1);
        world.queue_mutation(WorldMutation::Spawn {
            template: "Ranger".to_string(),
            owner: None,
            position: [0.0, 0.0, 0.0],
            health: 100.0,
        });
        assert_eq!(world.apply_pending_mutations(), 1);
        assert_eq!(world.entity(EntityId::FIRST).expect("spawned").health, 100.0);
    }

    #[test]
    fn cross_world_handle_does_not_resolve_in_foreign_store() {
        // Audit acceptance: a handle from world A must not resolve as an
        // unrelated object in world B, even when id AND generation collide.
        let mut world_a = GameWorld::new(1);
        let mut world_b = GameWorld::new(1);
        assert_ne!(world_a.entity_world_epoch(), world_b.entity_world_epoch());
        let id = EntityId::from_raw(777);
        assert_eq!(
            world_a.spawn_entity_at(
                id,
                TemplateRef::new("Ranger"),
                None,
                Transform::default(),
                100.0
            ),
            Some(id)
        );
        assert_eq!(
            world_b.spawn_entity_at(
                id,
                TemplateRef::new("Tank"),
                None,
                Transform::default(),
                100.0
            ),
            Some(id)
        );
        let handle_a = world_a.entity_handle(id).expect("live in A");
        let handle_b = world_b.entity_handle(id).expect("live in B");
        assert_eq!(handle_a.generation(), handle_b.generation());
        assert!(!handle_a.same_world(&handle_b));
        assert!(world_b.resolve_entity(handle_a).is_none());
        // And symmetrically, B's handle does not resolve in A.
        assert!(world_a.resolve_entity(handle_b).is_none());
    }

    #[test]
    fn same_world_resolution_still_works() {
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        let handle = world.entity_handle(id).expect("live");
        assert!(handle.same_world(&handle));
        assert_eq!(
            world.resolve_entity(handle).map(|e| e.id),
            Some(id)
        );
        assert!(world.world_mut().remove_entity(id));
        assert!(world.resolve_entity(handle).is_none());
    }

    #[test]
    fn legacy_epoch_zero_handle_still_resolves() {
        // `EntityHandle::new` mints epoch 0 (legacy bridge for snapshot
        // restore in Main, whose wire data never carried an epoch); such
        // handles keep resolving in any live store as before.
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        let live = world.entity_handle(id).expect("live");
        let legacy = EntityHandle::new(id, live.generation());
        assert_eq!(legacy.epoch(), 0);
        assert!(world.resolve_entity(legacy).is_some());
    }

    #[test]
    fn queued_mutation_with_live_primary_applies() {
        // Guard loop unaffected by the epoch: a Primary guard minted via
        // handle_of carries this world's epoch and still resolves at apply.
        let mut world = GameWorld::new(1);
        let id = world.spawn_entity(
            TemplateRef::new("Ranger"),
            None,
            Transform::default(),
            100.0,
        );
        world.queue_mutation(WorldMutation::SetHealth {
            target: id,
            health: 42.0,
        });
        assert_eq!(world.apply_pending_mutations(), 1);
        assert_eq!(world.entity(id).expect("live").health, 42.0);
    }
}
