//! Source state exits; these operations never enter another state.
use super::*;
impl Object {
    pub(crate) fn exit_base_ai_move_owned(
        &mut self,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        let goal = self.unit_ai_runtime.machine().normal_move_goal;
        self.exit_base_ai_move_at_goal_owned(goal, audio);
    }
    pub(crate) fn exit_base_ai_move_at_goal_owned(
        &mut self,
        goal: Option<Vec3>,
        audio: &mut crate::game_logic::ObjectAudio<'_>,
    ) {
        // AIInternalMoveToState::onExit: sound, endingMove, accurate final pose.
        audio.stop_loop(self);
        self.set_status_moving(false);
        if self.ultra_accurate
            && crate::game_logic::PathfindingGrid::is_doing_ground_movement_full(self)
        {
            if let Some(goal) = goal {
                let delta = goal - self.get_position();
                let cell = crate::game_logic::PATHFIND_CELL_SIZE_F_RESIDUAL;
                if delta.x * delta.x + delta.z * delta.z < cell * cell {
                    self.final_position = goal;
                    self.do_final_position = false;
                }
            }
        }
    }
    pub(crate) fn exit_base_ai_attack_owned(&mut self) {
        // AIAttackState::onExit. Do not use stop_attack: that enters a new state.
        self.set_status_firing_weapon(false);
        self.set_status_aiming_weapon(false);
        self.set_status_attacking(false);
        self.model_condition_bits &=
            !(1u128 << crate::game_logic::host_enum_table_residual::MC_BIT_ATTACKING);
        self.set_status_ignoring_stealth(false);
        self.clear_leech_range_mode_for_all_weapons();
        self.target = None;
        self.target_location = None;
        self.set_turret_target_object(None, false);
        self.unit_ai_runtime.set_machine_goal_object(None);
    }
    pub(crate) fn exit_base_ai_guard_owned(&mut self) {
        // Child lifetime is ended before the parent clears its discriminator.
        self.unit_ai_runtime.clear_guard();
        self.unit_ai_runtime.exit_guard_target_type();
    }
    pub(crate) fn clear_base_ai_control_after_exit(&mut self) {
        self.unit_ai_runtime.clear_machine_after_exit();
    }
    pub(crate) fn enter_base_ai_idle_control(&mut self, frame: u32, offset: u16) {
        self.idle_mood_reset_pending = false;
        self.unit_ai_runtime.enter_idle(frame, offset);
    }
}
