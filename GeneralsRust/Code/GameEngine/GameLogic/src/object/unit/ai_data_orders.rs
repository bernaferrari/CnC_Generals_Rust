//! Command metadata, guard targets, mood timers, and AI status.

use super::ai_data::UnitAiData;
use super::imports::*;

impl UnitAiData {
    pub(super) fn push_guard_target_type(&mut self, target_type: GuardTargetType) {
        if self.guard_target_type[1] == GuardTargetType::None_ {
            self.guard_target_type[1] = target_type;
        } else {
            self.guard_target_type[0] = target_type;
        }
    }
    pub(super) fn clear_guard_target_type(&mut self) {
        self.guard_target_type[1] = self.guard_target_type[0];
        self.guard_target_type[0] = GuardTargetType::None_;
    }

    pub(super) fn get_enter_target(&self) -> Option<ObjectID> {
        self.enter_target
    }
    pub(super) fn take_random_mood_offset(&mut self) -> bool {
        let set = self.randomly_offset_mood_check;
        self.randomly_offset_mood_check = false;
        set
    }
    pub(super) fn set_attitude(
        &mut self,
        attitude: AIAttitudeType,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        self.attitude = attitude;
        Ok(())
    }
    pub(super) fn get_attitude(&self) -> AIAttitudeType {
        self.attitude
    }

    pub(super) fn get_last_command_source(&self) -> CommandSourceType {
        self.last_command_source
    }
    pub(super) fn set_last_command_source(&mut self, source: CommandSourceType) {
        self.last_command_source = source;
    }
    pub(super) fn get_current_command(&self) -> Option<crate::ai::AiCommandType> {
        self.current_command
    }

    pub(super) fn is_in_rappel_state(&self) -> bool {
        self.rappel_state.is_some()
    }

    pub(super) fn is_ai_in_dead_state(&self) -> bool {
        self.ai_dead
    }
    pub(super) fn set_is_recruitable(&mut self, recruitable: Bool) {
        self.is_recruitable = recruitable;
    }

    pub(super) fn is_surrendered(&self) -> bool {
        self.surrendered_frames_left > 0
    }
    pub(super) fn get_surrendered_player_index(&self) -> Option<PlayerIndex> {
        self.surrendered_player_index
    }
    pub(super) fn get_next_mood_check_time(&self) -> u32 {
        self.next_mood_check_time
    }
    pub(super) fn set_next_mood_check_time(&mut self, frame: u32) {
        self.next_mood_check_time = frame;
        self.randomly_offset_mood_check = false;
    }

    pub(super) fn get_original_victim_pos(&self) -> Option<Coord3D> {
        self.original_victim_pos
    }
    pub(super) fn set_original_victim_pos(&mut self, pos: Option<Coord3D>) {
        self.original_victim_pos = pos;
    }

    pub(super) fn is_recruitable(&self) -> bool {
        self.is_recruitable
    }
}
