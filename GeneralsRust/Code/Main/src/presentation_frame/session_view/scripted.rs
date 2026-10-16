//! Scripted movement state belongs to the same SessionView as native pose.
//! Transport queues and positional save records remain separate adapters.
#[path = "path.rs"]
pub(crate) mod path;
use crate::game_logic::ObjectId;
use glam::Vec3;
use path::{ScriptCameraMoveTo, ScriptCameraPathMove};

#[derive(Debug)]
pub(crate) struct ScriptCameraState {
    pub(crate) script_camera_focus_estimate: Vec3,
    pub(crate) script_camera_move_to: Option<ScriptCameraMoveTo>,
    pub(crate) script_camera_path: Option<ScriptCameraPathMove>,
    pub(crate) camera_follow_target: Option<ObjectId>,
    pub(crate) camera_tether_play: Option<f32>,
    pub(crate) script_look_toward_object_id: Option<u32>,
    pub(crate) script_look_toward_hold_seconds: f32,
    pub(crate) script_default_camera_pitch: f32,
    pub(crate) script_default_camera_angle: f32,
    pub(crate) script_default_camera_max_height: f32,
    pub(crate) script_camera_freeze_time_armed: bool,
    pub(crate) script_camera_freeze_time: bool,
    pub(crate) script_camera_rotate_remaining: f32,
    pub(crate) script_camera_zoom_remaining: f32,
    pub(crate) script_camera_pitch_remaining: f32,
    pub(crate) script_camera_freeze_angle_armed: bool,
    pub(crate) script_camera_pending_final_speed_multiplier: Option<f32>,
}

impl Default for ScriptCameraState {
    fn default() -> Self {
        Self {
            script_camera_focus_estimate: Vec3::ZERO,
            script_camera_move_to: None,
            script_camera_path: None,
            camera_follow_target: None,
            camera_tether_play: None,
            script_look_toward_object_id: None,
            script_look_toward_hold_seconds: 0.0,
            script_default_camera_pitch: 0.0,
            script_default_camera_angle: 0.0,
            script_default_camera_max_height: 1.0,
            script_camera_freeze_time_armed: false,
            script_camera_freeze_time: false,
            script_camera_rotate_remaining: 0.0,
            script_camera_zoom_remaining: 0.0,
            script_camera_pitch_remaining: 0.0,
            script_camera_freeze_angle_armed: false,
            script_camera_pending_final_speed_multiplier: None,
        }
    }
}
