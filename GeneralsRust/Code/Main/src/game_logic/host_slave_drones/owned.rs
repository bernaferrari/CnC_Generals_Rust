//! Original SlavedUpdate definition and instance-owned Xfer fields.
use crate::game_logic::ObjectId;
use glam::Vec3;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlavedUpdateMetadata {
    pub guard_max_range: f32,
    pub guard_wander_range: f32,
    pub attack_range: f32,
    pub attack_wander_range: f32,
    pub scout_range: f32,
    pub scout_wander_range: f32,
    pub range_bonus_distance: f32,
    pub repair_range: f32,
    pub repair_min_altitude: f32,
    pub repair_max_altitude: f32,
    pub repair_rate: f32,
    pub repair_priority_percent: i32,
    pub min_ready_frames: i32,
    pub max_ready_frames: i32,
    pub min_weld_frames: i32,
    pub max_weld_frames: i32,
    pub welding_system: String,
    pub welding_bone: String,
    pub stay_on_master_layer: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SlavedUpdateState {
    pub master: Option<ObjectId>,
    pub guard_offset: Vec3,
    pub frames_to_wait: i32,
    pub repair_state: super::BattleDroneRepairState,
    pub repairing: bool,
}
impl SlavedUpdateState {
    pub fn end_repair(&mut self) {
        if self.repair_state != super::BattleDroneRepairState::None {
            self.repair_state = super::BattleDroneRepairState::None;
            self.frames_to_wait = 7;
            self.repairing = false;
        }
    }
    pub fn validate(&self) -> bool {
        self.guard_offset.is_finite() && self.frames_to_wait >= 0
    }
}
