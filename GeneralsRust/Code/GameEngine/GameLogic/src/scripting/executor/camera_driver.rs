//! Bind simulation camera actions to the receiving driver and existing client callback.

use super::ScriptActionDispatcher;
use crate::scripting::engine::ScriptEngine;
use generals_sim::GameLogicResult;
use generals_sim::scripting::camera_actions::{ScriptCameraWorld, execute_camera_action};
use generals_sim::scripting::driver::ScriptExecutionDriver;
use generals_sim::scripting::presentation::ScriptActionHandler;
use generals_sim::scripting::program::ScriptAction;
use generals_sim::scripting::world::{ScriptCameraRequest, ScriptOwnerQuery};
use generals_sim::scripting::{ScriptActionResult, ScriptError};
use glam::Vec3;
use std::sync::Arc;

impl ScriptActionDispatcher<'_> {
    pub(super) fn execute_camera_action(
        &self,
        action: &ScriptAction,
        driver: &mut dyn ScriptExecutionDriver<ScriptEngine>,
    ) -> Result<ScriptActionResult, ScriptError> {
        execute_camera_action(
            action,
            &mut CameraWorld {
                engine: self.context.borrowed_engine(),
                driver,
            },
        )
    }
}

struct CameraWorld<'engine, 'driver> {
    engine: &'engine ScriptEngine,
    driver: &'driver mut dyn ScriptExecutionDriver<ScriptEngine>,
}

impl CameraWorld<'_, '_> {
    fn retained_handler(&self) -> Option<Arc<dyn ScriptActionHandler>> {
        self.engine
            .with_execution_read(ScriptEngine::action_handler)
            .flatten()
    }
}

impl ScriptCameraWorld for CameraWorld<'_, '_> {
    fn waypoint_position(&mut self, name: &str) -> GameLogicResult<Option<Vec3>> {
        self.driver
            .script_waypoint_position(name)
            .unwrap_or_else(|| Ok(self.engine.authored_waypoint_position(name)))
    }

    fn named_object_id(&mut self, name: &str) -> Option<u32> {
        let this_object = self
            .engine
            .with_execution_read(ScriptEngine::script_object_id)
            .flatten();
        match self.driver.named_object_id(name, this_object) {
            ScriptOwnerQuery::Present(id) => Some(id),
            ScriptOwnerQuery::Missing => None,
            ScriptOwnerQuery::Unavailable => self.engine.standalone_camera_named_object(name),
        }
    }

    fn camera(&mut self, request: ScriptCameraRequest<'_>) -> GameLogicResult<()> {
        if let Some(result) = self.driver.camera(request) {
            return result;
        }
        self.retained_handler()
            .map_or(Ok(()), |handler| request.dispatch_to(handler.as_ref()))
    }

    fn oversize_terrain(&mut self, amount: i32) -> GameLogicResult<()> {
        self.retained_handler()
            .map_or(Ok(()), |handler| handler.oversize_terrain(amount))
    }
}
