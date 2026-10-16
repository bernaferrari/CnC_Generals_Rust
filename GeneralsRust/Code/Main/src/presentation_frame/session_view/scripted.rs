//! Scripted movement state belongs to the same SessionView as native pose.
//! Transport queues and positional save records remain separate adapters.
use super::waypoint::WaypointMotion;
use crate::game_logic::ObjectId;
use glam::Vec3;

/// Original rcInfo.trackObject payload. The frame counter lives only in
/// SessionCamera.camera_yaw_frame; an inactive payload retains its last target.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(crate) struct CameraObjectTracking {
    pub(crate) duration_ms: i32,
    pub(crate) turn_frames: u32,
    pub(crate) hold_frames: u32,
    pub(crate) last_position: Vec3,
    pub(crate) start_time_multiplier: i32,
    pub(crate) end_time_multiplier: i32,
}
impl CameraObjectTracking {
    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.duration_ms < 1
            || self.turn_frames != (self.duration_ms / 33).max(1) as u32
            || self.turn_frames == 0
            || self.turn_frames > i32::MAX as u32 / 33
            || self.hold_frames > i32::MAX as u32 / 33
            || self.turn_frames.checked_add(self.hold_frames).is_none()
            || !self.last_position.is_finite()
        {
            Err("invalid camera object tracking continuation".into())
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub(crate) struct ScriptCameraState {
    pub(crate) motion: WaypointMotion,
    pub(crate) path_arrived: bool,
    pub(crate) object_tracking: Option<CameraObjectTracking>,
    pub(crate) camera_follow_target: Option<ObjectId>,
    pub(crate) camera_tether_play: Option<f32>,
    pub(crate) script_look_toward_object_id: Option<u32>,
    pub(crate) script_default_camera_pitch: f32,
    pub(crate) script_default_camera_angle: f32,
    pub(crate) script_default_camera_max_height: f32,
    pub(crate) script_camera_freeze_time: bool,
}

impl Default for ScriptCameraState {
    fn default() -> Self {
        Self {
            motion: WaypointMotion::default(),
            path_arrived: false,
            object_tracking: None,
            camera_follow_target: None,
            camera_tether_play: None,
            script_look_toward_object_id: None,
            script_default_camera_pitch: 0.0,
            script_default_camera_angle: 0.0,
            script_default_camera_max_height: 1.0,
            script_camera_freeze_time: false,
        }
    }
}
