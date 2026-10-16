//! Historical enriched Rust camera records. Data only: never tick these values.
//! Field order/types are the existing positional bincode contract.
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct V1Ease {
    pub(super) in_t: f32,
    pub(super) out_t: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct V1MoveTo {
    pub(super) start: Vec3,
    pub(super) target: Vec3,
    pub(super) ease: V1Ease,
    pub(super) total_time_seconds: f32,
    pub(super) elapsed_seconds: f32,
    pub(super) shutter_frames: u32,
    pub(super) cur_shutter: u32,
    pub(super) last_ease: f32,
    pub(super) freeze_time: bool,
    pub(super) freeze_angle: bool,
    pub(super) look_toward: Option<Vec3>,
    pub(super) suppress_travel_look: bool,
    pub(super) speed_ramp_start_t: f32,
    pub(super) speed_ramp_start_multiplier: f32,
    pub(super) speed_ramp_final_multiplier: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct V1Path {
    pub(super) points: Vec<Vec3>,
    pub(super) segment_length: Vec<f32>,
    pub(super) total_distance: f32,
    pub(super) ease: V1Ease,
    pub(super) total_time_seconds: f32,
    pub(super) elapsed_seconds: f32,
    pub(super) cur_segment: usize,
    pub(super) cur_seg_distance: f32,
    pub(super) shutter_frames: u32,
    pub(super) cur_shutter: u32,
    pub(super) last_ease: f32,
    pub(super) freeze_time: bool,
    pub(super) freeze_angle: bool,
    pub(super) look_toward: Option<Vec3>,
    pub(super) look_toward_is_final: bool,
    pub(super) suppress_travel_look: bool,
    pub(super) rolling_average_frames: i32,
    pub(super) smoothed_focus: Option<Vec3>,
    pub(super) speed_ramp_start_t: f32,
    pub(super) speed_ramp_start_multiplier: f32,
    pub(super) speed_ramp_final_multiplier: f32,
    pub(super) start_angle: f32,
    pub(super) frozen_to_start_angle: bool,
}
