//! Tint envelopes, fade mode, locomotor/wheel info, and related snapshot.

use super::*;
use game_engine::common::system::{Snapshotable, Xfer, XferVersion};

/// Wheel information for vehicles (converted from C++ TWheelInfo)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WheelInfo {
    pub front_left_height_offset: f32,
    pub front_right_height_offset: f32,
    pub rear_left_height_offset: f32,
    pub rear_right_height_offset: f32,
    pub wheel_angle: f32,
    pub frames_airborne_counter: i32,
    pub frames_airborne: i32,
}

impl Default for WheelInfo {
    fn default() -> Self {
        Self {
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

/// Locomotor information for drawable physics (converted from C++ DrawableLocoInfo)
#[derive(Debug, Clone, PartialEq)]
pub struct LocoInfo {
    pub pitch: f32,
    pub pitch_rate: f32,
    pub roll: f32,
    pub roll_rate: f32,
    pub yaw: f32,
    pub acceleration_pitch: f32,
    pub acceleration_pitch_rate: f32,
    pub acceleration_roll: f32,
    pub acceleration_roll_rate: f32,
    pub overlap_z_velocity: f32,
    pub overlap_z: f32,
    pub wobble: f32,
    pub yaw_modulator: f32,
    pub pitch_modulator: f32,
    pub wheel_info: WheelInfo,
}

impl Default for LocoInfo {
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
            overlap_z_velocity: 0.0,
            overlap_z: 0.0,
            wobble: 1.0,
            yaw_modulator: 0.0,
            pitch_modulator: 0.0,
            wheel_info: WheelInfo::default(),
        }
    }
}

pub const DEFAULT_TINT_COLOR_FADE_RATE: f32 = 0.6;
pub const DEF_ATTACK_FRAMES: u32 = 1;
pub const DEF_SUSTAIN_FRAMES: u32 = 1;
pub const DEF_DECAY_FRAMES: u32 = 4;
pub const SUSTAIN_INDEFINITELY: u32 = 0xfffffffe;
pub const VERY_TRANSPARENT_MATERIAL_PASS_OPACITY: f32 = 0.001;
pub const MATERIAL_PASS_OPACITY_FADE_SCALAR: f32 = 0.8;
pub const DRAWABLE_FRAMES_PER_FLASH: u32 = 15;

const FADE_RATE_EPSILON: f32 = 0.001;

fn vec_length(v: Vector3) -> f32 {
    (v.x * v.x + v.y * v.y + v.z * v.z).sqrt()
}

fn vec_sub(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x - b.x, a.y - b.y, a.z - b.z)
}

fn vec_add(a: Vector3, b: Vector3) -> Vector3 {
    Vector3::new(a.x + b.x, a.y + b.y, a.z + b.z)
}

fn vec_scale(v: Vector3, s: f32) -> Vector3 {
    Vector3::new(v.x * s, v.y * s, v.z * s)
}

pub(crate) fn snap_denorm(value: f32) -> f32 {
    if value > -1e-20 && value < 1e-20 {
        0.0
    } else {
        value
    }
}

pub(crate) fn envelope_state_to_u8(state: EnvelopeState) -> u8 {
    match state {
        EnvelopeState::Rest => 0,
        EnvelopeState::Attack => 1,
        EnvelopeState::Decay => 2,
        EnvelopeState::Sustain => 3,
    }
}

