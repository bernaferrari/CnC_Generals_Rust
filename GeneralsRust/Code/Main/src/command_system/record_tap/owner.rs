//! Receiving replay file and command phases; no Core publication or singleton lookup.
use super::*;
use game_engine::common::recorder::{RecorderOwnerEffects, RecorderOwnerInputs, ReplayHeader};

impl ReplayPendingState {
    pub(crate) fn recorder_owned(&self) -> &game_engine::common::recorder::Recorder {
        &self.recorder
    }

    /// Engine reset/save commit only. GameLogic transient clear deliberately
    /// leaves the admitted file/reader alive, as C++ GameLogic::reset does.
    pub(crate) fn reset_recorder_owned(&mut self, inputs: RecorderOwnerInputs) {
        self.clear();
        self.recorder.reset_owned(inputs);
    }

    /// GameLogic.cpp2010: after preload/default camera, before start-position focus.
    pub(crate) fn initialize_replay_controls_owned(&mut self) -> bool {
        self.recorder.init_controls();
        self.recorder
            .take_controls_visibility_owned()
            .expect("explicit Recorder controls effect")
    }

    pub(crate) fn take_recorder_effects_owned(&mut self) -> RecorderOwnerEffects {
        self.recorder.take_owner_effects()
    }

    pub(crate) fn take_lifecycle_messages_owned(&mut self) -> Vec<GameMessage> {
        std::mem::take(&mut self.lifecycle_messages)
    }

    /// Worker-local reader moves with GameLogic through StartupLoadResult.
    pub(crate) fn playback_file_owned(
        &mut self,
        filename: String,
        in_game: bool,
    ) -> std::io::Result<bool> {
        let admitted = self.recorder.playback_file_owned(filename, in_game);
        let output = self.recorder.drain_pending_commands();
        apply_replay_messages(self, &output, false);
        admitted
    }

    pub(crate) fn load_replay_header_owned(
        &mut self,
        filename: String,
    ) -> std::io::Result<ReplayHeader> {
        self.recorder.load_replay_header(filename)
    }

    pub(crate) fn prepare_playback_owned(
        &mut self,
        filename: String,
        in_game: bool,
    ) -> game_engine::common::recorder::RecorderPlaybackAdmission {
        self.recorder.prepare_playback_owned(filename, in_game)
    }

    pub(crate) fn admit_prepared_playback_owned(
        &mut self,
        admission: game_engine::common::recorder::RecorderPlaybackAdmission,
    ) -> std::io::Result<bool> {
        let result = self.recorder.admit_prepared_playback_owned(admission);
        let output = self.recorder.drain_pending_commands();
        apply_replay_messages(self, &output, false);
        result
    }

    pub(crate) fn reset_recorder_for_match_owned(&mut self, map: String, seed: u32) {
        let mut inputs = self
            .recorder
            .owner_inputs()
            .expect("native Recorder has explicit inputs")
            .clone();
        inputs.map = map;
        inputs.seed = seed;
        self.reset_recorder_owned(inputs);
    }

    pub(crate) fn recording_lobby_owned(
        &mut self,
        info: game_engine::common::recorder::ReplayGameInfo,
        local_index: i32,
    ) {
        self.recorder.set_recording_lobby(info, local_index);
    }

    pub(crate) fn replay_version_differs_owned(
        &mut self,
        filename: String,
    ) -> std::io::Result<bool> {
        // C++ testVersionPlayback intentionally uses a forPlayback header probe
        // on this Recorder; confirmation retains its actual selected GameInfo.
        self.recorder.test_version_playback(filename)
    }

    pub(crate) fn stop_recording_owned(&mut self) {
        self.recorder.stop_recording();
    }

    pub(crate) fn stop_playback_owned(&mut self) {
        self.recorder.stop_playback();
        let output = self.recorder.drain_pending_commands();
        apply_replay_messages(self, &output, false);
    }

    pub(crate) fn record_command_owned(&mut self, command: &GameCommand) {
        if let Some(message) = game_command_to_message(command) {
            self.recorded_messages.push(message);
        }
    }

