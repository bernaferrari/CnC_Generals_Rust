//! UI effects borrowed for one synchronous script invocation.
//!
//! The simulation may cross the startup worker boundary; windows may not. The
//! driving application lends this interface during execution instead of storing
//! window resources or selecting another session's UI inside GameLogic.
pub(crate) trait ScriptUiEffects {
    fn popup_message(
        &mut self,
        _popup: &crate::game_logic::ScriptPopupMessageRequest,
    ) -> gamelogic::GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "popup requires receiving client services".into(),
        ))
    }

    /// Observation comes from the receiving ClientFrameWriter at each admitted
    /// step; no UI capability means headless execution creates no visual hint.
    fn admit_command_hint_client_frame(&mut self, _frame: Option<u32>) {}
    fn command_hint(
        &mut self,
        _attack: bool,
        _position: glam::Vec3,
        _source: u32,
        _eligible: bool,
    ) {
    }

    /// Apply `setCampaignVictorious` to this invocation's receiving application.
    /// UI-less paths reject the operation instead of mutating ambient state.
    fn set_campaign_victorious(&mut self, _victorious: bool) -> gamelogic::GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "campaign victory requires its receiving CampaignManager".into(),
        ))
    }
    /// CPP ProductionUpdate marks selected upgrade cameos after Academy callbacks.
    fn mark_control_bar_dirty(&mut self) {}
    fn has_cinematic_input(&self) -> bool {
        false
    }
    fn cinematic_input(
        &mut self,
        _enabled: bool,
        _was_enabled: bool,
        _deselect: &mut dyn FnMut(u32),
    ) -> gamelogic::GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "receiving cinematic input service unavailable".into(),
        ))
    }

    /// Immediate visual weather delivery to the receiving Client.
    fn weather_visible(&mut self, visible: bool);

    /// Apply C++ ScriptActions::winCreateFromScript at the next synchronous
    /// action-delivery boundary using the receiving client's window owner.
    fn create_win_lose_window(&mut self, _layout_filename: &str) -> gamelogic::GameLogicResult<()> {
        Ok(())
    }

    /// Apply C++ ScriptActions::winDestroy to the receiving client's tracked
    /// message window, preserving callback order within the current action.
    fn destroy_win_lose_window(&mut self) -> gamelogic::GameLogicResult<()> {
        Ok(())
    }

    /// C++ GameLogic::closeWindows, delivered before input disabling/window creation.
    fn close_game_windows(&mut self) -> gamelogic::GameLogicResult<()> {
        Err(gamelogic::GameLogicError::Configuration(
            "closeWindows requires its receiving GUI services".into(),
        ))
    }
    fn flush_window_callbacks(&mut self) -> gamelogic::GameLogicResult<()> {
        Ok(())
    }

    /// C++ ScriptActions::doLetterBoxMode: Hide(TRUE) or Show(FALSE), before
    /// publishing the letterbox flag and continuing to the next action.
    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
        _upgrade_source: game_engine::common::thing::receiving_module_content::UpgradeTooltipSource<
            '_,
        >,
        _game_ending: bool,
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn simulation_remains_send_without_window_resources() {
        fn assert_send<T: Send>() {}
        assert_send::<super::super::GameLogic>();
    }
}
