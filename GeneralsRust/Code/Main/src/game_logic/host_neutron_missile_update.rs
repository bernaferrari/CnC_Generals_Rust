//! Host NeutronMissileUpdate residual (superweapon loft → intermediate → dive).
//!
//! C++: `NeutronMissileUpdate` states PRELAUNCH / LAUNCH / ATTACK / DEAD.
//! Retail peel (`NeutronMissile` in WeaponObjects.ini):
//! - DistanceToTravelBeforeTurning **300**
//! - MaxTurnRate **7200** (deg/sec residual honesty)
//! - ForwardDamping **0.1**, RelativeSpeed **2.0**
//! - TargetFromDirectlyAbove **500**
//! - SpecialSpeedTime **1500**ms → **45**f, SpecialSpeedHeight **160**
//! - SpecialJitterDistance **0.4**, STRAIGHT_DOWN_SLOW_FACTOR **0.5**
//!
//! Residual playability slice:
//! - Launch loft to special height, no-turn climb residual
//! - Aim intermediate above target, then straight-down dive
//! - Ground contact → complete (caller destroys / slow-death)
//!
//! Fail-closed: not full bone launch offset / delivery decal / jitter FX /
//! physics damping matrix / calcTransform turn modulation.

use glam::Vec3;
use serde::{Deserialize, Serialize};

