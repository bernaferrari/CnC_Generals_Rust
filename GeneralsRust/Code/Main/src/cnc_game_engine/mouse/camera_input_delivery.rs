//! Lexical adapter operation over existing Engine input fields and its actual
//! Client frame loan. This carries no persistent state or ambient owner.
use super::*;
use crate::game_logic::{GameLogic, ScriptPresentationEffects};
pub(in crate::cnc_game_engine) struct CameraInputLoan<'a> {
    modes: &'a mut LookAtHostModes,
    rotating: &'a mut bool,
    anchor: &'a mut Option<(f32, f32)>,
    #[cfg(feature = "game_client")]
    frame: game_client::core::game_client::ClientFrameWriter<'a>,
    #[cfg(not(feature = "game_client"))]
    frame: u32,
}
impl<'a> CameraInputLoan<'a> {
    pub(in crate::cnc_game_engine) fn new(
        modes: &'a mut LookAtHostModes,
        rotating: &'a mut bool,
        anchor: &'a mut Option<(f32, f32)>,
        #[cfg(feature = "game_client")] frame: game_client::core::game_client::ClientFrameWriter<
            'a,
        >,
        #[cfg(not(feature = "game_client"))] frame: u32,
    ) -> Self {
        Self {
            modes,
            rotating,
            anchor,
            frame,
        }
    }
    fn receiving_frame(&self) -> u32 {
        #[cfg(feature = "game_client")]
        {
            self.frame.frame()
        }
        #[cfg(not(feature = "game_client"))]
        {
            self.frame
        }
    }
    pub(in crate::cnc_game_engine) fn begin_mmb(
        &mut self,
        position: (f32, f32),
        world: &GameLogic,
    ) {
        self.modes
            .lookat_stamp_mouse_activity(world.get_current_frame() as u32);
        *self.rotating = true;
        *self.anchor = Some(position);
        self.modes.mmb_original_anchor = Some(position);
        self.modes.mmb_press_frame = self.receiving_frame();
    }
    pub(in crate::cnc_game_engine) fn delivered_cursor(
        &mut self,
        position: Option<(f32, f32)>,
        in_match: bool,
        input_enabled: bool,
        world: &mut GameLogic,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        let Some(position) = position else {
            return Ok(false);
        };
        if !in_match || !input_enabled || !*self.rotating {
            return Ok(false);
        }
        let Some(anchor) = *self.anchor else {
            return Ok(false);
        };
        let dx = (position.0 as i32).wrapping_sub(anchor.0 as i32) as f32;
        self.modes
            .note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
        let result = world.admit_player_camera_rotation(dx * LOOKAT_MMB_YAW_FACTOR, effects);
        *self.anchor = Some(position);
        result.map(|_| true)
    }
    pub(in crate::cnc_game_engine) fn end_mmb(
        &mut self,
        position: (f32, f32),
        world: &mut GameLogic,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<bool, String> {
        self.modes
            .lookat_stamp_mouse_activity(world.get_current_frame() as u32);
        let original = self.modes.mmb_original_anchor.take();
        let press_frame = self.modes.mmb_press_frame;
        *self.rotating = false;
        *self.anchor = None;
        let Some(origin) = original else {
            return Ok(false);
        };
        let dx = (position.0 as i32).wrapping_sub(origin.0 as i32) as f32;
        let dy = (position.1 as i32).wrapping_sub(origin.1 as i32) as f32;
        if lookat_mmb_is_short_click(dx, dy, self.receiving_frame().wrapping_sub(press_frame)) {
            world.admit_short_mmb_defaults(effects)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(in crate::cnc_game_engine) fn delivered_wheel(
        &mut self,
        delta: &winit::event::MouseScrollDelta,
        world: &mut GameLogic,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> (bool, Result<(), String>) {
        let detents = super::camera::mouse_wheel_detents(delta);
        if detents.abs() >= 0.5 {
            self.modes
                .note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
            (
                true,
                world.admit_player_camera_wheel(detents as i32, effects),
            )
        } else {
            (false, Ok(()))
        }
    }
    pub(in crate::cnc_game_engine) fn held_keys(
        &mut self,
        left: bool,
        right: bool,
        zoom_in: bool,
        zoom_out: bool,
        rotate_delta: f32,
        zoom_steps: f32,
        world: &mut GameLogic,
        effects: &mut ScriptPresentationEffects<'_>,
    ) -> Result<(), String> {
        if left != right || zoom_in != zoom_out {
            self.modes
                .note_scripted_camera_player_cancel(ScriptedCameraPlayerCancel::Set);
            world.admit_player_camera_held_keys(
                left,
                right,
                zoom_in,
                zoom_out,
                rotate_delta,
                zoom_steps,
                effects,
            )?;
        }
        Ok(())
    }
}
#[cfg(all(test, feature = "game_client"))]
#[path = "camera_input_delivery_tests.rs"]
mod tests;
