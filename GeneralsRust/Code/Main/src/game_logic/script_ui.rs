//! UI effects borrowed for one synchronous script invocation.
//!
//! The simulation may cross the startup worker boundary; windows may not. The
//! driving application lends this interface during execution instead of storing
//! window resources or selecting another session's UI inside GameLogic.
pub(crate) trait ScriptUiEffects {
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
