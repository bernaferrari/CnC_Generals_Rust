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
        self.game_client
            .window_manager_owner()
            .with_manager(|manager| manager.configure_replay_menu_owned(inputs));
    }

    pub(super) fn initialize_receiving_replay_controls(&mut self) {
        let hidden = self
            .game_logic
            .replay_pending
            .initialize_replay_controls_owned();
        self.game_client
            .window_manager_owner()
            .with_manager(|manager| {
                game_client::gui::callbacks::replay_controls::apply_replay_control_visibility_owned(
                    manager, hidden,
                );
            });
    }

    /// Same source message-stream phase used by EOF/stop, not an end-frame effect queue.
    pub(super) fn dispatch_receiving_recorder_lifecycle(&mut self) -> Result<bool, String> {
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
                self.clear_receiving_game_data(true)?;
                if self.is_quitting() {
                    return Ok(handled);
                }
                // The clear already reset the receiving world. Do not reset
                // again and destroy the source score/shell windows.
                self.transition_to_state(GameState::Menu)?;
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
            self.host_start_game_from_ui(request)?;
            if self.is_quitting() {
                return Ok(handled);
            }
        }
        Ok(handled)
    }
}

impl ReplayMenuReceiver for CnCGameEngine {
    fn confirm_older(
        &mut self,
        playback: game_client::gui::callbacks::replay_menu::ingress::ReplayPlayback,
    ) -> Result<(), String> {
        self.with_receiving_menu_effects(|effects| {
            effects.with_manager(|manager, effects| {
                game_client::gui::callbacks::message_box::message_box_replay_confirmation(
                    playback, manager, effects,
                )
            })
        })?;
        Ok(())
    }
    fn no_selection(&mut self) -> Result<(), String> {
        self.with_receiving_menu_effects(|effects| {
            effects.with_manager(|manager, effects| {
                let _ = game_client::gui::callbacks::message_box::message_box_ok(
                    &game_client::game_text::GameText::fetch("GUI:NoFileSelected"),
                    &game_client::game_text::GameText::fetch("GUI:PleaseSelectAFile"),
                    None,
                    manager,
                    effects,
                );
            })
        })?;
        Ok(())
    }
    fn confirm_restart(&mut self) -> Result<(), String> {
        self.with_receiving_menu_effects(|effects| {
            effects.with_manager(|manager, effects| {
                game_client::gui::callbacks::message_box::message_box_restart_confirmation(
                    manager, effects,
                )
            })
        })?;
        Ok(())
    }

    fn version_differs(&mut self, filename: &str) -> bool {
        self.game_logic
            .replay_pending
            .replay_version_differs_owned(filename.to_owned())
            .unwrap_or(false)
    }

    fn playback_file(&mut self, filename: String) -> Result<(), String> {
        self.require_native_world_boundary("Recorder playback")
            .map_err(|error| error.to_string())?;
        // Filename survives actual clear/reset; no Recorder/world borrow crosses it.
        let in_game = self.game_logic.game_mode() != GameMode::None;
        let admission = self
            .game_logic
            .replay_pending
            .prepare_playback_owned(filename, in_game);
        if admission.requires_clear() {
            if !self.clear_receiving_game_data(true)? {
                return Err("Replay source clear failed on receiving native world".into());
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
        Ok(())
    }

    fn restart(&mut self) -> Result<(), String> {
        self.require_native_world_boundary("Recorder restart")
            .map_err(|error| error.to_string())?;
        self.with_receiving_menu_effects(|effects| {
            game_client::gui::callbacks::quit_menu::destroy_quit_menu(effects)
        })?;
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
        let had_world = self.game_logic.game_mode() != GameMode::None;
        if !self.clear_receiving_game_data(false)? && had_world {
            return Err("Restart source clear failed on receiving native world".into());
        }
        if self.is_quitting() {
            return Ok(());
        }
        if self.pending_state == Some(GameState::Exiting) {
            self.pending_state = None;
        }
        if !filename.is_empty() {
            self.playback_file(filename)?;
        } else if let Some(identity) = identity {
            self.host_restart_mission_from_dispatch(identity.dispatch)?;
        } else {
            warn!("Cannot restart without owned NewGame identity");
        }
        // Existing application UI compatibility boundary; owning this status is
        // separate from receiving Recorder/world ownership.
        game_client::helpers::TheInGameUI::set_client_quiet(true);
        self.game_logic.set_radar_client_quiet(true);
        Ok(())
    }
}
