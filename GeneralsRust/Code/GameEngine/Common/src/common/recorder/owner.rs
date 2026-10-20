//! Explicit receiving inputs and borrowed operations for the existing Recorder.
use super::*;

/// Immutable receiving application inputs; no callback can select another world.
/// `replay_directory` is the final Replays directory, not its user-data parent.
#[derive(Debug, Clone)]
pub struct RecorderOwnerInputs {
    pub map: String,
    pub seed: u32,
    pub replay_directory: PathBuf,
    pub exe_crc: u32,
    pub ini_crc: u32,
    pub save_stats: bool,
    pub stats_directory: PathBuf,
}

/// Effects of a completed Recorder operation. The receiver admits these at
/// the original lifecycle boundary; the explicit path never publishes them.
#[derive(Debug, Default)]
pub struct RecorderOwnerEffects {
    pub pending_map: Option<String>,
    pub admitted_seed: Option<u32>,
    pub crc_interval: Option<i32>,
    pub controls_hidden: Option<bool>,
}

/// Owned continuation across C++ playbackFile's synchronous clearGameData.
/// The driving receiver clears its world before consuming this admission.
pub struct RecorderPlaybackAdmission {
    filename: String,
    clear_current_game: bool,
}
impl RecorderPlaybackAdmission {
    pub fn requires_clear(&self) -> bool {
        self.clear_current_game
    }
}

impl Recorder {
    pub fn owner_inputs(&self) -> Option<&RecorderOwnerInputs> {
        self.owner_inputs.as_ref()
    }

    pub fn prepare_playback_owned(
        &mut self,
        filename: String,
        in_game: bool,
    ) -> RecorderPlaybackAdmission {
        assert!(
            self.owner_inputs.is_some(),
            "explicit Recorder requires owned inputs"
        );
        self.pending_commands.clear();
        RecorderPlaybackAdmission {
            filename,
            clear_current_game: !self.doing_analysis && in_game,
        }
    }

    /// Call after the receiving engine has synchronously cleared/reset if required.
    /// The continuation owns only the filename; no world/Recorder borrow survives reset.
    pub fn admit_prepared_playback_owned(
        &mut self,
        admission: RecorderPlaybackAdmission,
    ) -> Result<bool, std::io::Error> {
        assert!(
            self.owner_inputs.is_some(),
            "explicit Recorder requires owned inputs"
        );
        self.playback_file_with_in_game(admission.filename, false)
    }
    /// Reset only this receiving Recorder and replace its immutable match inputs.
    pub fn reset_owned(&mut self, inputs: RecorderOwnerInputs) {
        self.owner_inputs = Some(inputs);
        self.owner_effects = RecorderOwnerEffects::default();
        self.reset();
    }

    pub fn take_owner_effects(&mut self) -> RecorderOwnerEffects {
        std::mem::take(&mut self.owner_effects)
    }

    /// Admit visibility without consuming unrelated header effects.
    pub fn take_controls_visibility_owned(&mut self) -> Option<bool> {
        self.owner_effects.controls_hidden.take()
    }

    /// One synchronous borrowed command phase. The caller culls the receiving
    /// command list before playback and drains outputs before gameplay dispatch.
    /// No source, sink, frame, lobby, mode or UI callback is consulted.
    pub fn update_owned(&mut self, frame: u32, commands: &[GameMessage]) {
        assert!(
            self.owner_inputs.is_some(),
            "explicit Recorder requires owned inputs"
        );
        self.current_frame = frame;
        match self.mode {
            RecorderMode::Record | RecorderMode::None => {
                self.update_record_messages(commands.to_vec());
            }
            RecorderMode::Playback => self.update_playback(),
        }
    }

    /// Same C++ playback algorithm, with the receiving world's in-game fact.
    pub fn playback_file_owned(
        &mut self,
        filename: String,
        in_game: bool,
    ) -> Result<bool, std::io::Error> {
        assert!(
            self.owner_inputs.is_some(),
            "explicit Recorder requires owned inputs"
        );
        if in_game {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "active-world playback requires prepare/clear/admit continuation",
            ));
        }
        self.playback_file_with_in_game(filename, false)
    }

    pub(super) fn emit_lifecycle(&mut self, message: GameMessage) {
        if self.owner_inputs.is_some() {
            self.pending_commands.push(message);
        } else {
            append_game_message_to_the_message_stream(&message);
            if let Some(sink) = &self.command_sink {
                sink(message);
            } else {
                self.pending_commands.push(message);
            }
        }
    }
}
