//! DISABLE_INPUT / ENABLE_INPUT — C++ ScriptActions cinematic UI teardown.

use super::*;
use crate::helpers::TheInGameUI;

impl ScriptActionDispatcher {
    /// C++ ScriptActions.cpp:3176-3189 `doDisableInput`.
    pub(crate) fn do_disable_input(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Disabling user input");
        TheGameLogic::set_input_enabled(false);
        TheInGameUI::request_cinematic_input_lock(true);
        Ok(ScriptActionResult::Success)
    }

    /// C++ ScriptActions.cpp:3195-3198 `doEnableInput`.
    pub(crate) fn do_enable_input(&mut self) -> Result<ScriptActionResult, ScriptError> {
        log::info!("Enabling user input");
        TheGameLogic::set_input_enabled(true);
        TheInGameUI::request_cinematic_input_lock(false);
        Ok(ScriptActionResult::Success)
    }
}
