//! Host WaveGuideUpdate residual (dam flood wave after DamDie).
//!
//! C++: WaveGuide objects start DISABLED_DEFAULT; DamDie clears it. After
//! WaveDelay the wave initializes, moves along its facing, damages objects in
//! DamageRadius (DAMAGE_WATER / DEATH_FLOODED), topples props, sets FLOODED.
//!
//! Residual playability slice:
//! - WaveDelay 750ms before motion
//! - Speed 120 w/s residual (WaterWaveLocomotor)
//! - DamageRadius 25, DamageAmount 99999, ToppleForce 0.25
//! - Skip other WAVEGUIDE objects; set MODELCONDITION_FLOODED
//!
//! Fail-closed: not full shape transform / shoreline / bridge replace / water
//! height mesh push.

use serde::{Deserialize, Serialize};

pub const WAVE_GUIDE_LOGIC_FPS: f32 = 30.0;
pub const WAVE_DELAY_MS: u32 = 750;
pub const WAVE_SPEED_PER_SEC: f32 = 120.0;
pub const WAVE_DAMAGE_RADIUS: f32 = 25.0;
pub const WAVE_DAMAGE_AMOUNT: f32 = 99999.0;
pub const WAVE_TOPPLE_FORCE: f32 = 0.25;
/// C++ MODELCONDITION_FLOODED residual bit index.
pub const MC_BIT_FLOODED: u32 = 69;

#[inline]
pub fn ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * WAVE_GUIDE_LOGIC_FPS / 1000.0).round() as u32
}

pub fn wave_speed_per_frame() -> f32 {
    WAVE_SPEED_PER_SEC / WAVE_GUIDE_LOGIC_FPS
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostWaveGuideData {
    pub active_frame: u32,
    pub initialized: bool,
    pub done: bool,
    /// Facing residual (radians, yaw about Y).
    pub facing: f32,
    pub damage_applications: u32,
    pub topple_requests: u32,
}

impl Default for HostWaveGuideData {
    fn default() -> Self {
        Self {
            active_frame: 0,
            initialized: false,
            done: false,
            facing: 0.0,
            damage_applications: 0,
            topple_requests: 0,
        }
    }
}

impl HostWaveGuideData {
    pub fn ensure_active(&mut self, current_frame: u32) {
        if self.active_frame == 0 {
            self.active_frame = current_frame.max(1);
        }
    }

    pub fn is_moving(&self, current_frame: u32) -> bool {
        if self.done || self.active_frame == 0 {
            return false;
        }
        current_frame.saturating_sub(self.active_frame) >= ms_to_frames(WAVE_DELAY_MS)
    }

    /// Returns displacement (dx, dz) for this frame when moving.
    pub fn motion_delta(&mut self, current_frame: u32) -> Option<(f32, f32)> {
        if !self.is_moving(current_frame) {
            return None;
        }
        if !self.initialized {
            self.initialized = true;
        }
        let speed = wave_speed_per_frame();
        let dx = self.facing.cos() * speed;
        let dz = self.facing.sin() * speed;
        Some((dx, dz))
    }
}

/// True if template is a waveguide flood object.
pub fn is_wave_guide_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("waveguide") || n.contains("waterwave") || n.contains("floodwave")
}

/// C++ damage residual: objects in radius take unresistable flood damage.
pub fn wave_damage_at_distance(dist: f32) -> f32 {
    if dist <= WAVE_DAMAGE_RADIUS {
        WAVE_DAMAGE_AMOUNT
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wave_delay_then_moves() {
        let mut w = HostWaveGuideData::default();
        w.facing = 0.0; // +X
        w.ensure_active(10);
        assert!(w.motion_delta(10).is_none());
        let t = 10 + ms_to_frames(WAVE_DELAY_MS);
        let d = w.motion_delta(t).expect("moving");
        assert!(d.0 > 0.0);
        assert!((d.1).abs() < 0.01);
    }

    #[test]
    fn damage_inside_radius() {
        assert!(wave_damage_at_distance(10.0) >= 99999.0);
        assert_eq!(wave_damage_at_distance(40.0), 0.0);
    }
}
