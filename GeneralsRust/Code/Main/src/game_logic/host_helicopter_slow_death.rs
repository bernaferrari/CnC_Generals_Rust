//! Host HelicopterSlowDeathBehavior residual (spiral crash for rotorcraft).
//!
//! C++: extends SlowDeath with spiral orbit, self-spin, blade fly-off, ground hit.
//! Residual playability slice:
//! - Spiral yaw spin + forward drift while airborne
//! - Gravity sink after activation
//! - Ground hit → short settle → destroy / rubble spawn peel
//!
//! Fail-closed: not full blade OCL, particle attach bones, or audio matrix.

use serde::{Deserialize, Serialize};

pub const HELI_SLOW_DEATH_LOGIC_FPS: f32 = 30.0;
/// Default spiral turn rate residual (rad/frame peel).
pub const HELI_SPIRAL_TURN_RATE: f32 = 0.08;
/// Default spiral forward speed residual (world units/frame).
pub const HELI_SPIRAL_FORWARD_SPEED: f32 = 0.6;
/// Self spin residual (rad/frame).
pub const HELI_SELF_SPIN: f32 = 0.12;
/// Gravity residual while crashing.
pub const HELI_CRASH_GRAVITY: f32 = -0.35;
/// Frames after ground hit before destroy.
pub const HELI_GROUND_SETTLE_FRAMES: u32 = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostHelicopterSlowDeathData {
    pub active: bool,
    pub hit_ground: bool,
    pub hit_ground_frame: u32,
    pub orbit_angle: f32,
    pub self_spin: f32,
    pub forward_speed: f32,
    pub vertical_velocity: f32,
    /// Presentation orientation delta accumulated.
    pub orientation_delta: f32,
    pub done: bool,
}

impl Default for HostHelicopterSlowDeathData {
    fn default() -> Self {
        Self {
            active: false,
            hit_ground: false,
            hit_ground_frame: 0,
            orbit_angle: 0.0,
            self_spin: HELI_SELF_SPIN,
            forward_speed: HELI_SPIRAL_FORWARD_SPEED,
            vertical_velocity: 0.0,
            orientation_delta: 0.0,
            done: false,
        }
    }
}

impl HostHelicopterSlowDeathData {
    pub fn begin(&mut self) {
        if self.active || self.done {
            return;
        }
        self.active = true;
        self.hit_ground = false;
        self.vertical_velocity = 0.0;
        self.forward_speed = HELI_SPIRAL_FORWARD_SPEED;
        self.self_spin = HELI_SELF_SPIN;
        self.orientation_delta = 0.0;
    }

    pub fn is_active(&self) -> bool {
        self.active && !self.done
    }

    /// Tick crash. Returns (dx, dy, dz, d_orient) to apply; true when should destroy.
    pub fn tick(
        &mut self,
        current_frame: u32,
        height_above_terrain: f32,
    ) -> (f32, f32, f32, f32, bool) {
        if !self.active || self.done {
            return (0.0, 0.0, 0.0, 0.0, false);
        }
        if !self.hit_ground {
            // Spiral yaw
            self.orbit_angle += HELI_SPIRAL_TURN_RATE;
            self.orientation_delta += self.self_spin + HELI_SPIRAL_TURN_RATE;
            // Forward in spiral tangent
            let dx = self.orbit_angle.cos() * self.forward_speed;
            let dz = self.orbit_angle.sin() * self.forward_speed;
            self.forward_speed *= 0.998; // damping residual
                                         // Fall
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
                    self.self_spin,
                    false,
                );
            }
            return (dx, dy, dz, self.self_spin, false);
        }
        // Settled on ground
        if current_frame.saturating_sub(self.hit_ground_frame) >= HELI_GROUND_SETTLE_FRAMES {
            self.done = true;
            self.active = false;
            return (0.0, 0.0, 0.0, 0.0, true);
        }
        (0.0, 0.0, 0.0, 0.0, false)
    }
}

pub fn is_helicopter_slow_death_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("comanche")
        || n.contains("chinook")
        || n.contains("helicopter")
        || n.contains("heliox")
        || n.contains("combatcopter")
        || n.contains("helix") && !n.contains("napalm")
        || n.contains("hornet") && n.contains("helicopter")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heli_crash_hits_ground_and_finishes() {
        let mut h = HostHelicopterSlowDeathData::default();
        h.begin();
        let mut height = 40.0;
        let mut destroyed = false;
        for f in 0..600 {
            let (dx, dy, dz, _, done) = h.tick(f, height);
            height = (height + dy).max(0.0);
            let _ = (dx, dz);
            if done {
                destroyed = true;
                break;
            }
        }
        assert!(destroyed);
        assert!(h.hit_ground);
    }
}
