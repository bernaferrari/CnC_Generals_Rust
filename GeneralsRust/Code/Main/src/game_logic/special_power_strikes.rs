//! Host special-power / superweapon strike residual.
//!
//! Residual slice: host `DoSpecialPower` for DaisyCutter / A10 / ScudStorm /
//! ParticleCannon queues a real strike that completes with area damage on
//! host GameLogic objects. Fail-closed: not full retail OCL / aircraft /
//! beam / multiplayer superweapon parity.

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const SP_LOGIC_FPS: f32 = 30.0;

/// Host-supported superweapon strike kinds for this residual path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostSuperweaponKind {
    /// USA Daisy Cutter / Fuel Air Bomb / MOAB family.
    DaisyCutter,
    /// USA A-10 Thunderbolt missile strike.
    A10Strike,
    /// GLA SCUD Storm.
    ScudStorm,
    /// China Particle Uplink Cannon (ParticleUprising residual host path).
    ParticleCannon,
}

impl HostSuperweaponKind {
    /// Map a command-system power type to a host residual strike, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::DaisyCutter | SpecialPowerType::FuelAirBomb => {
                Some(HostSuperweaponKind::DaisyCutter)
            }
            SpecialPowerType::Airstrike => Some(HostSuperweaponKind::A10Strike),
            SpecialPowerType::ScudStorm => Some(HostSuperweaponKind::ScudStorm),
            SpecialPowerType::ParticleCannon => Some(HostSuperweaponKind::ParticleCannon),
            _ => None,
        }
    }

    /// Human-readable label for logs / honesty reports.
    pub fn label(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "DaisyCutter",
            HostSuperweaponKind::A10Strike => "A10Strike",
            HostSuperweaponKind::ScudStorm => "ScudStorm",
            HostSuperweaponKind::ParticleCannon => "ParticleCannon",
        }
    }

    /// Impact delay in logic frames before area damage applies.
    pub fn impact_delay_frames(self) -> u32 {
        match self {
            // FuelAirBombPower residual: impact_delay 3.0s @ 30 FPS.
            HostSuperweaponKind::DaisyCutter => 90,
            // A-10 flight/approach residual (shorter than full aircraft OCL).
            HostSuperweaponKind::A10Strike => 60,
            // SCUD launch-to-impact residual.
            HostSuperweaponKind::ScudStorm => 150,
            // Particle cannon charge residual (beam dwell deferred).
            HostSuperweaponKind::ParticleCannon => 120,
        }
    }

    /// Max damage at epicenter (host residual values; retail weapon tables deferred).
    pub fn max_damage(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 2000.0,
            HostSuperweaponKind::A10Strike => 500.0,
            HostSuperweaponKind::ScudStorm => 1500.0,
            HostSuperweaponKind::ParticleCannon => 3000.0,
        }
    }

    /// Outer damage radius (matches SpecialPower.ini RadiusCursorRadius where known).
    pub fn damage_radius(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 170.0,
            HostSuperweaponKind::A10Strike => 100.0,
            HostSuperweaponKind::ScudStorm => 200.0,
            HostSuperweaponKind::ParticleCannon => 50.0,
        }
    }

    /// Inner radius with full damage (two-stage falloff).
    pub fn falloff_inner(self) -> f32 {
        match self {
            HostSuperweaponKind::DaisyCutter => 100.0,
            HostSuperweaponKind::A10Strike => 40.0,
            HostSuperweaponKind::ScudStorm => 80.0,
            HostSuperweaponKind::ParticleCannon => 25.0,
        }
    }

    /// Audio event name queued on activation (host residual).
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "SuperweaponDaisyCutter",
            HostSuperweaponKind::A10Strike => "SuperweaponA10Strike",
            HostSuperweaponKind::ScudStorm => "SuperweaponScudStorm",
            HostSuperweaponKind::ParticleCannon => "SuperweaponParticleCannon",
        }
    }

    /// Audio event name queued on impact (host residual).
    pub fn impact_audio(self) -> &'static str {
        match self {
            HostSuperweaponKind::DaisyCutter => "DaisyCutterExplosion",
            HostSuperweaponKind::A10Strike => "A10StrikeImpact",
            HostSuperweaponKind::ScudStorm => "ScudStormImpact",
            HostSuperweaponKind::ParticleCannon => "ParticleCannonImpact",
        }
    }
}

/// Lifecycle of a queued host superweapon strike.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostStrikePhase {
    /// Queued after DoSpecialPower; waiting for impact frame.
    Queued,
    /// Impact resolved; area damage applied.
    Completed,
    /// Cancelled (source died / invalid) before impact.
    Cancelled,
}

