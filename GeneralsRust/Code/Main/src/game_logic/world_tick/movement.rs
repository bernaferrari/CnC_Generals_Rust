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
        self.refresh_movement_goal_objects(object_ids);
        self.repath_airborne_projectiles(object_ids);
        self.repath_if_move_goal_moved(object_ids);
        self.update_movement_locomotor_pass_with_effects(object_ids, dt, effects)
    }

    #[cfg(test)]
    pub fn drain_pending_transport_exits_for_test(&mut self) {
        self.drain_pending_transport_exits();
    }

    #[cfg(test)]
    pub fn update_movement_for_test(&mut self, object_ids: &[ObjectId], dt: f32) {
        self.update_movement(object_ids, dt)
            .expect("movement owner fixture remains valid");
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
