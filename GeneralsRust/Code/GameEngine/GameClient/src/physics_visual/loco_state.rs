//! Client-owned `DrawableLocoInfo` (`Drawable.h:102-135`, ctor `Drawable.cpp:161-186`).

use crate::drawable::{LocoInfo, WheelInfo};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PhysicsVisualLocoState {
    pub pitch: f32,
    pub pitch_rate: f32,
    pub roll: f32,
    pub roll_rate: f32,
    pub yaw: f32,
    pub acceleration_pitch: f32,
    pub acceleration_pitch_rate: f32,
    pub acceleration_roll: f32,
    pub acceleration_roll_rate: f32,
    pub overlap_z_vel: f32,
    pub overlap_z: f32,
    pub wobble: f32,
    pub yaw_modulator: f32,
    pub pitch_modulator: f32,
    pub front_left_height_offset: f32,
    pub front_right_height_offset: f32,
    pub rear_left_height_offset: f32,
    pub rear_right_height_offset: f32,
    pub wheel_angle: f32,
    pub frames_airborne_counter: i32,
    pub frames_airborne: i32,
}

impl Default for PhysicsVisualLocoState {
    fn default() -> Self {
        Self {
            pitch: 0.0,
            pitch_rate: 0.0,
            roll: 0.0,
            roll_rate: 0.0,
            yaw: 0.0,
            acceleration_pitch: 0.0,
            acceleration_pitch_rate: 0.0,
            acceleration_roll: 0.0,
            acceleration_roll_rate: 0.0,
            overlap_z_vel: 0.0,
            overlap_z: 0.0,
            wobble: 1.0,
            yaw_modulator: 0.0,
            pitch_modulator: 0.0,
            front_left_height_offset: 0.0,
            front_right_height_offset: 0.0,
            rear_left_height_offset: 0.0,
            rear_right_height_offset: 0.0,
            wheel_angle: 0.0,
            frames_airborne_counter: 0,
            frames_airborne: 0,
        }
    }
}

impl PhysicsVisualLocoState {
    #[must_use]
    pub fn from_loco_info(loco: &LocoInfo) -> Self {
        Self {
            pitch: loco.pitch,
            pitch_rate: loco.pitch_rate,
            roll: loco.roll,
            roll_rate: loco.roll_rate,
            yaw: loco.yaw,
            acceleration_pitch: loco.acceleration_pitch,
            acceleration_pitch_rate: loco.acceleration_pitch_rate,
            acceleration_roll: loco.acceleration_roll,
            acceleration_roll_rate: loco.acceleration_roll_rate,
            overlap_z_vel: loco.overlap_z_velocity,
            overlap_z: loco.overlap_z,
            wobble: loco.wobble,
            yaw_modulator: loco.yaw_modulator,
            pitch_modulator: loco.pitch_modulator,
            front_left_height_offset: loco.wheel_info.front_left_height_offset,
            front_right_height_offset: loco.wheel_info.front_right_height_offset,
            rear_left_height_offset: loco.wheel_info.rear_left_height_offset,
            rear_right_height_offset: loco.wheel_info.rear_right_height_offset,
            wheel_angle: loco.wheel_info.wheel_angle,
            frames_airborne_counter: loco.wheel_info.frames_airborne_counter,
            frames_airborne: loco.wheel_info.frames_airborne,
        }
    }

    pub fn write_to_loco_info(self, loco: &mut LocoInfo) {
        loco.pitch = self.pitch;
        loco.pitch_rate = self.pitch_rate;
        loco.roll = self.roll;
        loco.roll_rate = self.roll_rate;
        loco.yaw = self.yaw;
        loco.acceleration_pitch = self.acceleration_pitch;
        loco.acceleration_pitch_rate = self.acceleration_pitch_rate;
        loco.acceleration_roll = self.acceleration_roll;
        loco.acceleration_roll_rate = self.acceleration_roll_rate;
        loco.overlap_z_velocity = self.overlap_z_vel;
        loco.overlap_z = self.overlap_z;
        loco.wobble = self.wobble;
        loco.yaw_modulator = self.yaw_modulator;
        loco.pitch_modulator = self.pitch_modulator;
        loco.wheel_info = WheelInfo {
            front_left_height_offset: self.front_left_height_offset,
            front_right_height_offset: self.front_right_height_offset,
            rear_left_height_offset: self.rear_left_height_offset,
            rear_right_height_offset: self.rear_right_height_offset,
            wheel_angle: self.wheel_angle,
            frames_airborne_counter: self.frames_airborne_counter,
            frames_airborne: self.frames_airborne,
        };
    }
}
