//! DISABLE_INPUT / ENABLE_INPUT — C++ ScriptActions cinematic UI teardown.

use super::*;
use crate::helpers::TheInGameUI;

impl ScriptActionDispatcher<'_> {
    /// C++ ScriptActions.cpp:3176-3189 `doDisableInput`.
    pub(crate) fn do_disable_input(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Disabling user input");
        if let Some(result) =
            driver.world(crate::scripting::engine::ScriptWorldRequest::Input { enabled: false })
        {
            return result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string()));
        }
        TheGameLogic::set_input_enabled(false);
        TheInGameUI::request_cinematic_input_lock(true);
        Ok(ScriptActionResult::Success)
    }

    /// C++ ScriptActions.cpp:3195-3198 `doEnableInput`.
    pub(crate) fn do_enable_input(
        &mut self,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Enabling user input");
        if let Some(result) =
            driver.world(crate::scripting::engine::ScriptWorldRequest::Input { enabled: true })
        {
            return result
                .map(|()| ScriptActionResult::Success)
                .map_err(|error| ScriptError::ExecutionFailed(error.to_string()));
        }
        TheGameLogic::set_input_enabled(true);
        TheInGameUI::request_cinematic_input_lock(false);
        Ok(ScriptActionResult::Success)
    }
}
