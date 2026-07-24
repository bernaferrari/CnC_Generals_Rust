//! Host HelicopterSlowDeathUpdate / HelicopterSlowDeathBehavior residual.
//!
//! C++ file: `HelicopterSlowDeathUpdate.cpp` (class `HelicopterSlowDeathBehavior`).
//! Extends SlowDeath with spiral orbit, self-spin, blade fly-off, ground hit.
//!
//! Retail Comanche peel (`AmericaCINEUnit.ini` / AmericaAir):
//! - SpiralOrbitTurnRate **140** deg/s → **~0.0814** rad/frame
//! - SpiralOrbitForwardSpeed **350** → **~11.67** world units/frame
//! - SpiralOrbitForwardSpeedDamping **0.9999**
//! - MinSelfSpin **100** / MaxSelfSpin **300** deg/s
//! - SelfSpinUpdateDelay **100**ms → **3**f, UpdateAmount **10** deg
//! - FallHowFast **12%** of gravity
//! - Min/MaxBladeFlyOffDelay **1500**ms → **45**f
//! - SoundDeathLoop `ComancheDamagedLoop`
//!
//! Fail-closed: not full blade OCL, particle attach bones, eject pilot OCL matrix.

use serde::{Deserialize, Serialize};

pub const HELI_SLOW_DEATH_LOGIC_FPS: f32 = 30.0;
const DEG_TO_RAD: f32 = std::f32::consts::PI / 180.0;

#[inline]
pub fn heli_deg_per_sec_to_rad_per_frame(deg_per_sec: f32) -> f32 {
    deg_per_sec * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS
}

#[inline]
pub fn heli_velocity_per_sec_to_per_frame(v: f32) -> f32 {
    v / HELI_SLOW_DEATH_LOGIC_FPS
}

#[inline]
pub fn heli_ms_to_frames(ms: u32) -> u32 {
    ((ms as f32) * HELI_SLOW_DEATH_LOGIC_FPS / 1000.0).round() as u32
}

/// Retail SpiralOrbitTurnRate 140 deg/s.
pub const COMANCHE_SPIRAL_ORBIT_TURN_RATE_DEG_PER_SEC: f32 = 140.0;
pub const HELI_SPIRAL_TURN_RATE: f32 =
    COMANCHE_SPIRAL_ORBIT_TURN_RATE_DEG_PER_SEC * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS;
/// Retail SpiralOrbitForwardSpeed 350 (dist/sec → /frame).
pub const COMANCHE_SPIRAL_FORWARD_SPEED_PER_SEC: f32 = 350.0;
pub const HELI_SPIRAL_FORWARD_SPEED: f32 =
    COMANCHE_SPIRAL_FORWARD_SPEED_PER_SEC / HELI_SLOW_DEATH_LOGIC_FPS;
/// Retail SpiralOrbitForwardSpeedDamping.
pub const HELI_SPIRAL_FORWARD_SPEED_DAMPING: f32 = 0.9999;
/// Retail MinSelfSpin / MaxSelfSpin deg/s.
pub const COMANCHE_MIN_SELF_SPIN_DEG_PER_SEC: f32 = 100.0;
pub const COMANCHE_MAX_SELF_SPIN_DEG_PER_SEC: f32 = 300.0;
pub const HELI_MIN_SELF_SPIN: f32 =
    COMANCHE_MIN_SELF_SPIN_DEG_PER_SEC * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS;
pub const HELI_MAX_SELF_SPIN: f32 =
    COMANCHE_MAX_SELF_SPIN_DEG_PER_SEC * DEG_TO_RAD / HELI_SLOW_DEATH_LOGIC_FPS;
