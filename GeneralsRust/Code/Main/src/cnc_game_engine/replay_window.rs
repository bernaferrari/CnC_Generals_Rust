//! Native replay notifications finish synchronously on their receiving engine.
use super::*;
use game_client::gui::callbacks::replay_menu::ingress::ReplayMenuReceiver;

impl CnCGameEngine {
    pub(super) fn receiving_replay_menu_inputs(
        &self,
    ) -> game_client::gui::shell::replay_menu::ReplayMenuInputs {
        let recorder = self.game_logic.replay_pending.recorder_owned();
        let inputs = recorder.owner_inputs().expect("receiving Recorder inputs");
        let version = game_engine::common::version::get_version();
        game_client::gui::shell::replay_menu::ReplayMenuInputs {
            replay_directory: inputs.replay_directory.clone(),
            replay_extension: recorder.replay_extension().to_owned(),
            last_replay_filename: recorder.last_replay_filename().to_owned(),
            current_version: version.get_unicode_version(),
            current_version_number: version.get_version_number(),
            current_exe_crc: inputs.exe_crc,
            current_ini_crc: inputs.ini_crc,
            metadata: self.replay_menu_metadata.clone(),
        }
    }

    /// Complete application content/localization admission only. Ordinary world
    /// reset/save replacement retains this snapshot because definitions/text
    /// have not changed. Any future runtime reload must use this same boundary.
    pub(super) fn admit_replay_menu_application_metadata(&mut self) {
        self.replay_menu_metadata =
            game_client::gui::shell::replay_menu::ReplayMenuMetadataPolicy::capture_application();
        self.configure_receiving_replay_menu();
    }

    pub(super) fn configure_receiving_replay_menu(&self) {
        let inputs = self.receiving_replay_menu_inputs();
        game_client::gui::with_window_manager(|manager| {
            manager.configure_replay_menu_owned(inputs)
        });
    }

    pub(super) fn initialize_receiving_replay_controls(&mut self) {
        let hidden = self
            .game_logic
            .replay_pending
            .initialize_replay_controls_owned();
        game_client::gui::with_window_manager(|manager| {
            game_client::gui::callbacks::replay_controls::apply_replay_control_visibility_owned(
                manager, hidden,
            );
        });
    }

    /// Same source message-stream phase used by EOF/stop, not an end-frame effect queue.
    pub(super) fn dispatch_receiving_recorder_lifecycle(&mut self) -> bool {
        let messages = self
            .game_logic
            .replay_pending
            .take_lifecycle_messages_owned();
        let handled = !messages.is_empty();
        for message in messages {
            if matches!(
                message.get_type(),
                game_engine::common::message_stream::GameMessageType::ClearGameData
            ) {
                let show_score = !matches!(
                    self.game_logic.game_mode(),
                    GameMode::Shell | GameMode::None
                );
                self.clear_receiving_game_data(true);
                if self.is_quitting() {
                    return handled;
                }
                // The clear already reset the receiving world. Do not reset
                // again and destroy the source score/shell windows.
                self.transition_to_state(GameState::Menu);
                if show_score {
                    self.set_runtime_host_ui_screen_override(Some("ScoreScreen"));
                }
                continue;
            }
            let Some(dispatch) = Self::startup_new_game_dispatch_from_message(&message) else {
                continue;
            };
            let Some(request) = self.receiving_recorded_start_request(dispatch) else {
                continue;
            };
            self.host_start_game_from_ui(request);
            if self.is_quitting() {
                return handled;
            }
        }
        handled
    }
}

impl ReplayMenuReceiver for CnCGameEngine {
    fn version_differs(&mut self, filename: &str) -> bool {
        self.game_logic
            .replay_pending
            .replay_version_differs_owned(filename.to_owned())
            .unwrap_or(false)
    }

    fn playback_file(&mut self, filename: String) {
        if self
            .require_native_world_boundary("Recorder playback")
            .is_err()
        {
            return;
        }
        // Filename survives actual clear/reset; no Recorder/world borrow crosses it.
        let in_game = self.game_logic.game_mode() != GameMode::None;
        let admission = self
            .game_logic
            .replay_pending
            .prepare_playback_owned(filename, in_game);
        if admission.requires_clear() {
            if !self.clear_receiving_game_data(true) {
                return;
            }
        }
        match self
            .game_logic
            .replay_pending
            .admit_prepared_playback_owned(admission)
        {
            Ok(true) => {
                let effects = self.game_logic.replay_pending.take_recorder_effects_owned();
                if let Some(seed) = effects.admitted_seed {
                    self.game_logic.set_logic_random_seed(seed);
                }
                // C++ playbackFile appends NewGame to the message stream. Leave it
                // pending until the receiving client/message phase: the native WND
                // continuation must hide its source parent before NewGame starts.
            }
            Ok(false) => warn!("Replay header rejected after receiving clear"),
            Err(error) => warn!("Replay admission failed after receiving clear: {error}"),
        }
        // WND continuation hides the captured source parent after both success/error.
    }

    fn restart(&mut self) {
        if self
            .require_native_world_boundary("Recorder restart")
            .is_err()
        {
            return;
        }
        game_client::gui::callbacks::quit_menu::destroy_quit_menu();
        let filename = self
            .game_logic
            .replay_pending
            .recorder_owned()
            .get_current_replay_filename()
            .to_owned();
        let identity = self.last_new_game_identity();
        if self
            .game_logic
            .replay_pending
            .recorder_owned()
            .is_recording()
        {
            self.game_logic.replay_pending.stop_recording_owned();
        }
        // QuitMenu.cpp203 clears without a score screen even for replay.
        // playback_file now observes GAME_NONE and must not clear a second time.
        self.clear_receiving_game_data(false);
        if self.is_quitting() {
            return;
        }
        if self.pending_state == Some(GameState::Exiting) {
            self.pending_state = None;
        }
        if !filename.is_empty() {
            self.playback_file(filename);
        } else if let Some(identity) = identity {
            self.host_restart_mission_from_dispatch(identity.dispatch);
        } else {
            warn!("Cannot restart without owned NewGame identity");
        }
        // Existing application UI compatibility boundary; owning this status is
        // separate from receiving Recorder/world ownership.
        game_client::helpers::TheInGameUI::set_client_quiet(true);
        self.game_logic.set_radar_client_quiet(true);
    }
}

#[cfg(test)]
mod source_policy_tests {
    #[test]
    fn native_replay_and_restart_keep_distinct_source_clear_policies() {
        let source = include_str!("replay_window.rs")
            .split("#[cfg(test)]")
            .next()
            .unwrap();
        let playback = source
            .split("fn playback_file(")
            .nth(1)
            .unwrap()
            .split("fn restart(")
            .next()
            .unwrap();
        assert!(
            playback
                .find("self.clear_receiving_game_data(true)")
                .unwrap()
                < playback
                    .find(".admit_prepared_playback_owned(admission)")
                    .unwrap()
        );
        assert!(!playback.contains("dispatch_receiving_recorder_lifecycle()"));
        let restart = source.split("fn restart(").nth(1).unwrap();
        let capture = restart.find("get_current_replay_filename()").unwrap();
        let stop = restart.find("stop_recording_owned()").unwrap();
        let clear = restart
            .find("self.clear_receiving_game_data(false)")
            .unwrap();
        let resume = restart.find("self.playback_file(filename)").unwrap();
        assert!(capture < stop && stop < clear && clear < resume);
        assert!(!restart.contains("clear_receiving_game_data(true)"));
    }
}
