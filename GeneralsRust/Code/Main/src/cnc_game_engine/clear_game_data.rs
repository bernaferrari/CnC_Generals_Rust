//! Receiving native ClearGameData shared by message dispatch and Recorder ingress.
//! C++ GameLogicDispatch.cpp:223–279. Remaining compatibility UI adapters are
//! explicit here; this does not claim complete StatsCollector/ScoreScreen ownership.
use super::*;

impl CnCGameEngine {
    /// Clear the receiving live game before Recorder opens the requested file.
    /// Ordinary playback passes true; Restart passes false before calling playback.
    /// The caller owns its later menu transition / NEW_GAME source phase.
    pub(super) fn clear_receiving_game_data(
        &mut self,
        show_score_screen: bool,
    ) -> Result<bool, String> {
        self.require_native_world_boundary("ClearGameData")
            .map_err(|error| error.to_string())?;
        let mode = self.game_logic.game_mode();
        // Original isInGame is mode != GAME_NONE, including a shell/loading game.
        if mode == GameMode::None {
            return Ok(false);
        }
        self.host_set_paused(false);
        // The source closes the outgoing win/lose window before score collection.
        // Select this session's interpreter, never the globally installed one.
        if let Some(engine) = self.game_logic.owned_script_engine.as_ref() {
            engine.close_windows(false);
        }
        let show_score = Self::clear_game_data_should_push_score_screen(
            mode == GameMode::Shell,
            true,
            show_score_screen,
        );
        if show_score {
            let end_frame = self.game_logic.frame;
            let show_advice = matches!(mode, GameMode::Skirmish | GameMode::Internet);
            let advice = if show_advice {
                self.game_logic.score_academy_advice()
            } else {
                Default::default()
            };
            // Score callbacks must observe the outgoing game before reset.
            self.host_push_score_screen_like_cpp(end_frame, advice, show_advice)?;
        }
        self.host_reset_for_clear_game_data()
            .map_err(|error| error.to_string())?;
        self.reset_match_state_after_engine_reset()
            .map_err(|error| error.to_string())?;
        self.game_logic.finish_clear_game_data_after_reset();
        self.render_pipeline.set_presentation_frame(None);

        // Source suffix: HideControlBar, closeWindows, then show the cursor.
        #[cfg(feature = "game_client")]
        self.hide_owned_control_bar(true);
        if let Some(engine) = self.game_logic.owned_script_engine.as_ref() {
            engine.close_game_windows();
        }
        #[cfg(feature = "game_client")]
        if let Some(mouse) = self.game_client.input_mouse_mut() {
            mouse.set_visibility(true);
        }
        self.destroy_source_new_game_background();
        Ok(true)
    }
}

#[cfg(test)]
mod tests {}