/// Retail SelfSpinUpdateDelay 100ms.
pub const HELI_SELF_SPIN_UPDATE_DELAY_FRAMES: u32 = 3;
/// Retail SelfSpinUpdateAmount 10 deg → rad.
pub const HELI_SELF_SPIN_UPDATE_AMOUNT: f32 = 10.0 * DEG_TO_RAD;
/// Retail FallHowFast 12% of gravity magnitude residual.
pub const HELI_FALL_HOW_FAST: f32 = 0.12;
/// Host gravity magnitude residual (world Y down acceleration per frame² peel).
pub const HELI_GRAVITY_MAG: f32 = 0.5;
pub const HELI_CRASH_GRAVITY: f32 = -HELI_GRAVITY_MAG * HELI_FALL_HOW_FAST;
/// Retail blade fly-off delay 1500ms.
pub const HELI_BLADE_FLY_OFF_FRAMES: u32 = 45;
/// Frames after ground hit before destroy (host residual settle).
pub const HELI_GROUND_SETTLE_FRAMES: u32 = 30;
/// Retail SoundDeathLoop peel.
pub const HELI_SOUND_DEATH_LOOP: &str = "ComancheDamagedLoop";
/// Retail AttachParticle peel.
pub const HELI_ATTACH_PARTICLE: &str = "SootySmokeTrail";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostHelicopterSlowDeathData {
    pub active: bool,
    pub hit_ground: bool,
    pub hit_ground_frame: u32,
    pub activate_frame: u32,
    pub orbit_angle: f32,
    pub self_spin: f32,
    /// Direction of self-spin update (+1 / -1).
    pub self_spin_dir: f32,
    pub frames_since_spin_update: u32,
    pub forward_speed: f32,
    pub vertical_velocity: f32,
    pub orientation_delta: f32,
    pub blade_flew_off: bool,
    pub done: bool,
}

impl Default for HostHelicopterSlowDeathData {
    fn default() -> Self {
        Self {
            active: false,
            hit_ground: false,
            hit_ground_frame: 0,
            activate_frame: 0,
            orbit_angle: 0.0,
            self_spin: HELI_MIN_SELF_SPIN,
            self_spin_dir: 1.0,
            frames_since_spin_update: 0,
            forward_speed: HELI_SPIRAL_FORWARD_SPEED,
            vertical_velocity: 0.0,
            orientation_delta: 0.0,
            blade_flew_off: false,
            done: false,
        }
    }
}

impl HostHelicopterSlowDeathData {
    pub fn begin(&mut self) {
        self.begin_at_frame(0);
    }

    pub fn begin_at_frame(&mut self, frame: u32) {
        if self.active || self.done {
            return;
        }
        self.active = true;
        self.hit_ground = false;
        self.activate_frame = frame;
        self.vertical_velocity = 0.0;
        self.forward_speed = HELI_SPIRAL_FORWARD_SPEED;
        self.self_spin = HELI_MIN_SELF_SPIN;
        self.self_spin_dir = 1.0;
        self.frames_since_spin_update = 0;
        self.orientation_delta = 0.0;
        self.blade_flew_off = false;
    }

    pub fn is_active(&self) -> bool {
        self.active && !self.done
    }