pub const NEUTRON_NO_TURN_DIST: f32 = 300.0;
pub const NEUTRON_TARGET_FROM_ABOVE: f32 = 500.0;
pub const NEUTRON_SPECIAL_SPEED_TIME_MS: u32 = 1500;
pub const NEUTRON_SPECIAL_SPEED_TIME_FRAMES: u32 = 45; // 1500/1000*30
pub const NEUTRON_SPECIAL_SPEED_HEIGHT: f32 = 160.0;
pub const NEUTRON_RELATIVE_SPEED: f32 = 2.0;
pub const NEUTRON_FORWARD_DAMPING: f32 = 0.1;
pub const NEUTRON_STRAIGHT_DOWN_SLOW: f32 = 0.5;
pub const NEUTRON_MAX_TURN_RATE_DEG: f32 = 7200.0;
pub const NEUTRON_GROUND_EPSILON: f32 = 2.0;
/// Retail DeliveryDecalRadius residual.
pub const NEUTRON_DELIVERY_DECAL_RADIUS: f32 = 210.0;
pub const NEUTRON_LAUNCH_FX: &str = "FX_NeutronMissileLaunch";
pub const NEUTRON_IGNITION_FX: &str = "FX_NeutronMissileIgnition";
/// Host residual speed units per frame at RelativeSpeed=1.
pub const NEUTRON_BASE_SPEED_PER_FRAME: f32 = 12.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NeutronMissileFlightPhase {
    Prelaunch,
    Launch,
    AttackClimb,
    AttackDive,
    Dead,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostNeutronMissileUpdateData {
    pub phase: NeutronMissileFlightPhase,
    pub launcher_id: Option<u32>,
    pub target: Vec3,
    pub intermediate: Vec3,
    pub launch_pos: Vec3,
    pub no_turn_dist_left: f32,
    pub reached_intermediate: bool,
    pub launch_frame: u32,
    pub special_frames_left: u32,
    pub is_cruise: bool,
}

impl HostNeutronMissileUpdateData {
    /// Arm missile at launch toward world target (host Y-up).
    pub fn launch_at(
        launch_pos: Vec3,
        target: Vec3,
        launcher_id: Option<u32>,
        now: u32,
        is_cruise: bool,
    ) -> Self {
        let above = NEUTRON_TARGET_FROM_ABOVE;
        let intermediate = Vec3::new(target.x, target.y.max(0.0) + above, target.z);
        Self {
            phase: NeutronMissileFlightPhase::Launch,
            launcher_id,
            target,
            intermediate,
            launch_pos,
            no_turn_dist_left: NEUTRON_NO_TURN_DIST,
            reached_intermediate: false,
            launch_frame: now,
            special_frames_left: NEUTRON_SPECIAL_SPEED_TIME_FRAMES,
            is_cruise,
        }
    }

    pub fn for_template(
        template_name: &str,
        launch_pos: Vec3,
        target: Vec3,
        launcher_id: Option<u32>,
        now: u32,
    ) -> Option<Self> {
        if !is_neutron_missile_flight_template(template_name) {
            return None;
        }
        let is_cruise = template_name.to_ascii_lowercase().contains("cruise");
        Some(Self::launch_at(launch_pos, target, launcher_id, now, is_cruise))
    }

    /// One logic frame. Returns new position + velocity; `grounded` when dive hits terrain.
    pub fn tick(&mut self, pos: Vec3, _now: u32) -> NeutronMissileTick {
        if matches!(self.phase, NeutronMissileFlightPhase::Dead | NeutronMissileFlightPhase::Prelaunch)
        {
            return NeutronMissileTick {
                pos,
                vel: Vec3::ZERO,
                grounded: false,
                phase: self.phase,
            };
        }

        let speed = NEUTRON_BASE_SPEED_PER_FRAME * NEUTRON_RELATIVE_SPEED;
        let mut new_pos = pos;
        let mut vel = Vec3::ZERO;

        match self.phase {
            NeutronMissileFlightPhase::Launch => {
                // Special loft: climb SpecialSpeedHeight over SpecialSpeedTime.
                if self.special_frames_left > 0 {
                    let step = NEUTRON_SPECIAL_SPEED_HEIGHT
                        / NEUTRON_SPECIAL_SPEED_TIME_FRAMES as f32;
                    new_pos.y += step;
                    vel.y = step;
                    self.special_frames_left -= 1;
                    // horizontal hold during special loft residual
                } else {
                    self.phase = NeutronMissileFlightPhase::AttackClimb;
                }
            }
            NeutronMissileFlightPhase::AttackClimb => {
                let dest = if self.reached_intermediate {
                    self.target
                } else {
                    self.intermediate
                };
                let delta = dest - new_pos;
                let dist = delta.length().max(0.001);
                // no-turn residual: prefer vertical climb first
                if self.no_turn_dist_left > 0.0 && !self.reached_intermediate {
                    let climb = speed.min(self.no_turn_dist_left);
                    new_pos.y += climb;
                    vel.y = climb;
                    self.no_turn_dist_left -= climb;
                } else {
                    let step = speed.min(dist);
                    let dir = delta / dist;
                    new_pos += dir * step;
                    vel = dir * step;
                    // damping residual honesty
                    vel *= 1.0 - NEUTRON_FORWARD_DAMPING * 0.1;
                }
                if !self.reached_intermediate {
                    let d_inter = (new_pos - self.intermediate).length();
                    if d_inter < 25.0 || new_pos.y >= self.intermediate.y - 5.0 {
                        self.reached_intermediate = true;
                        new_pos.x = self.intermediate.x;
                        new_pos.z = self.intermediate.z;
                        // C++: vel becomes straight down * slow factor
                        let vlen = vel.length().max(speed * NEUTRON_STRAIGHT_DOWN_SLOW);
                        vel = Vec3::new(0.0, -vlen, 0.0);
                        self.phase = NeutronMissileFlightPhase::AttackDive;
                    }
                }
            }
            NeutronMissileFlightPhase::AttackDive => {
                let dest = self.target;
                let mut delta = dest - new_pos;
                // prefer down
                let down_speed = speed * NEUTRON_STRAIGHT_DOWN_SLOW;
                if (new_pos.x - dest.x).abs() > 1.0 || (new_pos.z - dest.z).abs() > 1.0 {
                    // snap horizontal residual toward target while diving
                    new_pos.x += (dest.x - new_pos.x) * 0.15;
                    new_pos.z += (dest.z - new_pos.z) * 0.15;
                }
                new_pos.y -= down_speed;
                vel = Vec3::new(0.0, -down_speed, 0.0);
                let ground_y = dest.y.max(0.0);
                if new_pos.y <= ground_y + NEUTRON_GROUND_EPSILON {
                    new_pos.y = ground_y;
                    self.phase = NeutronMissileFlightPhase::Dead;
                    return NeutronMissileTick {
                        pos: new_pos,
                        vel: Vec3::ZERO,
                        grounded: true,
                        phase: self.phase,
                    };
                }
                let _ = delta;
            }
            _ => {}
        }

        NeutronMissileTick {
            pos: new_pos,
            vel,
            grounded: false,
            phase: self.phase,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NeutronMissileTick {
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub phase: NeutronMissileFlightPhase,
}

pub fn is_neutron_missile_flight_template(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.contains("neutronmissile")
        || n.contains("nuclearmissile")
        || (n.contains("cruise") && n.contains("missile") && !n.contains("weapon"))
        || n == "cruisemissile"
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostNeutronMissileUpdateRegistry {
    pub launched: u32,
    pub intermediate_reached: u32,
    pub grounded: u32,
}

impl HostNeutronMissileUpdateRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_launch(&mut self) {
        self.launched = self.launched.saturating_add(1);
    }
    pub fn record_intermediate(&mut self) {
        self.intermediate_reached = self.intermediate_reached.saturating_add(1);
    }
    pub fn record_ground(&mut self) {
        self.grounded = self.grounded.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.launched > 0 || self.grounded > 0
    }
}

pub fn honesty_neutron_missile_update_residual_ok() -> bool {
    NEUTRON_SPECIAL_SPEED_TIME_FRAMES == 45
        && (NEUTRON_SPECIAL_SPEED_HEIGHT - 160.0).abs() < 0.1
        && (NEUTRON_TARGET_FROM_ABOVE - 500.0).abs() < 0.1
        && (NEUTRON_NO_TURN_DIST - 300.0).abs() < 0.1
        && (NEUTRON_DELIVERY_DECAL_RADIUS - 210.0).abs() < 0.1
        && NEUTRON_LAUNCH_FX == "FX_NeutronMissileLaunch"
        && is_neutron_missile_flight_template("NeutronMissile")
        && is_neutron_missile_flight_template("CruiseMissile")
        && !is_neutron_missile_flight_template("AmericaTankCrusader")
        && {
            let mut d = HostNeutronMissileUpdateData::launch_at(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(100.0, 0.0, 0.0),
                None,
                0,
                false,
            );
            let mut pos = Vec3::new(0.0, 0.0, 0.0);
            let mut saw_dive = false;
            let mut grounded = false;
            for f in 0..500 {
                let t = d.tick(pos, f);
                pos = t.pos;
                if matches!(t.phase, NeutronMissileFlightPhase::AttackDive) {
                    saw_dive = true;
                }
                if t.grounded {
                    grounded = true;
                    break;
                }
            }
            saw_dive && grounded && pos.y <= NEUTRON_GROUND_EPSILON + 0.1
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loft_then_dive_to_ground() {
        assert!(honesty_neutron_missile_update_residual_ok());
    }
}
