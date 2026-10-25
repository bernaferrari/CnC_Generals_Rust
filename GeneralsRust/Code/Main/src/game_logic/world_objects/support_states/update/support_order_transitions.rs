//! C++ supply return, dozer repair release and rejected special-order transitions.
use super::*;

impl GameLogic {
    /// C++ `aiDock(bestCenter)` — reserve a DockUpdate approach point.
    /// Do not path at the building center and do not ignore the structure.
    pub(in super::super) fn begin_return_to_supply_center(
        &mut self,
        object_id: ObjectId,
        team: Team,
        owner_player_id: Option<u32>,
        position: glam::Vec3,
    ) -> gamelogic::GameLogicResult<()> {
        let Some(center_id) =
            self.preferred_or_allied_supply_center(object_id, team, owner_player_id, position)
        else {
            return Ok(());
        };
        self.set_ai_state_decision_aware_for_ai(object_id, AIState::ReturningResources)?;
        let _claimed = self.try_claim_dock(center_id, object_id)?;
        Ok(())
    }

    pub(super) fn release_dozer_repair_order(
        &mut self,
        object_id: ObjectId,
    ) -> gamelogic::GameLogicResult<()> {
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.ignored_obstacle_id = None;
            obj.target = None;
            obj.target_location = None;

            obj.set_status_force_attack(false);
            obj.set_status_attacking(false);
            obj.set_actively_constructing(false);
        }
        self.set_ai_state_decision_aware_for_ai(object_id, AIState::Idle)?;
        {}
        self.dozer_internal_task_complete(object_id, true);
        Ok(())
    }

    pub(super) fn release_rejected_special(
        &mut self,
        object_id: ObjectId,
        effects: &mut crate::game_logic::ScriptPresentationEffects<'_>,
    ) -> gamelogic::GameLogicResult<()> {
        self.pending_special_abilities.remove(&object_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.ignored_obstacle_id = None;
            obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
        }
        self.set_ai_state_decision_aware_with_effects(object_id, AIState::Idle, effects)?;
        Ok(())
    }
}
