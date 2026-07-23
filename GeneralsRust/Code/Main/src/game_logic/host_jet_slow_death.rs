//! Host JetSlowDeathBehavior residual (fixed-wing air crash death).
//!
//! C++: extends SlowDeath with roll, fall fraction of gravity, ground hit FX/OCL,
//! final blow-up delay.
//!
//! Residual playability slice:
//! - Air death: roll + accelerated gravity fall until near terrain
//! - Ground hit: settle frames then destroy
//! - Ground death (already on ground): short delay destroy
//!
//! Fail-closed: not full FX/OCL phase bursts or death loop audio.

use serde::{Deserialize, Serialize};

pub const JET_SLOW_DEATH_LOGIC_FPS: f32 = 30.0;
/// Default roll rate residual (rad/frame) for many jets.
pub const JET_DEFAULT_ROLL_RATE: f32 = 0.2;
/// Roll rate delta residual (100% = no change per frame).
pub const JET_DEFAULT_ROLL_RATE_DELTA: f32 = 1.0;
/// FallHowFast 110% of gravity residual.
pub const JET_FALL_HOW_FAST: f32 = 1.10;
/// Host gravity residual (world Y up, negative).
pub const JET_GRAVITY: f32 = -1.0;
/// Frames after ground hit before final destroy residual.
pub const JET_FINAL_BLOWUP_DELAY_FRAMES: u32 = 15;
/// Significantly-above-terrain residual threshold.
pub const JET_SIGNIFICANT_ALTITUDE: f32 = 3.0;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostJetSlowDeathData {
    pub active: bool,
    pub started_on_ground: bool,
    pub hit_ground: bool,
    pub hit_ground_frame: u32,
    pub roll_rate: f32,
    pub roll_rate_delta: f32,
    pub fall_how_fast: f32,
    pub vertical_velocity: f32,
    pub roll_accum: f32,
    pub done: bool,
}

impl Default for HostJetSlowDeathData {
    fn default() -> Self {
        Self {
            active: false,
            started_on_ground: false,
            hit_ground: false,
            hit_ground_frame: 0,
            roll_rate: JET_DEFAULT_ROLL_RATE,
            roll_rate_delta: JET_DEFAULT_ROLL_RATE_DELTA,
            fall_how_fast: JET_FALL_HOW_FAST,
            vertical_velocity: 0.0,
            roll_accum: 0.0,
            done: false,
        }
    }
}

impl HostJetSlowDeathData {
    pub fn begin(&mut self, height_above_terrain: f32) {
        if self.active || self.done {
            return;
        }
        self.active = true;
        self.started_on_ground = height_above_terrain < JET_SIGNIFICANT_ALTITUDE;
        self.hit_ground = self.started_on_ground;
        self.hit_ground_frame = 0;
        self.roll_rate = JET_DEFAULT_ROLL_RATE;
        self.roll_rate_delta = JET_DEFAULT_ROLL_RATE_DELTA;
        self.fall_how_fast = JET_FALL_HOW_FAST;
        self.vertical_velocity = 0.0;
        self.roll_accum = 0.0;
        if self.started_on_ground {
            // Ground death: quick final residual.
            self.hit_ground = true;
        }
    }

    pub fn is_active(&self) -> bool {
        self.active && !self.done
    }

    /// Returns (dy, d_roll, should_destroy).
    pub fn tick(&mut self, current_frame: u32, height_above_terrain: f32) -> (f32, f32, bool) {
        if !self.active || self.done {
            return (0.0, 0.0, false);
        }

        if self.started_on_ground {
            // C++ ground death path: FX then destroy soon.
            if self.hit_ground_frame == 0 {
                self.hit_ground_frame = current_frame;
            }
            if current_frame.saturating_sub(self.hit_ground_frame) >= 5 {
                self.done = true;
                self.active = false;
                return (0.0, 0.0, true);
            }
            return (0.0, 0.0, false);
        }

        if !self.hit_ground {
            // Air death: roll + fall
            let d_roll = self.roll_rate;
            self.roll_accum += d_roll;
            self.roll_rate *= self.roll_rate_delta;
            // Fall: gravity * fallHowFast (C++ fraction of gravity)
            self.vertical_velocity += JET_GRAVITY * self.fall_how_fast;
            let dy = self.vertical_velocity;
            if height_above_terrain + dy <= 0.5 {
                self.hit_ground = true;
                self.hit_ground_frame = current_frame;
                self.vertical_velocity = 0.0;
                return (-height_above_terrain.max(0.0), d_roll, false);
            }
            return (dy, d_roll, false);
        }

        // On ground: wait final blow-up delay
        if current_frame.saturating_sub(self.hit_ground_frame) >= JET_FINAL_BLOWUP_DELAY_FRAMES {
            self.done = true;
            self.active = false;
            return (0.0, 0.0, true);
        }
        (0.0, 0.0, false)
    }
}

pub fn is_jet_slow_death_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    if n.contains("comanche")
        || n.contains("chinook")
        || n.contains("helicopter")
        || n.contains("helix")
    {
        return false; // heli path
    }
    n.contains("raptor")
        || n.contains("aurora") && !n.contains("bomb")
        || n.contains("stealthfighter")
        || n.contains("stealth_fighter")
        || n.contains("mig")
        || n.contains("fighter")
        || n.contains("bomber")
        || n.contains("spectre")
        || n.contains("cargoplane")
        || n.contains("b52")
        || n.contains("jet")
        || n.contains("a10")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jet_air_crash_hits_and_finishes() {
        let mut j = HostJetSlowDeathData::default();
        j.begin(50.0);
        let mut h = 50.0;
        let mut done = false;
        for f in 0..400 {
            let (dy, _, destroy) = j.tick(f, h);
            h = (h + dy).max(0.0);
            if destroy {
                done = true;
                break;
            }
        }
        assert!(done);
        assert!(j.hit_ground);
    }

    #[test]
    fn jet_ground_death_quick() {
        let mut j = HostJetSlowDeathData::default();
        j.begin(0.5);
        assert!(j.started_on_ground);
        let mut done = false;
        for f in 0..20 {
            if j.tick(f, 0.5).2 {
                done = true;
                break;
            }
        }
        assert!(done);
    }
}
