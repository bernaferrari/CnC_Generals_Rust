//! W3DView camera work belongs to an admitted CPU view, independently of GPU Draw.
use super::camera_motion::{AxisKind, AxisMotion, MotionAdmission, step_axis};
use super::display_clock::DisplayAdmission;
use super::mouse::{SHAKE_AXIS_PITCH, SHAKE_AXIS_ROLL, SHAKE_AXIS_YAW};
use super::start_game::script_camera_shaker_rotations;
use super::*;

impl CnCGameEngine {
    pub(super) fn advance_native_camera_view(
        &mut self,
        decision: DisplayAdmission,
        scroll: f32,
    ) -> Result<(), String> {
        if !decision.update_views && !decision.fast_camera_only {
            return Ok(());
        }
        let mut did_scripted_movement = false;
        if decision.update_views {
            // Follow precedes the pause-gated scalar movements. Position/yaw
            // use the exact frozen receiving lock identity, never nearest art.
            let follow = self
                .last_presentation_frame
                .as_ref()
                .map(|frame| (frame.camera_follow_object_id, frame.camera_follow_facts));
            if let Some((id, facts)) = follow {
                let paused = self.native_view_is_paused();
                did_scripted_movement |= self.apply_camera_follow_or_tether(id, facts, paused)?;
            }
        }

        // Read the receiving debug owner at W3DView's movement gate,
        // after follow. Script freeze deliberately does not gate this phase.
        let paused = self.native_view_is_paused();
        let disable_camera_movement = self.game_logic.camera_movement_disabled();
        did_scripted_movement |= self
            .game_logic
            .advance_receiving_camera_view(
                decision.update_views,
                decision.fast_camera_only,
                paused,
                disable_camera_movement,
                33,
                None,
            )
            .map_err(|error| error.to_string())?;
        if decision.update_views {
            // Full view only. Fast's early camera-only branch never reaches
            // shake, height adjustment, slave pose or matrix commitment.
            self.update_script_camera_shake();
            self.ease_camera_height_above_ground(scroll, did_scripted_movement);
        }
        Ok(())
    }
}

/// Source impulse spring; no wall clock, singleton, or second mutable owner.
/// Returning None preserves the previous completed pose on a rejected view.
pub(super) fn step_view_impulse(
    intensity: &mut f32,
    cosine: &mut f32,
    sine: &mut f32,
    full_view: bool,
) -> Option<Vec3> {
    if !full_view {
        return None;
    }
    let [x, z] = game_engine::common::view_impulse::ViewImpulse {
        intensity,
        cosine,
        sine,
    }
    .step();
    let offset = Vec3::new(x, 0.0, z);
    Some(offset)
}

/// W3DView::setCameraTransform steps rotational shakers, after the fast
/// View return. Impulse shake above is earlier and deliberately independent.
pub(super) use crate::presentation_frame::camera::shaker::step_transform_shakers;

#[cfg(test)]
#[path = "camera_view_effects_tests.rs"]
mod tests;
