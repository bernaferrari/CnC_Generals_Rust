//! Debug control belongs to the receiving ScriptEngine. A borrowed interpreter
//! is an explicit operation capability while with_owned_script_engine lends it;
//! no missing-owner default, mirrored bit, global lookup or active slot exists.
use super::*;
use gamelogic::scripting::engine::ScriptEngine;
impl GameLogic {
    pub(crate) fn script_debug_frozen(
        &self,
        interpreter: Option<&ScriptEngine>,
    ) -> gamelogic::GameLogicResult<bool> {
        let engine = interpreter
            .or(self.owned_script_engine.as_ref())
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "debug freeze requires the receiving ScriptEngine or its explicit loan".into(),
                )
            })?;
        Ok(engine.is_time_frozen_debug())
    }
    pub(crate) fn force_script_debug_continue(
        &self,
        interpreter: Option<&ScriptEngine>,
    ) -> gamelogic::GameLogicResult<()> {
        let engine = interpreter
            .or(self.owned_script_engine.as_ref())
            .ok_or_else(|| {
                gamelogic::GameLogicError::Configuration(
                    "debug continue requires the receiving ScriptEngine or its explicit loan"
                        .into(),
                )
            })?;
        engine.force_unfreeze_time();
        Ok(())
    }
    /// One actual native/headless camera offer. A rejected offer performs no
    /// debug query; an admitted movement reads its receiving interpreter once.
    pub(crate) fn advance_receiving_camera_view(
        &mut self,
        update_views: bool,
        fast_camera_only: bool,
        paused: bool,
        disable_camera_movement: bool,
        frame_ms: i32,
        interpreter: Option<&ScriptEngine>,
    ) -> gamelogic::GameLogicResult<bool> {
        if !update_views && !fast_camera_only {
            return Ok(false);
        }
        // W3DDisplay fast-camera branch calls movement directly, before the
        // full View debug/pause gate. That branch never queries debug control.
        let debug_frozen = !fast_camera_only && self.script_debug_frozen(interpreter)?;
        Ok(self.advance_camera_view(
            crate::presentation_frame::camera::motion::MotionAdmission {
                update_views,
                fast_camera_only,
                paused,
                debug_frozen,
                disable_camera_movement,
            },
            frame_ms,
        ))
    }
    /// Only a matching, successfully resolved lock object reaches the source
    /// history gate. Inactive, stale and missing facts never query debug control.
    pub(crate) fn admit_receiving_camera_follow_with_policy(
        &mut self,
        observed_id: Option<ObjectId>,
        facts: Option<crate::presentation_frame::follow::FollowTargetFacts>,
        paused: bool,
        interpreter: Option<&ScriptEngine>,
    ) -> gamelogic::GameLogicResult<crate::presentation_frame::follow::FollowAdmission> {
        let cell = self.camera_follow_partition_cell_size();
        self.admit_receiving_camera_follow(observed_id, facts, cell, paused, interpreter)
    }
    pub(crate) fn admit_receiving_camera_follow(
        &mut self,
        observed_id: Option<ObjectId>,
        facts: Option<crate::presentation_frame::follow::FollowTargetFacts>,
        partition_cell_size: f32,
        paused: bool,
        interpreter: Option<&ScriptEngine>,
    ) -> gamelogic::GameLogicResult<crate::presentation_frame::follow::FollowAdmission> {
        let resolved = self
            .session_view
            .scripted
            .camera_follow_target
            .is_some_and(|id| observed_id == Some(id) && facts.is_some_and(|facts| facts.id == id));
        let record_previous = if resolved {
            !(self.script_debug_frozen(interpreter)? || self.is_script_time_frozen()) && !paused
        } else {
            false
        };
        self.session_view
            .admit_camera_follow(observed_id, facts, partition_cell_size, record_previous)
            .map_err(|error| gamelogic::GameLogicError::Configuration(error.into()))
    }
    /// GameLogic.cpp3603 observes camera, debug, script in that exact order.
    /// Do not require a debug capability on a short-circuited camera branch.
    pub(crate) fn simulation_frozen_with_script_engine(
        &self,
        interpreter: Option<&ScriptEngine>,
    ) -> gamelogic::GameLogicResult<bool> {
        Ok(self.is_script_camera_time_frozen()
            || self.script_debug_frozen(interpreter)?
            || self.is_script_time_frozen())
    }
}
#[cfg(test)]
#[path = "debug_freeze_tests.rs"]
mod tests;
