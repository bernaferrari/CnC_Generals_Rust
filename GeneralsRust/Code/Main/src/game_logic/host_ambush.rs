//! Host GLA Rebel Ambush special-power residual.
//!
//! Residual slice: host `DoSpecialPower` for SPECIAL_AMBUSH / SuperweaponRebelAmbush
//! queues a spawn at the target location. After a residual fade/approach delay
//! (retail OCL FadeTime = 3000 ms), infantry units spawn near the target
//! (spread-formation residual).
//!
//! Fail-closed honesty:
//! - Not full OCL CreateObject / FadeIn module path
//! - Not full science upgrade OCL matrix (Ambush2/3 payload tiers)
//! - Not OCLAdjustPositionToPassable / DiesOnBadLand water-drown path
//! - Not SharedSyncedTimer / multiplayer academy classification

use super::ObjectId;
use crate::command_system::SpecialPowerType;
use glam::Vec3;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Logic frames per second (host fixed step).
pub const AMBUSH_LOGIC_FPS: f32 = 30.0;

/// Residual unit count for Rebel Ambush1 (retail SUPERWEAPON_RebelAmbush1 Count = 4).
pub const GLA_AMBUSH1_UNIT_COUNT: u32 = 4;

/// Residual scatter radius around target (RadiusCursorRadius = 50 residual).
pub const AMBUSH_SPAWN_RADIUS: f32 = 40.0;

/// Residual infantry template used when retail GLAInfantryRebel is unavailable.
pub const AMBUSH_RESIDUAL_TEMPLATE: &str = "TestInfantry";

/// Preferred retail template name for GLA ambush residual.
pub const GLA_REBEL_TEMPLATE: &str = "GLAInfantryRebel";

/// Host residual ambush kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HostAmbushKind {
    /// GLA Rebel Ambush / SuperweaponRebelAmbush residual (Ambush1 payload).
    GLARebelAmbush,
}

impl HostAmbushKind {
    /// Map a command-system power type to a host residual ambush, if supported.
    pub fn from_command_power(power: &SpecialPowerType) -> Option<Self> {
        match power {
            SpecialPowerType::Ambush => Some(HostAmbushKind::GLARebelAmbush),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => "GLARebelAmbush",
        }
    }

    /// Fade / approach delay in logic frames before infantry spawn.
    /// Residual of retail OCL FadeTime = 3000 ms → ~3s @ 30 FPS = 90 frames.
    pub fn spawn_delay_frames(self) -> u32 {
        match self {
            HostAmbushKind::GLARebelAmbush => 90,
        }
    }

    /// Number of residual infantry to spawn at ambush time.
    pub fn unit_count(self) -> u32 {
        match self {
            HostAmbushKind::GLARebelAmbush => GLA_AMBUSH1_UNIT_COUNT,
        }
    }

    /// Scatter radius for residual spawn positions.
    pub fn spawn_radius(self) -> f32 {
        match self {
            HostAmbushKind::GLARebelAmbush => AMBUSH_SPAWN_RADIUS,
        }
    }

    /// Preferred unit template for this residual kind.
    pub fn unit_template(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => GLA_REBEL_TEMPLATE,
        }
    }

    /// Audio event name queued on activation (host residual).
    pub fn activate_audio(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => "RebelAmbushActivated",
        }
    }

    /// Audio event name queued when units appear (host residual).
    pub fn spawn_audio(self) -> &'static str {
        match self {
            HostAmbushKind::GLARebelAmbush => "RebelAmbushSpawn",
        }
    }
}

/// Lifecycle of a queued host ambush.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HostAmbushPhase {
    /// Queued after DoSpecialPower; waiting for spawn frame.
    Queued,
    /// Spawn resolved; infantry created near target.
    Completed,
    /// Cancelled (source died / invalid) before spawn.
    Cancelled,
}

/// One pending or completed host ambush mission.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostAmbushMission {
    pub id: u32,
    pub kind: HostAmbushKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub activate_frame: u32,
    pub spawn_frame: u32,
    pub phase: HostAmbushPhase,
    /// Template used (or intended) for spawned infantry.
    pub unit_template: String,
    /// Number of units requested at queue time.
    pub unit_count: u32,
    /// Object ids of infantry successfully created at spawn.
    pub spawned_unit_ids: Vec<ObjectId>,
}

/// Spawn plan for one due ambush (computed before mutable create).
#[derive(Debug, Clone)]
pub struct HostAmbushSpawnPlan {
    pub mission_id: u32,
    pub kind: HostAmbushKind,
    pub source_object: ObjectId,
    pub source_team: super::Team,
    pub target_position: Vec3,
    pub unit_template: String,
    pub spawn_positions: Vec<Vec3>,
}

/// Host registry of ambush missions that queue and complete.
#[derive(Debug, Clone, Default)]
pub struct HostAmbushRegistry {
    next_id: u32,
    missions: HashMap<u32, HostAmbushMission>,
    completed_this_frame: Vec<u32>,
    activated_this_frame: Vec<u32>,
}

