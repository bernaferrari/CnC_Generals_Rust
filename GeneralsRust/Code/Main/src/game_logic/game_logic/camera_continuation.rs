//! Scripted continuation projected from the session view owner.
//! V2 owns the complete source-compatible motion; historical records are data only.
//! Original C++ View v1 and the historical positional Logic layout stay unchanged.
use super::*;
use crate::game_logic::mission_scripts::*;
use crate::presentation_frame::camera::continuation::SessionCameraState;
use crate::presentation_frame::scripted::CameraObjectTracking;
use crate::presentation_frame::waypoint::WaypointMotion;
use crate::save_load::camera_codec::CameraCodec;
use glam::Vec3;
use path_v1::{V1Ease, V1MoveTo, V1Path};
use serde::{Deserialize, Serialize};

const MAX_BYTES: usize = 4 * 1024 * 1024;
const MAX_ITEMS: usize = 4096;
#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct LogicCameraContinuation {
    pending_camera_focus: Option<Vec3>,
    script_camera_focus_estimate: Vec3,
    script_camera_move_to: Option<V1MoveTo>,
    script_camera_path: Option<V1Path>,
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
    motion: WaypointMotion,
    camera: Option<SessionCameraState>,
    target: Option<Vec3>,
    path_arrived: bool,
    object_tracking: Option<CameraObjectTracking>,
    follow_snap_immediate: bool,
}

fn codec() -> CameraCodec<MAX_BYTES> {
    CameraCodec
}

impl LogicCameraContinuation {
    pub(crate) fn owned_camera_state(&self) -> Option<&SessionCameraState> {
        self.owned_extras.camera.as_ref()
    }

    pub(crate) fn owned_follow_snap_immediate(&self) -> bool {
        self.owned_extras.follow_snap_immediate
    }

    pub(crate) fn owned_target(&self) -> Option<Vec3> {
        self.owned_extras.target
    }

    pub(crate) fn validate_native_projection(
        &self,
        camera: &crate::presentation_frame::camera::SessionCamera,
        target: Vec3,
    ) -> Result<(), String> {
        if let Some(owned) = &self.owned_extras.camera {
            owned.validate_native_projection(camera)?;
            if self
                .owned_extras
                .target
                .map(|value| value.to_array())
                .map(|v| v.map(f32::to_bits))
                != Some(target.to_array().map(f32::to_bits))
            {
                return Err("logic/native camera target continuation disagrees".into());
            }
        }
        Ok(())
    }

    fn reject_old_active_path(&self) -> Result<(), String> {
        if self.script_look_toward_object_id.is_some() {
            return Err(
                "unsupported old Rust active camera object tracking; missing source tracking state"
                    .into(),
            );
        }
        if self.script_camera_move_to.is_some() || self.script_camera_path.is_some() {
            return Err(
                "unsupported old Rust active camera path continuation; missing source motion state"
                    .into(),
            );
        }
        Ok(())
    }
}

#[path = "camera_continuation/path_v1.rs"]
mod path_v1;

impl GameLogic {
    pub(crate) fn capture_camera_continuation(&self) -> Result<Vec<u8>, String> {
        let camera = SessionCameraState::capture(&self.session_view.camera);
        let [rotate_remaining, zoom_remaining, pitch_remaining] =
            camera.remaining_axis_seconds(self.session_view.scripted.object_tracking.as_ref());
        let state = LogicCameraContinuation {
            pending_camera_focus: self.pending_camera_focus.clone(),
            script_camera_focus_estimate: self.session_view.target,
            script_camera_move_to: None,
            script_camera_path: None,
            camera_follow_target: self.session_view.scripted.camera_follow_target.clone(),
            camera_tether_play: self.session_view.scripted.camera_tether_play.clone(),
            script_look_toward_object_id: self
                .session_view
                .scripted
                .script_look_toward_object_id
                .clone(),
            script_look_toward_hold_seconds: self
                .session_view
                .scripted
                .object_tracking
                .as_ref()
                .map_or(0., |tracking| tracking.hold_frames as f32 * 33. / 1000.),
            script_default_camera_pitch: self
                .session_view
                .scripted
                .script_default_camera_pitch
                .clone(),
            script_default_camera_angle: self
                .session_view
                .scripted
                .script_default_camera_angle
                .clone(),
            script_default_camera_max_height: self
                .session_view
                .scripted
                .script_default_camera_max_height
                .clone(),
            script_camera_freeze_time_armed: false,
            script_camera_freeze_time: self.session_view.scripted.script_camera_freeze_time,
            script_camera_rotate_remaining: rotate_remaining,
            script_camera_zoom_remaining: zoom_remaining,
            script_camera_pitch_remaining: pitch_remaining,
            script_camera_freeze_angle_armed: false,
            script_camera_pending_final_speed_multiplier: None,
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
                motion: self.session_view.scripted.motion.clone(),
                camera: Some(camera),
                target: Some(self.session_view.target),
                path_arrived: self.session_view.scripted.path_arrived,
                object_tracking: self.session_view.scripted.object_tracking.clone(),
                follow_snap_immediate: self.session_view.camera.camera_follow_snap_immediate,
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
        self.session_view.scripted.motion = state.owned_extras.motion;
        self.session_view.scripted.path_arrived = state.owned_extras.path_arrived;
        self.session_view.scripted.object_tracking = state.owned_extras.object_tracking;
        self.session_view.target = state
            .owned_extras
            .target
            .unwrap_or(state.script_camera_focus_estimate);
        if let Some(camera) = state.owned_extras.camera {
            camera.admit(&mut self.session_view.camera);
        }
        self.session_view.camera.camera_follow_snap_immediate =
            state.owned_extras.follow_snap_immediate;
        self.session_view.scripted.camera_follow_target = state.camera_follow_target;
        self.session_view.scripted.camera_tether_play = state.camera_tether_play;
        self.session_view.scripted.script_look_toward_object_id =
            state.script_look_toward_object_id;
        self.session_view.scripted.script_default_camera_pitch = state.script_default_camera_pitch;
        self.session_view.scripted.script_default_camera_angle = state.script_default_camera_angle;
        self.session_view.scripted.script_default_camera_max_height =
            state.script_default_camera_max_height;
        self.session_view.scripted.script_camera_freeze_time = state.script_camera_freeze_time;
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
