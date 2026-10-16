//! Native continuation of the existing logic camera owner, not a second camera.
//! W3DView's movement records/timers are retained verbatim; original View v1 stays unchanged.
use super::script_camera::{ParabolicEase, ScriptCameraMoveTo, ScriptCameraPathMove};
use super::*;
use crate::game_logic::mission_scripts::*;
use crate::save_load::camera_codec::CameraCodec;
use glam::Vec3;
use serde::{Deserialize, Serialize};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ITEMS: usize = 4096;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LogicCameraContinuation {
    pending_camera_focus: Option<Vec3>,
    script_camera_focus_estimate: Vec3,
    script_camera_move_to: Option<ScriptCameraMoveTo>,
    script_camera_path: Option<ScriptCameraPathMove>,
    camera_follow_target: Option<ObjectId>,
    camera_tether_play: Option<f32>,
    script_look_toward_object_id: Option<u32>,
    script_look_toward_hold_seconds: f32,
    script_default_camera_pitch: f32,
    script_default_camera_angle: f32,
    script_default_camera_max_height: f32,
    script_camera_freeze_time_armed: bool,
    script_camera_freeze_time: bool,
    script_camera_rotate_remaining: f32,
    script_camera_zoom_remaining: f32,
    script_camera_pitch_remaining: f32,
    script_camera_freeze_angle_armed: bool,
    script_camera_pending_final_speed_multiplier: Option<f32>,
    visual_speed_multiplier: f32,
    script_time_frozen_by_script: bool,
    pending_script_fps_limit: Option<i32>,
    pending_camera_zoom_reset: bool,
    pending_camera_zoom_reset_duration: f32,
    pending_camera_zoom_reset_ease_in: f32,
    pending_camera_zoom_reset_ease_out: f32,
    pending_camera_zoom: Option<CameraZoomRequest>,
    pending_camera_pitch: Option<CameraPitchRequest>,
    pending_camera_rotate: Option<CameraRotateRequest>,
    pending_camera_look_toward: Option<CameraLookTowardWaypointRequest>,
    pending_camera_slave_mode_enable: Option<CameraSlaveModeRequest>,
    pending_camera_slave_mode_disable: bool,
    pending_screen_shakes: Vec<ScreenShakeRequest>,
    pending_camera_add_shakers: Vec<CameraAddShakerRequest>,
    pending_view_guardband: Option<ViewGuardbandRequest>,
    native_view_guardband: [f32; 2],
    pending_camera_bw_mode: Option<CameraBwModeRequest>,
    pending_camera_motion_blur: Vec<CameraMotionBlurRequest>,
    script_skybox_enabled: bool,
    cinematic_letterbox: bool,
    hooks: CameraHookContinuation,
    // Never extend the historical positional record; extras have a versioned envelope.
    #[serde(skip)]
    owned_extras: CameraOwnedExtras,
}

#[derive(Debug, Clone, Default)]
struct CameraOwnedExtras {
    #[cfg(feature = "game_client")]
    filter: game_client::display::view_filter::ViewFilterState,
    pending_focus_previous_target: Option<Vec3>,
}

fn codec() -> CameraCodec<MAX_BYTES> {
    CameraCodec
}