impl HostAmbushRegistry {
    pub fn new() -> Self {
        Self {
            next_id: 1,
            missions: HashMap::new(),
            completed_this_frame: Vec::new(),
            activated_this_frame: Vec::new(),
        }
    }

    pub fn clear(&mut self) {
        self.missions.clear();
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
        self.next_id = 1;
    }

    pub fn clear_frame_events(&mut self) {
        self.completed_this_frame.clear();
        self.activated_this_frame.clear();
    }

    pub fn next_id(&self) -> u32 {
        self.next_id
    }

    pub fn restore_from_snapshot(
        &mut self,
        next_id: u32,
        missions: impl IntoIterator<Item = HostAmbushMission>,
    ) {
        self.clear();
        let mut max_id = 0_u32;
        for mission in missions {
            max_id = max_id.max(mission.id);
            self.missions.insert(mission.id, mission);
        }
        self.next_id = next_id.max(max_id.saturating_add(1)).max(1);
    }

    pub fn mission_count(&self) -> usize {
        self.missions.len()
    }

    pub fn pending_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Queued)
            .count()
    }

    pub fn completed_count(&self) -> usize {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Completed)
            .count()
    }

    pub fn get(&self, id: u32) -> Option<&HostAmbushMission> {
        self.missions.get(&id)
    }

    pub fn missions_snapshot(&self) -> Vec<HostAmbushMission> {
        let mut v: Vec<_> = self.missions.values().cloned().collect();
        v.sort_by_key(|m| m.id);
        v
    }

    pub fn pending_of_kind(&self, kind: HostAmbushKind) -> Vec<&HostAmbushMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Queued && m.kind == kind)
            .collect()
    }

    pub fn completed_of_kind(&self, kind: HostAmbushKind) -> Vec<&HostAmbushMission> {
        self.missions
            .values()
            .filter(|m| m.phase == HostAmbushPhase::Completed && m.kind == kind)
            .collect()
    }

    pub fn activated_this_frame(&self) -> &[u32] {
        &self.activated_this_frame
    }

    pub fn completed_this_frame(&self) -> &[u32] {
        &self.completed_this_frame
    }

    /// Circular scatter spawn positions around target (SpreadFormation residual).
    pub fn spawn_positions(center: Vec3, unit_count: u32, radius: f32) -> Vec<Vec3> {
        if unit_count == 0 {
            return Vec::new();
        }
        let n = unit_count as usize;
        let mut positions = Vec::with_capacity(n);
        let angle_step = std::f32::consts::TAU / n as f32;
        for i in 0..n {
            let angle = i as f32 * angle_step;
            // Alternate inner/outer ring residual so units are not stacked.
            let dist = if n == 1 {
                0.0
            } else if i % 2 == 0 {
                radius * 0.6
            } else {
                radius
            };
            positions.push(Vec3::new(
                center.x + angle.cos() * dist,
                center.y,
                center.z + angle.sin() * dist,
            ));
        }
        positions
    }

    /// Queue an ambush mission. Returns host mission id.
    pub fn queue(
        &mut self,
        kind: HostAmbushKind,
        source_object: ObjectId,
        source_team: super::Team,
        target_position: Vec3,
        activate_frame: u32,
        unit_template: impl Into<String>,
    ) -> u32 {
        let id = self.next_id;
        self.next_id = self.next_id.saturating_add(1).max(1);
        let spawn_frame = activate_frame.saturating_add(kind.spawn_delay_frames());
        let mission = HostAmbushMission {
            id,
            kind,
            source_object,
            source_team,
            target_position,
            activate_frame,
            spawn_frame,
            phase: HostAmbushPhase::Queued,
            unit_template: unit_template.into(),
            unit_count: kind.unit_count(),
            spawned_unit_ids: Vec::new(),
        };
        self.missions.insert(id, mission);
        self.activated_this_frame.push(id);
        id
    }

    /// Build spawn plans for all missions whose spawn frame has arrived.
    pub fn plan_due_spawns(&self, current_frame: u32) -> Vec<HostAmbushSpawnPlan> {
        let mut plans = Vec::new();
        for mission in self.missions.values() {
            if mission.phase != HostAmbushPhase::Queued || current_frame < mission.spawn_frame {
                continue;
            }
            let spawn_positions = Self::spawn_positions(
                mission.target_position,
                mission.unit_count,
                mission.kind.spawn_radius(),
            );
            plans.push(HostAmbushSpawnPlan {
                mission_id: mission.id,
                kind: mission.kind,
                source_object: mission.source_object,
                source_team: mission.source_team,
                target_position: mission.target_position,
                unit_template: mission.unit_template.clone(),
                spawn_positions,
            });
        }
        plans.sort_by_key(|p| p.mission_id);
        plans
    }

    /// Record spawn results after GameLogic created units.
    pub fn record_spawn_complete(&mut self, mission_id: u32, spawned_unit_ids: Vec<ObjectId>) {
        if let Some(mission) = self.missions.get_mut(&mission_id) {
            if mission.phase == HostAmbushPhase::Queued {
                mission.phase = HostAmbushPhase::Completed;
                mission.spawned_unit_ids = spawned_unit_ids;
                self.completed_this_frame.push(mission_id);
            }
        }
    }

    /// Cancel pending missions owned by a destroyed source object.
    pub fn cancel_for_source(&mut self, source: ObjectId) {
        for mission in self.missions.values_mut() {
            if mission.source_object == source && mission.phase == HostAmbushPhase::Queued {
                mission.phase = HostAmbushPhase::Cancelled;
            }
        }
    }

    // --- Honesty flags (host residual; do not claim full retail parity) ---

    pub fn honesty_queue_ok(&self, kind: HostAmbushKind) -> bool {
        !self.pending_of_kind(kind).is_empty()
    }

    /// True if at least one mission of `kind` completed with one or more units spawned.
    pub fn honesty_complete_ok(&self, kind: HostAmbushKind) -> bool {
        self.completed_of_kind(kind)
            .iter()
            .any(|m| m.phase == HostAmbushPhase::Completed && !m.spawned_unit_ids.is_empty())
    }

    /// Combined host path honesty: completed ambush with spawned infantry.
    pub fn honesty_host_path_ok(&self, kind: HostAmbushKind) -> bool {
        self.honesty_complete_ok(kind)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::game_logic::Team;

    #[test]
    fn ambush_maps_from_command_power() {
        assert_eq!(
            HostAmbushKind::from_command_power(&SpecialPowerType::Ambush),
            Some(HostAmbushKind::GLARebelAmbush)
        );
        assert_eq!(
            HostAmbushKind::from_command_power(&SpecialPowerType::Paradrop),
            None
        );
        assert_eq!(
            HostAmbushKind::from_command_power(&SpecialPowerType::DaisyCutter),
            None
        );
    }

    #[test]
    fn queue_and_complete_ambush_spawn_plan() {
        let mut reg = HostAmbushRegistry::new();
        let id = reg.queue(
            HostAmbushKind::GLARebelAmbush,
            ObjectId(1),
            Team::GLA,
            Vec3::new(100.0, 0.0, 50.0),
            0,
            AMBUSH_RESIDUAL_TEMPLATE,
        );
        assert!(reg.honesty_queue_ok(HostAmbushKind::GLARebelAmbush));
        assert!(!reg.honesty_complete_ok(HostAmbushKind::GLARebelAmbush));

        let mission = reg.get(id).expect("mission");
        assert_eq!(mission.spawn_frame, 90);
        assert_eq!(mission.phase, HostAmbushPhase::Queued);
        assert_eq!(mission.unit_count, GLA_AMBUSH1_UNIT_COUNT);

        assert!(reg.plan_due_spawns(89).is_empty());

        let plans = reg.plan_due_spawns(90);
        assert_eq!(plans.len(), 1);
        assert_eq!(
            plans[0].spawn_positions.len(),
            GLA_AMBUSH1_UNIT_COUNT as usize
        );
        // All spawn positions should be near target.
        for pos in &plans[0].spawn_positions {
            let dx = pos.x - 100.0;
            let dz = pos.z - 50.0;
            let dist = (dx * dx + dz * dz).sqrt();
            assert!(dist <= AMBUSH_SPAWN_RADIUS + 0.1);
        }

        let spawned = vec![ObjectId(10), ObjectId(11), ObjectId(12), ObjectId(13)];
        reg.record_spawn_complete(id, spawned.clone());
        assert!(reg.honesty_complete_ok(HostAmbushKind::GLARebelAmbush));
        assert!(reg.honesty_host_path_ok(HostAmbushKind::GLARebelAmbush));
        assert_eq!(reg.get(id).unwrap().phase, HostAmbushPhase::Completed);
        assert_eq!(reg.get(id).unwrap().spawned_unit_ids, spawned);
    }

    #[test]
    fn spawn_positions_circle_formation() {
        let positions = HostAmbushRegistry::spawn_positions(Vec3::ZERO, 4, 40.0);
        assert_eq!(positions.len(), 4);
        for pos in &positions {
            let dist = (pos.x * pos.x + pos.z * pos.z).sqrt();
            assert!(dist > 0.0);
            assert!(dist <= 40.0 + 0.01);
        }
    }

    #[test]
    fn restore_from_snapshot_keeps_pending_spawn_frame() {
        let mut reg = HostAmbushRegistry::new();
        let id = reg.queue(
            HostAmbushKind::GLARebelAmbush,
            ObjectId(9),
            Team::GLA,
            Vec3::new(1.0, 0.0, 2.0),
            10,
            AMBUSH_RESIDUAL_TEMPLATE,
        );
        let snap = reg.missions_snapshot();
        let next = reg.next_id();

        let mut loaded = HostAmbushRegistry::new();
        loaded.restore_from_snapshot(next, snap);
        assert_eq!(loaded.pending_count(), 1);
        let m = loaded.get(id).expect("restored mission");
        assert_eq!(m.spawn_frame, 100);
        assert_eq!(m.phase, HostAmbushPhase::Queued);
        assert_eq!(loaded.next_id(), next);
    }
}
