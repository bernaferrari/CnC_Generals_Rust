//! Per-id generation for `EntityId` resolution.
//!
//! C++ `ObjectID` is monotonic and never reused in a session
//! (`GameLogic::allocateObjectID`, `GameLogic.cpp:3816-3821`;
//! `findObjectByID` is a vector slot or NULL, `GameLogic.h:386-400`).
//! Rust keeps `EntityId` as that wire type. The store tracks a `u32`
//! generation bumped on remove so a mutation aimed at a removed-then-reused
//! id fail-closes instead of writing the new occupant.

use super::entities::{Entity, EntityId, EntityStore, TemplateRef, Transform};
use super::{PlayerId, WorldMutation};

/// Resolution handle. Not the wire type — `EntityId` stays the saved id.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EntityHandle {
    id: EntityId,
    generation: u32,
}

impl EntityHandle {
    pub const fn new(id: EntityId, generation: u32) -> Self {
        Self { id, generation }
    }

    pub const fn id(self) -> EntityId {
        self.id
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

/// Queued mutation plus the generation captured at enqueue.
#[derive(Debug, Clone)]
pub(crate) struct GuardedMutation {
    pub mutation: WorldMutation,
    pub guard: Option<EntityHandle>,
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
            .map(|generation| EntityHandle::new(id, generation))
    }

    pub fn resolve(&self, handle: EntityHandle) -> Option<&Entity> {
        let live = self.live_generation(handle.id)?;
        if live != handle.generation {
            return None;
        }
        self.alive.get(&handle.id)
    }

    pub fn resolve_mut(&mut self, handle: EntityHandle) -> Option<&mut Entity> {
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

pub(crate) fn guard_for(store: &EntityStore, mutation: &WorldMutation) -> Option<EntityHandle> {
    store.handle_of(primary_entity_id(mutation)?)
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
}