    /// Tick crash. Returns (dx, dy, dz, d_orient, should_destroy, blade_fly_off_event).
    pub fn tick(
        &mut self,
        current_frame: u32,
        height_above_terrain: f32,
    ) -> (f32, f32, f32, f32, bool, bool) {
        if !self.active || self.done {
            return (0.0, 0.0, 0.0, 0.0, false, false);
        }

        let mut blade_event = false;
        if !self.blade_flew_off
            && current_frame.saturating_sub(self.activate_frame) >= HELI_BLADE_FLY_OFF_FRAMES
        {
            self.blade_flew_off = true;
            blade_event = true;
        }

        // Self-spin update residual (oscillate between min/max).
        self.frames_since_spin_update = self.frames_since_spin_update.saturating_add(1);
        if self.frames_since_spin_update >= HELI_SELF_SPIN_UPDATE_DELAY_FRAMES {
            self.frames_since_spin_update = 0;
            self.self_spin += self.self_spin_dir * HELI_SELF_SPIN_UPDATE_AMOUNT;
            if self.self_spin >= HELI_MAX_SELF_SPIN {
                self.self_spin = HELI_MAX_SELF_SPIN;
                self.self_spin_dir = -1.0;
            } else if self.self_spin <= HELI_MIN_SELF_SPIN {
                self.self_spin = HELI_MIN_SELF_SPIN;
                self.self_spin_dir = 1.0;
            }
        }

        if !self.hit_ground {
            self.orbit_angle += HELI_SPIRAL_TURN_RATE;
            let d_orient = self.self_spin + HELI_SPIRAL_TURN_RATE;
            self.orientation_delta += d_orient;
            let dx = self.orbit_angle.cos() * self.forward_speed;
            let dz = self.orbit_angle.sin() * self.forward_speed;
            self.forward_speed *= HELI_SPIRAL_FORWARD_SPEED_DAMPING;
            self.vertical_velocity += HELI_CRASH_GRAVITY;
            let dy = self.vertical_velocity;
            if height_above_terrain + dy <= 0.5 {
                self.hit_ground = true;
                self.hit_ground_frame = current_frame;
                self.vertical_velocity = 0.0;
                return (
                    dx,
                    -height_above_terrain.max(0.0),
                    dz,
                    d_orient,
                    false,
                    blade_event,
                );
            }
            return (dx, dy, dz, d_orient, false, blade_event);
        }

        if current_frame.saturating_sub(self.hit_ground_frame) >= HELI_GROUND_SETTLE_FRAMES {
            self.done = true;
            self.active = false;
            return (0.0, 0.0, 0.0, 0.0, true, blade_event);
        }
        (0.0, 0.0, 0.0, 0.0, false, blade_event)
    }
}

/// Alias: C++ source file name residual for port matrix matching.
pub type HelicopterSlowDeathUpdateData = HostHelicopterSlowDeathData;

pub fn is_helicopter_slow_death_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("comanche")
        || n.contains("chinook")
        || n.contains("helicopter")
        || n.contains("combatcopter")
        || (n.contains("helix") && !n.contains("napalm") && !n.contains("nuke"))
}

pub fn honesty_helicopter_slow_death_update_residual_ok() -> bool {
    (HELI_SPIRAL_TURN_RATE - heli_deg_per_sec_to_rad_per_frame(140.0)).abs() < 1.0e-5
        && (HELI_SPIRAL_FORWARD_SPEED - heli_velocity_per_sec_to_per_frame(350.0)).abs() < 1.0e-5
        && (HELI_SPIRAL_FORWARD_SPEED_DAMPING - 0.9999).abs() < 1.0e-6
        && HELI_BLADE_FLY_OFF_FRAMES == heli_ms_to_frames(1500)
        && HELI_SELF_SPIN_UPDATE_DELAY_FRAMES == heli_ms_to_frames(100)
        && HELI_SOUND_DEATH_LOOP == "ComancheDamagedLoop"
        && is_helicopter_slow_death_template("AmericaHelicopterComanche")
        && !is_helicopter_slow_death_template("AmericaTankCrusader")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn residual_pack_and_crash() {
        assert!(honesty_helicopter_slow_death_update_residual_ok());
        let mut h = HostHelicopterSlowDeathData::default();
        h.begin_at_frame(0);
        let mut height = 40.0;
        let mut destroyed = false;
        let mut blade = false;
        for f in 0..800 {
            let (dx, dy, dz, _, done, blade_ev) = h.tick(f, height);
            height = (height + dy).max(0.0);
            let _ = (dx, dz);
            if blade_ev {
                blade = true;
            }
            if done {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed);
        assert!(h.hit_ground);
        assert!(blade || h.blade_flew_off);
    }

    #[test]
    fn self_spin_stays_in_band() {
        let mut h = HostHelicopterSlowDeathData::default();
        h.begin();
        for f in 0..100 {
            let _ = h.tick(f, 100.0);
            assert!(h.self_spin >= HELI_MIN_SELF_SPIN - 1e-4);
            assert!(h.self_spin <= HELI_MAX_SELF_SPIN + 1e-4);
        }
    }
}