    /// Original NewGame payload is admitted by its receiving engine dispatcher,
    /// never reconstructed from Core difficulty/rank/FPS globals.
    pub(crate) fn record_new_game_owned(
        &mut self,
        mode: GameMode,
        seed: u32,
        difficulty: i32,
        rank: i32,
        max_fps: i32,
    ) {
        self.recorder.set_recording_seed(seed);
        let mut message = GameMessage::new(GameMessageType::NewGame);
        for argument in [game_mode_to_new_game_code(mode), difficulty, rank, max_fps] {
            message.append_integer_argument(argument);
        }
        self.recorded_messages.push(message);
    }

    pub(crate) fn record_camera_owned(&mut self, pose: ReplayCameraPose) {
        // All six source observations are supplied, including valid zero cursor/pixel.
        self.recorded_messages.push(replay_camera_to_message(pose));
    }

    pub(crate) fn record_team_slot_owned(
        &mut self,
        player_index: i32,
        slot: u8,
        kind: u8,
        ids: &[ObjectId],
    ) {
        if self.recorder.is_playback() {
            return;
        }
        let ty = match kind {
            0 => GameMessageType::CreateTeamSlot(slot),
            1 => GameMessageType::SelectTeamSlot(slot),
            _ => GameMessageType::AddTeamSlot(slot),
        };
        let mut message = GameMessage::with_player(ty, player_index);
        if kind == 0 {
            for id in ids {
                message.append_object_id_argument(object_id_to_message(*id));
            }
        }
        self.recorded_messages.push(message);
    }

    pub(crate) fn logic_crc_due_owned(&self, frame: u32) -> bool {
        let interval = self.recorder.get_game_info().get_crc_interval();
        interval > 0 && frame > 0 && frame % interval == 0
    }

    pub(crate) fn flush_owned(
        &mut self,
        frame: u32,
        host_queue: &mut VecDeque<GameCommand>,
        state_crc: u32,
        local_player_index: i32,
        local_player_name_key: Option<&str>,
    ) {
        if self.last_recorder_update_frame == Some(frame) {
            return;
        }
        let routed = self.take_incoming();
        let (crc_taps, mut direct_taps): (Vec<_>, Vec<_>) =
            std::mem::take(&mut self.recorded_messages)
                .into_iter()
                .partition(|message| matches!(message.get_type(), GameMessageType::LogicCRC(_)));
        direct_taps.extend(routed.iter().cloned());
        direct_taps.extend(crc_taps);
        let playback = self.recorder.is_playback();
        let posted_crc = if self.logic_crc_due_owned(frame) {
            let crc = if self.last_logic_crc_frame == frame {
                self.last_logic_crc
            } else {
                state_crc
            };
            if self.last_logic_crc_frame != frame {
                let mut message =
                    GameMessage::with_player(GameMessageType::LogicCRC(crc), local_player_index);
                message.append_boolean_argument(playback);
                direct_taps.push(message);
                self.last_logic_crc = crc;
                self.last_logic_crc_frame = frame;
            }
            Some(crc)
        } else {
            None
        };
        if playback {
            direct_taps.retain(keep_command_during_playback);
        }
        self.recorder.update_owned(frame, &direct_taps);
        if playback {
            if let Some(crc) = posted_crc {
                self.recorder
                    .handle_crc_message_with_name_key(crc, false, local_player_name_key);
            }
            host_queue.retain(|command| game_command_to_message(command).is_none());
            // Source command list retains allowed live commands before file commands.
            direct_taps.extend(self.recorder.drain_pending_commands());
            apply_replay_messages(self, &direct_taps, false);
        } else {
            apply_replay_messages(self, &routed, false);
            let outputs = self.recorder.drain_pending_commands();
            apply_replay_messages(self, &outputs, false);
        }
        self.last_recorder_update_frame = Some(frame);
        host_queue.extend(std::mem::take(&mut self.commands));
    }
}

#[cfg(test)]
#[path = "owner_tests.rs"]
mod tests;
