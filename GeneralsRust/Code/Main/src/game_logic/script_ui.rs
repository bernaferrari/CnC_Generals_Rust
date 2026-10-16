//! UI effects borrowed for one synchronous script invocation.
//!
//! The simulation may cross the startup worker boundary; windows may not. The
//! driving application lends this interface during execution instead of storing
//! window resources or selecting another session's UI inside GameLogic.
pub(crate) trait ScriptUiEffects {
    /// C++ ScriptActions::doLetterBoxMode: Hide(TRUE) or Show(FALSE), before
    /// publishing the letterbox flag and continuing to the next action.
    fn letterbox(
        &mut self,
        camera: &mut crate::presentation_frame::camera::SessionCamera,
        enabled: bool,
        replay: bool,
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
