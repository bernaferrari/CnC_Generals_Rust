//! Host ScudStormMissile live ballistic flight residual.
//!
//! Wraps `special_power_strikes` MissileAI peels into per-object flight state:
//! loft → turn → dive → HeightDie, then impact damage residual bookkeeping.
//!
//! Retail peels (WeaponObjects.ini `Object ScudStormMissile`):
//! - DistanceToTravelBeforeTurning **500**
//! - DistanceToTargetBeforeDiving **200**
//! - HeightDie TargetHeight **15**, OnlyWhenMovingDown
//! - Mass **500**, IgnitionFX **FX_ScudStormIgnition**
//! - ClipSize **9** scatter via `scud_storm_points`
//!
//! Fail-closed: not full MissileAIUpdate physics / locomotor spring matrix /
//! exhaust particle systems.

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::game_logic::special_power_strikes::{
    scud_missile_loft_phase, scud_missile_speed_per_frame, scud_missile_spawn_height,
    scud_missile_thrust_wobble, ScudMissileLoftPhase, SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET,
    SCUD_STORM_MISSILE_IGNITION_FX, SCUD_STORM_MISSILE_OBJECT, SCUD_STORM_MISSILE_COUNT,
    scud_storm_points,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostScudStormMissileFlightData {
    pub target: Vec3,
    pub launch: Vec3,
    pub traveled: f32,
    pub phase: ScudMissileLoftPhase,
    pub missile_index: u32,
    pub ignition_fx_played: bool,
    pub launcher_id: Option<u32>,
}

impl HostScudStormMissileFlightData {
    pub fn start(
        launch: Vec3,
        target: Vec3,
        missile_index: u32,
        launcher_id: Option<u32>,
    ) -> Self {
        let mut launch = launch;
        launch.y = scud_missile_spawn_height().max(launch.y);
        Self {
            target,
            launch,
            traveled: 0.0,
            phase: ScudMissileLoftPhase::Loft,
            missile_index,
            ignition_fx_played: false,
            launcher_id,
        }
    }

    /// One frame ballistic residual. Returns (pos, vel, grounded, ignition_fx).
    pub fn tick(&mut self, pos: Vec3, frame: u32) -> ScudMissileTick {
        let step = scud_missile_speed_per_frame();
        let to_target = Vec3::new(self.target.x - pos.x, 0.0, self.target.z - pos.z);
        let dist_h = (to_target.x * to_target.x + to_target.z * to_target.z).sqrt();
        let phase = scud_missile_loft_phase(self.traveled, dist_h, pos.y);
        self.phase = phase;

        let mut ignition_fx = false;
        if !self.ignition_fx_played {
            self.ignition_fx_played = true;
            ignition_fx = true;
        }

        if phase == ScudMissileLoftPhase::HeightDie
            || (pos.y <= SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET && self.traveled > 10.0)
        {
            return ScudMissileTick {
                pos: Vec3::new(self.target.x, self.target.y.max(0.0), self.target.z),
                vel: Vec3::ZERO,
                grounded: true,
                phase,
                ignition_fx: false,
            };
        }

        let mut new_pos = pos;
        let mut vel = Vec3::ZERO;
        let wobble = scud_missile_thrust_wobble(frame.wrapping_add(self.missile_index));

        match phase {
            ScudMissileLoftPhase::Loft => {
                // Climb + advance while no-turn distance residual.
                let horiz = step * 0.35;
                if dist_h > 1.0 {
                    new_pos.x += to_target.x / dist_h * horiz;
                    new_pos.z += to_target.z / dist_h * horiz;
                }
                new_pos.y += step * 0.85 + wobble * 0.5;
                vel = new_pos - pos;
                self.traveled += vel.length();
            }
            ScudMissileLoftPhase::Turn => {
                // Preferred height cruise toward target.
                let horiz = step;
                if dist_h > 1.0 {
                    new_pos.x += to_target.x / dist_h * horiz;
                    new_pos.z += to_target.z / dist_h * horiz;
                }
                // hold altitude residual near preferred
                let preferred = scud_missile_spawn_height() * 0.85;
                new_pos.y += (preferred - new_pos.y) * 0.08 + wobble * 0.3;
                vel = new_pos - pos;
                self.traveled += horiz;
            }
            ScudMissileLoftPhase::Dive => {
                let horiz = step * 0.5;
                if dist_h > 1.0 {
                    new_pos.x += to_target.x / dist_h * horiz;
                    new_pos.z += to_target.z / dist_h * horiz;
                } else {
                    new_pos.x = self.target.x;
                    new_pos.z = self.target.z;
                }
                new_pos.y -= step * 1.1;
                vel = new_pos - pos;
                self.traveled += vel.length();
                if new_pos.y <= SCUD_STORM_MISSILE_HEIGHT_DIE_TARGET {
                    self.phase = ScudMissileLoftPhase::HeightDie;
                    return ScudMissileTick {
                        pos: Vec3::new(self.target.x, self.target.y.max(0.0), self.target.z),
                        vel: Vec3::ZERO,
                        grounded: true,
                        phase: ScudMissileLoftPhase::HeightDie,
                        ignition_fx: false,
                    };
                }
            }
            ScudMissileLoftPhase::HeightDie => {
                return ScudMissileTick {
                    pos: Vec3::new(self.target.x, self.target.y.max(0.0), self.target.z),
                    vel: Vec3::ZERO,
                    grounded: true,
                    phase,
                    ignition_fx: false,
                };
            }
        }

        ScudMissileTick {
            pos: new_pos,
            vel,
            grounded: false,
            phase: self.phase,
            ignition_fx,
        }
    }
}

#[derive(Debug, Clone)]
pub struct ScudMissileTick {
    pub pos: Vec3,
    pub vel: Vec3,
    pub grounded: bool,
    pub phase: ScudMissileLoftPhase,
    pub ignition_fx: bool,
}

/// Scheduled ClipSize staggered spawn residual.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingScudMissileSpawn {
    pub spawn_frame: u32,
    pub source_id: u32,
    pub team_ordinal: u8,
    pub launch: Vec3,
    pub target: Vec3,
    pub missile_index: u32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HostScudStormMissileFlightRegistry {
    pub launched: u32,
    pub grounded: u32,
    pub ignition_fx: u32,
    pub exhaust_fx: u32,
    pub pending: Vec<PendingScudMissileSpawn>,
    pub scheduled: u32,
}

impl HostScudStormMissileFlightRegistry {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn clear(&mut self) {
        *self = Self::default();
    }
    pub fn record_launch(&mut self, n: u32) {
        self.launched = self.launched.saturating_add(n);
    }
    pub fn schedule_wave(
        &mut self,
        activate_frame: u32,
        source_id: u32,
        team_ordinal: u8,
        launch: Vec3,
        targets: &[Vec3],
    ) {
        use crate::game_logic::special_power_strikes::{
            scud_delay_between_frames, SCUD_STORM_PRE_ATTACK_FRAMES,
        };
        let mut frame = activate_frame.saturating_add(SCUD_STORM_PRE_ATTACK_FRAMES);
        for (i, target) in targets.iter().enumerate() {
            if i > 0 {
                frame = frame.saturating_add(scud_delay_between_frames(i as u32));
            }
            self.pending.push(PendingScudMissileSpawn {
                spawn_frame: frame,
                source_id,
                team_ordinal,
                launch,
                target: *target,
                missile_index: i as u32,
            });
            self.scheduled = self.scheduled.saturating_add(1);
        }
    }
    /// Drain pending spawns due on or before `frame`.
    pub fn take_due_spawns(&mut self, frame: u32) -> Vec<PendingScudMissileSpawn> {
        let mut due = Vec::new();
        let mut keep = Vec::new();
        for p in self.pending.drain(..) {
            if p.spawn_frame <= frame {
                due.push(p);
            } else {
                keep.push(p);
            }
        }
        self.pending = keep;
        due
    }
    pub fn record_ground(&mut self) {
        self.grounded = self.grounded.saturating_add(1);
    }
    pub fn record_ignition(&mut self) {
        self.ignition_fx = self.ignition_fx.saturating_add(1);
    }
    pub fn record_exhaust(&mut self) {
        self.exhaust_fx = self.exhaust_fx.saturating_add(1);
    }
    pub fn honesty_host_path_ok(&self) -> bool {
        self.launched > 0 || self.grounded > 0
    }
}

pub fn honesty_scud_storm_missile_flight_residual_ok() -> bool {
    SCUD_STORM_MISSILE_COUNT == 9
        && SCUD_STORM_MISSILE_OBJECT == "ScudStormMissile"
        && SCUD_STORM_MISSILE_IGNITION_FX == "FX_ScudStormIgnition"
        && crate::game_logic::special_power_strikes::SCUD_STORM_MISSILE_EXHAUST
            == "ScudMissileExhaust"
        && scud_storm_points(Vec3::ZERO).len() == 9
        && {
            let mut reg = HostScudStormMissileFlightRegistry::new();
            let pts = scud_storm_points(Vec3::new(100.0, 0.0, 0.0));
            reg.schedule_wave(0, 1, 0, Vec3::ZERO, &pts);
            reg.pending.len() == 9
                && reg.pending[0].spawn_frame
                    == crate::game_logic::special_power_strikes::SCUD_STORM_PRE_ATTACK_FRAMES
                && reg.pending[1].spawn_frame > reg.pending[0].spawn_frame
        }
        && {
            let mut d = HostScudStormMissileFlightData::start(
                Vec3::new(0.0, 0.0, 0.0),
                Vec3::new(400.0, 0.0, 0.0),
                0,
                None,
            );
            let mut pos = d.launch;
            let mut grounded = false;
            for f in 0..400 {
                let t = d.tick(pos, f);
                pos = t.pos;
                if t.grounded {
                    grounded = true;
                    break;
                }
            }
            grounded
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loft_reaches_height_die() {
        assert!(honesty_scud_storm_missile_flight_residual_ok());
    }
}