pub(crate) fn envelope_state_from_u8(value: u8) -> EnvelopeState {
    match value {
        1 => EnvelopeState::Attack,
        2 => EnvelopeState::Decay,
        3 => EnvelopeState::Sustain,
        _ => EnvelopeState::Rest,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FadingMode {
    None,
    FadingIn,
    FadingOut,
}

/// Tint envelope for color animation effects (converted from C++ TintEnvelope)
#[derive(Debug, Clone, PartialEq)]
pub struct TintEnvelope {
    pub attack_rate: Vector3,
    pub decay_rate: Vector3,
    pub peak_color: Vector3,
    pub current_color: Vector3,
    pub sustain_counter: u32,
    pub state: EnvelopeState,
    pub is_effective: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum EnvelopeState {
    Rest,
    Attack,
    Decay,
    Sustain,
}

impl Default for TintEnvelope {
    fn default() -> Self {
        Self::new()
    }
}

impl TintEnvelope {
    pub fn new() -> Self {
        Self {
            attack_rate: Vector3::zero(),
            decay_rate: Vector3::zero(),
            peak_color: Vector3::zero(),
            current_color: Vector3::zero(),
            sustain_counter: 0,
            state: EnvelopeState::Rest,
            is_effective: false,
        }
    }

    pub fn play(
        &mut self,
        peak_color: Vector3,
        attack_frames: u32,
        decay_frames: u32,
        sustain_frames: u32,
    ) {
        // C++ TintEnvelope::play + setAttackFrames/setDecayFrames.
        // Attack rate is (peak - current) / frames so signed peaks fade in
        // from the live color. Decay rate is -peak / frames and is *added*
        // during Decay. Completion uses vector length, not per-channel >=.
        self.peak_color = peak_color;
        self.set_attack_frames(attack_frames);
        self.set_decay_frames(decay_frames);
        self.state = EnvelopeState::Attack;
        self.sustain_counter = sustain_frames;
        self.is_effective = true;
        if vec_length(vec_sub(self.current_color, self.peak_color)) <= FADE_RATE_EPSILON {
            self.state = EnvelopeState::Sustain;
        }
    }

    fn set_attack_frames(&mut self, frames: u32) {
        let recip = 1.0 / frames.max(1) as f32;
        self.attack_rate = vec_scale(vec_sub(self.peak_color, self.current_color), recip);
    }

    fn set_decay_frames(&mut self, frames: u32) {
        let recip = -1.0 / frames.max(1) as f32;
        self.decay_rate = vec_scale(self.peak_color, recip);
    }

    pub fn sustain(&mut self) {
        self.state = EnvelopeState::Sustain;
    }

    pub fn release(&mut self) {
        self.state = EnvelopeState::Decay;
    }

    pub fn rest(&mut self) {
        self.state = EnvelopeState::Rest;
        self.current_color = Vector3::zero();
        self.is_effective = false;
    }

    pub fn color(&self) -> Vector3 {
        self.current_color
    }

    pub fn update(&mut self) {
        match self.state {
            EnvelopeState::Rest => {
                self.current_color = Vector3::zero();
                self.is_effective = false;
            }
            EnvelopeState::Decay => {
                let decay_len = vec_length(self.decay_rate);
                let current_len = vec_length(self.current_color);
                if decay_len > current_len || current_len <= FADE_RATE_EPSILON {
                    self.state = EnvelopeState::Rest;
                    self.is_effective = false;
                } else {
                    self.current_color = vec_add(self.decay_rate, self.current_color);
                    self.is_effective = true;
                }
            }
            EnvelopeState::Attack => {
                let delta = vec_sub(self.current_color, self.peak_color);
                let delta_len = vec_length(delta);
                if vec_length(self.attack_rate) > delta_len || delta_len <= FADE_RATE_EPSILON {
                    if self.sustain_counter != 0 {
                        self.state = EnvelopeState::Sustain;
                    } else {
                        self.state = EnvelopeState::Decay;
                    }
                } else {
                    self.current_color = vec_add(self.attack_rate, self.current_color);
                    self.is_effective = true;
                }
            }
            EnvelopeState::Sustain => {
                if self.sustain_counter == SUSTAIN_INDEFINITELY {
                    // C++ SUSTAIN_INDEFINITELY stays until release().
                } else if self.sustain_counter > 0 {
                    self.sustain_counter -= 1;
                } else {
                    self.release();
                }
            }
        }
    }
}

impl Snapshotable for TintEnvelope {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut attack_rate = self.attack_rate;
        xfer_vector3(xfer, &mut attack_rate)?;

        let mut decay_rate = self.decay_rate;
        xfer_vector3(xfer, &mut decay_rate)?;

        let mut peak_color = self.peak_color;
        xfer_vector3(xfer, &mut peak_color)?;

        let mut current_color = self.current_color;
        xfer_vector3(xfer, &mut current_color)?;

        let mut sustain_counter = self.sustain_counter;
        xfer.xfer_unsigned_int(&mut sustain_counter)
            .map_err(|e| format!("{:?}", e))?;

        let mut effective = self.is_effective;
        xfer.xfer_bool(&mut effective)
            .map_err(|e| format!("{:?}", e))?;

        let mut state = envelope_state_to_u8(self.state);
        xfer.xfer_unsigned_byte(&mut state)
            .map_err(|e| format!("{:?}", e))?;

        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        const CURRENT_VERSION: XferVersion = 1;
        let mut version = CURRENT_VERSION;
        xfer.xfer_version(&mut version, CURRENT_VERSION)
            .map_err(|e| format!("{:?}", e))?;

        xfer_vector3(xfer, &mut self.attack_rate)?;
        xfer_vector3(xfer, &mut self.decay_rate)?;
        xfer_vector3(xfer, &mut self.peak_color)?;
        xfer_vector3(xfer, &mut self.current_color)?;

        let mut sustain_counter = self.sustain_counter;
        xfer.xfer_unsigned_int(&mut sustain_counter)
            .map_err(|e| format!("{:?}", e))?;
        self.sustain_counter = sustain_counter;

        let mut effective = self.is_effective;
        xfer.xfer_bool(&mut effective)
            .map_err(|e| format!("{:?}", e))?;
        self.is_effective = effective;

        let mut state = envelope_state_to_u8(self.state);
        xfer.xfer_unsigned_byte(&mut state)
            .map_err(|e| format!("{:?}", e))?;
        self.state = envelope_state_from_u8(state);

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for LocoInfo {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut pitch = self.pitch;
        xfer.xfer_real(&mut pitch).map_err(|e| format!("{:?}", e))?;
        self.pitch = pitch;

        let mut pitch_rate = self.pitch_rate;
        xfer.xfer_real(&mut pitch_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.pitch_rate = pitch_rate;

        let mut roll = self.roll;
        xfer.xfer_real(&mut roll).map_err(|e| format!("{:?}", e))?;
        self.roll = roll;

        let mut roll_rate = self.roll_rate;
        xfer.xfer_real(&mut roll_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.roll_rate = roll_rate;

        let mut yaw = self.yaw;
        xfer.xfer_real(&mut yaw).map_err(|e| format!("{:?}", e))?;
        self.yaw = yaw;

        let mut accel_pitch = self.acceleration_pitch;
        xfer.xfer_real(&mut accel_pitch)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_pitch = accel_pitch;

        let mut accel_pitch_rate = self.acceleration_pitch_rate;
        xfer.xfer_real(&mut accel_pitch_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_pitch_rate = accel_pitch_rate;

        let mut accel_roll = self.acceleration_roll;
        xfer.xfer_real(&mut accel_roll)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_roll = accel_roll;

        let mut accel_roll_rate = self.acceleration_roll_rate;
        xfer.xfer_real(&mut accel_roll_rate)
            .map_err(|e| format!("{:?}", e))?;
        self.acceleration_roll_rate = accel_roll_rate;

        let mut overlap_z_velocity = self.overlap_z_velocity;
        xfer.xfer_real(&mut overlap_z_velocity)
            .map_err(|e| format!("{:?}", e))?;
        self.overlap_z_velocity = overlap_z_velocity;

        let mut overlap_z = self.overlap_z;
        xfer.xfer_real(&mut overlap_z)
            .map_err(|e| format!("{:?}", e))?;
        self.overlap_z = overlap_z;

        let mut wobble = self.wobble;
        xfer.xfer_real(&mut wobble)
            .map_err(|e| format!("{:?}", e))?;
        self.wobble = wobble;

        self.wheel_info.xfer(xfer)?;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}

impl Snapshotable for WheelInfo {
    fn crc(&self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut version: u8 = 0;
        xfer.xfer_version(&mut version, 1)
            .map_err(|e| e.to_string())?;
        Ok(())
    }

    fn xfer(&mut self, xfer: &mut dyn Xfer) -> Result<(), String> {
        let mut front_left_height_offset = self.front_left_height_offset;
        xfer.xfer_real(&mut front_left_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.front_left_height_offset = front_left_height_offset;

        let mut front_right_height_offset = self.front_right_height_offset;
        xfer.xfer_real(&mut front_right_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.front_right_height_offset = front_right_height_offset;

        let mut rear_left_height_offset = self.rear_left_height_offset;
        xfer.xfer_real(&mut rear_left_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.rear_left_height_offset = rear_left_height_offset;

        let mut rear_right_height_offset = self.rear_right_height_offset;
        xfer.xfer_real(&mut rear_right_height_offset)
            .map_err(|e| format!("{:?}", e))?;
        self.rear_right_height_offset = rear_right_height_offset;

        let mut wheel_angle = self.wheel_angle;
        xfer.xfer_real(&mut wheel_angle)
            .map_err(|e| format!("{:?}", e))?;
        self.wheel_angle = wheel_angle;

        let mut frames_airborne_counter = self.frames_airborne_counter;
        xfer.xfer_int(&mut frames_airborne_counter)
            .map_err(|e| format!("{:?}", e))?;
        self.frames_airborne_counter = frames_airborne_counter;

        let mut frames_airborne = self.frames_airborne;
        xfer.xfer_int(&mut frames_airborne)
            .map_err(|e| format!("{:?}", e))?;
        self.frames_airborne = frames_airborne;

        Ok(())
    }

    fn load_post_process(&mut self) -> Result<(), String> {
        Ok(())
    }
}
