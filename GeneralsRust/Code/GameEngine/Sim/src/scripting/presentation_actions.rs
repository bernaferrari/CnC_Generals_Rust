//! Authored presentation actions on the match's borrowed presentation owner.

use super::camera_actions::{ScriptCameraWorld, try_execute_camera_action};
use super::driver::ScriptExecutionDriver;
use super::interpreter::ScriptInterpreter;
use super::map_actions::{ScriptMapWorld, try_execute_map_action};
use super::media_actions::{ScriptMediaWorld, try_execute_media_action};
use super::mission_actions::{ScriptMissionWorld, try_execute_mission_action};
use super::program::ScriptAction;
use super::{ScriptActionResult, ScriptError};

pub trait ScriptPresentationWorld:
    ScriptCameraWorld + ScriptMapWorld + ScriptMediaWorld + ScriptMissionWorld
{
}

impl<T: ScriptCameraWorld + ScriptMapWorld + ScriptMediaWorld + ScriptMissionWorld>
    ScriptPresentationWorld for T
{
}

impl ScriptInterpreter {
    /// Standalone callers can decline the presentation loan and use their
    /// separate compatibility services.
    pub fn try_execute_presentation_action(
        &self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver,
    ) -> Result<Option<ScriptActionResult>, ScriptError> {
        let Some(world) = driver.presentation_world() else {
            return Ok(None);
        };
        if let Some(result) = try_execute_camera_action(action, self, world)? {
            return Ok(Some(result));
        }
        if let Some(result) = try_execute_map_action(action, self, world)? {
            return Ok(Some(result));
        }
        if let Some(result) = try_execute_media_action(action, self, world)? {
            return Ok(Some(result));
        }
        try_execute_mission_action(action, self, world)
    }
}
