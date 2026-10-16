//! Delivered player camera setters; never an idle presentation clock.
use super::super::*;
use crate::game_logic::ScriptPresentationEffects;
impl GameLogic {
    pub(crate) fn admit_player_camera_rotation(
        &mut self,
        delta: f32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        if !delta.is_finite() {
            return Err("nonfinite player camera rotation".into());
        }
        self.session_view.camera.camera_yaw_radians += delta;
        crate::presentation_frame::camera::motion::normalize_yaw(
            &mut self.session_view.camera.camera_yaw_radians,
        );
        // setAngle is still a setter when delta==0; no approximate equality gate.
        self.cancel_scripted_camera_from_player_set();
        self.commit_native_camera_with_effects(effects)
    }
    pub(crate) fn admit_player_camera_height_step(
        &mut self,
        steps: f32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        if !steps.is_finite() {
            return Err("nonfinite player camera height step".into());
        }
        let (min, max) = self.session_view.camera.settings.bounds();
        let goal = self.session_view.camera.native_camera.desired() + steps * 10.;
        self.session_view
            .camera
            .native_camera
            .set_height(goal, min, max);
        self.cancel_scripted_camera_from_player_set();
        self.session_view.camera.invalidate_constraints();
        self.session_view.camera.scripted_camera_constraint_widen = None;
        self.commit_native_camera_with_effects(effects)
    }
    pub(crate) fn admit_player_camera_wheel(
        &mut self,
        spin: i32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        // LookAtXlat339–347: each integer spin is a complete virtual setter.
        for _ in 0..spin.unsigned_abs() {
            self.admit_player_camera_height_step(if spin > 0 { -1. } else { 1. }, effects)?;
        }
        Ok(())
    }
    pub(crate) fn admit_player_camera_held_keys(
        &mut self,
        left: bool,
        right: bool,
        zoom_in: bool,
        zoom_out: bool,
        rotate_delta: f32,
        zoom_steps: f32,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        // InGameUI1833–1851 suppresses BOTH opposite directions before setters.
        if left && !right {
            self.admit_player_camera_rotation(-rotate_delta, effects)?;
        }
        if right && !left {
            self.admit_player_camera_rotation(rotate_delta, effects)?;
        }
        if zoom_in && !zoom_out {
            self.admit_player_camera_height_step(-zoom_steps, effects)?;
        }
        if zoom_out && !zoom_in {
            self.admit_player_camera_height_step(zoom_steps, effects)?;
        }
        Ok(())
    }
}
#[cfg(test)]
#[path = "camera_player_input_tests.rs"]
mod tests;