/// One pending or completed host superweapon strike.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostSpecialPowerStrike {
    pub id: u32,
    pub kind: HostSuperweaponKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub impact_frame: u32,
    pub phase: HostStrikePhase,
    /// Total damage dealt across all hit objects at impact.
    pub total_damage_applied: f32,
    /// Number of enemy/neutral objects that received damage.
    pub objects_hit: u32,
    /// Number of objects destroyed by this strike.
    pub objects_destroyed: u32,
}

/// Damage application plan for a single victim (computed before mutable apply).
#[derive(Debug, Clone, Copy)]
pub struct HostStrikeDamageHit {
    pub target_id: ObjectId,
    pub damage: f32,
}

/// Result of resolving one strike at impact time.
#[derive(Debug, Clone)]
pub struct HostStrikeImpactPlan {
    pub strike_id: u32,
    pub kind: HostSuperweaponKind,
    pub target_position: Vec3,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub hits: Vec<HostStrikeDamageHit>,
}

/// Host registry of superweapon strikes that queue and complete.
#[derive(Debug, Clone, Default)]
pub struct HostSpecialPowerStrikeRegistry {
    next_id: u32,
    strikes: HashMap<u32, HostSpecialPowerStrike>,
    /// Strikes that completed impact this frame (presentation / honesty drain).
    completed_this_frame: Vec<u32>,
    /// Strikes activated this frame.
    activated_this_frame: Vec<u32>,
}

impl HostSpecialPowerStrikeRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            strikes: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.strikes.clear();
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.next_id = 1;
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
    }

    pub fn strike_count(&self) -> usize {
        self.strikes.len()
    }

    pub fn pending_count(&self) -> usize {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Queued)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostSpecialPowerStrike> {
        self.strikes.get(&id)
    }

    pub fn strikes_snapshot(&self) -> Vec<HostSpecialPowerStrike> {
        let mut v: Vec<_> = self.strikes.values().cloned().collect();
        v.sort_by_key(|s| s.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostSuperweaponKind) -> Vec<&HostSpecialPowerStrike> {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Queued && s.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostSuperweaponKind) -> Vec<&HostSpecialPowerStrike> {
        self.strikes
            .values()
            .filter(|s| s.phase == HostStrikePhase::Completed && s.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Queue a superweapon strike. Returns host strike id.
    pub fn queue(
        &mut self,
        kind: HostSuperweaponKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let impact_frame = activate_frame.saturating_add(kind.impact_delay_frames());
        let strike = HostSpecialPowerStrike {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            impact_frame,
            phase: HostStrikePhase::Queued,
            total_damage_applied: 0.0,
            objects_hit: 0,
            objects_destroyed: 0,
        };
        self.strikes.insert(id, strike);
        self.activated_this_frame.push(id);
        id
    }

    /// Compute falloff damage for distance from epicenter.
    pub fn damage_at_distance(kind: HostSuperweaponKind, distance: f32) -> f32 {
        let radius = kind.damage_radius();
        let inner = kind.falloff_inner();
        let max = kind.max_damage();
        if distance <= inner {
            max
        } else if distance >= radius {
            0.0
        } else {
            let range = (radius - inner).max(f32::EPSILON);
            let t = (distance - inner) / range;
            max * (1.0 - t).max(0.0)
        }
    }

    /// Build impact damage plans for all strikes whose impact frame has arrived.
    /// Does not mutate object health — GameLogic applies hits.
    pub fn plan_due_impacts(
        &self,
        current_frame: u32,
        object_positions: &[(ObjectId, Vec3, super::Team, bool)],
    ) -> Vec<HostStrikeImpactPlan> {
        let mut plans = Vec::new();
        for strike in self.strikes.values() {
            if strike.phase != HostStrikePhase::Queued || current_frame < strike.impact_frame {
                continue;
            }
            let mut hits = Vec::new();
            for &(id, pos, team, alive) in object_positions {
                if !alive || id == strike.source_object {
                    continue;
                }
                // Fail-closed residual: do not damage friendlies (same team).
                if team == strike.source_team {
                    continue;
                }
                let dist = horizontal_distance(pos, strike.target_position);
                let dmg = Self::damage_at_distance(strike.kind, dist);
                if dmg > 0.0 {
                    hits.push(HostStrikeDamageHit {
                        target_id: id,
                        damage: dmg,
                    });
                }
            }
            plans.push(HostStrikeImpactPlan {
                strike_id: strike.id,
                kind: strike.kind,
                target_position: strike.target_position,
                source_object: strike.source_object,
                source_team: strike.source_team,
                hits,
            });
        }
        plans.sort_by_key(|p| p.strike_id);
        plans
    }

    /// Record impact results after GameLogic applied damage.
    pub fn record_impact_complete(
        &mut self,
        strike_id: u32,
        total_damage: f32,
        objects_hit: u32,
        objects_destroyed: u32,
    ) {
        if let Some(strike) = self.strikes.get_mut(&strike_id) {
            if strike.phase == HostStrikePhase::Queued {
                strike.phase = HostStrikePhase::Completed;
                strike.total_damage_applied = total_damage;
                strike.objects_hit = objects_hit;
                strike.objects_destroyed = objects_destroyed;
                self.completed_this_frame.push(strike_id);
            }
        }
    }

    /// Cancel pending strikes owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for strike in self.strikes.values_mut() {
            if strike.source_object == source && strike.phase == HostStrikePhase::Queued {
                strike.phase = HostStrikePhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    /// True if at least one strike of `kind` is currently queued.
    pub fn honesty_queue_ok(&self, kind: HostSuperweaponKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one strike of `kind` completed with damage applied
    /// (or completed cleanly with zero victims in radius — still "completed").
    pub fn honesty_complete_ok(&self, kind: HostSuperweaponKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|s| s.phase == HostStrikePhase::Completed)
    }

    /// Combined host path honesty: a completed strike exists for `kind`.
    pub fn honesty_host_path_ok(&self, kind: HostSuperweaponKind) -> bool {
        self.honesty_complete_ok(kind)
    }
}

fn horizontal_distance(a: Vec3, b: Vec3) -> f32 {
    let dx = a.x - b.x;
    let dz = a.z - b.z;
    (dx * dx + dz * dz).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn daisy_cutter_maps_from_command_powers() {
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::DaisyCutter),
            Some(HostSuperweaponKind::DaisyCutter)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::FuelAirBomb),
            Some(HostSuperweaponKind::DaisyCutter)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::Airstrike),
            Some(HostSuperweaponKind::A10Strike)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::ScudStorm),
            Some(HostSuperweaponKind::ScudStorm)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::ParticleCannon),
            Some(HostSuperweaponKind::ParticleCannon)
        );
        assert_eq!(
            HostSuperweaponKind::from_command_power(&SpecialPowerType::RadarScan),
            None
        );
    }

    #[test]
    fn queue_and_complete_daisy_cutter_damage_plan() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        let id = reg.queue(
            HostSuperweaponKind::DaisyCutter,
            ObjectId(1),
            Team::USA,
            Vec3::new(100.0, 0.0, 100.0),
            0,
        );
        assert!(reg.honesty_queue_ok(HostSuperweaponKind::DaisyCutter));
        assert!(!reg.honesty_complete_ok(HostSuperweaponKind::DaisyCutter));

        let strike = reg.get(id).expect("strike");
        assert_eq!(strike.impact_frame, 90);
        assert_eq!(strike.phase, HostStrikePhase::Queued);

        // Before impact frame: no plans.
        let objects = vec![
            (ObjectId(1), Vec3::new(0.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(2), Vec3::new(100.0, 0.0, 100.0), Team::GLA, true),
            (ObjectId(3), Vec3::new(500.0, 0.0, 500.0), Team::GLA, true),
        ];
        assert!(reg.plan_due_impacts(89, &objects).is_empty());

        let plans = reg.plan_due_impacts(90, &objects);
        assert_eq!(plans.len(), 1);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(2));
        assert!((plans[0].hits[0].damage - 2000.0).abs() < 0.01);

        reg.record_impact_complete(id, 2000.0, 1, 1);
        assert!(reg.honesty_complete_ok(HostSuperweaponKind::DaisyCutter));
        assert!(reg.honesty_host_path_ok(HostSuperweaponKind::DaisyCutter));
        assert_eq!(reg.get(id).unwrap().phase, HostStrikePhase::Completed);
    }

    #[test]
    fn falloff_two_stage_matches_fab_shape() {
        let kind = HostSuperweaponKind::DaisyCutter;
        assert!((HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 0.0) - 2000.0).abs() < 0.1);
        assert!(
            (HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 100.0) - 2000.0).abs() < 0.1
        );
        let mid = HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 135.0);
        assert!((mid - 1000.0).abs() < 1.0, "mid falloff expected ~1000, got {mid}");
        assert_eq!(
            HostSpecialPowerStrikeRegistry::damage_at_distance(kind, 170.0),
            0.0
        );
    }

    #[test]
    fn friendly_fire_excluded_from_plan() {
        let mut reg = HostSpecialPowerStrikeRegistry::new();
        reg.queue(
            HostSuperweaponKind::A10Strike,
            ObjectId(1),
            Team::USA,
            Vec3::ZERO,
            0,
        );
        let objects = vec![
            (ObjectId(1), Vec3::ZERO, Team::USA, true),
            (ObjectId(2), Vec3::new(5.0, 0.0, 0.0), Team::USA, true),
            (ObjectId(3), Vec3::new(5.0, 0.0, 0.0), Team::China, true),
        ];
        let plans = reg.plan_due_impacts(60, &objects);
        assert_eq!(plans[0].hits.len(), 1);
        assert_eq!(plans[0].hits[0].target_id, ObjectId(3));
    }
}
