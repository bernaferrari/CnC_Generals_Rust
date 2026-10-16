//! W3DView camera work belongs to an admitted CPU view, independently of GPU Draw.
use super::camera_motion::{AxisKind, AxisMotion, MotionAdmission, step_axis};
use super::display_clock::DisplayAdmission;
use super::mouse::{SHAKE_AXIS_PITCH, SHAKE_AXIS_ROLL, SHAKE_AXIS_YAW};
use super::start_game::script_camera_shaker_rotations;
use super::*;

impl CnCGameEngine {
    pub(super) fn advance_native_camera_view(&mut self, decision: DisplayAdmission, scroll: f32) {
        if !decision.update_views && !decision.fast_camera_only {
            return;
        }
        let mut did_scripted_movement = false;
        if decision.update_views {
            // Follow precedes the pause-gated scalar movements. Position/yaw
            // use the exact frozen receiving lock identity, never nearest art.
            let follow = self.last_presentation_frame.as_ref().map(|frame| {
                (
                    frame.camera_follow_object_id,
                    frame.camera_follow_facts,
                    frame.script_time_frozen,
                )
            });
            if let Some((id, facts, script_frozen)) = follow {
                let record_previous = !script_frozen && !self.native_view_is_paused();
                did_scripted_movement |=
                    self.apply_camera_follow_or_tether(id, facts, record_previous);
            }
        }

        let admission = MotionAdmission {
            update_views: decision.update_views,
            fast_camera_only: decision.fast_camera_only,
            paused: self.native_view_is_paused(),
            // Main has no debug-freeze command owner yet (hq-pem9z). Script
            // freeze intentionally does not gate W3DView camera movements.
            debug_frozen: false,
            disable_camera_movement: game_engine::common::global_data::read()
                .writable
                .disable_camera_movement,
        };
        did_scripted_movement |= self.game_logic.advance_camera_view(admission, 33);
        if decision.update_views {
            // Full view only. Fast's early camera-only branch never reaches
            // shake, height adjustment, slave pose or matrix commitment.
            self.update_script_camera_shake();
            self.ease_camera_height_above_ground(scroll, did_scripted_movement);
            if !self.script_time_fast() {
                self.update_camera_transform_shakers();
            }
        }
    }
    /// Source setCameraTransform: after height adjustment, before slave pose.
    fn update_camera_transform_shakers(&mut self) {
        step_transform_shakers(
            &mut self.game_logic.session_view.camera.script_camera_shakers,
            true,
        );
        let eye = self.native_camera_base_pose().0;
        let mut rotation = Vec3::ZERO;
        for shaker in &self.game_logic.session_view.camera.script_camera_shakers {
            rotation += script_camera_shaker_rotations(shaker, eye);
        }
        rotation.x = rotation.x.clamp(-SHAKE_AXIS_PITCH, SHAKE_AXIS_PITCH);
        rotation.y = rotation.y.clamp(-SHAKE_AXIS_YAW, SHAKE_AXIS_YAW);
        rotation.z = rotation.z.clamp(-SHAKE_AXIS_ROLL, SHAKE_AXIS_ROLL);
        self.game_logic.session_view.camera.camera_shake_rotation = rotation;
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
pub(super) fn step_transform_shakers(shakers: &mut Vec<ScriptCameraShaker>, commit: bool) {
    if !commit {
        return;
    }
    const SCRIPT_CAMERA_SHAKER_STEP: f32 = 1.0 / 30.0;
    for shaker in shakers.iter_mut() {
        shaker.elapsed_seconds += SCRIPT_CAMERA_SHAKER_STEP;
    }
    shakers.retain(|shaker| shaker.elapsed_seconds < shaker.duration_seconds);
}

#[cfg(test)]
#[path = "camera_view_effects_tests.rs"]
mod tests;
