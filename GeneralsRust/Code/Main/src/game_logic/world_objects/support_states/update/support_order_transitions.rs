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
    ) {
        let Some(center_id) =
            self.preferred_or_allied_supply_center(object_id, team, owner_player_id, position)
        else {
            return;
        };
        self.set_ai_state_decision_aware(object_id, AIState::ReturningResources);
        let _ = self.try_claim_dock(center_id, object_id);
    }

    pub(super) fn release_dozer_repair_order(&mut self, object_id: ObjectId) {
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
        self.set_ai_state_decision_aware(object_id, AIState::Idle);
        {}
        self.dozer_internal_task_complete(object_id, true);
        let _ = self.dozer_idle_resume_pending_build(object_id);
    }

    pub(super) fn release_rejected_special(&mut self, object_id: ObjectId) {
        self.pending_special_abilities.remove(&object_id);
        if let Some(obj) = self.objects.get_mut(&object_id) {
            obj.stop_moving(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            obj.ignored_obstacle_id = None;
            obj.clear_target(&mut crate::game_logic::ObjectAudio::queue(
                &mut self.queued_audio_events,
            ));
            if obj.ai_state != AIState::Idle {
                obj.set_ai_state(
                    AIState::Idle,
                    &mut crate::game_logic::ObjectAudio::queue(&mut self.queued_audio_events),
                );
            }
        }
    }
}