impl GameLogic {
    pub(crate) fn capture_camera_continuation(&self) -> Result<Vec<u8>, String> {
        let state = LogicCameraContinuation {
            pending_camera_focus: self.pending_camera_focus.clone(),
            script_camera_focus_estimate: self.script_camera_focus_estimate.clone(),
            script_camera_move_to: self.script_camera_move_to.clone(),
            script_camera_path: self.script_camera_path.clone(),
            camera_follow_target: self.camera_follow_target.clone(),
            camera_tether_play: self.camera_tether_play.clone(),
            script_look_toward_object_id: self.script_look_toward_object_id.clone(),
            script_look_toward_hold_seconds: self.script_look_toward_hold_seconds.clone(),
            script_default_camera_pitch: self.script_default_camera_pitch.clone(),
            script_default_camera_angle: self.script_default_camera_angle.clone(),
            script_default_camera_max_height: self.script_default_camera_max_height.clone(),
            script_camera_freeze_time_armed: self.script_camera_freeze_time_armed.clone(),
            script_camera_freeze_time: self.script_camera_freeze_time.clone(),
            script_camera_rotate_remaining: self.script_camera_rotate_remaining.clone(),
            script_camera_zoom_remaining: self.script_camera_zoom_remaining.clone(),
            script_camera_pitch_remaining: self.script_camera_pitch_remaining.clone(),
            script_camera_freeze_angle_armed: self.script_camera_freeze_angle_armed.clone(),
            script_camera_pending_final_speed_multiplier: self
                .script_camera_pending_final_speed_multiplier
                .clone(),
            visual_speed_multiplier: self.visual_speed_multiplier.clone(),
            script_time_frozen_by_script: self.script_time_frozen_by_script.clone(),
            pending_script_fps_limit: self.pending_script_fps_limit.clone(),
            pending_camera_zoom_reset: self.pending_camera_zoom_reset.clone(),
            pending_camera_zoom_reset_duration: self.pending_camera_zoom_reset_duration.clone(),
            pending_camera_zoom_reset_ease_in: self.pending_camera_zoom_reset_ease_in.clone(),
            pending_camera_zoom_reset_ease_out: self.pending_camera_zoom_reset_ease_out.clone(),
            pending_camera_zoom: self.pending_camera_zoom.clone(),
            pending_camera_pitch: self.pending_camera_pitch.clone(),
            pending_camera_rotate: self.pending_camera_rotate.clone(),
            pending_camera_look_toward: self.pending_camera_look_toward.clone(),
            pending_camera_slave_mode_enable: self.pending_camera_slave_mode_enable.clone(),
            pending_camera_slave_mode_disable: self.pending_camera_slave_mode_disable.clone(),
            pending_screen_shakes: self.pending_screen_shakes.clone(),
            pending_camera_add_shakers: self.pending_camera_add_shakers.clone(),
            pending_view_guardband: self.pending_view_guardband.clone(),
            native_view_guardband: self.native_view_guardband.clone(),
            pending_camera_bw_mode: self.pending_camera_bw_mode.clone(),
            pending_camera_motion_blur: self.pending_camera_motion_blur.clone(),
            script_skybox_enabled: self.script_skybox_enabled.clone(),
            cinematic_letterbox: self.cinematic_letterbox.clone(),
            hooks: self.mission_scripts.capture_camera_continuation()?,
            owned_extras: CameraOwnedExtras {
                #[cfg(feature = "game_client")]
                filter: self.session_view.filter.clone(),
                pending_focus_previous_target: self.session_view.pending_focus_previous_target,
            },
        };
        state.validate()?;
        owned_envelope::encode(&state)
    }
    pub(crate) fn decode_camera_continuation(
        bytes: &[u8],
    ) -> Result<LogicCameraContinuation, String> {
        if bytes.len() > MAX_BYTES {
            return Err("camera continuation exceeds 4 MiB".into());
        }
        let state = owned_envelope::decode(bytes)?;
        state.validate()?;
        Ok(state)
    }
    pub(crate) fn admit_camera_continuation(&mut self, state: LogicCameraContinuation) {
        #[cfg(feature = "game_client")]
        {
            self.session_view.filter = state.owned_extras.filter;
        }
        self.session_view.pending_focus_previous_target =
            state.owned_extras.pending_focus_previous_target;
        self.pending_camera_focus = state.pending_camera_focus;
        self.script_camera_focus_estimate = state.script_camera_focus_estimate;
        self.script_camera_move_to = state.script_camera_move_to;
        self.script_camera_path = state.script_camera_path;
        self.camera_follow_target = state.camera_follow_target;
        self.camera_tether_play = state.camera_tether_play;
        self.script_look_toward_object_id = state.script_look_toward_object_id;
        self.script_look_toward_hold_seconds = state.script_look_toward_hold_seconds;
        self.script_default_camera_pitch = state.script_default_camera_pitch;
        self.script_default_camera_angle = state.script_default_camera_angle;
        self.script_default_camera_max_height = state.script_default_camera_max_height;
        self.script_camera_freeze_time_armed = state.script_camera_freeze_time_armed;
        self.script_camera_freeze_time = state.script_camera_freeze_time;
        self.script_camera_rotate_remaining = state.script_camera_rotate_remaining;
        self.script_camera_zoom_remaining = state.script_camera_zoom_remaining;
        self.script_camera_pitch_remaining = state.script_camera_pitch_remaining;
        self.script_camera_freeze_angle_armed = state.script_camera_freeze_angle_armed;
        self.script_camera_pending_final_speed_multiplier =
            state.script_camera_pending_final_speed_multiplier;
        self.visual_speed_multiplier = state.visual_speed_multiplier;
        self.script_time_frozen_by_script = state.script_time_frozen_by_script;
        self.pending_script_fps_limit = state.pending_script_fps_limit;
        self.pending_camera_zoom_reset = state.pending_camera_zoom_reset;
        self.pending_camera_zoom_reset_duration = state.pending_camera_zoom_reset_duration;
        self.pending_camera_zoom_reset_ease_in = state.pending_camera_zoom_reset_ease_in;
        self.pending_camera_zoom_reset_ease_out = state.pending_camera_zoom_reset_ease_out;
        self.pending_camera_zoom = state.pending_camera_zoom;
        self.pending_camera_pitch = state.pending_camera_pitch;
        self.pending_camera_rotate = state.pending_camera_rotate;
        self.pending_camera_look_toward = state.pending_camera_look_toward;
        self.pending_camera_slave_mode_enable = state.pending_camera_slave_mode_enable;
        self.pending_camera_slave_mode_disable = state.pending_camera_slave_mode_disable;
        self.pending_screen_shakes = state.pending_screen_shakes;
        self.pending_camera_add_shakers = state.pending_camera_add_shakers;
        self.pending_view_guardband = state.pending_view_guardband;
        self.native_view_guardband = state.native_view_guardband;
        self.pending_camera_bw_mode = state.pending_camera_bw_mode;
        self.pending_camera_motion_blur = state.pending_camera_motion_blur;
        self.script_skybox_enabled = state.script_skybox_enabled;
        self.cinematic_letterbox = state.cinematic_letterbox;
        self.mission_scripts.admit_camera_continuation(state.hooks);
    }
}

#[path = "camera_continuation/validation.rs"]
mod validation;
use validation::ValidateCamera;

#[cfg(test)]
#[path = "camera_continuation_tests.rs"]
mod tests;

#[path = "camera_continuation/owned_envelope.rs"]
mod owned_envelope;

#[cfg(all(test, feature = "game_client"))]
#[path = "camera_continuation/filter_owner_tests.rs"]
mod filter_owner_tests;
