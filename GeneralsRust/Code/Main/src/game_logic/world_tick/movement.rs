//! Host tick `impl GameLogic` — `movement`.
#![allow(unused_imports, non_snake_case)]
use super::super::*;
impl GameLogic {
    /// Update movement for all objects
    pub(in super::super) fn update_movement(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
    ) -> gamelogic::GameLogicResult<()> {
        self.update_movement_with_effects(object_ids, dt, &mut Default::default())
    }

    pub(in super::super) fn update_movement_with_effects(
        &mut self,
        object_ids: &[ObjectId],
        dt: f32,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        // Receiving projectile actors are advanced by the projectile phase.
        let projectiles = self.combat_system.get_projectiles();
        let is_owned_projectile = |id: &ObjectId| {
            projectiles.get(id).is_some_and(|shot| {
                shot.projectile_actor_birth.is_some_and(|birth| {
                    self.spawn_identity_matches(*id, birth.0, birth.1)
                })
            })
        };
        let filtered;
        let object_ids = if projectiles.values().any(|shot| shot.projectile_actor_birth.is_some()) {
            filtered = object_ids.iter().copied().filter(|id| !is_owned_projectile(id)).collect::<Vec<_>>();
            filtered.as_slice()
        } else {
            object_ids
        };
        for &id in object_ids {
            if self.objects.lookup(&id).is_some_and(|unit| {
                unit.pending_quick_path_goal
                    && !unit.waiting_for_path
                    && !unit.is_disabled()
                    && !unit.host_skip_dead_locomotor()
                    && !unit.is_kind_of(KindOf::Immobile)
            }) {
                self.on_waited_path_arrived(id);
            }
        }
        self.refresh_movement_goal_objects(object_ids);
        self.repath_airborne_projectiles(object_ids);
        self.repath_if_move_goal_moved(object_ids);
        self.update_movement_locomotor_pass_with_effects(object_ids, dt, effects)
    }

    #[cfg(test)]
    pub fn drain_pending_transport_exits_for_test(&mut self) {
        self.drain_pending_transport_exits().expect("transport exit succeeds");
    }

    #[cfg(test)]
    pub fn update_movement_for_test(&mut self, object_ids: &[ObjectId], dt: f32) {
        self.with_own_services_mut(|this| {
            this.update_movement(object_ids, dt)
                .expect("movement owner fixture remains valid");
        })
    }

    #[cfg(test)]
    pub fn move_object_with_pathfinding_for_test(
        &mut self,
        object_id: ObjectId,
        target_position: Vec3,
        ai_state_override: Option<AIState>,
    ) {
        self.move_object_with_pathfinding(object_id, target_position, ai_state_override);
    }
}

#[cfg(test)]
#[path = "movement_fixtures.rs"]
mod fixtures;

#[cfg(test)]
#[path = "movement_tests.rs"]
mod tests;
